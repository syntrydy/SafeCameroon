use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use safe_cameroon_application::authorization::{Capability, authorize};
use safe_cameroon_application::rate_limit::RateLimitScope;
use safe_cameroon_application::{ReportValidationError, prepare_anonymous_report};
use safe_cameroon_domain::{ReportSourceChannel, ReportStatus};
use safe_cameroon_infrastructure::postgres::{ReportSummary, SubmissionResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::idempotency::idempotency_key_from_headers;
use crate::rate_limit::enforce_rate_limit;
use crate::request_id::request_id_from_headers;
use crate::reviewer::actor_from_headers;
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

    let idempotency_key = idempotency_key_from_headers(&headers, request_id)?;
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

const DEFAULT_LIMIT: u32 = 50;
/// A hard ceiling regardless of what the caller asks for, so a single
/// request can never force an unbounded scan/response.
const MAX_LIMIT: u32 = 100;

#[derive(Deserialize)]
pub struct ListReportsQuery {
    status: Option<ReportStatus>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Serialize)]
pub struct ReportSummaryResponse {
    report_id: Uuid,
    source_channel: ReportSourceChannel,
    status: ReportStatus,
    raw_content: String,
    received_at: String,
}

fn report_summary_response(report: &ReportSummary) -> ReportSummaryResponse {
    ReportSummaryResponse {
        report_id: report.id,
        source_channel: report.source_channel,
        status: report.status,
        raw_content: report.raw_content.clone(),
        received_at: report.received_at.clone(),
    }
}

/// A reviewer's only way to read report content before deciding whether to
/// open a case (`POST /v1/cases`) — there is still no single-report `GET`,
/// only this list.
pub async fn list_reports(
    State(state): State<AppState>,
    Query(query): Query<ListReportsQuery>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<ReportSummaryResponse>>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ViewCase).map_err(|_| ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "Only an identified reviewer may list reports.",
        request_id,
    })?;

    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let reports = state
        .reports
        .list(query.status, limit, offset)
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "REPORT_QUERY_FAILED",
            message: "Reports could not be read. Please try again.",
            request_id,
        })?;

    Ok(Json(reports.iter().map(report_summary_response).collect()))
}
