//! Subscription and consumer delivery-preference endpoints
//! (docs/SUBSCRIPTION_ENGINE.md). Managing either is gated by
//! [`Capability::ManageSubscriptions`] — an identified reviewer only, matching
//! the existing capability model (AGENTS.md: subscriptions are not yet
//! citizen self-service; docs/OPEN_QUESTIONS.md).
//!
//! Channel/strategy/operator values are accepted and returned as their plain
//! database strings (`ChannelType::from_database_value`,
//! `DeliveryStrategy::from_database_value`, `Comparison::from_database_value`)
//! rather than adding `serde` derives to those domain enums, mirroring how
//! `alerts.rs` validates `target_geography` as a plain `String` through
//! `TargetGeography::new` instead.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use safe_cameroon_application::authorization::{
    Capability, authorize, authorize_consumer_management,
};
use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_domain::{
    AlertVisibility, CaseEventType, ChannelEndpoint, ChannelType, Comparison, ConsumerId,
    DeliveryPreference, DeliveryPreferenceError, DeliveryStrategy, EmptySubscriptionRules, GeoArea,
    IncidentType, Membership, Role, Severity, Subscription, SubscriptionId, SubscriptionRule,
};
use safe_cameroon_infrastructure::postgres::SubscriptionUpdateOutcome;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::request_id::request_id_from_headers;
use crate::reviewer::actor_from_headers;
use crate::state::AppState;

fn persistence_failed(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "SUBSCRIPTION_PERSISTENCE_FAILED",
        message: "The request could not be saved. Please try again.",
        request_id,
    }
}

fn not_authorized(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "Only an identified reviewer may manage subscriptions.",
        request_id,
    }
}

fn not_authorized_for_consumer(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "You may not manage this organization's notification settings.",
        request_id,
    }
}

/// A `PlatformAdmin` may manage any consumer; when the consumer is linked to
/// a reviewer `Organization` (`Organization::consumer_id`), only a reviewer
/// belonging to that same organization may manage it. A consumer with no
/// linked organization (a standalone consumer, or a citizen's self-managed
/// one) has no real owner to scope against, so any identified reviewer may
/// still manage it — see `authorize_consumer_management`'s doc comment.
async fn authorize_consumer_access(
    state: &AppState,
    actor: Actor,
    consumer_id: ConsumerId,
    request_id: Uuid,
) -> Result<(), ApiError> {
    let Actor::Reviewer(reviewer_id) = actor else {
        // `authorize(actor, Capability::ManageSubscriptions)` already
        // rejected `Actor::Automated` before this is ever called.
        return Err(not_authorized(request_id));
    };
    let membership = state
        .organizations
        .find_membership(reviewer_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
        .unwrap_or(Membership {
            role: Role::Member,
            organization_id: None,
        });
    let owning_organization_id = state
        .organizations
        .find_organization_id_by_consumer_id(consumer_id)
        .await
        .map_err(|_| persistence_failed(request_id))?;
    if let Some(owning_organization_id) = owning_organization_id {
        let organization = state
            .organizations
            .find_by_id(owning_organization_id)
            .await
            .map_err(|_| persistence_failed(request_id))?;
        let is_active = organization
            .map(|organization| organization.is_active())
            .unwrap_or(true);
        if !is_active && membership.role != Role::PlatformAdmin {
            return Err(not_authorized_for_consumer(request_id));
        }
    }
    authorize_consumer_management(membership, owning_organization_id)
        .map_err(|_| not_authorized_for_consumer(request_id))
}

// --- Subscription rules ----------------------------------------------------

#[derive(Deserialize)]
#[serde(tag = "rule", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SubscriptionRuleInput {
    IncidentType { values: Vec<IncidentType> },
    Severity { operator: String, value: Severity },
    EventType { values: Vec<CaseEventType> },
    Geography { areas: Vec<String> },
    Visibility { values: Vec<AlertVisibility> },
}

