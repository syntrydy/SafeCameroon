//! Session tokens issued after a successful reviewer login (prompt 09).
//! HMAC-SHA256 over `reviewer_id:expires_at`, the same real, working scheme
//! `HmacSignedWebhookVerifier`/`HmacSignedAttachmentStorage` already use —
//! stateless and self-verifying, so no session table/lookup is needed on
//! every request. Revocation before expiry is not supported (a deliberate
//! simplification; see docs/SECURITY_PRIVACY.md section 10 for the incident
//! response path if a token is compromised before it naturally expires).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::hex;

type HmacSha256 = Hmac<Sha256>;

const DEFAULT_TTL: Duration = Duration::from_secs(12 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedSessionToken {
    pub token: String,
    pub expires_in: Duration,
}

#[derive(Clone)]
pub struct ReviewerSessionTokenIssuer {
    secret: Vec<u8>,
    ttl: Duration,
}

impl ReviewerSessionTokenIssuer {
    pub fn new(secret: impl Into<Vec<u8>>) -> Self {
        Self {
            secret: secret.into(),
            ttl: DEFAULT_TTL,
        }
    }

    fn sign(&self, reviewer_id: Uuid, expires_at: u64) -> String {
        let mut mac = HmacSha256::new_from_slice(&self.secret)
            .expect("HMAC-SHA256 accepts a key of any length");
        mac.update(format!("{reviewer_id}:{expires_at}").as_bytes());
        hex::encode(&mac.finalize().into_bytes())
    }

    pub fn issue(&self, reviewer_id: Uuid) -> IssuedSessionToken {
        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_secs()
            + self.ttl.as_secs();
        let signature = self.sign(reviewer_id, expires_at);
        IssuedSessionToken {
            token: format!("{reviewer_id}.{expires_at}.{signature}"),
            expires_in: self.ttl,
        }
    }

    /// Parses and verifies `token`, returning the reviewer it was issued
    /// for. Any structural problem (wrong shape, bad signature, expired,
    /// signed with a different secret) is treated identically as "not a
    /// valid token" rather than distinguished — a caller only ever needs to
    /// know whether to trust it.
    pub fn verify(&self, token: &str) -> Option<Uuid> {
        let mut parts = token.splitn(3, '.');
        let reviewer_id = Uuid::parse_str(parts.next()?).ok()?;
        let expires_at: u64 = parts.next()?.parse().ok()?;
        let signature = parts.next()?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_secs();
        if expires_at < now {
            return None;
        }

        let signature_bytes = hex::decode(signature)?;
        let mut mac = HmacSha256::new_from_slice(&self.secret)
            .expect("HMAC-SHA256 accepts a key of any length");
        mac.update(format!("{reviewer_id}:{expires_at}").as_bytes());
        mac.verify_slice(&signature_bytes).ok()?;

        Some(reviewer_id)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn an_issued_token_verifies_back_to_the_reviewer_it_was_issued_for() {
        let issuer = ReviewerSessionTokenIssuer::new(b"secret".to_vec());
        let reviewer_id = uuid::Uuid::new_v4();
        let issued = issuer.issue(reviewer_id);
        assert_eq!(issuer.verify(&issued.token), Some(reviewer_id));
    }

    #[test]
    fn a_token_signed_with_a_different_secret_does_not_verify() {
        let issuer = ReviewerSessionTokenIssuer::new(b"secret".to_vec());
        let other_issuer = ReviewerSessionTokenIssuer::new(b"a different secret".to_vec());
        let issued = issuer.issue(uuid::Uuid::new_v4());
        assert_eq!(other_issuer.verify(&issued.token), None);
    }

    #[test]
    fn a_tampered_subject_does_not_verify() {
        let issuer = ReviewerSessionTokenIssuer::new(b"secret".to_vec());
        let issued = issuer.issue(uuid::Uuid::new_v4());
        let mut parts: Vec<&str> = issued.token.splitn(3, '.').collect();
        let another_id = uuid::Uuid::new_v4().to_string();
        parts[0] = &another_id;
        let tampered = parts.join(".");
        assert_eq!(issuer.verify(&tampered), None);
    }

    #[test]
    fn an_expired_token_does_not_verify() {
        let issuer = ReviewerSessionTokenIssuer::new(b"secret".to_vec());
        let reviewer_id = uuid::Uuid::new_v4();
        let expired_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 1;
        let signature = issuer.sign(reviewer_id, expired_at);
        let expired_token = format!("{reviewer_id}.{expired_at}.{signature}");
        assert_eq!(issuer.verify(&expired_token), None);
    }

    #[test]
    fn a_malformed_token_does_not_verify_and_does_not_panic() {
        let issuer = ReviewerSessionTokenIssuer::new(b"secret".to_vec());
        for malformed in ["", "not-a-token", "only.two-parts", "a.b.c.d"] {
            assert_eq!(issuer.verify(malformed), None);
        }
    }
}
