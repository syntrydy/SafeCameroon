//! Delivery planning and the delivery lifecycle (docs/CHANNELS.md,
//! docs/EVENTS.md section 4). A [`Delivery`] is one attempt-tracking record
//! for getting an already-matched [`Alert`] to one consumer over one channel
//! endpoint; it never decides *whether* a case is true or *who* should be
//! notified — that is [`crate::evaluate_subscriptions`]'s job.
//!
//! Delivery preferences (channel endpoints, fan-out strategy) are modeled
//! per-[`ConsumerId`] rather than per-[`crate::Subscription`]: a
//! [`crate::ConsumerMatch`] has already deduplicated multiple matching
//! subscriptions down to one consumer, and there is no persisted `Consumer`
//! or per-subscription delivery config yet (docs/OPEN_QUESTIONS.md). A
//! consumer having one set of contact endpoints and one fallback strategy for
//! every alert it receives is the simplest model that fits what actually
//! exists today; per-subscription overrides can be added later without
//! changing the [`Delivery`] shape.

use core::fmt;

use crate::{Alert, AlertId, ConsumerId, ConsumerMatch, SubscriptionId};

/// The first supported outbound channels (docs/CHANNELS.md section 3). A
/// closed enum here is what makes fallback/priority planning exhaustive and
/// checkable; new channels are added as variants, never as free-form strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelType {
    WhatsApp,
    Sms,
    Email,
}

impl ChannelType {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::WhatsApp => "WHATSAPP",
            Self::Sms => "SMS",
            Self::Email => "EMAIL",
        }
    }

    pub fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "WHATSAPP" => Some(Self::WhatsApp),
            "SMS" => Some(Self::Sms),
            "EMAIL" => Some(Self::Email),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyChannelEndpointAddress;

impl fmt::Display for EmptyChannelEndpointAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a channel endpoint address cannot be blank")
    }
}

impl std::error::Error for EmptyChannelEndpointAddress {}

/// A channel plus the address a message is sent to on it (a phone number, a
/// WhatsApp-enabled number, an email address). Never validated for
/// deliverability here — only that it is not blank; provider adapters own
/// format/deliverability checks (docs/CHANNELS.md section 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelEndpoint {
    channel: ChannelType,
    address: String,
}

impl ChannelEndpoint {
    pub fn new(
        channel: ChannelType,
        address: impl Into<String>,
    ) -> Result<Self, EmptyChannelEndpointAddress> {
        let address = address.into();
        let trimmed = address.trim();
        if trimmed.is_empty() {
            return Err(EmptyChannelEndpointAddress);
        }
        Ok(Self {
            channel,
            address: trimmed.to_owned(),
        })
    }

    pub fn channel(&self) -> ChannelType {
        self.channel
    }

    pub fn address(&self) -> &str {
        &self.address
    }
}

/// The named fan-out strategies from docs/CHANNELS.md section 6. Kept as
/// three explicit variants (rather than collapsing `PrimaryFallback` into a
/// two-element `PriorityList`) because the vocabulary is what gets persisted
/// and shown to reviewers; [`DeliveryPreference::tiers`] is what actually
/// interprets each one, and treats the fallback and priority-list cases
/// identically (a sequential chain), differing only from `All` (fan-out).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStrategy {
    All,
    PrimaryFallback,
    PriorityList,
}

impl DeliveryStrategy {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::PrimaryFallback => "PRIMARY_FALLBACK",
            Self::PriorityList => "PRIORITY_LIST",
        }
    }

    pub fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "ALL" => Some(Self::All),
            "PRIMARY_FALLBACK" => Some(Self::PrimaryFallback),
            "PRIORITY_LIST" => Some(Self::PriorityList),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryPreferenceError {
    EmptyChannelList,
    DuplicateChannel { channel: ChannelType },
}

impl fmt::Display for DeliveryPreferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyChannelList => {
                write!(f, "a delivery preference must list at least one channel")
            }
            Self::DuplicateChannel { channel } => {
                write!(f, "{channel:?} was listed more than once")
            }
        }
    }
}

impl std::error::Error for DeliveryPreferenceError {}

