//! Reviewer account credentials (prompt 09, docs/SECURITY_PRIVACY.md section
//! 1: "authenticated platform access" is a distinct identity boundary from
//! anonymous reporting). Password hashing lives here because it is pure
//! computation with no I/O; issuing/verifying the session token handed back
//! after a successful login needs an HMAC secret and lives in
//! `safe_cameroon_infrastructure`, mirroring how `HmacSignedAttachmentStorage`
//! owns signing for short-lived attachment URLs.

use core::fmt;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use async_trait::async_trait;
use uuid::Uuid;

const MIN_PASSWORD_LENGTH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordError {
    TooShort,
}

impl fmt::Display for PasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(
                f,
                "password must be at least {MIN_PASSWORD_LENGTH} characters"
            ),
        }
    }
}

impl std::error::Error for PasswordError {}

/// Argon2id with a freshly generated random salt (the OWASP-recommended
/// default), encoded as a self-describing PHC string so the algorithm/salt/
/// parameters travel with the hash — no separate salt column needed.
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(PasswordError::TooShort);
    }
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("hashing with a freshly generated salt never fails");
    Ok(hash.to_string())
}

/// Never panics on a malformed `hash` — treats it as a verification failure,
/// since `hash` ultimately comes from a database row a caller does not fully
/// control the shape of.
pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRevocationError {
    pub reason: String,
}

impl fmt::Display for SessionRevocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for SessionRevocationError {}

/// Whether a reviewer's sessions issued before a point in time have been
/// force-invalidated (docs/SECURITY_PRIVACY.md section 10: "incident
/// response" — a compromised credential or an offboarded reviewer must not
/// have to wait out a token's natural expiry). "Logout everywhere" per
/// reviewer, not a growing per-token revocation list: a token issued *after*
/// the revocation (a fresh login) is unaffected. Async because a durable
/// implementation needs to persist the revocation timestamp (an in-process
/// store alone would forget on restart) — mirrors
/// [`crate::rate_limit::RateLimiter`]/[`crate::webhook::WebhookReplayGuard`]'s
/// same reasoning.
#[async_trait]
pub trait SessionRevocationStore: Send + Sync {
    async fn is_session_revoked(
        &self,
        reviewer_id: Uuid,
        issued_at_epoch_seconds: i64,
    ) -> Result<bool, SessionRevocationError>;

    /// Invalidates every session currently issued for `reviewer_id`, as of
    /// now.
    async fn revoke_all_sessions(&self, reviewer_id: Uuid) -> Result<(), SessionRevocationError>;
}

/// A process-local, non-durable [`SessionRevocationStore`] for tests only —
/// mirrors [`crate::webhook::InMemoryReplayGuard`].
#[derive(Default)]
pub struct InMemorySessionRevocationStore {
    revoked_at: Mutex<HashMap<Uuid, i64>>,
}

#[async_trait]
impl SessionRevocationStore for InMemorySessionRevocationStore {
    async fn is_session_revoked(
        &self,
        reviewer_id: Uuid,
        issued_at_epoch_seconds: i64,
    ) -> Result<bool, SessionRevocationError> {
        let revoked_at = self
            .revoked_at
            .lock()
            .expect("in-memory session revocation store mutex must not be poisoned");
        // `<=` (not `<`): a token issued in the same whole second as a
        // revocation is treated as revoked. `issued_at` only ever has
        // one-second resolution (`ReviewerSessionTokenIssuer::verify`
        // reconstructs it from a whole-second `expires_at`), so this is the
        // fail-safe choice for that unavoidably ambiguous window, mirroring
        // how the real Postgres-backed store's sub-second `revoked_at`
        // compared against a floored `issued_at` behaves in practice.
        Ok(revoked_at
            .get(&reviewer_id)
            .is_some_and(|revoked_at| issued_at_epoch_seconds <= *revoked_at))
    }

    async fn revoke_all_sessions(&self, reviewer_id: Uuid) -> Result<(), SessionRevocationError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_secs() as i64;
        self.revoked_at
            .lock()
            .expect("in-memory session revocation store mutex must not be poisoned")
            .insert(reviewer_id, now);
        Ok(())
    }
}

impl InMemorySessionRevocationStore {
    /// Test-only: revokes as of an explicit timestamp instead of "now", so a
    /// test can assert ordering relative to a just-issued token
    /// deterministically without depending on two operations landing in
    /// different wall-clock seconds.
    pub fn revoke_all_sessions_at(&self, reviewer_id: Uuid, revoked_at_epoch_seconds: i64) {
        self.revoked_at
            .lock()
            .expect("in-memory session revocation store mutex must not be poisoned")
            .insert(reviewer_id, revoked_at_epoch_seconds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now_epoch_seconds() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    #[tokio::test]
    async fn a_reviewer_with_no_revocation_is_never_revoked() {
        let store = InMemorySessionRevocationStore::default();
        assert!(
            !store
                .is_session_revoked(Uuid::new_v4(), now_epoch_seconds())
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn a_token_issued_before_revocation_is_revoked_and_after_is_not() {
        let store = InMemorySessionRevocationStore::default();
        let reviewer_id = Uuid::new_v4();
        let issued_before = now_epoch_seconds() - 10;

        store.revoke_all_sessions(reviewer_id).await.unwrap();
        let issued_after = now_epoch_seconds() + 10;

        assert!(
            store
                .is_session_revoked(reviewer_id, issued_before)
                .await
                .unwrap()
        );
        assert!(
            !store
                .is_session_revoked(reviewer_id, issued_after)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn revocation_only_affects_the_reviewer_it_was_issued_for() {
        let store = InMemorySessionRevocationStore::default();
        let revoked_reviewer = Uuid::new_v4();
        let other_reviewer = Uuid::new_v4();
        let issued_before = now_epoch_seconds() - 10;
        store.revoke_all_sessions(revoked_reviewer).await.unwrap();

        assert!(
            !store
                .is_session_revoked(other_reviewer, issued_before)
                .await
                .unwrap()
        );
    }

    #[test]
    fn a_hashed_password_verifies_against_the_same_password() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
    }

    #[test]
    fn a_hashed_password_does_not_verify_against_a_different_password() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(!verify_password("wrong password entirely", &hash));
    }

    #[test]
    fn hashing_the_same_password_twice_produces_different_hashes() {
        let first = hash_password("correct horse battery staple").unwrap();
        let second = hash_password("correct horse battery staple").unwrap();
        assert_ne!(
            first, second,
            "a fresh random salt must be used for every hash"
        );
    }

    #[test]
    fn a_password_shorter_than_the_minimum_is_rejected() {
        assert_eq!(hash_password("short1"), Err(PasswordError::TooShort));
    }

    #[test]
    fn verify_password_rejects_a_malformed_hash_instead_of_panicking() {
        assert!(!verify_password("anything", "not a real password hash"));
    }
}
