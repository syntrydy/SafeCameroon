use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use safe_cameroon_application::authorization::{Capability, authorize};
use safe_cameroon_application::rate_limit::RateLimitScope;
use safe_cameroon_application::{ReportValidationError, prepare_anonymous_report};
use safe_cameroon_domain::{IncidentType, ReportId, ReportSourceChannel, ReportStatus};
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
    /// The reporter's own guess at the incident type, if the intake UI asked.
    /// Optional and never authoritative -- see `AnonymousReport::reported_incident_type`.
    #[serde(default)]
    incident_type: Option<IncidentType>,
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
    let submission = prepare_anonymous_report(
        request.content,
        request_id,
        idempotency_key,
        request.incident_type,
    )
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

/// Generous ceiling for a 2-minute recording (issue #160's own estimate:
/// ~360-480KB of compressed speech audio at 24-32kbps) with real headroom
/// for container/codec overhead across browsers -- a hard cap, not a target.
pub(crate) const MAX_AUDIO_BYTES: usize = 8 * 1024 * 1024;

fn transcription_format_from_content_type(content_type: Option<&str>) -> &'static str {
    // Strips "audio/" and any ";codecs=..." parameter, e.g.
    // "audio/webm;codecs=opus" -> "webm" -- passed through to the provider
    // as-is (issue #160: browser-recorded formats aren't fully verified
    // against the provider's accepted list; an unsupported format simply
    // fails the transcription request, surfaced to the user as an error).
    match content_type
        .and_then(|value| value.split(';').next())
        .and_then(|mime| mime.strip_prefix("audio/"))
    {
        Some("webm") => "webm",
        Some("ogg") => "ogg",
        Some("wav") | Some("wave") | Some("x-wav") => "wav",
        Some("mp4") | Some("m4a") | Some("x-m4a") => "m4a",
        Some("mpeg") | Some("mp3") => "mp3",
        Some("aac") => "aac",
        Some("flac") => "flac",
        _ => "webm",
    }
}

fn not_authorized_voice_disabled(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        code: "VOICE_REPORTS_DISABLED",
        message: "Voice reporting is not available on this deployment.",
        request_id,
    }
}

#[derive(Serialize)]
pub struct TranscribeAudioResponse {
    transcript: String,
}

/// Transcribes one voice recording so the citizen app can show it back for
/// confirmation before it becomes report text (issue #160) -- this never
/// persists anything and never creates a report itself; the confirmed text
/// still goes through `create_anonymous_report` exactly like typed text.
pub async fn transcribe_audio(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<TranscribeAudioResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);

    if !state.voice_reports_enabled {
        return Err(not_authorized_voice_disabled(request_id));
    }

    enforce_rate_limit(
        state.rate_limiter.as_ref(),
        RateLimitScope::VoiceTranscriptionRequest,
        source_key_from_headers(&headers),
        request_id,
    )
    .await?;

    if body.is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "EMPTY_AUDIO",
            message: "No audio was received.",
            request_id,
        });
    }
    if body.len() > MAX_AUDIO_BYTES {
        return Err(ApiError {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            code: "AUDIO_TOO_LARGE",
            message: "The recording is too long. Please keep recordings under 2 minutes.",
            request_id,
        });
    }

    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    let format = transcription_format_from_content_type(content_type);

    let transcript = state
        .audio_transcriber
        .transcribe(&body, format)
        .await
        .map_err(|_| ApiError {
            status: StatusCode::BAD_GATEWAY,
            code: "TRANSCRIPTION_FAILED",
            message: "The recording could not be transcribed. Please try again or type your report instead.",
            request_id,
        })?;

    Ok(Json(TranscribeAudioResponse { transcript }))
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
    reported_incident_type: Option<IncidentType>,
}

fn report_summary_response(report: &ReportSummary) -> ReportSummaryResponse {
    ReportSummaryResponse {
        report_id: report.id,
        source_channel: report.source_channel,
        status: report.status,
        raw_content: report.raw_content.clone(),
        received_at: report.received_at.clone(),
        reported_incident_type: report.reported_incident_type,
    }
}

/// Lists reports before a reviewer has settled on one — `get_report` below
/// is for reading a single already-known report (e.g. one already linked to
/// a case) without re-filtering the whole list to find it again.
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

pub async fn get_report(
    State(state): State<AppState>,
    Path(report_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
) -> Result<Json<ReportSummaryResponse>, ApiError> {
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
        message: "Only an identified reviewer may view a report.",
        request_id,
    })?;

    let report = state
        .reports
        .find_by_id(ReportId::from_uuid(report_id))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "REPORT_QUERY_FAILED",
            message: "The report could not be read. Please try again.",
            request_id,
        })?
        .ok_or(ApiError {
            status: StatusCode::NOT_FOUND,
            code: "REPORT_NOT_FOUND",
            message: "No report exists with the given id.",
            request_id,
        })?;

    Ok(Json(report_summary_response(&report)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_browser_recording_content_types_to_a_provider_format() {
        assert_eq!(
            transcription_format_from_content_type(Some("audio/webm;codecs=opus")),
            "webm"
        );
        assert_eq!(
            transcription_format_from_content_type(Some("audio/ogg;codecs=opus")),
            "ogg"
        );
        assert_eq!(
            transcription_format_from_content_type(Some("audio/mp4")),
            "m4a"
        );
        assert_eq!(
            transcription_format_from_content_type(Some("audio/wav")),
            "wav"
        );
    }

    #[test]
    fn falls_back_to_webm_for_an_unknown_or_missing_content_type() {
        assert_eq!(transcription_format_from_content_type(None), "webm");
        assert_eq!(
            transcription_format_from_content_type(Some("application/octet-stream")),
            "webm"
        );
    }
}