/// A consumer's contact endpoints and how to fan out across them
/// (docs/SUBSCRIPTION_ENGINE.md section 9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryPreference {
    strategy: DeliveryStrategy,
    channels: Vec<ChannelEndpoint>,
}

impl DeliveryPreference {
    pub fn new(
        strategy: DeliveryStrategy,
        channels: Vec<ChannelEndpoint>,
    ) -> Result<Self, DeliveryPreferenceError> {
        if channels.is_empty() {
            return Err(DeliveryPreferenceError::EmptyChannelList);
        }
        let mut seen: Vec<ChannelType> = Vec::with_capacity(channels.len());
        for endpoint in &channels {
            if seen.contains(&endpoint.channel()) {
                return Err(DeliveryPreferenceError::DuplicateChannel {
                    channel: endpoint.channel(),
                });
            }
            seen.push(endpoint.channel());
        }
        Ok(Self { strategy, channels })
    }

    pub fn strategy(&self) -> DeliveryStrategy {
        self.strategy
    }

    pub fn channels(&self) -> &[ChannelEndpoint] {
        &self.channels
    }

    /// Each configured endpoint paired with its fallback tier: `0` is
    /// attempted immediately, `1` only if every tier-`0` delivery for the
    /// same alert/consumer fails, and so on. `All` puts every endpoint at
    /// tier `0`; `PrimaryFallback`/`PriorityList` number endpoints by their
    /// configured order. Deciding *when* a later tier actually fires is a
    /// job-processing concern, not planning's (docs/CHANNELS.md section 6).
    fn tiers(&self) -> Vec<(u8, &ChannelEndpoint)> {
        self.channels
            .iter()
            .enumerate()
            .map(|(index, endpoint)| {
                let tier = match self.strategy {
                    DeliveryStrategy::All => 0,
                    // Bounded by ChannelType's variant count, which is far
                    // below u8::MAX, so this cast never truncates.
                    DeliveryStrategy::PrimaryFallback | DeliveryStrategy::PriorityList => {
                        index as u8
                    }
                };
                (tier, endpoint)
            })
            .collect()
    }
}

/// A stable dedup/idempotency key: `consumer_id + alert_id + channel +
/// endpoint` (docs/CHANNELS.md section 5, docs/SUBSCRIPTION_ENGINE.md section
/// 8). Hashing this for storage is an infrastructure concern, mirroring how
/// report idempotency keys are hashed at the application boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeliveryIdempotencyKey(String);

impl DeliveryIdempotencyKey {
    pub fn new(
        consumer_id: ConsumerId,
        alert_id: AlertId,
        channel: ChannelType,
        endpoint_address: &str,
    ) -> Self {
        Self(format!(
            "{}:{}:{}:{}",
            consumer_id.as_uuid(),
            alert_id.as_uuid(),
            channel.as_database_value(),
            endpoint_address,
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidRetryPolicy;

impl fmt::Display for InvalidRetryPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a retry policy must allow at least one attempt")
    }
}

impl std::error::Error for InvalidRetryPolicy {}

/// How many total attempts (including the first) a delivery may make before
/// a retryable failure is treated as permanent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    max_attempts: u32,
}

impl RetryPolicy {
    pub fn new(max_attempts: u32) -> Result<Self, InvalidRetryPolicy> {
        if max_attempts == 0 {
            return Err(InvalidRetryPolicy);
        }
        Ok(Self { max_attempts })
    }

    /// No documented default attempt count exists yet
    /// (docs/OPEN_QUESTIONS.md); five is a deliberately explicit starting
    /// point rather than an implicit/hidden one, and is easy to change once
    /// a provider SLA is chosen.
    pub fn standard() -> Self {
        Self { max_attempts: 5 }
    }

    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }
}

/// docs/CHANNELS.md section 4. `Failed` is deliberately not a resting
/// [`Delivery`] status: a failed attempt is recorded on the
/// [`DeliveryAttempt`] itself (`outcome: Failed { .. }`), and the aggregate
/// moves straight to whichever of `Retrying`/`FailedPermanently` follows, so
/// there is never an ambiguous state asking "has this been triaged yet?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStatus {
    Queued,
    Sending,
    Sent,
    Delivered,
    Retrying,
    FailedPermanently,
}

