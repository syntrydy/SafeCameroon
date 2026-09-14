//! Shared rate-limit enforcement for the endpoints that need it
//! (`reports.rs`, `auth.rs::login`) — the port and the policy (which scopes
//! exist, their limits/windows) live in
//! `safe_cameroon_application::rate_limit`; this only maps its `bool` result
//! onto a stable HTTP error.

use axum::http::StatusCode;
use safe_cameroon_application::rate_limit::{RateLimitScope, RateLimiter};
use uuid::Uuid;

use crate::error::ApiError;

pub async fn enforce_rate_limit(
    rate_limiter: &dyn RateLimiter,
    scope: RateLimitScope,
    source_key: &str,
    request_id: Uuid,
) -> Result<(), ApiError> {
    let within_limit = rate_limiter
        .record_and_check(scope, source_key)
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "RATE_LIMIT_CHECK_FAILED",
            message: "Could not verify the request rate. Please try again.",
            request_id,
        })?;

    if within_limit {
        Ok(())
    } else {
        Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "RATE_LIMIT_EXCEEDED",
            message: "Too many requests. Please try again later.",
            request_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use safe_cameroon_application::rate_limit::InMemoryRateLimiter;
    use uuid::Uuid;

    #[tokio::test]
    async fn allows_requests_within_the_limit() {
        let limiter = InMemoryRateLimiter::default();
        for _ in 0..RateLimitScope::ReviewerLoginAttempt.limit() {
            assert!(
                enforce_rate_limit(
                    &limiter,
                    RateLimitScope::ReviewerLoginAttempt,
                    "someone",
                    Uuid::new_v4(),
                )
                .await
                .is_ok()
            );
        }
    }

    #[tokio::test]
    async fn refuses_the_request_once_the_limit_is_exceeded() {
        let limiter = InMemoryRateLimiter::default();
        for _ in 0..RateLimitScope::ReviewerLoginAttempt.limit() {
            enforce_rate_limit(
                &limiter,
                RateLimitScope::ReviewerLoginAttempt,
                "someone",
                Uuid::new_v4(),
            )
            .await
            .unwrap();
        }
        let error = enforce_rate_limit(
            &limiter,
            RateLimitScope::ReviewerLoginAttempt,
            "someone",
            Uuid::new_v4(),
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(error.code, "RATE_LIMIT_EXCEEDED");
    }
}