fn to_domain_rule(
    input: SubscriptionRuleInput,
    request_id: Uuid,
) -> Result<SubscriptionRule, ApiError> {
    match input {
        SubscriptionRuleInput::IncidentType { values } => {
            Ok(SubscriptionRule::IncidentType(values))
        }
        SubscriptionRuleInput::Severity { operator, value } => {
            let operator = Comparison::from_database_value(&operator).ok_or(ApiError {
                status: StatusCode::BAD_REQUEST,
                code: "INVALID_SEVERITY_OPERATOR",
                message: "operator must be one of GREATER_THAN, GREATER_THAN_OR_EQUAL, EQUAL, LESS_THAN_OR_EQUAL, LESS_THAN.",
                request_id,
            })?;
            Ok(SubscriptionRule::Severity { operator, value })
        }
        SubscriptionRuleInput::EventType { values } => Ok(SubscriptionRule::EventType(values)),
        SubscriptionRuleInput::Geography { areas } => {
            let areas: Vec<GeoArea> = areas
                .into_iter()
                .map(GeoArea::new)
                .collect::<Result<_, _>>()
                .map_err(|_| ApiError {
                    status: StatusCode::BAD_REQUEST,
                    code: "INVALID_GEOGRAPHY_AREA",
                    message: "each area must be non-blank.",
                    request_id,
                })?;
            if areas.is_empty() {
                return Err(ApiError {
                    status: StatusCode::BAD_REQUEST,
                    code: "INVALID_GEOGRAPHY_AREA",
                    message: "areas must contain at least one area.",
                    request_id,
                });
            }
            Ok(SubscriptionRule::Geography(areas))
        }
        SubscriptionRuleInput::Visibility { values } => Ok(SubscriptionRule::Visibility(values)),
    }
}

#[derive(Serialize)]
#[serde(tag = "rule", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SubscriptionRuleOutput {
    IncidentType {
        values: Vec<IncidentType>,
    },
    Severity {
        operator: &'static str,
        value: Severity,
    },
    EventType {
        values: Vec<CaseEventType>,
    },
    Geography {
        areas: Vec<String>,
    },
    Visibility {
        values: Vec<AlertVisibility>,
    },
}

fn rule_output(rule: &SubscriptionRule) -> SubscriptionRuleOutput {
    match rule {
        SubscriptionRule::IncidentType(values) => SubscriptionRuleOutput::IncidentType {
            values: values.clone(),
        },
        SubscriptionRule::Severity { operator, value } => SubscriptionRuleOutput::Severity {
            operator: operator.as_database_value(),
            value: *value,
        },
        SubscriptionRule::EventType(values) => SubscriptionRuleOutput::EventType {
            values: values.clone(),
        },
        SubscriptionRule::Geography(areas) => SubscriptionRuleOutput::Geography {
            areas: areas.iter().map(|area| area.as_str().to_owned()).collect(),
        },
        SubscriptionRule::Visibility(values) => SubscriptionRuleOutput::Visibility {
            values: values.clone(),
        },
    }
}

#[derive(Deserialize)]
pub struct CreateSubscriptionRequest {
    consumer_id: Uuid,
    rules: Vec<SubscriptionRuleInput>,
}

#[derive(Serialize)]
pub struct SubscriptionResponse {
    subscription_id: Uuid,
    consumer_id: Uuid,
    version: u32,
    rules: Vec<SubscriptionRuleOutput>,
}

fn subscription_response(subscription: &Subscription) -> SubscriptionResponse {
    SubscriptionResponse {
        subscription_id: subscription.id().as_uuid(),
        consumer_id: subscription.consumer_id().as_uuid(),
        version: subscription.version(),
        rules: subscription.rules().iter().map(rule_output).collect(),
    }
}

