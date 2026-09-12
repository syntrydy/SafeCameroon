//! Delivery workflow use cases: turning matched subscriptions into planned
//! deliveries, and recording each attempt's outcome. Reuses
//! [`Actor`](crate::case_workflow::Actor) so every audit trail in the system
//! shares one vocabulary, even though in practice a delivery is almost always
//! driven by [`Actor::Automated`] (a worker), not a human reviewer.

use std::collections::HashMap;

use safe_cameroon_domain::{
    Alert, AuditEventId, ConsumerId, ConsumerMatch, Delivery, DeliveryAttempt, DeliveryEvent,
    DeliveryPreference, DeliveryTransitionError, OutboxEventId, RetryPolicy,
    plan_deliveries as plan_deliveries_domain,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::case_workflow::Actor;

/// A single planned delivery plus the metadata needed to persist it: the
/// `DELIVERY_REQUESTED` event, an idempotency-key digest (mirroring how
/// report idempotency keys are hashed before they reach persistence), and
/// audit/outbox identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedDelivery {
    pub delivery: Delivery,
    pub event: DeliveryEvent,
    pub idempotency_key_hash: Vec<u8>,
    pub actor: Actor,
    pub audit_event_id: AuditEventId,
    pub outbox_event_id: OutboxEventId,
    pub request_id: Uuid,
}

/// Plans deliveries for an alert's matched consumers (docs/SUBSCRIPTION_ENGINE.md
/// section 2), attaching the audit/outbox metadata each planned delivery
/// needs before it can be persisted.
pub fn plan_deliveries(
    alert: &Alert,
    consumer_matches: &[ConsumerMatch],
    preferences: &HashMap<ConsumerId, DeliveryPreference>,
    retry_policy: RetryPolicy,
    actor: Actor,
    request_id: Uuid,
) -> Vec<PlannedDelivery> {
    plan_deliveries_domain(alert, consumer_matches, preferences, retry_policy)
        .into_iter()
        .map(|(delivery, event)| {
            let idempotency_key_hash =
                Sha256::digest(delivery.idempotency_key().as_str().as_bytes()).to_vec();
            PlannedDelivery {
                delivery,
                event,
                idempotency_key_hash,
                actor,
                audit_event_id: AuditEventId::new(),
                outbox_event_id: OutboxEventId::new(),
                request_id,
            }
        })
        .collect()
}

/// Deliberately omits `endpoint_address`: it is a consumer contact address,
/// not case/alert content, but keeping the outbox payload to routing
/// metadata only avoids giving every outbox consumer contact-list access
/// (docs/DATA_GOVERNANCE.md), mirroring `alert_created_event_payload`.
pub fn delivery_requested_event_payload(planned: &PlannedDelivery) -> serde_json::Value {
    json!({
        "delivery_id": planned.delivery.id(),
        "alert_id": planned.delivery.alert_id(),
        "consumer_id": planned.delivery.consumer_id(),
        "channel": planned.delivery.channel().as_database_value(),
        "tier": planned.delivery.tier(),
    })
}

/// A transition that produces an event but no [`DeliveryAttempt`]
/// (`start_attempt`, `record_delivered`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryTransition {
    pub event: DeliveryEvent,
    pub actor: Actor,
    pub audit_event_id: AuditEventId,
    pub outbox_event_id: OutboxEventId,
    pub request_id: Uuid,
}

impl DeliveryTransition {
    fn new(event: DeliveryEvent, actor: Actor, request_id: Uuid) -> Self {
        Self {
            event,
            actor,
            audit_event_id: AuditEventId::new(),
            outbox_event_id: OutboxEventId::new(),
            request_id,
        }
    }
}

/// A transition that also produces a [`DeliveryAttempt`] (`record_success`,
/// `record_failure`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryAttemptTransition {
    pub event: DeliveryEvent,
    pub attempt: DeliveryAttempt,
    pub actor: Actor,
    pub audit_event_id: AuditEventId,
    pub outbox_event_id: OutboxEventId,
    pub request_id: Uuid,
}

impl DeliveryAttemptTransition {
    fn new(event: DeliveryEvent, attempt: DeliveryAttempt, actor: Actor, request_id: Uuid) -> Self {
        Self {
            event,
            attempt,
            actor,
            audit_event_id: AuditEventId::new(),
            outbox_event_id: OutboxEventId::new(),
            request_id,
        }
    }
}

fn delivery_event_payload(delivery: &Delivery, event: &DeliveryEvent) -> serde_json::Value {
    json!({
        "delivery_id": delivery.id(),
        "alert_id": delivery.alert_id(),
        "consumer_id": delivery.consumer_id(),
        "channel": delivery.channel().as_database_value(),
        "aggregate_version": event.aggregate_version,
    })
}

pub fn start_delivery_attempt(
    delivery: &mut Delivery,
    actor: Actor,
    request_id: Uuid,
) -> Result<DeliveryTransition, DeliveryTransitionError> {
    let event = delivery.start_attempt()?;
    Ok(DeliveryTransition::new(event, actor, request_id))
}

pub fn record_delivery_success(
    delivery: &mut Delivery,
    provider_message_id: Option<String>,
    actor: Actor,
    request_id: Uuid,
) -> Result<DeliveryAttemptTransition, DeliveryTransitionError> {
    let (event, attempt) = delivery.record_success(provider_message_id)?;
    Ok(DeliveryAttemptTransition::new(
        event, attempt, actor, request_id,
    ))
}

