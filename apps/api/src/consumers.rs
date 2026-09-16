//! Consumer registration and listing (docs/API.md section 5). A consumer is
//! a receiver identity subscriptions and deliveries belong to
//! (docs/DOMAIN_MODEL.md section 9) — reviewer-managed today, matching
//! `subscriptions.rs`'s "not yet citizen self-service" note, gated by the
//! same `Capability::ManageSubscriptions` used to manage a consumer's
//! subscriptions. No audit event on registration, matching the existing
//! precedent that `PostgresSubscriptionRepository::create` itself has none
//! (only subscription *updates* are audited).

use axum::{
    Json,
    extract::{Path, Query, State},
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

const DEFAULT_LIMIT: u32 = 50;
/// A hard ceiling regardless of what the caller asks for, so a single
/// request can never force an unbounded scan/response.
const MAX_LIMIT: u32 = 100;

#[derive(Deserialize)]
pub struct ListConsumersQuery {
    consumer_type: Option<ConsumerType>,
    limit: Option<u32>,
    offset: Option<u32>,
}

/// Previously the only way to find a consumer was to already know its id
/// (from a prior `create` response) — this is a reviewer's first way to
/// browse registered consumers at all, e.g. before setting up a
/// subscription for one.
pub async fn list_consumers(
    State(state): State<AppState>,
    Query(query): Query<ListConsumersQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<ConsumerResponse>>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ManageSubscriptions).map_err(|_| not_authorized(request_id))?;

    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let consumers = state
        .consumers
        .list(query.consumer_type, limit, offset)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok(Json(consumers.iter().map(consumer_response).collect()))
}