pub async fn create_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateSubscriptionRequest>,
) -> Result<(StatusCode, Json<SubscriptionResponse>), ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ManageSubscriptions).map_err(|_| not_authorized(request_id))?;
    let consumer_id = ConsumerId::from_uuid(request.consumer_id);
    authorize_consumer_access(&state, actor, consumer_id, request_id).await?;

    let rules = request
        .rules
        .into_iter()
        .map(|rule| to_domain_rule(rule, request_id))
        .collect::<Result<Vec<_>, _>>()?;

    let subscription = Subscription::new(SubscriptionId::new(), consumer_id, 1, rules).map_err(
        |EmptySubscriptionRules| ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "EMPTY_SUBSCRIPTION_RULES",
            message: "rules must contain at least one rule.",
            request_id,
        },
    )?;

    state
        .subscriptions
        .create(&subscription)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok((
        StatusCode::CREATED,
        Json(subscription_response(&subscription)),
    ))
}

const DEFAULT_LIMIT: u32 = 50;
/// A hard ceiling regardless of what the caller asks for, so a single
/// request can never force an unbounded scan/response.
const MAX_LIMIT: u32 = 100;

#[derive(Deserialize)]
pub struct ListAllSubscriptionsQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

/// Every subscription across every consumer, most recently created first --
/// deliberately not org-scoped, matching how `list_alerts`/`list_organizations`
/// also give any identified reviewer flat bulk visibility (subscriptions are
/// not yet citizen self-service, AGENTS.md; a finer-grained restriction is a
/// separate, unrequested concern).
pub async fn list_all_subscriptions(
    State(state): State<AppState>,
    Query(query): Query<ListAllSubscriptionsQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<SubscriptionResponse>>, ApiError> {
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

    let subscriptions = state
        .subscriptions
        .list_all_page(limit, offset)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok(Json(
        subscriptions.iter().map(subscription_response).collect(),
    ))
}

pub async fn list_subscriptions_for_consumer(
    State(state): State<AppState>,
    Path(consumer_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<SubscriptionResponse>>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ManageSubscriptions).map_err(|_| not_authorized(request_id))?;
    let consumer_id = ConsumerId::from_uuid(consumer_id);
    authorize_consumer_access(&state, actor, consumer_id, request_id).await?;

    let subscriptions = state
        .subscriptions
        .find_by_consumer(consumer_id)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok(Json(
        subscriptions.iter().map(subscription_response).collect(),
    ))
}

#[derive(Deserialize)]
pub struct UpdateSubscriptionRequest {
    rules: Vec<SubscriptionRuleInput>,
}

/// Replaces a subscription's rule set wholesale (docs/SUBSCRIPTION_ENGINE.md
/// section 11), bumping its version. Optimistic concurrency on the version
/// read at the start of this request means a concurrent edit is reported as
/// `SUBSCRIPTION_MODIFIED_CONCURRENTLY` rather than silently overwritten.
pub async fn update_subscription(
    State(state): State<AppState>,
    Path(subscription_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<UpdateSubscriptionRequest>,
) -> Result<Json<SubscriptionResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ManageSubscriptions).map_err(|_| not_authorized(request_id))?;

    let mut subscription = state
        .subscriptions
        .find_by_id(SubscriptionId::from_uuid(subscription_id))
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or(ApiError {
            status: StatusCode::NOT_FOUND,
            code: "SUBSCRIPTION_NOT_FOUND",
            message: "No subscription exists with the given id.",
            request_id,
        })?;
    authorize_consumer_access(&state, actor, subscription.consumer_id(), request_id).await?;

    let rules = request
        .rules
        .into_iter()
        .map(|rule| to_domain_rule(rule, request_id))
        .collect::<Result<Vec<_>, _>>()?;

    subscription
        .update_rules(rules)
        .map_err(|EmptySubscriptionRules| ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "EMPTY_SUBSCRIPTION_RULES",
            message: "rules must contain at least one rule.",
            request_id,
        })?;

    match state
        .subscriptions
        .update(&subscription, actor, request_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
    {
        SubscriptionUpdateOutcome::Updated => Ok(Json(subscription_response(&subscription))),
        SubscriptionUpdateOutcome::Conflict => Err(ApiError {
            status: StatusCode::CONFLICT,
            code: "SUBSCRIPTION_MODIFIED_CONCURRENTLY",
            message: "The subscription changed since it was last read. Reload and retry.",
            request_id,
        }),
    }
}