pub fn record_delivery_delivered(
    delivery: &mut Delivery,
    actor: Actor,
    request_id: Uuid,
) -> Result<DeliveryTransition, DeliveryTransitionError> {
    let event = delivery.record_delivered()?;
    Ok(DeliveryTransition::new(event, actor, request_id))
}

pub fn record_delivery_failure(
    delivery: &mut Delivery,
    retryable: bool,
    reason: impl Into<String>,
    actor: Actor,
    request_id: Uuid,
) -> Result<DeliveryAttemptTransition, DeliveryTransitionError> {
    let (event, attempt) = delivery.record_failure(retryable, reason)?;
    Ok(DeliveryAttemptTransition::new(
        event, attempt, actor, request_id,
    ))
}

pub fn delivery_transition_event_payload(
    delivery: &Delivery,
    transition: &DeliveryTransition,
) -> serde_json::Value {
    delivery_event_payload(delivery, &transition.event)
}

pub fn delivery_attempt_event_payload(
    delivery: &Delivery,
    transition: &DeliveryAttemptTransition,
) -> serde_json::Value {
    delivery_event_payload(delivery, &transition.event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use safe_cameroon_domain::{
        AlertField, AlertFieldValue, AlertPolicy, Case, CaseStatus, ChannelEndpoint, ChannelType,
        DeliveryEventType, DeliveryStatus, DeliveryStrategy, IncidentType, ReportId, Severity,
        SubscriptionId, TargetGeography,
    };

    fn alert() -> Alert {
        let (mut case, _) = Case::create(IncidentType::MissingChild, ReportId::new());
        case.transition_to(CaseStatus::UnderReview).unwrap();
        case.transition_to(CaseStatus::Verified).unwrap();
        let policy = AlertPolicy::missing_child_community_v1();
        let (alert, _) = Alert::create_from_case(
            &case,
            &policy,
            Severity::High,
            TargetGeography::new("Douala").unwrap(),
            vec![AlertFieldValue {
                field: AlertField::IncidentCategory,
                value: "MISSING_CHILD".into(),
            }],
        )
        .unwrap();
        alert
    }

    fn planned_delivery() -> PlannedDelivery {
        let alert = alert();
        let consumer_id = ConsumerId::new();
        let preference = DeliveryPreference::new(
            DeliveryStrategy::All,
            vec![ChannelEndpoint::new(ChannelType::WhatsApp, "+237600000000").unwrap()],
        )
        .unwrap();
        let mut preferences = HashMap::new();
        preferences.insert(consumer_id, preference);
        let consumer_matches = vec![ConsumerMatch {
            consumer_id,
            matching_subscriptions: vec![SubscriptionId::new()],
        }];

        let mut planned = plan_deliveries(
            &alert,
            &consumer_matches,
            &preferences,
            RetryPolicy::standard(),
            Actor::Automated,
            Uuid::new_v4(),
        );
        planned.remove(0)
    }

    #[test]
    fn plans_a_delivery_with_an_idempotency_hash_and_never_leaks_the_endpoint_address() {
        let planned = planned_delivery();

        assert_eq!(planned.idempotency_key_hash.len(), 32);
        assert_ne!(
            planned.idempotency_key_hash,
            planned
                .delivery
                .idempotency_key()
                .as_str()
                .as_bytes()
                .to_vec()
        );

        let payload = delivery_requested_event_payload(&planned);
        let payload_string = payload.to_string();
        assert!(!payload_string.contains("+237600000000"));
    }

    #[test]
    fn drives_a_delivery_from_queued_to_sent_through_the_workflow_wrappers() {
        let mut planned = planned_delivery();

        let started =
            start_delivery_attempt(&mut planned.delivery, Actor::Automated, Uuid::new_v4())
                .unwrap();
        assert_eq!(started.event.event_type, DeliveryEventType::DeliveryStarted);
        assert_eq!(planned.delivery.status(), DeliveryStatus::Sending);

        let succeeded = record_delivery_success(
            &mut planned.delivery,
            Some("provider-msg-1".into()),
            Actor::Automated,
            Uuid::new_v4(),
        )
        .unwrap();
        assert_eq!(succeeded.event.event_type, DeliveryEventType::DeliverySent);
        assert_eq!(planned.delivery.status(), DeliveryStatus::Sent);

        let delivered =
            record_delivery_delivered(&mut planned.delivery, Actor::Automated, Uuid::new_v4())
                .unwrap();
        assert_eq!(
            delivered.event.event_type,
            DeliveryEventType::DeliveryDelivered
        );
        assert_eq!(planned.delivery.status(), DeliveryStatus::Delivered);
    }

    #[test]
    fn records_a_permanent_failure_through_the_workflow_wrapper() {
        let mut planned = planned_delivery();
        start_delivery_attempt(&mut planned.delivery, Actor::Automated, Uuid::new_v4()).unwrap();

        let failed = record_delivery_failure(
            &mut planned.delivery,
            false,
            "invalid phone number",
            Actor::Automated,
            Uuid::new_v4(),
        )
        .unwrap();

        assert_eq!(failed.event.event_type, DeliveryEventType::DeliveryFailed);
        assert_eq!(planned.delivery.status(), DeliveryStatus::FailedPermanently);
        let payload = delivery_attempt_event_payload(&planned.delivery, &failed);
        assert_eq!(payload["delivery_id"], json!(planned.delivery.id()));
    }
}
