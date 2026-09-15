//! Verifies a Google ID token by calling Google's own `tokeninfo` endpoint
//! rather than fetching/caching Google's JWKS and checking the RS256
//! signature locally — Google does the cryptographic verification for us.
//! This trades a network round-trip (and Google's documented rate limits on
//! that endpoint) for a much smaller, easier-to-audit adapter, which is the
//! right trade for a pilot's reviewer login volume (AGENTS.md: "do not
//! overengineer the first release"). Revisit with local JWKS verification if
//! login volume ever makes that endpoint a bottleneck.

use async_trait::async_trait;
use safe_cameroon_application::google_identity::{
    GoogleIdentityVerifier, IdentityVerificationError, VerifiedGoogleIdentity,
};
use serde::Deserialize;

const TOKENINFO_URL: &str = "https://oauth2.googleapis.com/tokeninfo";

#[derive(Debug, Deserialize)]
struct TokenInfo {
    aud: Option<String>,
    email: Option<String>,
    email_verified: Option<String>,
}

/// The claims a verified token must satisfy, kept separate from the HTTP
/// call itself so this security-relevant check is unit-testable without a
/// network (`fetch_token_info` is the only part that actually calls out).
fn validate_token_info(
    info: TokenInfo,
    expected_client_id: &str,
) -> Result<VerifiedGoogleIdentity, IdentityVerificationError> {
    let reject = |reason: &str| IdentityVerificationError {
        reason: reason.to_owned(),
    };

    if info.aud.as_deref() != Some(expected_client_id) {
        return Err(reject("id token was not issued for this application"));
    }
    if info.email_verified.as_deref() != Some("true") {
        return Err(reject("Google account email is not verified"));
    }
    let email = info
        .email
        .ok_or_else(|| reject("Google did not return an email"))?;

    Ok(VerifiedGoogleIdentity { email })
}

pub struct GoogleTokenInfoVerifier {
    http: reqwest::Client,
    expected_client_id: String,
}

impl GoogleTokenInfoVerifier {
    pub fn new(expected_client_id: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            expected_client_id,
        }
    }

    async fn fetch_token_info(
        &self,
        id_token: &str,
    ) -> Result<TokenInfo, IdentityVerificationError> {
        let unreachable = || IdentityVerificationError {
            reason: "could not reach Google to verify this sign-in".to_owned(),
        };

        let response = self
            .http
            .get(TOKENINFO_URL)
            .query(&[("id_token", id_token)])
            .send()
            .await
            .map_err(|_| unreachable())?;

        if !response.status().is_success() {
            return Err(IdentityVerificationError {
                reason: "Google rejected this id token".to_owned(),
            });
        }

        response
            .json::<TokenInfo>()
            .await
            .map_err(|_| IdentityVerificationError {
                reason: "unexpected response from Google".to_owned(),
            })
    }
}

#[async_trait]
impl GoogleIdentityVerifier for GoogleTokenInfoVerifier {
    async fn verify_id_token(
        &self,
        id_token: &str,
    ) -> Result<VerifiedGoogleIdentity, IdentityVerificationError> {
        let info = self.fetch_token_info(id_token).await?;
        validate_token_info(info, &self.expected_client_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token_info(aud: &str, email: &str, email_verified: &str) -> TokenInfo {
        TokenInfo {
            aud: Some(aud.to_owned()),
            email: Some(email.to_owned()),
            email_verified: Some(email_verified.to_owned()),
        }
    }

    #[test]
    fn a_matching_audience_and_verified_email_is_accepted() {
        let identity = validate_token_info(
            token_info("client-1", "reviewer@example.test", "true"),
            "client-1",
        )
        .unwrap();
        assert_eq!(identity.email, "reviewer@example.test");
    }

    #[test]
    fn a_token_issued_for_a_different_client_is_rejected() {
        let error = validate_token_info(
            token_info("someone-elses-client", "reviewer@example.test", "true"),
            "client-1",
        )
        .unwrap_err();
        assert_eq!(error.reason, "id token was not issued for this application");
    }

    #[test]
    fn an_unverified_email_is_rejected() {
        let error = validate_token_info(
            token_info("client-1", "reviewer@example.test", "false"),
            "client-1",
        )
        .unwrap_err();
        assert_eq!(error.reason, "Google account email is not verified");
    }

    #[test]
    fn a_missing_email_verified_claim_is_rejected() {
        let info = TokenInfo {
            aud: Some("client-1".to_owned()),
            email: Some("reviewer@example.test".to_owned()),
            email_verified: None,
        };
        let error = validate_token_info(info, "client-1").unwrap_err();
        assert_eq!(error.reason, "Google account email is not verified");
    }

    #[test]
    fn a_missing_email_claim_is_rejected() {
        let info = TokenInfo {
            aud: Some("client-1".to_owned()),
            email: None,
            email_verified: Some("true".to_owned()),
        };
        let error = validate_token_info(info, "client-1").unwrap_err();
        assert_eq!(error.reason, "Google did not return an email");
    }
}
