//! Until the auth foundation (prompt 09) exists, the caller's reviewer
//! identity is taken from an explicit `X-Reviewer-Id` header. Its absence
//! means the request is treated as an automated/system actor, which the
//! application layer restricts to the lowest-stakes actions only (starting
//! case review; raising an internal/partner alert).

use axum::http::{HeaderMap, StatusCode};
use safe_cameroon_application::case_workflow::Actor;
use uuid::Uuid;

use crate::error::ApiError;

const REVIEWER_HEADER: &str = "X-Reviewer-Id";

pub fn actor_from_headers(headers: &HeaderMap, request_id: Uuid) -> Result<Actor, ApiError> {
    let Some(value) = headers.get(REVIEWER_HEADER) else {
        return Ok(Actor::Automated);
    };
    let reviewer_id = value
        .to_str()
        .ok()
        .and_then(|value| Uuid::parse_str(value.trim()).ok())
        .ok_or(ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "INVALID_REVIEWER_ID",
            message: "X-Reviewer-Id must be a UUID.",
            request_id,
        })?;
    Ok(Actor::Reviewer(reviewer_id))
}