impl DeliveryStatus {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::Queued => "QUEUED",
            Self::Sending => "SENDING",
            Self::Sent => "SENT",
            Self::Delivered => "DELIVERED",
            Self::Retrying => "RETRYING",
            Self::FailedPermanently => "FAILED_PERMANENTLY",
        }
    }

    pub fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "QUEUED" => Some(Self::Queued),
            "SENDING" => Some(Self::Sending),
            "SENT" => Some(Self::Sent),
            "DELIVERED" => Some(Self::Delivered),
            "RETRYING" => Some(Self::Retrying),
            "FAILED_PERMANENTLY" => Some(Self::FailedPermanently),
            _ => None,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Delivered | Self::FailedPermanently)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryTransitionError {
    pub from: DeliveryStatus,
    pub to: DeliveryStatus,
}

impl fmt::Display for DeliveryTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "delivery cannot transition from {:?} to {:?}",
            self.from, self.to
        )
    }
}

impl std::error::Error for DeliveryTransitionError {}

/// docs/EVENTS.md section 4, verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryEventType {
    DeliveryRequested,
    DeliveryStarted,
    DeliverySent,
    DeliveryDelivered,
    DeliveryFailed,
    DeliveryRetried,
}

impl DeliveryEventType {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::DeliveryRequested => "DELIVERY_REQUESTED",
            Self::DeliveryStarted => "DELIVERY_STARTED",
            Self::DeliverySent => "DELIVERY_SENT",
            Self::DeliveryDelivered => "DELIVERY_DELIVERED",
            Self::DeliveryFailed => "DELIVERY_FAILED",
            Self::DeliveryRetried => "DELIVERY_RETRIED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryEvent {
    pub id: crate::DeliveryEventId,
    pub delivery_id: crate::DeliveryId,
    pub event_type: DeliveryEventType,
    pub aggregate_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryAttemptOutcome {
    Sent { provider_message_id: Option<String> },
    Failed { retryable: bool, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryAttempt {
    pub id: crate::DeliveryAttemptId,
    pub delivery_id: crate::DeliveryId,
    pub attempt_number: u32,
    pub outcome: DeliveryAttemptOutcome,
}

/// One attempt-tracking record for getting an alert to one consumer over one
/// channel endpoint (docs/DOMAIN_MODEL.md section 12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    id: crate::DeliveryId,
    alert_id: AlertId,
    consumer_id: ConsumerId,
    channel: ChannelType,
    endpoint_address: String,
    tier: u8,
    idempotency_key: DeliveryIdempotencyKey,
    matching_subscription_ids: Vec<SubscriptionId>,
    retry_policy: RetryPolicy,
    status: DeliveryStatus,
    attempt_count: u32,
    version: u64,
}

impl Delivery {
    #[allow(clippy::too_many_arguments)]
    fn plan_one(
        alert_id: AlertId,
        consumer_id: ConsumerId,
        endpoint: &ChannelEndpoint,
        tier: u8,
        matching_subscription_ids: Vec<SubscriptionId>,
        retry_policy: RetryPolicy,
    ) -> (Self, DeliveryEvent) {
        let delivery = Self {
            id: crate::DeliveryId::new(),
            alert_id,
            consumer_id,
            channel: endpoint.channel(),
            endpoint_address: endpoint.address().to_owned(),
            tier,
            idempotency_key: DeliveryIdempotencyKey::new(
                consumer_id,
                alert_id,
                endpoint.channel(),
                endpoint.address(),
            ),
            matching_subscription_ids,
            retry_policy,
            status: DeliveryStatus::Queued,
            attempt_count: 0,
            version: 1,
        };
        let event = delivery.event(DeliveryEventType::DeliveryRequested);
        (delivery, event)
    }

    /// Rebuilds a delivery aggregate from persisted state; performs no
    /// validation, mirroring [`crate::Alert::reconstitute`]. Only the
    /// idempotency key's hash is persisted (not the key itself), so it is
    /// recomputed here the same deterministic way it was first derived.
    #[allow(clippy::too_many_arguments)]
    pub fn reconstitute(
        id: crate::DeliveryId,
        alert_id: AlertId,
        consumer_id: ConsumerId,
        channel: ChannelType,
        endpoint_address: String,
        tier: u8,
        matching_subscription_ids: Vec<SubscriptionId>,
        retry_policy: RetryPolicy,
        status: DeliveryStatus,
        attempt_count: u32,
        version: u64,
    ) -> Self {
        let idempotency_key =
            DeliveryIdempotencyKey::new(consumer_id, alert_id, channel, &endpoint_address);
        Self {
            id,
            alert_id,
            consumer_id,
            channel,
            endpoint_address,
            tier,
            idempotency_key,
            matching_subscription_ids,
            retry_policy,
            status,
            attempt_count,
            version,
        }
    }

    pub fn id(&self) -> crate::DeliveryId {
        self.id
    }
    pub fn alert_id(&self) -> AlertId {
        self.alert_id
    }
    pub fn consumer_id(&self) -> ConsumerId {
        self.consumer_id
    }
    pub fn channel(&self) -> ChannelType {
        self.channel
    }
    pub fn endpoint_address(&self) -> &str {
        &self.endpoint_address
    }
    pub fn tier(&self) -> u8 {
        self.tier
    }
    pub fn idempotency_key(&self) -> &DeliveryIdempotencyKey {
        &self.idempotency_key
    }
    pub fn matching_subscription_ids(&self) -> &[SubscriptionId] {
        &self.matching_subscription_ids
    }
    pub fn status(&self) -> DeliveryStatus {
        self.status
    }
    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }
    pub fn max_attempts(&self) -> u32 {
        self.retry_policy.max_attempts()
    }
    pub fn version(&self) -> u64 {
        self.version
    }

    fn event(&self, event_type: DeliveryEventType) -> DeliveryEvent {
        DeliveryEvent {
            id: crate::DeliveryEventId::new(),
            delivery_id: self.id,
            event_type,
            aggregate_version: self.version,
        }
    }

    fn transition_error(&self, to: DeliveryStatus) -> DeliveryTransitionError {
        DeliveryTransitionError {
            from: self.status,
            to,
        }
    }

    /// `Queued`/`Retrying` -> `Sending`. Starts (or restarts, after a
    /// retryable failure) one send attempt.
    pub fn start_attempt(&mut self) -> Result<DeliveryEvent, DeliveryTransitionError> {
        if !matches!(
            self.status,
            DeliveryStatus::Queued | DeliveryStatus::Retrying
        ) {
            return Err(self.transition_error(DeliveryStatus::Sending));
        }
        self.status = DeliveryStatus::Sending;
        self.attempt_count += 1;
        self.version += 1;
        Ok(self.event(DeliveryEventType::DeliveryStarted))
    }

    /// `Sending` -> `Sent`.
    pub fn record_success(
        &mut self,
        provider_message_id: Option<String>,
    ) -> Result<(DeliveryEvent, DeliveryAttempt), DeliveryTransitionError> {
        if self.status != DeliveryStatus::Sending {
            return Err(self.transition_error(DeliveryStatus::Sent));
        }
        self.status = DeliveryStatus::Sent;
        self.version += 1;
        let attempt = DeliveryAttempt {
            id: crate::DeliveryAttemptId::new(),
            delivery_id: self.id,
            attempt_number: self.attempt_count,
            outcome: DeliveryAttemptOutcome::Sent {
                provider_message_id,
            },
        };
        Ok((self.event(DeliveryEventType::DeliverySent), attempt))
    }

    /// `Sent` -> `Delivered`, from an asynchronous provider callback.
    pub fn record_delivered(&mut self) -> Result<DeliveryEvent, DeliveryTransitionError> {
        if self.status != DeliveryStatus::Sent {
            return Err(self.transition_error(DeliveryStatus::Delivered));
        }
        self.status = DeliveryStatus::Delivered;
        self.version += 1;
        Ok(self.event(DeliveryEventType::DeliveryDelivered))
    }

    /// `Sending` -> `Retrying` (if `retryable` and attempts remain) or
    /// `FailedPermanently` otherwise.
    pub fn record_failure(
        &mut self,
        retryable: bool,
        reason: impl Into<String>,
    ) -> Result<(DeliveryEvent, DeliveryAttempt), DeliveryTransitionError> {
        if self.status != DeliveryStatus::Sending {
            // Both possible next statuses report the same "from" state, so
            // FailedPermanently is as good a `to` to report as Retrying.
            return Err(self.transition_error(DeliveryStatus::FailedPermanently));
        }
        let will_retry = retryable && self.attempt_count < self.retry_policy.max_attempts();
        self.status = if will_retry {
            DeliveryStatus::Retrying
        } else {
            DeliveryStatus::FailedPermanently
        };
        self.version += 1;
        let attempt = DeliveryAttempt {
            id: crate::DeliveryAttemptId::new(),
            delivery_id: self.id,
            attempt_number: self.attempt_count,
            outcome: DeliveryAttemptOutcome::Failed {
                retryable,
                reason: reason.into(),
            },
        };
        let event_type = if will_retry {
            DeliveryEventType::DeliveryRetried
        } else {
            DeliveryEventType::DeliveryFailed
        };
        Ok((self.event(event_type), attempt))
    }
}

/// Plans deliveries for every consumer a subscription matched
/// (docs/SUBSCRIPTION_ENGINE.md section 2: "match decisions -> consumer
/// dedup -> delivery planning"). A consumer with no configured
/// [`DeliveryPreference`] is skipped rather than guessed at — there is
/// nowhere to send a message without a configured endpoint.
pub fn plan_deliveries(
    alert: &Alert,
    consumer_matches: &[ConsumerMatch],
    preferences: &std::collections::HashMap<ConsumerId, DeliveryPreference>,
    retry_policy: RetryPolicy,
) -> Vec<(Delivery, DeliveryEvent)> {
    let mut planned = Vec::new();
    for consumer_match in consumer_matches {
        let Some(preference) = preferences.get(&consumer_match.consumer_id) else {
            continue;
        };
        for (tier, endpoint) in preference.tiers() {
            planned.push(Delivery::plan_one(
                alert.id(),
                consumer_match.consumer_id,
                endpoint,
                tier,
                consumer_match.matching_subscriptions.clone(),
                retry_policy,
            ));
        }
    }
    planned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AlertField, AlertFieldValue, AlertPolicy, Case, CaseStatus, IncidentType, ReportId,
        Severity, TargetGeography,
    };

