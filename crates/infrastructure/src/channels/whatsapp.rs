use async_trait::async_trait;
use safe_cameroon_application::channel::{
    Channel, ChannelError, ChannelSendOutcome, EndpointValidationError, OutboundMessage,
};
use safe_cameroon_domain::ChannelType;
use uuid::Uuid;

use super::looks_like_e164;

/// Mock/sandbox WhatsApp Business Cloud API adapter. Stands in for a real
/// vendor integration until credentials exist; endpoint validation and error
/// mapping are real, `send` only simulates a provider response.
#[derive(Debug, Clone, Copy, Default)]
pub struct WhatsAppChannel;

/// The shape a real WhatsApp Cloud API error would take (docs/CHANNELS.md
/// section 8: "each provider adapter must encapsulate provider-specific
/// payloads and errors"). Never crosses `send`'s `ChannelError` boundary
/// unmapped.
#[derive(Debug, Clone, PartialEq, Eq)]
enum WhatsAppProviderError {
    InvalidDestinationNumber,
}

impl From<WhatsAppProviderError> for ChannelError {
    fn from(error: WhatsAppProviderError) -> Self {
        match error {
            WhatsAppProviderError::InvalidDestinationNumber => ChannelError {
                retryable: false,
                message: "WhatsApp: destination is not a WhatsApp-reachable number".into(),
            },
        }
    }
}

fn validate(address: &str) -> Result<(), EndpointValidationError> {
    if looks_like_e164(address) {
        Ok(())
    } else {
        Err(EndpointValidationError {
            channel: ChannelType::WhatsApp,
            reason: "expected E.164 format, e.g. +237600000000".into(),
        })
    }
}

#[async_trait]
impl Channel for WhatsAppChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::WhatsApp
    }

    fn validate_endpoint(&self, endpoint_address: &str) -> Result<(), EndpointValidationError> {
        validate(endpoint_address)
    }

    async fn send(&self, message: OutboundMessage) -> Result<ChannelSendOutcome, ChannelError> {
        if validate(&message.endpoint_address).is_err() {
            return Err(WhatsAppProviderError::InvalidDestinationNumber.into());
        }
        println!(
            "[WhatsApp sandbox] -> {}: {}",
            message.endpoint_address, message.body
        );
        Ok(ChannelSendOutcome {
            provider_message_id: Some(format!("wa-sandbox-{}", Uuid::new_v4())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_e164_addresses_only() {
        assert!(WhatsAppChannel.validate_endpoint("+237600000000").is_ok());
        assert!(WhatsAppChannel.validate_endpoint("0600000000").is_err());
        assert!(WhatsAppChannel.validate_endpoint("+abc").is_err());
    }

    #[tokio::test]
    async fn send_rejects_an_invalid_number_with_a_permanent_error() {
        let error = WhatsAppChannel
            .send(OutboundMessage {
                endpoint_address: "not-a-number".into(),
                body: "hello".into(),
            })
            .await
            .unwrap_err();
        assert!(!error.retryable);
    }

    #[tokio::test]
    async fn send_succeeds_for_a_valid_number() {
        let outcome = WhatsAppChannel
            .send(OutboundMessage {
                endpoint_address: "+237600000000".into(),
                body: "hello".into(),
            })
            .await
            .unwrap();
        assert!(
            outcome
                .provider_message_id
                .unwrap()
                .starts_with("wa-sandbox-")
        );
    }
}
