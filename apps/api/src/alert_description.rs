//! LLM-assisted alert-description generation (docs/AI.md, CLAUDE.md "AI
//! integration"). Gated the same way as report extraction
//! (`Capability::ViewCase`) -- this is an aid to drafting an alert, not a
//! distinct privilege, and it never changes case or alert state itself.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use safe_cameroon_application::alert_description_generation::request_description_generation;
use safe_cameroon_application::authorization::{Capability, authorize};
use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_domain::CaseId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::request_id::request_id_from_headers;
use crate::reviewer::actor_from_headers;
use crate::state::AppState;

fn not_authorized(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "Only an identified reviewer may request a description suggestion.",
        request_id,
    }
}

fn case_not_found(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        code: "CASE_NOT_FOUND",
        message: "No case exists with the given id.",
        request_id,
    }
}

fn persistence_failed(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "ALERT_DESCRIPTION_GENERATION_PERSISTENCE_FAILED",
        message: "The suggestion could not be saved. Please try again.",
        request_id,
    }
}

#[derive(Deserialize)]
pub struct GenerateAlertDescriptionRequest {
    source_text: String,
}

#[derive(Serialize)]
pub struct AlertDescriptionSuggestionResponse {
    description_en: String,
    description_fr: String,
    provider: String,
    model: String,
    prompt_version: String,
}

/// Requests a fresh formal EN/FR pair from the configured provider and
/// persists it -- never a cache lookup, mirroring `create_extraction`'s
/// "a reviewer explicitly asking again should get a fresh attempt."
pub async fn generate_alert_description(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
    Json(request): Json<GenerateAlertDescriptionRequest>,
) -> Result<(StatusCode, Json<AlertDescriptionSuggestionResponse>), ApiError> {
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

    let source_text = request.source_text.trim();
    if source_text.is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "EMPTY_SOURCE_TEXT",
            message: "source_text cannot be blank.",
            request_id,
        });
    }

    let case_id = CaseId::from_uuid(case_id);
    state
        .cases
        .find_by_id(case_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or_else(|| case_not_found(request_id))?;

    let record = request_description_generation(
        state.alert_description_generator.as_ref(),
        case_id,
        source_text,
        reviewer_id,
    )
    .await
    .map_err(|_| ApiError {
        status: StatusCode::BAD_GATEWAY,
        code: "ALERT_DESCRIPTION_GENERATION_FAILED",
        message: "The suggestion could not be generated. Please try again.",
        request_id,
    })?;

    state
        .alert_description_generations
        .create(&record)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok((
        StatusCode::CREATED,
        Json(AlertDescriptionSuggestionResponse {
            description_en: record.description.description_en,
            description_fr: record.description.description_fr,
            provider: record.provider,
            model: record.model,
            prompt_version: record.prompt_version,
        }),
    ))
}
