//! Reviewer session revocation (prompt 09, docs/SECURITY_PRIVACY.md section
//! 1: "authenticated platform access" is a distinct identity boundary from
//! anonymous reporting). Reviewer identity itself is verified by Google
//! sign-in (`crate::google_identity`) rather than a locally stored
//! credential, so there is nothing to hash here; issuing/verifying the
//! session token handed back after a successful login needs an HMAC secret
//! and lives in `safe_cameroon_infrastructure`, mirroring how
//! `HmacSignedAttachmentStorage` owns signing for short-lived attachment
//! URLs.

use core::fmt;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use uuid::Uuid;

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
}
