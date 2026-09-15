//! A port for verifying a Google-issued ID token names a real, email-
//! verified Google account, without this crate (or the reviewer table)
//! ever seeing or storing a password. Reviewers authenticate by signing in
//! with Google; the actual cryptographic/HTTP verification is Google's own
//! job, done by an infrastructure adapter behind this trait
//! (AGENTS.md: "the domain must not import ... HTTP provider SDKs" —
//! `safe_cameroon_application` stays free of any Google-specific type).

use core::fmt;

use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedGoogleIdentity {
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityVerificationError {
    pub reason: String,
}

impl fmt::Display for IdentityVerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for IdentityVerificationError {}

#[async_trait]
pub trait GoogleIdentityVerifier: Send + Sync {
    /// Verifies a Google ID token (the `credential` a Google Identity
    /// Services sign-in returns) and returns the account's verified email.
    async fn verify_id_token(
        &self,
        id_token: &str,
    ) -> Result<VerifiedGoogleIdentity, IdentityVerificationError>;
}

/// A token that always fails verification, for exercising the failure path
/// against [`FakeGoogleIdentityVerifier`] without a real Google outage.
pub const INVALID_GOOGLE_TOKEN: &str = "invalid-google-token";

/// A deterministic, non-network [`GoogleIdentityVerifier`] for tests:
/// treats the id token as if it already were the verified email — mirrors
/// how [`crate::reviewer_auth::InMemorySessionRevocationStore`] stands in
/// for a durable store.
#[derive(Debug, Clone, Copy, Default)]
pub struct FakeGoogleIdentityVerifier;

#[async_trait]
impl GoogleIdentityVerifier for FakeGoogleIdentityVerifier {
    async fn verify_id_token(
        &self,
        id_token: &str,
    ) -> Result<VerifiedGoogleIdentity, IdentityVerificationError> {
        if id_token == INVALID_GOOGLE_TOKEN {
            return Err(IdentityVerificationError {
                reason: "invalid token".to_owned(),
            });
        }
        Ok(VerifiedGoogleIdentity {
            email: id_token.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_fake_verifier_treats_the_token_as_the_email() {
        let identity = FakeGoogleIdentityVerifier
            .verify_id_token("reviewer@example.test")
            .await
            .unwrap();
        assert_eq!(identity.email, "reviewer@example.test");
    }

    #[tokio::test]
    async fn a_fake_verifier_rejects_the_invalid_token_sentinel() {
        let error = FakeGoogleIdentityVerifier
            .verify_id_token(INVALID_GOOGLE_TOKEN)
            .await
            .unwrap_err();
        assert_eq!(error.reason, "invalid token");
    }
}
