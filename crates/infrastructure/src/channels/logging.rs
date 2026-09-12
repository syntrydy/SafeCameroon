use async_trait::async_trait;
use safe_cameroon_application::channel::{
    Channel, ChannelError, ChannelSendOutcome, OutboundMessage,
};
use safe_cameroon_domain::ChannelType;

/// A placeholder adapter that logs to stdout and always succeeds. Stands in
/// for a real WhatsApp/SMS/Email provider until prompt 08 adds mock/sandbox
/// adapters behind the same [`Channel`] port; carries no vendor SDK.
pub struct LoggingChannel {
    channel_type: ChannelType,
}

impl LoggingChannel {
    pub fn new(channel_type: ChannelType) -> Self {
        Self { channel_type }
    }
}

#[async_trait]
impl Channel for LoggingChannel {
    fn channel_type(&self) -> ChannelType {
        self.channel_type
    }

    async fn send(&self, message: OutboundMessage) -> Result<ChannelSendOutcome, ChannelError> {
        println!(
            "[{:?}] -> {}: {}",
            self.channel_type, message.endpoint_address, message.body
        );
        Ok(ChannelSendOutcome {
            provider_message_id: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn always_succeeds_and_reports_its_own_channel_type() {
        let channel = LoggingChannel::new(ChannelType::WhatsApp);
        assert_eq!(channel.channel_type(), ChannelType::WhatsApp);

        let outcome = channel
            .send(OutboundMessage {
                endpoint_address: "+237600000000".into(),
                body: "test message".into(),
            })
            .await
            .unwrap();
        assert_eq!(outcome.provider_message_id, None);
    }
}
