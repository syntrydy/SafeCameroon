//! Self-service citizen subscription creation
//! (`apps/api/src/citizen_subscriptions.rs`, prompts/11_CITIZEN_EXPERIENCE.md).
//! Every other subscription today is reviewer-managed
//! (crates/domain/src/subscription.rs, apps/api/src/subscriptions.rs) --
//! this path has no reviewer account behind it, so it needs its own
//! ownership proof instead: a private, unguessable management token,
//! generated here and returned to the caller exactly once. Only its
//! SHA-256 digest is ever persisted, mirroring how anonymous report
//! reference codes are hashed (`crate::prepare_anonymous_report`).

use core::fmt;

use safe_cameroon_domain::{
    AlertVisibility, ChannelEndpoint, ChannelType, Comparison, Consumer, ConsumerType,
    DeliveryPreference, DeliveryPreferenceError, DeliveryStrategy, EmptyChannelEndpointAddress,
    EmptyGeoArea, GeoArea, IncidentType, Severity, Subscription, SubscriptionId, SubscriptionRule,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Every self-service citizen subscription is scoped to these visibilities
/// only, decided server-side and non-negotiable by the caller
/// (docs/OPEN_QUESTIONS.md: "Should citizens receive only community/public
/// alerts or selected partner-level alerts?" -- resolved as community/public
/// only for anonymous self-signup, since Partner/Internal presuppose a
/// vetted organizational relationship an anonymous citizen doesn't have).
pub const CITIZEN_ALERT_VISIBILITIES: [AlertVisibility; 2] =
    [AlertVisibility::Community, AlertVisibility::Public];

const CONSUMER_NAME: &str = "Citizen (self-subscribed)";

pub fn generate_management_token() -> String {
    Uuid::new_v4().to_string()
}

pub fn hash_management_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// Constant-time so a network-observable timing difference can never be used
/// to guess a valid token one byte at a time (mirrors
/// `HmacSignedAttachmentStorage::verify`'s reason for constant-time
/// comparison, applied here without a keyed MAC since there is no per-request
/// secret to sign with -- only a stored digest to compare against).
pub fn verify_management_token(token: &str, stored_hash: &[u8]) -> bool {
    constant_time_eq(&hash_management_token(token), stored_hash)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CitizenSubscriptionError {
    EmptyIncidentTypes,
    InvalidGeography,
    InvalidPushSubscription,
}

impl fmt::Display for CitizenSubscriptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIncidentTypes => write!(f, "at least one incident type must be selected"),
            Self::InvalidGeography => write!(f, "geography cannot be blank"),
            Self::InvalidPushSubscription => write!(f, "push subscription cannot be blank"),
        }
    }
}

impl std::error::Error for CitizenSubscriptionError {}

impl From<EmptyGeoArea> for CitizenSubscriptionError {
    fn from(_: EmptyGeoArea) -> Self {
        Self::InvalidGeography
    }
}

impl From<EmptyChannelEndpointAddress> for CitizenSubscriptionError {
    fn from(_: EmptyChannelEndpointAddress) -> Self {
        Self::InvalidPushSubscription
    }
}

pub struct CitizenSubscriptionRequest {
    pub incident_types: Vec<IncidentType>,
    pub minimum_severity: Severity,
    pub geography: String,
    /// The browser's `PushSubscription`, already validated and re-serialized
    /// to canonical JSON by the caller (`apps/api/src/citizen_subscriptions.rs`)
    /// -- kept a plain `String` here so this crate never depends on a
    /// specific push provider's types (a channel is not a provider).
    pub push_subscription_json: String,
}

#[derive(Debug)]
pub struct PreparedCitizenSubscription {
    pub consumer: Consumer,
    pub delivery_preference: DeliveryPreference,
    pub subscription: Subscription,
    pub management_token: String,
    pub management_token_hash: Vec<u8>,
}

