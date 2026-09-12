//! Webhook verification and replay-protection ports (docs/CHANNELS.md
//! section 8, prompt 08: "implement webhook verification/replay protection
//! interfaces where callbacks exist"; docs/API.md section 7). A provider
//! callback is untrusted input from the public internet; nothing here ever
//! applies a status change without going through a [`WebhookVerifier`] and a
//! [`WebhookReplayGuard`] first. `crate::delivery_workflow::apply_webhook_event`
//! is what turns a verified, deduplicated [`WebhookEvent`] into a delivery
//! status transition; `apps/api`'s `POST /v1/webhooks/{channel}/{provider}`
//! handler is what receives the raw request and drives all of this.

use core::fmt;
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use safe_cameroon_domain::ChannelType;

/// The channel-neutral outcome a provider callback reports, mapped down from
/// whatever shape that provider's webhook payload actually has
/// (docs/CHANNELS.md section 8: "delivery status mapping").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderDeliveryStatus {
    Delivered,
    Failed { retryable: bool, reason: String },
}

/// A verified, parsed callback. `provider_event_id` is what
/// [`WebhookReplayGuard`] dedups on; `provider_message_id` is what a future
/// handler looks up the original [`crate::delivery_workflow::PlannedDelivery`]
/// by (it is the same id `ChannelSendOutcome::provider_message_id` carried).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookEvent {
    pub provider_event_id: String,
    pub provider_message_id: String,
    pub status: ProviderDeliveryStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookVerificationError {
    pub reason: String,
}

impl fmt::Display for WebhookVerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for WebhookVerificationError {}

/// Verifies a raw inbound callback's authenticity and parses it. An adapter
/// must reject (never partially trust) a payload whose signature does not
/// verify, is missing, or does not parse — a forged callback must not be
/// able to move a delivery to `Delivered` or trigger a retry it did not earn.
pub trait WebhookVerifier: Send + Sync {
    fn channel_type(&self) -> ChannelType;

    fn verify_and_parse(
        &self,
        headers: &[(String, String)],
        raw_body: &[u8],
    ) -> Result<WebhookEvent, WebhookVerificationError>;
}

/// Looks up the [`WebhookVerifier`] responsible for one `(channel,
/// provider)` pair (docs/API.md section 7: `POST /v1/webhooks/{channel}/
/// {provider}` names both, since one channel can have more than one
/// provider). Pure wiring — mirrors [`crate::channel::ChannelRegistry`].
#[derive(Default)]
pub struct WebhookVerifierRegistry {
    verifiers: HashMap<(ChannelType, String), Arc<dyn WebhookVerifier>>,
}

impl WebhookVerifierRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, provider: impl Into<String>, verifier: Arc<dyn WebhookVerifier>) {
        self.verifiers
            .insert((verifier.channel_type(), provider.into()), verifier);
    }

    pub fn get(&self, channel: ChannelType, provider: &str) -> Option<&Arc<dyn WebhookVerifier>> {
        self.verifiers.get(&(channel, provider.to_owned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookReplayError {
    pub reason: String,
}

impl fmt::Display for WebhookReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for WebhookReplayError {}

/// Tracks which provider webhook events have already been applied, so a
/// provider's at-least-once callback delivery never applies the same status
/// transition twice (docs/EVENTS.md section 7: "every event consumer must
/// assume duplicates"). Async because a durable implementation needs to
/// persist what it has seen (an in-process guard alone would forget on
/// restart, defeating the point).
#[async_trait]
pub trait WebhookReplayGuard: Send + Sync {
    /// Returns `true` the first time `(channel, provider_event_id)` is seen,
    /// `false` on every subsequent call with the same pair. Scoped by
    /// channel so two different providers can never collide on event id.
    async fn record_if_new(
        &self,
        channel: ChannelType,
        provider_event_id: &str,
    ) -> Result<bool, WebhookReplayError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubVerifier(ChannelType);

    impl WebhookVerifier for StubVerifier {
        fn channel_type(&self) -> ChannelType {
            self.0
        }

        fn verify_and_parse(
            &self,
            _headers: &[(String, String)],
            _raw_body: &[u8],
        ) -> Result<WebhookEvent, WebhookVerificationError> {
            unimplemented!("not exercised by the registry lookup test")
        }
    }

    #[test]
    fn registry_looks_up_by_both_channel_and_provider() {
        let mut registry = WebhookVerifierRegistry::new();
        registry.register("sandbox", Arc::new(StubVerifier(ChannelType::WhatsApp)));

        assert!(registry.get(ChannelType::WhatsApp, "sandbox").is_some());
        assert!(
            registry
                .get(ChannelType::WhatsApp, "other-provider")
                .is_none()
        );
        assert!(registry.get(ChannelType::Sms, "sandbox").is_none());
    }
}
