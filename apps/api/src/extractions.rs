//! AI report-extraction endpoints (docs/AI.md, CLAUDE.md "AI integration").
//! Gated the same way as every other report-read endpoint
//! (`Capability::ViewCase`) -- extraction is an aid to reading a report, not
//! a distinct privilege, and it never changes report or case state itself.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use safe_cameroon_application::ai_extraction::request_report_extraction;
use safe_cameroon_application::authorization::{Capability, authorize};
use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_domain::{ExtractedReportFields, ReportId};
use serde::Serialize;
use uuid::Uuid;

use crate::error::ApiError;
use crate::request_id::request_id_from_headers;
use crate::reviewer::actor_from_headers;
use crate::state::AppState;

fn not_authorized(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "Only an identified reviewer may request an extraction.",
        request_id,
    }
}

fn report_not_found(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        code: "REPORT_NOT_FOUND",
        message: "No report exists with the given id.",
        request_id,
    }
}

fn persistence_failed(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "EXTRACTION_PERSISTENCE_FAILED",
        message: "The extraction could not be saved. Please try again.",
        request_id,
    }
}

#[derive(Serialize)]
pub struct ExtractedFieldsResponse {
    person_description: Option<String>,
    age: Option<String>,
    time: Option<String>,
    place: Option<String>,
    incident_category: Option<String>,
    vehicle_details: Option<String>,
    contact_request: Option<String>,
}

impl From<&ExtractedReportFields> for ExtractedFieldsResponse {
    fn from(fields: &ExtractedReportFields) -> Self {
        Self {
            person_description: fields.person_description.clone(),
            age: fields.age.clone(),
            time: fields.time.clone(),
            place: fields.place.clone(),
            incident_category: fields.incident_category.clone(),
            vehicle_details: fields.vehicle_details.clone(),
            contact_request: fields.contact_request.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct ExtractionResponse {
    report_id: Uuid,
    requested_by: Uuid,
    provider: String,
    model: String,
    prompt_version: String,
    fields: ExtractedFieldsResponse,
}

/// Requests a fresh extraction from the configured provider and persists it
/// -- never a cache lookup, since a reviewer explicitly asking again (e.g.
/// after a follow-up adds detail to the report) should get a fresh attempt.
pub async fn create_extraction(
    State(state): State<AppState>,
    Path(report_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
) -> Result<(StatusCode, Json<ExtractionResponse>), ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ViewCase).map_err(|_| not_authorized(request_id))?;
    let Actor::Reviewer(reviewer_id) = actor else {
        return Err(not_authorized(request_id));
    };

    let report_id = ReportId::from_uuid(report_id);
    let report = state
        .reports
        .find_by_id(report_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or_else(|| report_not_found(request_id))?;

    let record = request_report_extraction(
        state.report_extractor.as_ref(),
        report_id,
        &report.raw_content,
        reviewer_id,
    )
    .await
    .map_err(|error| ApiError {
        status: StatusCode::BAD_GATEWAY,
        code: "EXTRACTION_FAILED",
        message: extraction_failure_message(&error),
        request_id,
    })?;

    state
        .extractions
        .create(&record)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok((
        StatusCode::CREATED,
        Json(ExtractionResponse {
            report_id: record.report_id.as_uuid(),
            requested_by: record.requested_by,
            provider: record.provider,
            model: record.model,
            prompt_version: record.prompt_version,
            fields: ExtractedFieldsResponse::from(&record.fields),
        }),
    ))
}

/// A single stable message regardless of the underlying provider error
/// (network, timeout, malformed schema): the reviewer-facing action either
/// way is "try again", and the real cause belongs in server logs, not a
/// client-facing string built from a provider's own error text.
fn extraction_failure_message(
    _error: &safe_cameroon_application::ai_extraction::ExtractionError,
) -> &'static str {
    "The extraction could not be completed. Please try again."
}

pub async fn list_extractions(
    State(state): State<AppState>,
    Path(report_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<ExtractionResponse>>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ViewCase).map_err(|_| not_authorized(request_id))?;

    let report_id = ReportId::from_uuid(report_id);
    if !state
        .reports
        .exists(report_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
    {
        return Err(report_not_found(request_id));
    }

    let records = state
        .extractions
        .find_by_report(report_id)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok(Json(
        records
            .iter()
            .map(|record| ExtractionResponse {
                report_id: record.report_id.as_uuid(),
                requested_by: record.requested_by,
                provider: record.provider.clone(),
                model: record.model.clone(),
                prompt_version: record.prompt_version.clone(),
                fields: ExtractedFieldsResponse::from(&record.fields),
            })
            .collect(),
    ))
}
