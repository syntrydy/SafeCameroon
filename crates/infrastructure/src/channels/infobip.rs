//! Infobip (https://www.infobip.com) adapter for the SMS channel -- the
//! first real (non-mock) SMS provider, replacing `SmsChannel` when
//! `INFOBIP_BASE_URL`/`INFOBIP_API_KEY`/`INFOBIP_SENDER` are configured
//! (`apps/worker/src/main.rs`).

use std::time::Duration;

use async_trait::async_trait;
use safe_cameroon_application::channel::{
    Channel, ChannelError, ChannelSendOutcome, EndpointValidationError, OutboundMessage,
};
use safe_cameroon_domain::ChannelType;
use serde::{Deserialize, Serialize};

use super::looks_like_e164;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub struct InfobipSmsChannel {
    http: reqwest::Client,
    /// Account-specific API base URL Infobip issues per account (e.g.
    /// `https://9kx1z1.api.infobip.com`) -- unlike Resend, there is no
    /// single shared endpoint across accounts.
    base_url: String,
    api_key: String,
    /// The registered sender id/name shown as the SMS's "from" (e.g.
    /// `"Sentinel"`) -- alphanumeric sender IDs are subject to per-country
    /// registration with Infobip; there is no safe universal default.
    sender: String,
}

impl InfobipSmsChannel {
    pub fn new(base_url: String, api_key: String, sender: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("reqwest client with only a timeout must build"),
            base_url: base_url.trim_end_matches('/').to_owned(),
            api_key,
            sender,
        }
    }
}

#[derive(Serialize)]
struct SendSmsRequest<'a> {
    messages: [SmsMessage<'a>; 1],
}

#[derive(Serialize)]
struct SmsMessage<'a> {
    destinations: [Destination<'a>; 1],
    from: &'a str,
    text: &'a str,
}

#[derive(Serialize)]
struct Destination<'a> {
    to: &'a str,
}

#[derive(Deserialize)]
struct SendSmsResponse {
    messages: Vec<SentMessage>,
}

#[derive(Deserialize)]
struct SentMessage {
    #[serde(rename = "messageId")]
    message_id: String,
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
impl Channel for InfobipSmsChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Sms
    }

    fn validate_endpoint(&self, endpoint_address: &str) -> Result<(), EndpointValidationError> {
        validate(endpoint_address)
    }

    async fn send(&self, message: OutboundMessage) -> Result<ChannelSendOutcome, ChannelError> {
        validate(&message.endpoint_address).map_err(|error| ChannelError {
            retryable: false,
            message: format!("SMS: {}", error.reason),
        })?;

        // Infobip expects destination numbers in international format
        // without a leading "+"; our own endpoint addresses are stored
        // with one (E.164, `looks_like_e164`).
        let destination = message.endpoint_address.trim_start_matches('+');

        let request = SendSmsRequest {
            messages: [SmsMessage {
                destinations: [Destination { to: destination }],
                from: &self.sender,
                text: &message.body,
            }],
        };

        let response = self
            .http
            .post(format!("{}/sms/2/text/advanced", self.base_url))
            .header("Authorization", format!("App {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|error| ChannelError {
                retryable: true,
                message: format!("SMS: could not reach Infobip: {error}"),
            })?;

        let status = response.status();
        if status.is_success() {
            let parsed: SendSmsResponse = response.json().await.map_err(|error| ChannelError {
                retryable: true,
                message: format!("SMS: unexpected response from Infobip: {error}"),
            })?;
            let message_id = parsed.messages.into_iter().next().map(|m| m.message_id);
            return Ok(ChannelSendOutcome {
                provider_message_id: message_id,
            });
        }

        // Infobip's own error guidance: 401/403 (bad API key/account) and
        // 400/404/422 (malformed request/destination) are never fixed by
        // retrying unchanged; 429 (rate limited) and 5xx are transient.
        let retryable = status.as_u16() == 429 || status.is_server_error();
        Err(ChannelError {
            retryable,
            message: format!("SMS: Infobip rejected the request ({status})"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel() -> InfobipSmsChannel {
        InfobipSmsChannel::new(
            "https://example.api.infobip.com".into(),
            "key".into(),
            "Sentinel".into(),
        )
    }

    #[test]
    fn validates_e164_addresses_only() {
        assert!(channel().validate_endpoint("+237600000000").is_ok());
        assert!(channel().validate_endpoint("0600000000").is_err());
    }

    #[test]
    fn strips_a_trailing_slash_from_the_base_url() {
        let channel = InfobipSmsChannel::new(
            "https://example.api.infobip.com/".into(),
            "key".into(),
            "Sentinel".into(),
        );
        assert_eq!(channel.base_url, "https://example.api.infobip.com");
    }

    #[tokio::test]
    async fn send_rejects_an_invalid_number_without_a_network_call() {
        let error = channel()
            .send(OutboundMessage {
                endpoint_address: "not-a-number".into(),
                body: "hello".into(),
            })
            .await
            .unwrap_err();
        assert!(!error.retryable);
    }
}
