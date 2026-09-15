//! Consumer registration (docs/API.md section 5). A consumer is a receiver
//! identity subscriptions and deliveries belong to (docs/DOMAIN_MODEL.md
//! section 9) — reviewer-managed today, matching `subscriptions.rs`'s "not
//! yet citizen self-service" note, gated by the same
//! `Capability::ManageSubscriptions` used to manage a consumer's
//! subscriptions. No audit event, matching the existing precedent that
//! `PostgresSubscriptionRepository::create` itself has none (only
//! subscription *updates* are audited).

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use safe_cameroon_application::authorization::{Capability, authorize};
use safe_cameroon_domain::{Consumer, ConsumerId, ConsumerType, EmptyConsumerName};
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
        message: "Only an identified reviewer may manage consumers.",
        request_id,
    }
}

fn consumer_not_found(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        code: "CONSUMER_NOT_FOUND",
        message: "No consumer exists with the given id.",
        request_id,
    }
}

fn persistence_failed(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "CONSUMER_PERSISTENCE_FAILED",
        message: "The consumer could not be saved. Please try again.",
        request_id,
    }
}

#[derive(Deserialize)]
pub struct CreateConsumerRequest {
    name: String,
    consumer_type: ConsumerType,
}

#[derive(Serialize)]
pub struct ConsumerResponse {
    consumer_id: Uuid,
    name: String,
    consumer_type: ConsumerType,
}

fn consumer_response(consumer: &Consumer) -> ConsumerResponse {
    ConsumerResponse {
        consumer_id: consumer.id().as_uuid(),
        name: consumer.name().to_owned(),
        consumer_type: consumer.consumer_type(),
    }
}

pub async fn register_consumer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateConsumerRequest>,
) -> Result<(StatusCode, Json<ConsumerResponse>), ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ManageSubscriptions).map_err(|_| not_authorized(request_id))?;

    let consumer =
        Consumer::new(request.name, request.consumer_type).map_err(|EmptyConsumerName| {
            ApiError {
                status: StatusCode::BAD_REQUEST,
                code: "INVALID_CONSUMER_NAME",
                message: "name cannot be blank.",
                request_id,
            }
        })?;

    state
        .consumers
        .create(&consumer)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok((StatusCode::CREATED, Json(consumer_response(&consumer))))
}

pub async fn get_consumer(
    State(state): State<AppState>,
    Path(consumer_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ConsumerResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ManageSubscriptions).map_err(|_| not_authorized(request_id))?;

    let consumer = state
        .consumers
        .find_by_id(ConsumerId::from_uuid(consumer_id))
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or_else(|| consumer_not_found(request_id))?;

    Ok(Json(consumer_response(&consumer)))
}
