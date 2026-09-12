//! Webhook verification and replay-protection ports (docs/CHANNELS.md
//! section 8, prompt 08: "implement webhook verification/replay protection
//! interfaces where callbacks exist"). A provider callback is untrusted
//! input from the public internet; nothing here ever applies a status
//! change without going through a [`WebhookVerifier`] first.
//!
//! Wiring an actual HTTP endpoint that looks up the delivery a webhook
//! refers to and applies the resulting event through `delivery_workflow` is
//! a separate follow-up (it needs a delivery lookup by provider message id,
//! which does not exist yet); this module only defines the boundary
//! providers' adapters implement against.

use core::fmt;

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

/// Tracks which provider webhook events have already been applied, so a
/// provider's at-least-once callback delivery never applies the same status
/// transition twice (docs/EVENTS.md section 7: "every event consumer must
/// assume duplicates").
pub trait WebhookReplayGuard: Send + Sync {
    /// Returns `true` the first time `provider_event_id` is seen, `false` on
    /// every subsequent call with the same id.
    fn record_if_new(&self, provider_event_id: &str) -> bool;
}
