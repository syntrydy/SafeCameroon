//! The caller's reviewer identity comes from a verified session token
//! (`Authorization: Bearer <token>`, issued by `POST /v1/auth/login` —
//! `auth.rs`), not a client-supplied id (prompt 09; docs/SECURITY_PRIVACY.md
//! section 1: "authenticated platform access" is a distinct identity
//! boundary from anonymous reporting). No header at all means the request is
//! treated as an automated/system actor, which the application layer
//! restricts to the lowest-stakes actions only (starting case review;
//! raising an internal/partner alert) — a present-but-invalid header is
//! rejected outright rather than silently downgraded to `Automated`.

use axum::http::{HeaderMap, StatusCode};
use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_infrastructure::auth::ReviewerSessionTokenIssuer;
use uuid::Uuid;

use crate::error::ApiError;

const AUTHORIZATION_HEADER: &str = "Authorization";
const BEARER_PREFIX: &str = "Bearer ";

pub fn actor_from_headers(
    session_tokens: &ReviewerSessionTokenIssuer,
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
    let reviewer_id = session_tokens.verify(token).ok_or(ApiError {
        status: StatusCode::UNAUTHORIZED,
        code: "INVALID_OR_EXPIRED_SESSION",
        message: "The session token is invalid or has expired.",
        request_id,
    })?;
    Ok(Actor::Reviewer(reviewer_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issuer() -> ReviewerSessionTokenIssuer {
        ReviewerSessionTokenIssuer::new(b"test-secret".to_vec())
    }

    #[test]
    fn no_authorization_header_is_treated_as_automated() {
        let actor = actor_from_headers(&issuer(), &HeaderMap::new(), Uuid::new_v4()).unwrap();
        assert_eq!(actor, Actor::Automated);
    }

    #[test]
    fn a_valid_bearer_token_resolves_to_its_reviewer() {
        let issuer = issuer();
        let reviewer_id = Uuid::new_v4();
        let issued = issuer.issue(reviewer_id);
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION_HEADER,
            format!("Bearer {}", issued.token).parse().unwrap(),
        );

        let actor = actor_from_headers(&issuer, &headers, Uuid::new_v4()).unwrap();
        assert_eq!(actor, Actor::Reviewer(reviewer_id));
    }

    #[test]
    fn a_non_bearer_authorization_header_is_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION_HEADER, "Basic dXNlcjpwYXNz".parse().unwrap());

        let error = actor_from_headers(&issuer(), &headers, Uuid::new_v4()).unwrap_err();
        assert_eq!(error.code, "INVALID_AUTHORIZATION_HEADER");
    }

    #[test]
    fn a_token_signed_with_a_different_secret_is_rejected() {
        let other_issuer = ReviewerSessionTokenIssuer::new(b"a different secret".to_vec());
        let issued = other_issuer.issue(Uuid::new_v4());
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION_HEADER,
            format!("Bearer {}", issued.token).parse().unwrap(),
        );

        let error = actor_from_headers(&issuer(), &headers, Uuid::new_v4()).unwrap_err();
        assert_eq!(error.code, "INVALID_OR_EXPIRED_SESSION");
    }
}