    fn alert() -> Alert {
        let (mut case, _) = Case::create(IncidentType::MissingChild, ReportId::new());
        case.transition_to(CaseStatus::UnderReview).unwrap();
        case.transition_to(CaseStatus::Verified).unwrap();
        let policy = AlertPolicy::missing_child_community_v1();
        let (alert, _) = Alert::create_from_case(
            &case,
            &policy,
            Severity::High,
            TargetGeography::new("Douala").unwrap(),
            vec![AlertFieldValue {
                field: AlertField::IncidentCategory,
                value: "MISSING_CHILD".into(),
            }],
        )
        .unwrap();
        alert
    }

    fn endpoint(channel: ChannelType, address: &str) -> ChannelEndpoint {
        ChannelEndpoint::new(channel, address).unwrap()
    }

    fn single_delivery(status_setup: impl FnOnce(&mut Delivery)) -> Delivery {
        let alert = alert();
        let preference = DeliveryPreference::new(
            DeliveryStrategy::All,
            vec![endpoint(ChannelType::WhatsApp, "+237600000000")],
        )
        .unwrap();
        let mut preferences = std::collections::HashMap::new();
        let consumer_id = ConsumerId::new();
        preferences.insert(consumer_id, preference);
        let consumer_matches = vec![ConsumerMatch {
            consumer_id,
            matching_subscriptions: vec![SubscriptionId::new()],
        }];
        let mut planned = plan_deliveries(
            &alert,
            &consumer_matches,
            &preferences,
            RetryPolicy::standard(),
        );
        let (mut delivery, _) = planned.remove(0);
        status_setup(&mut delivery);
        delivery
    }

