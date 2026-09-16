//! Resend (https://resend.com) adapter for the email channel -- the first
//! real (non-mock) email provider, replacing `EmailChannel` when
//! `RESEND_API_KEY` is configured (`apps/worker/src/main.rs`).

use std::time::Duration;

use async_trait::async_trait;
use safe_cameroon_application::channel::{
    Channel, ChannelError, ChannelSendOutcome, EndpointValidationError, OutboundMessage,
};
use safe_cameroon_domain::ChannelType;
use serde::{Deserialize, Serialize};

use super::looks_like_email;

const RESEND_URL: &str = "https://api.resend.com/emails";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const SUBJECT: &str = "SafeCameroon Alert";

pub struct ResendEmailChannel {
    http: reqwest::Client,
    api_key: String,
    /// The verified sender identity Resend requires, e.g.
    /// `"SafeCameroon Alerts <alerts@example.org>"` -- there is no safe
    /// default, since it must match a domain verified in the Resend
    /// account this API key belongs to.
    from_address: String,
}

impl ResendEmailChannel {
    pub fn new(api_key: String, from_address: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("reqwest client with only a timeout must build"),
            api_key,
            from_address,
        }
    }
}

#[derive(Serialize)]
struct SendEmailRequest<'a> {
    from: &'a str,
    to: [&'a str; 1],
    subject: &'a str,
    text: &'a str,
}

#[derive(Deserialize)]
struct SendEmailResponse {
    id: String,
}

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
impl Channel for ResendEmailChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Email
    }

    fn validate_endpoint(&self, endpoint_address: &str) -> Result<(), EndpointValidationError> {
        validate(endpoint_address)
    }

    async fn send(&self, message: OutboundMessage) -> Result<ChannelSendOutcome, ChannelError> {
        validate(&message.endpoint_address).map_err(|error| ChannelError {
            retryable: false,
            message: format!("Email: {}", error.reason),
        })?;

        let request = SendEmailRequest {
            from: &self.from_address,
            to: [&message.endpoint_address],
            subject: SUBJECT,
            text: &message.body,
        };

        let response = self
            .http
            .post(RESEND_URL)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|error| ChannelError {
                retryable: true,
                message: format!("Email: could not reach Resend: {error}"),
            })?;

        let status = response.status();
        if status.is_success() {
            let parsed: SendEmailResponse =
                response.json().await.map_err(|error| ChannelError {
                    retryable: true,
                    message: format!("Email: unexpected response from Resend: {error}"),
                })?;
            return Ok(ChannelSendOutcome {
                provider_message_id: Some(parsed.id),
            });
        }

        // Resend's own retry guidance (docs/api-reference/errors): 409
        // (a same-day idempotency-key collision, harmless to retry with a
        // fresh key), 429 (rate limited), and 5xx are transient; everything
        // else (400-405, 422) is a malformed/rejected request that retrying
        // unchanged can never fix.
        let retryable = matches!(status.as_u16(), 409 | 429) || status.is_server_error();
        Err(ChannelError {
            retryable,
            message: format!("Email: Resend rejected the request ({status})"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_well_formed_addresses_only() {
        let channel = ResendEmailChannel::new("key".into(), "alerts@example.org".into());
        assert!(channel.validate_endpoint("ngo@example.cm").is_ok());
        assert!(channel.validate_endpoint("not-an-email").is_err());
    }

    #[tokio::test]
    async fn send_rejects_an_invalid_address_without_a_network_call() {
        let channel = ResendEmailChannel::new("key".into(), "alerts@example.org".into());
        let error = channel
            .send(OutboundMessage {
                endpoint_address: "not-an-email".into(),
                body: "hello".into(),
            })
            .await
            .unwrap_err();
        assert!(!error.retryable);
    }
}
