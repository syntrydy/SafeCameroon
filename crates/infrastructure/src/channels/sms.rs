use async_trait::async_trait;
use safe_cameroon_application::channel::{
    Channel, ChannelError, ChannelSendOutcome, EndpointValidationError, OutboundMessage,
};
use safe_cameroon_domain::ChannelType;
use uuid::Uuid;

use super::looks_like_e164;

/// Mock/sandbox SMS gateway adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct SmsChannel;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SmsProviderError {
    InvalidDestinationNumber,
}

impl From<SmsProviderError> for ChannelError {
    fn from(error: SmsProviderError) -> Self {
        match error {
            SmsProviderError::InvalidDestinationNumber => ChannelError {
                retryable: false,
                message: "SMS: destination is not a valid phone number".into(),
            },
        }
    }
}

fn validate(address: &str) -> Result<(), EndpointValidationError> {
    if looks_like_e164(address) {
        Ok(())
    } else {
        Err(EndpointValidationError {
            channel: ChannelType::Sms,
            reason: "expected E.164 format, e.g. +237600000000".into(),
        })
    }
}

#[async_trait]
impl Channel for SmsChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Sms
    }

    fn validate_endpoint(&self, endpoint_address: &str) -> Result<(), EndpointValidationError> {
        validate(endpoint_address)
    }

    async fn send(&self, message: OutboundMessage) -> Result<ChannelSendOutcome, ChannelError> {
        if validate(&message.endpoint_address).is_err() {
            return Err(SmsProviderError::InvalidDestinationNumber.into());
        }
        println!(
            "[SMS sandbox] -> {}: {}",
            message.endpoint_address, message.body
        );
        Ok(ChannelSendOutcome {
            provider_message_id: Some(format!("sms-sandbox-{}", Uuid::new_v4())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_e164_addresses_only() {
        assert!(SmsChannel.validate_endpoint("+237600000000").is_ok());
        assert!(SmsChannel.validate_endpoint("0600000000").is_err());
    }

    #[tokio::test]
    async fn send_rejects_an_invalid_number_with_a_permanent_error() {
        let error = SmsChannel
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
        let outcome = SmsChannel
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
                .starts_with("sms-sandbox-")
        );
    }
}