pub fn prepare_citizen_subscription(
    request: CitizenSubscriptionRequest,
) -> Result<PreparedCitizenSubscription, CitizenSubscriptionError> {
    if request.incident_types.is_empty() {
        return Err(CitizenSubscriptionError::EmptyIncidentTypes);
    }
    let geo_area = GeoArea::new(request.geography)?;
    let endpoint = ChannelEndpoint::new(ChannelType::Push, request.push_subscription_json)?;

    let consumer =
        Consumer::new(CONSUMER_NAME, ConsumerType::Citizen).expect("CONSUMER_NAME is non-blank");

    let delivery_preference = DeliveryPreference::new(DeliveryStrategy::All, vec![endpoint])
        .unwrap_or_else(|error| match error {
            DeliveryPreferenceError::EmptyChannelList
            | DeliveryPreferenceError::DuplicateChannel { .. } => {
                unreachable!(
                    "exactly one channel is passed, so it is never empty or duplicated: {error}"
                )
            }
        });

    let rules = vec![
        SubscriptionRule::IncidentType(request.incident_types),
        SubscriptionRule::Severity {
            operator: Comparison::GreaterThanOrEqual,
            value: request.minimum_severity,
        },
        SubscriptionRule::Geography(geo_area),
        SubscriptionRule::Visibility(CITIZEN_ALERT_VISIBILITIES.to_vec()),
    ];
    let subscription = Subscription::new(SubscriptionId::new(), consumer.id(), 1, rules)
        .expect("rules is never empty: it always includes at least the incident-type rule");

    let management_token = generate_management_token();
    let management_token_hash = hash_management_token(&management_token);

    Ok(PreparedCitizenSubscription {
        consumer,
        delivery_preference,
        subscription,
        management_token,
        management_token_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_request() -> CitizenSubscriptionRequest {
        CitizenSubscriptionRequest {
            incident_types: vec![IncidentType::MissingChild],
            minimum_severity: Severity::High,
            geography: "Douala".into(),
            push_subscription_json:
                r#"{"endpoint":"https://push.example/abc","keys":{"p256dh":"key","auth":"secret"}}"#
                    .into(),
        }
    }

    #[test]
    fn prepares_a_citizen_subscription_scoped_to_community_and_public_visibility() {
        let prepared = prepare_citizen_subscription(valid_request()).unwrap();

        assert_eq!(prepared.consumer.consumer_type(), ConsumerType::Citizen);
        assert!(
            prepared
                .subscription
                .rules()
                .contains(&SubscriptionRule::Visibility(
                    CITIZEN_ALERT_VISIBILITIES.to_vec()
                ))
        );
        assert_eq!(
            prepared.delivery_preference.channels()[0].channel(),
            ChannelType::Push
        );
    }

    #[test]
    fn returns_a_management_token_whose_hash_matches_what_is_persisted() {
        let prepared = prepare_citizen_subscription(valid_request()).unwrap();

        assert!(verify_management_token(
            &prepared.management_token,
            &prepared.management_token_hash
        ));
        assert!(!verify_management_token(
            "a-completely-different-token",
            &prepared.management_token_hash
        ));
    }

    #[test]
    fn rejects_no_incident_types() {
        let mut request = valid_request();
        request.incident_types = vec![];
        assert_eq!(
            prepare_citizen_subscription(request).unwrap_err(),
            CitizenSubscriptionError::EmptyIncidentTypes
        );
    }

    #[test]
    fn rejects_blank_geography() {
        let mut request = valid_request();
        request.geography = "   ".into();
        assert_eq!(
            prepare_citizen_subscription(request).unwrap_err(),
            CitizenSubscriptionError::InvalidGeography
        );
    }

    #[test]
    fn rejects_a_blank_push_subscription() {
        let mut request = valid_request();
        request.push_subscription_json = "".into();
        assert_eq!(
            prepare_citizen_subscription(request).unwrap_err(),
            CitizenSubscriptionError::InvalidPushSubscription
        );
    }

    #[test]
    fn generated_management_tokens_are_unique() {
        assert_ne!(generate_management_token(), generate_management_token());
    }
}
