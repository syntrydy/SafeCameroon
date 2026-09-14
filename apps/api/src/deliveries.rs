//! Delivery visibility (docs/OBSERVABILITY.md: "which deliveries were
//! attempted for this alert?", "why did a delivery fail?"). Read-only,
//! gated by the same [`Capability::ViewCase`] attachment download already
//! uses — an identified reviewer, not a new capability for what is
//! structurally the same "may this reviewer see case-adjacent detail"
//! question.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use safe_cameroon_application::authorization::{Capability, authorize};
use safe_cameroon_domain::{
    AlertId, Delivery, DeliveryAttempt, DeliveryAttemptOutcome, DeliveryId,
};
use serde::Serialize;
use uuid::Uuid;

use crate::error::ApiError;
use crate::request_id::request_id_from_headers;
use crate::reviewer::actor_from_headers;
use crate::state::AppState;

fn persistence_failed(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "DELIVERY_PERSISTENCE_FAILED",
        message: "The request could not be completed. Please try again.",
        request_id,
    }
}

fn not_authorized(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "Only an identified reviewer may view delivery detail.",
        request_id,
    }
}

#[derive(Serialize)]
pub struct DeliveryResponse {
    delivery_id: Uuid,
    alert_id: Uuid,
    consumer_id: Uuid,
    channel: &'static str,
    endpoint_address: String,
    tier: u8,
    status: &'static str,
    attempt_count: u32,
    max_attempts: u32,
    version: u64,
}

fn delivery_response(delivery: &Delivery) -> DeliveryResponse {
    DeliveryResponse {
        delivery_id: delivery.id().as_uuid(),
        alert_id: delivery.alert_id().as_uuid(),
        consumer_id: delivery.consumer_id().as_uuid(),
        channel: delivery.channel().as_database_value(),
        endpoint_address: delivery.endpoint_address().to_owned(),
        tier: delivery.tier(),
        status: delivery.status().as_database_value(),
        attempt_count: delivery.attempt_count(),
        max_attempts: delivery.max_attempts(),
        version: delivery.version(),
    }
}

pub async fn list_deliveries_for_alert(
    State(state): State<AppState>,
    Path(alert_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<DeliveryResponse>>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ViewCase).map_err(|_| not_authorized(request_id))?;

    let deliveries = state
        .deliveries
        .find_by_alert_id(AlertId::from_uuid(alert_id))
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok(Json(deliveries.iter().map(delivery_response).collect()))
}

#[derive(Serialize)]
pub struct DeliveryAttemptResponse {
    attempt_number: u32,
    outcome: &'static str,
    provider_message_id: Option<String>,
    retryable: Option<bool>,
    failure_reason: Option<String>,
}

fn attempt_response(attempt: &DeliveryAttempt) -> DeliveryAttemptResponse {
    match &attempt.outcome {
        DeliveryAttemptOutcome::Sent {
            provider_message_id,
        } => DeliveryAttemptResponse {
            attempt_number: attempt.attempt_number,
            outcome: "SENT",
            provider_message_id: provider_message_id.clone(),
            retryable: None,
            failure_reason: None,
        },
        DeliveryAttemptOutcome::Failed { retryable, reason } => DeliveryAttemptResponse {
            attempt_number: attempt.attempt_number,
            outcome: "FAILED",
            provider_message_id: None,
            retryable: Some(*retryable),
            failure_reason: Some(reason.clone()),
        },
    }
}

#[derive(Serialize)]
pub struct DeliveryDetailResponse {
    #[serde(flatten)]
    delivery: DeliveryResponse,
    attempts: Vec<DeliveryAttemptResponse>,
}

pub async fn get_delivery(
    State(state): State<AppState>,
    Path(delivery_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DeliveryDetailResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ViewCase).map_err(|_| not_authorized(request_id))?;

    let delivery_id = DeliveryId::from_uuid(delivery_id);
    let delivery = state
        .deliveries
        .find_by_id(delivery_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or(ApiError {
            status: StatusCode::NOT_FOUND,
            code: "DELIVERY_NOT_FOUND",
            message: "No delivery exists with the given id.",
            request_id,
        })?;
    let attempts = state
        .deliveries
        .find_attempts(delivery_id)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok(Json(DeliveryDetailResponse {
        delivery: delivery_response(&delivery),
        attempts: attempts.iter().map(attempt_response).collect(),
    }))
}
