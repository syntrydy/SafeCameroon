//! Reference implementations of the webhook ports
//! ([`safe_cameroon_application::webhook`]). [`HmacSignedWebhookVerifier`] is
//! a real, working HMAC-SHA256 signature check (the same scheme WhatsApp
//! Cloud API and many other providers use), parameterized by channel and
//! shared secret; every mock/sandbox channel in this crate uses the same
//! payload shape today, so one implementation covers all three until a real
//! provider integration replaces it with that provider's own scheme.
//! [`InMemoryReplayGuard`] is a process-local reference implementation kept
//! for tests; [`PostgresWebhookReplayGuard`] is the durable one a real
//! deployment needs, so replay protection survives a restart.

use std::collections::HashSet;
use std::sync::Mutex;

use async_trait::async_trait;
use hmac::{Hmac, Mac};
use safe_cameroon_application::webhook::{
    ProviderDeliveryStatus, WebhookEvent, WebhookReplayError, WebhookReplayGuard,
    WebhookVerificationError, WebhookVerifier,
};
use safe_cameroon_domain::ChannelType;
use sha2::Sha256;
use sqlx::PgPool;

type HmacSha256 = Hmac<Sha256>;

const SIGNATURE_HEADER: &str = "x-signature";

fn decode_hex(value: &str) -> Result<Vec<u8>, WebhookVerificationError> {
    if value.len() % 2 != 0 {
        return Err(WebhookVerificationError {
            reason: "signature header is not valid hex".into(),
        });
    }
    (0..value.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&value[i..i + 2], 16).map_err(|_| WebhookVerificationError {
                reason: "signature header is not valid hex".into(),
            })
        })
        .collect()
}

/// Verifies `x-signature: <hex HMAC-SHA256 of the raw body>` and parses the
/// mock JSON payload `{"event_id", "message_id", "status", "retryable"?,
/// "reason"?}` every sandbox channel in this crate emits.
pub struct HmacSignedWebhookVerifier {
    channel_type: ChannelType,
    secret: Vec<u8>,
}

impl HmacSignedWebhookVerifier {
    pub fn new(channel_type: ChannelType, secret: impl Into<Vec<u8>>) -> Self {
        Self {
            channel_type,
            secret: secret.into(),
        }
    }
}

impl WebhookVerifier for HmacSignedWebhookVerifier {
    fn channel_type(&self) -> ChannelType {
        self.channel_type
    }

    fn verify_and_parse(
        &self,
        headers: &[(String, String)],
        raw_body: &[u8],
    ) -> Result<WebhookEvent, WebhookVerificationError> {
        let signature_header = headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(SIGNATURE_HEADER))
            .map(|(_, value)| value.as_str())
            .ok_or_else(|| WebhookVerificationError {
                reason: "missing signature header".into(),
            })?;
        let signature = decode_hex(signature_header)?;

        let mut mac = HmacSha256::new_from_slice(&self.secret)
            .expect("HMAC-SHA256 accepts a key of any length");
        mac.update(raw_body);
        mac.verify_slice(&signature)
            .map_err(|_| WebhookVerificationError {
                reason: "signature does not match the payload".into(),
            })?;

        let payload: serde_json::Value =
            serde_json::from_slice(raw_body).map_err(|error| WebhookVerificationError {
                reason: format!("invalid payload: {error}"),
            })?;
        let field = |name: &str| -> Result<String, WebhookVerificationError> {
            payload
                .get(name)
                .and_then(|value| value.as_str())
                .map(str::to_owned)
                .ok_or_else(|| WebhookVerificationError {
                    reason: format!("missing or non-string field: {name}"),
                })
        };

        let provider_event_id = field("event_id")?;
        let provider_message_id = field("message_id")?;
        let status = match field("status")?.as_str() {
            "DELIVERED" => ProviderDeliveryStatus::Delivered,
            "FAILED" => ProviderDeliveryStatus::Failed {
                retryable: payload
                    .get("retryable")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                reason: field("reason")?,
            },
            other => {
                return Err(WebhookVerificationError {
                    reason: format!("unrecognized status: {other}"),
                });
            }
        };

        Ok(WebhookEvent {
            provider_event_id,
            provider_message_id,
            status,
        })
    }
}

fn replay_key(channel: ChannelType, provider_event_id: &str) -> String {
    format!("{}:{}", channel.as_database_value(), provider_event_id)
}

/// Process-local reference [`WebhookReplayGuard`]. Not durable across
/// restarts — fine for tests and local development, not for production.
#[derive(Default)]
pub struct InMemoryReplayGuard {
    seen: Mutex<HashSet<String>>,
}

