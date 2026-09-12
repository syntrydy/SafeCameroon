//! Provider webhook endpoint (docs/API.md section 7). A provider callback
//! is untrusted input from the public internet: nothing here ever mutates
//! anything before the raw request passes `WebhookVerifier` and
//! `WebhookReplayGuard`, and this handler only ever touches `Delivery`
//! state — never `Alert`/`Case` truth.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_application::delivery_workflow::{WebhookAppliedTransition, apply_webhook_event};
use safe_cameroon_domain::ChannelType;
use safe_cameroon_infrastructure::postgres::DeliveryTransitionOutcome;
use uuid::Uuid;

use crate::error::ApiError;
use crate::request_id::request_id_from_headers;
use crate::state::AppState;

fn persistence_failed(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "WEBHOOK_PERSISTENCE_FAILED",
        message: "The webhook event could not be recorded. Please retry.",
        request_id,
    }
}

/// Always `Ok(StatusCode::OK)` once verification/dedup pass, even when there
/// is nothing left to do (an unknown delivery, a stale transition, or a
/// concurrent update already applied it) — a webhook provider retries an
/// endpoint that responds with an error, and retrying would not change any
/// of those outcomes, so returning 200 is what actually stops the retries.
pub async fn receive_webhook(
    State(state): State<AppState>,
    Path((channel_segment, provider)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let request_id = request_id_from_headers(&headers);

    let channel = ChannelType::from_database_value(&channel_segment.to_ascii_uppercase()).ok_or(
        ApiError {
            status: StatusCode::NOT_FOUND,
            code: "UNKNOWN_WEBHOOK_CHANNEL",
            message: "No channel is registered with this name.",
            request_id,
        },
    )?;
    let verifier = state
        .webhook_verifiers
        .get(channel, &provider)
        .ok_or(ApiError {
            status: StatusCode::NOT_FOUND,
            code: "UNKNOWN_WEBHOOK_PROVIDER",
            message: "No webhook provider is registered for this channel.",
            request_id,
        })?;

    let header_pairs: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_owned(), value.to_owned()))
        })
        .collect();

    let event = verifier.verify_and_parse(&header_pairs, &body).map_err(|_| {
        tracing::warn!(%request_id, channel = ?channel, %provider, "webhook verification failed");
        ApiError {
            status: StatusCode::UNAUTHORIZED,
            code: "WEBHOOK_VERIFICATION_FAILED",
            message: "The webhook signature or payload could not be verified.",
            request_id,
        }
    })?;

    let is_new = state
        .webhook_replay_guard
        .record_if_new(channel, &event.provider_event_id)
        .await
        .map_err(|_| persistence_failed(request_id))?;
    if !is_new {
        tracing::info!(%request_id, channel = ?channel, provider_event_id = %event.provider_event_id, "duplicate webhook event ignored");
        return Ok(StatusCode::OK);
    }

    let Some(mut delivery) = state
        .deliveries
        .find_by_provider_message_id(&event.provider_message_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
    else {
        tracing::warn!(%request_id, channel = ?channel, "webhook event refers to an unknown delivery");
        return Ok(StatusCode::OK);
    };
    let delivery_id = delivery.id();

    let applied = match apply_webhook_event(&mut delivery, &event, Actor::Automated, request_id) {
        Ok(applied) => applied,
        // The delivery is not in a state this event makes sense for (e.g.
        // already terminal); nothing more to do.
        Err(_) => {
            tracing::warn!(%request_id, delivery_id = %delivery_id.as_uuid(), "webhook event does not match the delivery's current state");
            return Ok(StatusCode::OK);
        }
    };

    let outcome = match applied {
        WebhookAppliedTransition::Delivered(transition) => {
            state
                .deliveries
                .apply_transition(&delivery, &transition)
                .await
        }
        WebhookAppliedTransition::Failed(transition) => {
            state
                .deliveries
                .apply_attempt_transition(&delivery, &transition)
                .await
        }
    }
    .map_err(|_| persistence_failed(request_id))?;

    tracing::info!(%request_id, delivery_id = %delivery_id.as_uuid(), channel = ?channel, ?outcome, "webhook event applied");
    match outcome {
        DeliveryTransitionOutcome::Applied | DeliveryTransitionOutcome::Conflict => {
            Ok(StatusCode::OK)
        }
    }
}