    #[test]
    fn channel_endpoint_rejects_blank_address() {
        assert!(ChannelEndpoint::new(ChannelType::Sms, "   ").is_err());
        assert_eq!(
            ChannelEndpoint::new(ChannelType::Sms, " +237600000000 ")
                .unwrap()
                .address(),
            "+237600000000"
        );
    }

    #[test]
    fn delivery_preference_rejects_an_empty_channel_list() {
        assert_eq!(
            DeliveryPreference::new(DeliveryStrategy::All, vec![]).unwrap_err(),
            DeliveryPreferenceError::EmptyChannelList
        );
    }

    #[test]
    fn delivery_preference_rejects_a_duplicate_channel() {
        let error = DeliveryPreference::new(
            DeliveryStrategy::PriorityList,
            vec![
                endpoint(ChannelType::WhatsApp, "+237600000000"),
                endpoint(ChannelType::WhatsApp, "+237600000001"),
            ],
        )
        .unwrap_err();
        assert_eq!(
            error,
            DeliveryPreferenceError::DuplicateChannel {
                channel: ChannelType::WhatsApp
            }
        );
    }

    #[test]
    fn retry_policy_rejects_zero_attempts() {
        assert_eq!(RetryPolicy::new(0).unwrap_err(), InvalidRetryPolicy);
        assert_eq!(RetryPolicy::new(3).unwrap().max_attempts(), 3);
    }