// --- Delivery preference -----------------------------------------------------

#[derive(Deserialize)]
pub struct ChannelEndpointInput {
    channel: String,
    address: String,
}

#[derive(Deserialize)]
pub struct SetDeliveryPreferenceRequest {
    strategy: String,
    channels: Vec<ChannelEndpointInput>,
}

#[derive(Serialize)]
pub struct ChannelEndpointOutput {
    channel: &'static str,
    address: String,
}

#[derive(Serialize)]
pub struct DeliveryPreferenceResponse {
    consumer_id: Uuid,
    strategy: &'static str,
    channels: Vec<ChannelEndpointOutput>,
}

fn delivery_preference_response(
    consumer_id: ConsumerId,
    preference: &DeliveryPreference,
) -> DeliveryPreferenceResponse {
    DeliveryPreferenceResponse {
        consumer_id: consumer_id.as_uuid(),
        strategy: preference.strategy().as_database_value(),
        channels: preference
            .channels()
            .iter()
            .map(|endpoint| ChannelEndpointOutput {
                channel: endpoint.channel().as_database_value(),
                address: endpoint.address().to_owned(),
            })
            .collect(),
    }
}

pub async fn set_delivery_preference(
    State(state): State<AppState>,
    Path(consumer_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<SetDeliveryPreferenceRequest>,
) -> Result<Json<DeliveryPreferenceResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ManageSubscriptions).map_err(|_| not_authorized(request_id))?;
    let consumer_id = ConsumerId::from_uuid(consumer_id);
    authorize_consumer_access(&state, actor, consumer_id, request_id).await?;

    let strategy = DeliveryStrategy::from_database_value(&request.strategy).ok_or(ApiError {
        status: StatusCode::BAD_REQUEST,
        code: "INVALID_DELIVERY_STRATEGY",
        message: "strategy must be one of ALL, PRIMARY_FALLBACK, PRIORITY_LIST.",
        request_id,
    })?;

    let channels = request
        .channels
        .into_iter()
        .map(|input| {
            let channel = ChannelType::from_database_value(&input.channel).ok_or(ApiError {
                status: StatusCode::BAD_REQUEST,
                code: "INVALID_CHANNEL",
                message: "channel must be one of WHATSAPP, SMS, EMAIL.",
                request_id,
            })?;
            ChannelEndpoint::new(channel, input.address).map_err(|_| ApiError {
                status: StatusCode::BAD_REQUEST,
                code: "INVALID_CHANNEL_ENDPOINT_ADDRESS",
                message: "address cannot be blank.",
                request_id,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let preference = DeliveryPreference::new(strategy, channels).map_err(|error| match error {
        DeliveryPreferenceError::EmptyChannelList => ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "EMPTY_DELIVERY_CHANNELS",
            message: "channels must contain at least one channel.",
            request_id,
        },
        DeliveryPreferenceError::DuplicateChannel { .. } => ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "DUPLICATE_DELIVERY_CHANNEL",
            message: "each channel may be listed at most once.",
            request_id,
        },
    })?;

    state
        .delivery_preferences
        .upsert(consumer_id, &preference)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok(Json(delivery_preference_response(consumer_id, &preference)))
}

pub async fn get_delivery_preference(
    State(state): State<AppState>,
    Path(consumer_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DeliveryPreferenceResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ManageSubscriptions).map_err(|_| not_authorized(request_id))?;
    let consumer_id = ConsumerId::from_uuid(consumer_id);
    authorize_consumer_access(&state, actor, consumer_id, request_id).await?;

    let preference = state
        .delivery_preferences
        .find_by_consumer(consumer_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or(ApiError {
            status: StatusCode::NOT_FOUND,
            code: "DELIVERY_PREFERENCE_NOT_FOUND",
            message: "No delivery preference is set for this consumer.",
            request_id,
        })?;

    Ok(Json(delivery_preference_response(consumer_id, &preference)))
}