impl InMemoryReplayGuard {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl WebhookReplayGuard for InMemoryReplayGuard {
    async fn record_if_new(
        &self,
        channel: ChannelType,
        provider_event_id: &str,
    ) -> Result<bool, WebhookReplayError> {
        let mut seen = self
            .seen
            .lock()
            .expect("in-memory replay guard mutex must not be poisoned");
        Ok(seen.insert(replay_key(channel, provider_event_id)))
    }
}

/// Durable, Postgres-backed [`WebhookReplayGuard`]: an insert that conflicts
/// on the `(channel, provider_event_id)` primary key means this event has
/// already been recorded.
#[derive(Clone)]
pub struct PostgresWebhookReplayGuard {
    pool: PgPool,
}

impl PostgresWebhookReplayGuard {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WebhookReplayGuard for PostgresWebhookReplayGuard {
    async fn record_if_new(
        &self,
        channel: ChannelType,
        provider_event_id: &str,
    ) -> Result<bool, WebhookReplayError> {
        let result = sqlx::query(
            r#"
            INSERT INTO webhook_replay_events (channel, provider_event_id)
            VALUES ($1::channel_type, $2)
            ON CONFLICT (channel, provider_event_id) DO NOTHING
            "#,
        )
        .bind(channel.as_database_value())
        .bind(provider_event_id)
        .execute(&self.pool)
        .await
        .map_err(|error| WebhookReplayError {
            reason: error.to_string(),
        })?;
        Ok(result.rows_affected() == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign(secret: &[u8], body: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(secret).unwrap();
        mac.update(body);
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    #[test]
    fn verifies_and_parses_a_correctly_signed_delivered_event() {
        let secret = b"shared-secret";
        let body = br#"{"event_id":"evt-1","message_id":"wa-sandbox-1","status":"DELIVERED"}"#;
        let verifier = HmacSignedWebhookVerifier::new(ChannelType::WhatsApp, secret.to_vec());
        let headers = vec![("X-Signature".to_owned(), sign(secret, body))];

        let event = verifier.verify_and_parse(&headers, body).unwrap();
        assert_eq!(event.provider_event_id, "evt-1");
        assert_eq!(event.provider_message_id, "wa-sandbox-1");
        assert_eq!(event.status, ProviderDeliveryStatus::Delivered);
    }

    #[test]
    fn parses_a_failed_event_with_retryable_and_reason() {
        let secret = b"shared-secret";
        let body = br#"{"event_id":"evt-2","message_id":"wa-sandbox-2","status":"FAILED","retryable":true,"reason":"timeout"}"#;
        let verifier = HmacSignedWebhookVerifier::new(ChannelType::WhatsApp, secret.to_vec());
        let headers = vec![("x-signature".to_owned(), sign(secret, body))];

        let event = verifier.verify_and_parse(&headers, body).unwrap();
        assert_eq!(
            event.status,
            ProviderDeliveryStatus::Failed {
                retryable: true,
                reason: "timeout".into()
            }
        );
    }

    #[test]
    fn rejects_a_missing_signature_header() {
        let verifier = HmacSignedWebhookVerifier::new(ChannelType::WhatsApp, b"secret".to_vec());
        let error = verifier.verify_and_parse(&[], b"{}").unwrap_err();
        assert!(error.reason.contains("missing signature header"));
    }

    #[test]
    fn rejects_a_signature_that_does_not_match_the_body() {
        let verifier = HmacSignedWebhookVerifier::new(ChannelType::WhatsApp, b"secret".to_vec());
        let headers = vec![("x-signature".to_owned(), sign(b"wrong-secret", b"{}"))];
        let error = verifier.verify_and_parse(&headers, b"{}").unwrap_err();
        assert!(error.reason.contains("does not match"));
    }

    #[test]
    fn rejects_a_tampered_body_even_with_a_previously_valid_signature() {
        let secret = b"shared-secret";
        let original_body =
            br#"{"event_id":"evt-1","message_id":"wa-sandbox-1","status":"DELIVERED"}"#;
        let signature = sign(secret, original_body);
        let tampered_body =
            br#"{"event_id":"evt-1","message_id":"wa-sandbox-1","status":"FAILED"}"#;
        let verifier = HmacSignedWebhookVerifier::new(ChannelType::WhatsApp, secret.to_vec());

        let error = verifier
            .verify_and_parse(&[("x-signature".to_owned(), signature)], tampered_body)
            .unwrap_err();
        assert!(error.reason.contains("does not match"));
    }

    #[tokio::test]
    async fn in_memory_replay_guard_accepts_an_id_once_and_rejects_repeats() {
        let guard = InMemoryReplayGuard::new();
        assert!(
            guard
                .record_if_new(ChannelType::WhatsApp, "evt-1")
                .await
                .unwrap()
        );
        assert!(
            !guard
                .record_if_new(ChannelType::WhatsApp, "evt-1")
                .await
                .unwrap()
        );
        assert!(
            guard
                .record_if_new(ChannelType::WhatsApp, "evt-2")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn in_memory_replay_guard_scopes_by_channel() {
        let guard = InMemoryReplayGuard::new();
        assert!(
            guard
                .record_if_new(ChannelType::WhatsApp, "evt-1")
                .await
                .unwrap()
        );
        assert!(
            guard
                .record_if_new(ChannelType::Sms, "evt-1")
                .await
                .unwrap(),
            "the same event id on a different channel must be treated as distinct"
        );
    }
}