    #[test]
    fn all_strategy_assigns_tier_zero_to_every_channel() {
        let alert = alert();
        let preference = DeliveryPreference::new(
            DeliveryStrategy::All,
            vec![
                endpoint(ChannelType::WhatsApp, "+237600000000"),
                endpoint(ChannelType::Sms, "+237600000000"),
                endpoint(ChannelType::Email, "ngo@example.cm"),
            ],
        )
        .unwrap();
        let consumer_id = ConsumerId::new();
        let mut preferences = std::collections::HashMap::new();
        preferences.insert(consumer_id, preference);
        let consumer_matches = vec![ConsumerMatch {
            consumer_id,
            matching_subscriptions: vec![SubscriptionId::new()],
        }];

        let planned = plan_deliveries(
            &alert,
            &consumer_matches,
            &preferences,
            RetryPolicy::standard(),
        );

        assert_eq!(planned.len(), 3);
        for (delivery, _) in &planned {
            assert_eq!(delivery.tier(), 0);
            assert_eq!(delivery.status(), DeliveryStatus::Queued);
        }
    }

    #[test]
    fn primary_fallback_and_priority_list_assign_sequential_tiers() {
        for strategy in [
            DeliveryStrategy::PrimaryFallback,
            DeliveryStrategy::PriorityList,
        ] {
            let alert = alert();
            let preference = DeliveryPreference::new(
                strategy,
                vec![
                    endpoint(ChannelType::WhatsApp, "+237600000000"),
                    endpoint(ChannelType::Sms, "+237600000000"),
                ],
            )
            .unwrap();
            let consumer_id = ConsumerId::new();
            let mut preferences = std::collections::HashMap::new();
            preferences.insert(consumer_id, preference);
            let consumer_matches = vec![ConsumerMatch {
                consumer_id,
                matching_subscriptions: vec![SubscriptionId::new()],
            }];

            let planned = plan_deliveries(
                &alert,
                &consumer_matches,
                &preferences,
                RetryPolicy::standard(),
            );

            let tiers: Vec<u8> = planned
                .iter()
                .map(|(delivery, _)| delivery.tier())
                .collect();
            assert_eq!(tiers, vec![0, 1], "strategy {strategy:?}");
        }
    }

    #[test]
    fn planning_skips_consumers_without_a_configured_preference() {
        let alert = alert();
        let preferences = std::collections::HashMap::new();
        let consumer_matches = vec![ConsumerMatch {
            consumer_id: ConsumerId::new(),
            matching_subscriptions: vec![SubscriptionId::new()],
        }];

        let planned = plan_deliveries(
            &alert,
            &consumer_matches,
            &preferences,
            RetryPolicy::standard(),
        );

        assert!(planned.is_empty());
    }

