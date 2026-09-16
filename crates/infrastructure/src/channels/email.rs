use async_trait::async_trait;
use safe_cameroon_application::channel::{
    Channel, ChannelError, ChannelSendOutcome, EndpointValidationError, OutboundMessage,
};
use safe_cameroon_domain::ChannelType;
use uuid::Uuid;

use super::looks_like_email;

/// Mock/sandbox email adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmailChannel;

#[derive(Debug, Clone, PartialEq, Eq)]
enum EmailProviderError {
    InvalidAddress,
}

impl From<EmailProviderError> for ChannelError {
    fn from(error: EmailProviderError) -> Self {
        match error {
            EmailProviderError::InvalidAddress => ChannelError {
                retryable: false,
                message: "Email: destination is not a valid email address".into(),
            },
        }
    }
}

/// Deliberately conservative: only rejects addresses that could never be
/// deliverable (no `@`, an empty local/domain part, or no `.` in the
/// domain). Real mailbox existence can only be confirmed by the provider.
fn validate(address: &str) -> Result<(), EndpointValidationError> {
    if looks_like_email(address) {
        Ok(())
    } else {
        Err(EndpointValidationError {
            channel: ChannelType::Email,
            reason: "expected a well-formed email address".into(),
        })
    }
}

#[async_trait]
impl Channel for EmailChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Email
    }

    fn validate_endpoint(&self, endpoint_address: &str) -> Result<(), EndpointValidationError> {
        validate(endpoint_address)
    }

    async fn send(&self, message: OutboundMessage) -> Result<ChannelSendOutcome, ChannelError> {
        if validate(&message.endpoint_address).is_err() {
            return Err(EmailProviderError::InvalidAddress.into());
        }
        println!(
            "[Email sandbox] -> {}: {}",
            message.endpoint_address, message.body
        );
        Ok(ChannelSendOutcome {
            provider_message_id: Some(format!("email-sandbox-{}", Uuid::new_v4())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_well_formed_addresses_only() {
        assert!(EmailChannel.validate_endpoint("ngo@example.cm").is_ok());
        assert!(EmailChannel.validate_endpoint("not-an-email").is_err());
        assert!(EmailChannel.validate_endpoint("@example.cm").is_err());
        assert!(EmailChannel.validate_endpoint("ngo@").is_err());
        assert!(EmailChannel.validate_endpoint("ngo@localhost").is_err());
        assert!(EmailChannel.validate_endpoint("ng o@example.cm").is_err());
    }

    #[tokio::test]
    async fn send_rejects_an_invalid_address_with_a_permanent_error() {
        let error = EmailChannel
            .send(OutboundMessage {
                endpoint_address: "not-an-email".into(),
                body: "hello".into(),
            })
            .await
            .unwrap_err();
        assert!(!error.retryable);
    }

    #[tokio::test]
    async fn send_succeeds_for_a_valid_address() {
        let outcome = EmailChannel
            .send(OutboundMessage {
                endpoint_address: "ngo@example.cm".into(),
                body: "hello".into(),
            })
            .await
            .unwrap();
        assert!(
            outcome
                .provider_message_id
                .unwrap()
                .starts_with("email-sandbox-")
        );
    }
}
