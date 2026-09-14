//! The caller's reviewer identity comes from a verified, non-revoked session
//! token (`Authorization: Bearer <token>`, issued by `POST /v1/auth/login`
//! — `auth.rs`), not a client-supplied id (prompt 09; docs/SECURITY_PRIVACY.md
//! section 1: "authenticated platform access" is a distinct identity
//! boundary from anonymous reporting). No header at all means the request is
//! treated as an automated/system actor, which the application layer
//! restricts to the lowest-stakes actions only (starting case review;
//! raising an internal/partner alert) — a present-but-invalid or revoked
//! header is rejected outright rather than silently downgraded to
//! `Automated`.

use axum::http::{HeaderMap, StatusCode};
use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_application::reviewer_auth::SessionRevocationStore;
use safe_cameroon_infrastructure::auth::ReviewerSessionTokenIssuer;
use uuid::Uuid;

use crate::error::ApiError;

const AUTHORIZATION_HEADER: &str = "Authorization";
const BEARER_PREFIX: &str = "Bearer ";

fn invalid_or_expired_session(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::UNAUTHORIZED,
        code: "INVALID_OR_EXPIRED_SESSION",
        message: "The session token is invalid or has expired.",
        request_id,
    }
}

pub async fn actor_from_headers(
    session_tokens: &ReviewerSessionTokenIssuer,
    revocation_store: &impl SessionRevocationStore,
    headers: &HeaderMap,
    request_id: Uuid,
) -> Result<Actor, ApiError> {
    let Some(value) = headers.get(AUTHORIZATION_HEADER) else {
        return Ok(Actor::Automated);
    };
    let token = value
        .to_str()
        .ok()
        .and_then(|value| value.strip_prefix(BEARER_PREFIX))
        .ok_or(ApiError {
            status: StatusCode::UNAUTHORIZED,
            code: "INVALID_AUTHORIZATION_HEADER",
            message: "Authorization must be a Bearer session token.",
            request_id,
        })?;
    let verified = session_tokens
        .verify(token)
        .ok_or_else(|| invalid_or_expired_session(request_id))?;

    let revoked = revocation_store
        .is_session_revoked(verified.reviewer_id, verified.issued_at as i64)
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "SESSION_REVOCATION_CHECK_FAILED",
            message: "Could not verify the session. Please try again.",
            request_id,
        })?;
    if revoked {
        return Err(invalid_or_expired_session(request_id));
    }

    Ok(Actor::Reviewer(verified.reviewer_id))
}

#[cfg(test)]
mod tests {
    use safe_cameroon_application::reviewer_auth::InMemorySessionRevocationStore;

    use super::*;

    fn issuer() -> ReviewerSessionTokenIssuer {
        ReviewerSessionTokenIssuer::new(b"test-secret".to_vec())
    }

    #[tokio::test]
    async fn no_authorization_header_is_treated_as_automated() {
        let actor = actor_from_headers(
            &issuer(),
            &InMemorySessionRevocationStore::default(),
            &HeaderMap::new(),
            Uuid::new_v4(),
        )
        .await
        .unwrap();
        assert_eq!(actor, Actor::Automated);
    }

    #[tokio::test]
    async fn a_valid_bearer_token_resolves_to_its_reviewer() {
        let issuer = issuer();
        let reviewer_id = Uuid::new_v4();
        let issued = issuer.issue(reviewer_id);
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION_HEADER,
            format!("Bearer {}", issued.token).parse().unwrap(),
        );

        let actor = actor_from_headers(
            &issuer,
            &InMemorySessionRevocationStore::default(),
            &headers,
            Uuid::new_v4(),
        )
        .await
        .unwrap();
        assert_eq!(actor, Actor::Reviewer(reviewer_id));
    }

    #[tokio::test]
    async fn a_non_bearer_authorization_header_is_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION_HEADER, "Basic dXNlcjpwYXNz".parse().unwrap());

        let error = actor_from_headers(
            &issuer(),
            &InMemorySessionRevocationStore::default(),
            &headers,
            Uuid::new_v4(),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "INVALID_AUTHORIZATION_HEADER");
    }

    #[tokio::test]
    async fn a_token_signed_with_a_different_secret_is_rejected() {
        let other_issuer = ReviewerSessionTokenIssuer::new(b"a different secret".to_vec());
        let issued = other_issuer.issue(Uuid::new_v4());
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION_HEADER,
            format!("Bearer {}", issued.token).parse().unwrap(),
        );

        let error = actor_from_headers(
            &issuer(),
            &InMemorySessionRevocationStore::default(),
            &headers,
            Uuid::new_v4(),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "INVALID_OR_EXPIRED_SESSION");
    }

    #[tokio::test]
    async fn a_revoked_reviewers_token_is_rejected() {
        let issuer = issuer();
        let reviewer_id = Uuid::new_v4();
        let issued = issuer.issue(reviewer_id);
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION_HEADER,
            format!("Bearer {}", issued.token).parse().unwrap(),
        );

        // Revoked exactly at (not after) the token's own issued_at: the
        // ambiguous same-instant case is deliberately treated as revoked
        // (see `InMemorySessionRevocationStore::is_session_revoked`'s `<=`),
        // so this is already the strictest case a real revocation covers.
        let verified_issued_at = issuer.verify(&issued.token).unwrap().issued_at;
        let revocation_store = InMemorySessionRevocationStore::default();
        revocation_store.revoke_all_sessions_at(reviewer_id, verified_issued_at as i64);

        let error = actor_from_headers(&issuer, &revocation_store, &headers, Uuid::new_v4())
            .await
            .unwrap_err();
        assert_eq!(error.code, "INVALID_OR_EXPIRED_SESSION");
    }

    #[tokio::test]
    async fn a_fresh_login_after_revocation_is_still_accepted() {
        let issuer = issuer();
        let reviewer_id = Uuid::new_v4();
        let revocation_store = InMemorySessionRevocationStore::default();
        // Revoked well in the past, so a token issued "now" (below) is
        // unambiguously after it regardless of wall-clock second boundaries.
        revocation_store.revoke_all_sessions_at(reviewer_id, 0);

        let issued = issuer.issue(reviewer_id);
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION_HEADER,
            format!("Bearer {}", issued.token).parse().unwrap(),
        );

        let actor = actor_from_headers(&issuer, &revocation_store, &headers, Uuid::new_v4())
            .await
            .unwrap();
        assert_eq!(actor, Actor::Reviewer(reviewer_id));
    }
}