    #[test]
    fn planning_produces_a_distinct_idempotency_key_per_delivery_and_carries_traceability() {
        let alert = alert();
        let preference = DeliveryPreference::new(
            DeliveryStrategy::All,
            vec![
                endpoint(ChannelType::WhatsApp, "+237600000000"),
                endpoint(ChannelType::Sms, "+237600000000"),
            ],
        )
        .unwrap();
        let consumer_id = ConsumerId::new();
        let mut preferences = std::collections::HashMap::new();
        preferences.insert(consumer_id, preference);
        let subscription_a = SubscriptionId::new();
        let subscription_b = SubscriptionId::new();
        let consumer_matches = vec![ConsumerMatch {
            consumer_id,
            matching_subscriptions: vec![subscription_a, subscription_b],
        }];

        let planned = plan_deliveries(
            &alert,
            &consumer_matches,
            &preferences,
            RetryPolicy::standard(),
        );

        assert_eq!(planned.len(), 2);
        let keys: std::collections::HashSet<&str> = planned
            .iter()
            .map(|(delivery, _)| delivery.idempotency_key().as_str())
            .collect();
        assert_eq!(keys.len(), 2, "idempotency keys must not collide");
        for (delivery, event) in &planned {
            assert_eq!(
                delivery.matching_subscription_ids(),
                &[subscription_a, subscription_b]
            );
            assert_eq!(event.event_type, DeliveryEventType::DeliveryRequested);
        }
    }

    #[test]
    fn start_attempt_transitions_queued_to_sending_and_counts_the_attempt() {
        let mut delivery = single_delivery(|_| {});
        assert_eq!(delivery.attempt_count(), 0);

        let event = delivery.start_attempt().unwrap();
        assert_eq!(event.event_type, DeliveryEventType::DeliveryStarted);
        assert_eq!(delivery.status(), DeliveryStatus::Sending);
        assert_eq!(delivery.attempt_count(), 1);
    }

    #[test]
    fn start_attempt_is_rejected_unless_queued_or_retrying() {
        let mut delivery = single_delivery(|delivery| {
            delivery.start_attempt().unwrap();
            delivery.record_success(None).unwrap();
        });
        assert_eq!(delivery.status(), DeliveryStatus::Sent);

        let error = delivery.start_attempt().unwrap_err();
        assert_eq!(error.from, DeliveryStatus::Sent);
        assert_eq!(error.to, DeliveryStatus::Sending);
    }

    #[test]
    fn record_success_transitions_sending_to_sent_and_produces_an_attempt() {
        let mut delivery = single_delivery(|delivery| {
            delivery.start_attempt().unwrap();
        });

        let (event, attempt) = delivery
            .record_success(Some("provider-msg-1".into()))
            .unwrap();
        assert_eq!(event.event_type, DeliveryEventType::DeliverySent);
        assert_eq!(delivery.status(), DeliveryStatus::Sent);
        assert_eq!(attempt.attempt_number, 1);
        assert_eq!(
            attempt.outcome,
            DeliveryAttemptOutcome::Sent {
                provider_message_id: Some("provider-msg-1".into())
            }
        );
    }

    #[test]
    fn record_success_is_rejected_unless_sending() {
        let mut delivery = single_delivery(|_| {});
        let error = delivery.record_success(None).unwrap_err();
        assert_eq!(error.from, DeliveryStatus::Queued);
    }

    #[test]
    fn record_delivered_transitions_sent_to_delivered_but_not_from_anywhere_else() {
        let mut delivery = single_delivery(|delivery| {
            delivery.start_attempt().unwrap();
            delivery.record_success(None).unwrap();
        });

        let event = delivery.record_delivered().unwrap();
        assert_eq!(event.event_type, DeliveryEventType::DeliveryDelivered);
        assert_eq!(delivery.status(), DeliveryStatus::Delivered);
        assert!(delivery.status().is_terminal());

        let error = delivery.record_delivered().unwrap_err();
        assert_eq!(error.from, DeliveryStatus::Delivered);
    }

