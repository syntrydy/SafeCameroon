//! The provider-neutral channel port (docs/CHANNELS.md section 2): "a
//! channel is not a provider." This module defines the boundary a worker
//! dispatches deliveries across and providers implement against; it must
//! never depend on a vendor SDK (prompt 08 owns concrete adapters —
//! mock/sandbox WhatsApp/SMS/Email implementations and webhook
//! verification — behind this same trait).

use core::fmt;
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use safe_cameroon_domain::{Alert, ChannelType, Delivery};

/// What a channel needs to send one message. `body` is a single,
/// channel-agnostic rendering of the alert's safe fields
/// (docs/CHANNELS.md section 7 describes channel-specific formatting —
/// WhatsApp template, shortened SMS, rich email — as a later adapter
/// concern; every version still has to originate from this same content).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundMessage {
    pub endpoint_address: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelSendOutcome {
    pub provider_message_id: Option<String>,
}

/// A provider/network failure, carrying enough information for
/// [`crate::delivery_workflow::record_delivery_failure`] to decide whether a
/// retry is worthwhile. Provider-specific error payloads are mapped down to
/// this shape inside the adapter, per docs/CHANNELS.md section 8 ("do not
/// let provider-specific response formats leak into domain code").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelError {
    pub retryable: bool,
    pub message: String,
}

impl fmt::Display for ChannelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ChannelError {}

/// docs/CHANNELS.md section 2, verbatim shape.
#[async_trait]
pub trait Channel: Send + Sync {
    fn channel_type(&self) -> ChannelType;

    async fn send(&self, message: OutboundMessage) -> Result<ChannelSendOutcome, ChannelError>;
}

/// Builds the message a worker hands to whichever [`Channel`] matches
/// `delivery.channel()`. Kept deliberately plain-text and identical across
/// channels for now; per-channel rendering is a prompt 08 adapter concern.
pub fn build_outbound_message(alert: &Alert, delivery: &Delivery) -> OutboundMessage {
    let body = alert
        .fields()
        .iter()
        .map(|field| format!("{}: {}", field.field.as_database_value(), field.value))
        .collect::<Vec<_>>()
        .join("\n");
    OutboundMessage {
        endpoint_address: delivery.endpoint_address().to_owned(),
        body,
    }
}

/// Looks up the [`Channel`] responsible for a [`ChannelType`] at dispatch
/// time. Pure wiring — no vendor SDK, no I/O of its own.
#[derive(Clone, Default)]
pub struct ChannelRegistry {
    channels: HashMap<ChannelType, Arc<dyn Channel>>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, channel: Arc<dyn Channel>) {
        self.channels.insert(channel.channel_type(), channel);
    }

    pub fn get(&self, channel_type: ChannelType) -> Option<&Arc<dyn Channel>> {
        self.channels.get(&channel_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use safe_cameroon_domain::{
        AlertField, AlertFieldValue, AlertPolicy, Case, CaseStatus, ChannelEndpoint, ConsumerId,
        ConsumerMatch, DeliveryPreference, DeliveryStrategy, IncidentType, ReportId, RetryPolicy,
        Severity, SubscriptionId, TargetGeography,
    };

    struct RecordingChannel {
        channel_type: ChannelType,
    }

    #[async_trait]
    impl Channel for RecordingChannel {
        fn channel_type(&self) -> ChannelType {
            self.channel_type
        }

        async fn send(&self, message: OutboundMessage) -> Result<ChannelSendOutcome, ChannelError> {
            if message.body.is_empty() {
                return Err(ChannelError {
                    retryable: false,
                    message: "empty body".into(),
                });
            }
            Ok(ChannelSendOutcome {
                provider_message_id: Some(format!("echo:{}", message.endpoint_address)),
            })
        }
    }

    fn alert_and_delivery() -> (Alert, Delivery) {
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

        let consumer_id = ConsumerId::new();
        let preference = DeliveryPreference::new(
            DeliveryStrategy::All,
            vec![ChannelEndpoint::new(ChannelType::WhatsApp, "+237600000000").unwrap()],
        )
        .unwrap();
        let mut preferences = std::collections::HashMap::new();
        preferences.insert(consumer_id, preference);
        let consumer_matches = vec![ConsumerMatch {
            consumer_id,
            matching_subscriptions: vec![SubscriptionId::new()],
        }];
        let mut planned = safe_cameroon_domain::plan_deliveries(
            &alert,
            &consumer_matches,
            &preferences,
            RetryPolicy::standard(),
        );
        let (delivery, _) = planned.remove(0);
        (alert, delivery)
    }

    #[test]
    fn builds_a_message_from_the_alerts_safe_fields_and_the_deliverys_endpoint() {
        let (alert, delivery) = alert_and_delivery();
        let message = build_outbound_message(&alert, &delivery);
        assert_eq!(message.endpoint_address, "+237600000000");
        assert!(message.body.contains("INCIDENT_CATEGORY: MISSING_CHILD"));
    }

    #[tokio::test]
    async fn registry_dispatches_to_the_channel_matching_the_message_type() {
        let mut registry = ChannelRegistry::new();
        registry.register(Arc::new(RecordingChannel {
            channel_type: ChannelType::WhatsApp,
        }));

        let channel = registry.get(ChannelType::WhatsApp).expect("registered");
        let outcome = channel
            .send(OutboundMessage {
                endpoint_address: "+237600000000".into(),
                body: "hello".into(),
            })
            .await
            .unwrap();
        assert_eq!(
            outcome.provider_message_id,
            Some("echo:+237600000000".into())
        );

        assert!(registry.get(ChannelType::Sms).is_none());
    }
}
