use axum::{Json, extract::State, http::StatusCode};
use safe_cameroon_application::rate_limit::RateLimitScope;
use safe_cameroon_application::{ReportValidationError, prepare_anonymous_report};
use safe_cameroon_infrastructure::postgres::SubmissionResult;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::rate_limit::enforce_rate_limit;
use crate::request_id::request_id_from_headers;
use crate::source_key::source_key_from_headers;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CreateReportRequest {
    content: String,
}

#[derive(Serialize)]
pub struct CreateReportResponse {
    report_id: Uuid,
    reference_code: String,
    status: &'static str,
}

pub async fn create_anonymous_report(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(request): Json<CreateReportRequest>,
) -> Result<(StatusCode, Json<CreateReportResponse>), ApiError> {
    let request_id = request_id_from_headers(&headers);
    enforce_rate_limit(
        state.rate_limiter.as_ref(),
        RateLimitScope::AnonymousReportSubmission,
        source_key_from_headers(&headers),
        request_id,
    )
    .await?;

    let idempotency_key = headers
        .get("Idempotency-Key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(key) = idempotency_key {
        if !(8..=256).contains(&key.len()) {
            return Err(ApiError {
                status: StatusCode::BAD_REQUEST,
                code: "INVALID_IDEMPOTENCY_KEY",
                message: "Idempotency-Key must be between 8 and 256 characters.",
                request_id,
            });
        }
    }
    let submission = prepare_anonymous_report(request.content, request_id, idempotency_key)
        .map_err(|error| {
            let (code, message) = match error {
                ReportValidationError::EmptyContent => {
                    ("INVALID_REPORT_CONTENT", "Report content cannot be blank.")
                }
                ReportValidationError::ContentTooLong => {
                    ("INVALID_REPORT_CONTENT", "Report content is too long.")
                }
            };
            ApiError {
                status: StatusCode::BAD_REQUEST,
                code,
                message,
                request_id,
            }
        })?;

    let result = state
        .reports
        .submit_anonymous(&submission)
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "REPORT_PERSISTENCE_FAILED",
            message: "The report could not be saved. Please try again.",
            request_id,
        })?;

    if matches!(result, SubmissionResult::Duplicate { .. }) {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            code: "IDEMPOTENCY_KEY_REUSED",
            message: "This Idempotency-Key was already used for a report.",
            request_id,
        });
    }

    Ok((
        StatusCode::CREATED,
        Json(CreateReportResponse {
            report_id: submission.report.id.as_uuid(),
            reference_code: submission.reference_code,
            status: "RECEIVED",
        }),
    ))
}