    #[test]
    fn a_retryable_failure_with_attempts_remaining_moves_to_retrying() {
        let mut delivery = single_delivery(|delivery| {
            delivery.start_attempt().unwrap();
        });

        let (event, attempt) = delivery.record_failure(true, "provider timeout").unwrap();
        assert_eq!(event.event_type, DeliveryEventType::DeliveryRetried);
        assert_eq!(delivery.status(), DeliveryStatus::Retrying);
        assert!(!delivery.status().is_terminal());
        assert_eq!(
            attempt.outcome,
            DeliveryAttemptOutcome::Failed {
                retryable: true,
                reason: "provider timeout".into()
            }
        );
    }

    #[test]
    fn a_retryable_failure_that_exhausts_the_retry_policy_fails_permanently() {
        // A one-attempt retry policy means the very first failure already
        // exhausts it.
        let alert = alert();
        let preference = DeliveryPreference::new(
            DeliveryStrategy::All,
            vec![endpoint(ChannelType::WhatsApp, "+237600000000")],
        )
        .unwrap();
        let consumer_id = ConsumerId::new();
        let mut preferences = std::collections::HashMap::new();
        preferences.insert(consumer_id, preference);
        let consumer_matches = vec![ConsumerMatch {
            consumer_id,
            matching_subscriptions: vec![SubscriptionId::new()],
        }];
        let mut planned = plan_deliveries(
            &alert,
            &consumer_matches,
            &preferences,
            RetryPolicy::new(1).unwrap(),
        );
        let (mut delivery, _) = planned.remove(0);
        delivery.start_attempt().unwrap();

        let (event, _) = delivery.record_failure(true, "provider timeout").unwrap();
        assert_eq!(event.event_type, DeliveryEventType::DeliveryFailed);
        assert_eq!(delivery.status(), DeliveryStatus::FailedPermanently);
        assert!(delivery.status().is_terminal());
    }

    #[test]
    fn a_non_retryable_failure_fails_permanently_even_on_the_first_attempt() {
        let mut delivery = single_delivery(|delivery| {
            delivery.start_attempt().unwrap();
        });

        let (event, attempt) = delivery
            .record_failure(false, "invalid phone number")
            .unwrap();
        assert_eq!(event.event_type, DeliveryEventType::DeliveryFailed);
        assert_eq!(delivery.status(), DeliveryStatus::FailedPermanently);
        assert_eq!(
            attempt.outcome,
            DeliveryAttemptOutcome::Failed {
                retryable: false,
                reason: "invalid phone number".into()
            }
        );
    }

    #[test]
    fn record_failure_is_rejected_unless_sending() {
        let mut delivery = single_delivery(|_| {});
        let error = delivery.record_failure(true, "n/a").unwrap_err();
        assert_eq!(error.from, DeliveryStatus::Queued);
    }

    #[test]
    fn a_delivery_can_retry_across_multiple_attempts_before_failing_permanently() {
        let alert = alert();
        let preference = DeliveryPreference::new(
            DeliveryStrategy::All,
            vec![endpoint(ChannelType::WhatsApp, "+237600000000")],
        )
        .unwrap();
        let consumer_id = ConsumerId::new();
        let mut preferences = std::collections::HashMap::new();
        preferences.insert(consumer_id, preference);
        let consumer_matches = vec![ConsumerMatch {
            consumer_id,
            matching_subscriptions: vec![SubscriptionId::new()],
        }];
        let mut planned = plan_deliveries(
            &alert,
            &consumer_matches,
            &preferences,
            RetryPolicy::new(2).unwrap(),
        );
        let (mut delivery, _) = planned.remove(0);

        delivery.start_attempt().unwrap();
        let (event, _) = delivery.record_failure(true, "timeout").unwrap();
        assert_eq!(event.event_type, DeliveryEventType::DeliveryRetried);
        assert_eq!(delivery.status(), DeliveryStatus::Retrying);

        delivery.start_attempt().unwrap();
        assert_eq!(delivery.attempt_count(), 2);
        let (event, _) = delivery.record_failure(true, "timeout again").unwrap();
        assert_eq!(event.event_type, DeliveryEventType::DeliveryFailed);
        assert_eq!(delivery.status(), DeliveryStatus::FailedPermanently);
    }
}
