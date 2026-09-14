//! Deterministic subscription matching (docs/SUBSCRIPTION_ENGINE.md). Answers
//! "which consumers should receive this alert, and why?" — never whether a
//! case is true, and never who to notify by way of a model (ADR-007, prompt
//! 06: "Do not use an LLM for recipient selection"). Every decision here is a
//! pure function of an [`Alert`] and a [`Subscription`]'s rules.

use core::fmt;

use crate::{
    Alert, CaseEventType, ConsumerId, IncidentType, Severity, SubscriptionId, TargetGeography,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Comparison {
    GreaterThan,
    GreaterThanOrEqual,
    Equal,
    LessThanOrEqual,
    LessThan,
}

impl Comparison {
    fn evaluate<T: PartialOrd>(self, lhs: T, rhs: T) -> bool {
        match self {
            Self::GreaterThan => lhs > rhs,
            Self::GreaterThanOrEqual => lhs >= rhs,
            Self::Equal => lhs == rhs,
            Self::LessThanOrEqual => lhs <= rhs,
            Self::LessThan => lhs < rhs,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::GreaterThan => ">",
            Self::GreaterThanOrEqual => ">=",
            Self::Equal => "==",
            Self::LessThanOrEqual => "<=",
            Self::LessThan => "<",
        }
    }

    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::GreaterThan => "GREATER_THAN",
            Self::GreaterThanOrEqual => "GREATER_THAN_OR_EQUAL",
            Self::Equal => "EQUAL",
            Self::LessThanOrEqual => "LESS_THAN_OR_EQUAL",
            Self::LessThan => "LESS_THAN",
        }
    }

    pub fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "GREATER_THAN" => Some(Self::GreaterThan),
            "GREATER_THAN_OR_EQUAL" => Some(Self::GreaterThanOrEqual),
            "EQUAL" => Some(Self::Equal),
            "LESS_THAN_OR_EQUAL" => Some(Self::LessThanOrEqual),
            "LESS_THAN" => Some(Self::LessThan),
            _ => None,
        }
    }
}

/// A coarse, human-named subscriber area (a region, city, or municipality
/// name — never raw coordinates). Matching is a case-insensitive containment
/// check against the alert's [`TargetGeography`] description; this is a
/// deliberately simple stand-in until a canonical Cameroon administrative
/// geography model and PostGIS-backed geometries exist
/// (docs/OPEN_QUESTIONS.md, docs/SUBSCRIPTION_ENGINE.md section 5). Swapping
/// in true spatial intersection later only changes
/// [`GeoArea::matches_target`]; every other type here stays the same shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoArea(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyGeoArea;

impl fmt::Display for EmptyGeoArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "subscriber geo area cannot be blank")
    }
}

impl std::error::Error for EmptyGeoArea {}

impl GeoArea {
    pub fn new(name: impl Into<String>) -> Result<Self, EmptyGeoArea> {
        let name = name.into();
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(EmptyGeoArea);
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn matches_target(&self, target: &TargetGeography) -> bool {
        target
            .as_str()
            .to_lowercase()
            .contains(&self.0.to_lowercase())
    }
}

/// The closed set of conditions a subscription can express (docs/SUBSCRIPTION_ENGINE.md
/// section 4). Each variant matches against a structural [`Alert`] attribute,
/// never against sensitive [`crate::AlertFieldValue`] content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionRule {
    IncidentType(Vec<IncidentType>),
    Severity {
        operator: Comparison,
        value: Severity,
    },
    EventType(Vec<CaseEventType>),
    Geography(GeoArea),
}

/// One rule's contribution to a [`MatchDecision`], carrying enough detail to
/// reconstruct a human-readable explanation (docs/SUBSCRIPTION_ENGINE.md
/// section 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchReason {
    pub rule: SubscriptionRule,
    pub matched: bool,
}

impl fmt::Display for MatchReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verdict = if self.matched { "match" } else { "no match" };
        match &self.rule {
            SubscriptionRule::IncidentType(values) => {
                write!(f, "incident type: expected one of {values:?} - {verdict}")
            }
            SubscriptionRule::Severity { operator, value } => {
                write!(
                    f,
                    "severity: alert {} {value:?} - {verdict}",
                    operator.as_str()
                )
            }
            SubscriptionRule::EventType(values) => {
                write!(f, "event: expected one of {values:?} - {verdict}")
            }
            SubscriptionRule::Geography(area) => {
                write!(
                    f,
                    "geography: target intersects {} - {verdict}",
                    area.as_str()
                )
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptySubscriptionRules;

impl fmt::Display for EmptySubscriptionRules {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a subscription must declare at least one rule")
    }
}

impl std::error::Error for EmptySubscriptionRules {}

/// A named set of conditions plus the consumer that owns them
/// (docs/DOMAIN_MODEL.md section 10). Delivery preferences (channel,
/// priority, fallback) are a separate, later concern
/// (docs/SUBSCRIPTION_ENGINE.md section 9) and are not modeled here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    id: SubscriptionId,
    consumer_id: ConsumerId,
    version: u32,
    rules: Vec<SubscriptionRule>,
}

impl Subscription {
    pub fn new(
        id: SubscriptionId,
        consumer_id: ConsumerId,
        version: u32,
        rules: Vec<SubscriptionRule>,
    ) -> Result<Self, EmptySubscriptionRules> {
        if rules.is_empty() {
            return Err(EmptySubscriptionRules);
        }
        Ok(Self {
            id,
            consumer_id,
            version,
            rules,
        })
    }

    pub fn id(&self) -> SubscriptionId {
        self.id
    }

    pub fn consumer_id(&self) -> ConsumerId {
        self.consumer_id
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn rules(&self) -> &[SubscriptionRule] {
        &self.rules
    }
}

/// A single subscription's outcome for one alert. Captures
/// [`Subscription::version`] at evaluation time (not just the subscription's
/// id) so a historical decision remains explainable even after the
/// subscription's live rules change (docs/SUBSCRIPTION_ENGINE.md section 11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchDecision {
    pub subscription_id: SubscriptionId,
    pub subscription_version: u32,
    pub consumer_id: ConsumerId,
    pub matched: bool,
    pub reasons: Vec<MatchReason>,
}

fn evaluate_rule(rule: &SubscriptionRule, alert: &Alert) -> MatchReason {
    let matched = match rule {
        SubscriptionRule::IncidentType(values) => values.contains(&alert.incident_type()),
        SubscriptionRule::Severity { operator, value } => {
            operator.evaluate(alert.severity(), *value)
        }
        SubscriptionRule::EventType(values) => values.contains(&alert.trigger()),
        SubscriptionRule::Geography(area) => area.matches_target(alert.target_geography()),
    };
    MatchReason {
        rule: rule.clone(),
        matched,
    }
}

/// A subscription matches an alert only when every one of its rules matches;
/// an empty reason list cannot occur, since [`Subscription::new`] rejects an
/// empty rule set.
pub fn evaluate_subscription(subscription: &Subscription, alert: &Alert) -> MatchDecision {
    let reasons: Vec<MatchReason> = subscription
        .rules()
        .iter()
        .map(|rule| evaluate_rule(rule, alert))
        .collect();
    let matched = reasons.iter().all(|reason| reason.matched);
    MatchDecision {
        subscription_id: subscription.id(),
        subscription_version: subscription.version(),
        consumer_id: subscription.consumer_id(),
        matched,
        reasons,
    }
}

/// Evaluates an already-narrowed candidate set (docs/SUBSCRIPTION_ENGINE.md
/// section 7: indexed/geographic candidate filtering happens before this
/// point, once subscriptions are persisted; this function is the
/// deterministic core the filtered set is handed to).
pub fn evaluate_subscriptions(subscriptions: &[Subscription], alert: &Alert) -> Vec<MatchDecision> {
    subscriptions
        .iter()
        .map(|subscription| evaluate_subscription(subscription, alert))
        .collect()
}

/// One consumer's matched subscriptions for an alert. The full list is kept
/// for traceability (docs/SUBSCRIPTION_ENGINE.md section 8) even though a
/// consumer should receive one effective delivery per channel/endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerMatch {
    pub consumer_id: ConsumerId,
    pub matching_subscriptions: Vec<SubscriptionId>,
}

/// Groups matched decisions by consumer, preserving the order consumers were
/// first seen in `decisions`.
pub fn deduplicate_by_consumer(decisions: &[MatchDecision]) -> Vec<ConsumerMatch> {
    let mut result: Vec<ConsumerMatch> = Vec::new();
    for decision in decisions.iter().filter(|decision| decision.matched) {
        match result
            .iter_mut()
            .find(|consumer_match| consumer_match.consumer_id == decision.consumer_id)
        {
            Some(consumer_match) => consumer_match
                .matching_subscriptions
                .push(decision.subscription_id),
            None => result.push(ConsumerMatch {
                consumer_id: decision.consumer_id,
                matching_subscriptions: vec![decision.subscription_id],
            }),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AlertField, AlertFieldValue, AlertPolicy, Case, CaseStatus, ReportId};

    fn verified_case(incident_type: IncidentType) -> Case {
        let (mut case, _) = Case::create(incident_type, ReportId::new());
        case.transition_to(CaseStatus::UnderReview).unwrap();
        case.transition_to(CaseStatus::Verified).unwrap();
        case
    }

    fn community_alert(incident_type: IncidentType, severity: Severity, area: &str) -> Alert {
        let case = verified_case(incident_type);
        let policy = AlertPolicy::missing_child_community_v1();
        let (alert, _) = Alert::create_from_case(
            &case,
            &policy,
            severity,
            TargetGeography::new(area).unwrap(),
            vec![AlertFieldValue {
                field: AlertField::IncidentCategory,
                value: "MISSING_CHILD".into(),
            }],
        )
        .unwrap();
        alert
    }

    fn subscription(rules: Vec<SubscriptionRule>) -> Subscription {
        Subscription::new(SubscriptionId::new(), ConsumerId::new(), 1, rules).unwrap()
    }

    #[test]
    fn subscription_requires_at_least_one_rule() {
        assert_eq!(
            Subscription::new(SubscriptionId::new(), ConsumerId::new(), 1, vec![]).unwrap_err(),
            EmptySubscriptionRules
        );
    }

    #[test]
    fn decision_table_incident_type_rule() {
        let alert = community_alert(IncidentType::MissingChild, Severity::High, "Douala");
        for (values, expected) in [
            (vec![IncidentType::MissingChild], true),
            (
                vec![
                    IncidentType::MissingChild,
                    IncidentType::OtherProtectionIncident,
                ],
                true,
            ),
            (vec![IncidentType::OtherProtectionIncident], false),
        ] {
            let subscription = subscription(vec![SubscriptionRule::IncidentType(values)]);
            let decision = evaluate_subscription(&subscription, &alert);
            assert_eq!(decision.matched, expected);
        }
    }

    #[test]
    fn decision_table_severity_comparisons() {
        let alert = community_alert(IncidentType::MissingChild, Severity::High, "Douala");
        for (operator, threshold, expected) in [
            (Comparison::GreaterThanOrEqual, Severity::Medium, true),
            (Comparison::GreaterThanOrEqual, Severity::High, true),
            (Comparison::GreaterThanOrEqual, Severity::Critical, false),
            (Comparison::GreaterThan, Severity::Medium, true),
            (Comparison::GreaterThan, Severity::High, false),
            (Comparison::Equal, Severity::High, true),
            (Comparison::Equal, Severity::Low, false),
            (Comparison::LessThanOrEqual, Severity::High, true),
            (Comparison::LessThanOrEqual, Severity::Medium, false),
            (Comparison::LessThan, Severity::Critical, true),
            (Comparison::LessThan, Severity::High, false),
        ] {
            let subscription = subscription(vec![SubscriptionRule::Severity {
                operator,
                value: threshold,
            }]);
            let decision = evaluate_subscription(&subscription, &alert);
            assert_eq!(
                decision.matched, expected,
                "{operator:?} {threshold:?} against HIGH expected {expected}"
            );
        }
    }

    #[test]
    fn decision_table_event_type_rule() {
        let alert = community_alert(IncidentType::MissingChild, Severity::High, "Douala");
        assert_eq!(alert.trigger(), CaseEventType::CaseVerified);

        for (values, expected) in [
            (vec![CaseEventType::CaseVerified], true),
            (
                vec![CaseEventType::CaseVerified, CaseEventType::CaseResolved],
                true,
            ),
            (vec![CaseEventType::CaseResolved], false),
        ] {
            let subscription = subscription(vec![SubscriptionRule::EventType(values)]);
            assert_eq!(
                evaluate_subscription(&subscription, &alert).matched,
                expected
            );
        }
    }

    #[test]
    fn decision_table_geography_rule() {
        let alert = community_alert(
            IncidentType::MissingChild,
            Severity::High,
            "Douala - Bonamoussadi",
        );
        for (area, expected) in [
            ("Douala", true),
            ("douala", true),
            ("Bonamoussadi", true),
            ("Yaounde", false),
        ] {
            let subscription = subscription(vec![SubscriptionRule::Geography(
                GeoArea::new(area).unwrap(),
            )]);
            assert_eq!(
                evaluate_subscription(&subscription, &alert).matched,
                expected
            );
        }
    }

    #[test]
    fn a_subscription_matches_only_when_every_rule_matches() {
        let alert = community_alert(IncidentType::MissingChild, Severity::High, "Douala");
        let subscription = subscription(vec![
            SubscriptionRule::IncidentType(vec![IncidentType::MissingChild]),
            SubscriptionRule::Severity {
                operator: Comparison::GreaterThanOrEqual,
                value: Severity::High,
            },
            SubscriptionRule::Geography(GeoArea::new("Yaounde").unwrap()),
        ]);

        let decision = evaluate_subscription(&subscription, &alert);
        assert!(
            !decision.matched,
            "geography rule fails, so the whole subscription must not match"
        );
        assert_eq!(decision.reasons.len(), 3);
        assert!(decision.reasons[0].matched);
        assert!(decision.reasons[1].matched);
        assert!(!decision.reasons[2].matched);
    }

    #[test]
    fn match_decision_captures_the_subscription_version_at_evaluation_time() {
        let alert = community_alert(IncidentType::MissingChild, Severity::High, "Douala");
        let subscription = Subscription::new(
            SubscriptionId::new(),
            ConsumerId::new(),
            7,
            vec![SubscriptionRule::IncidentType(vec![
                IncidentType::MissingChild,
            ])],
        )
        .unwrap();

        let decision = evaluate_subscription(&subscription, &alert);
        assert_eq!(decision.subscription_version, 7);
    }

    #[test]
    fn match_reason_renders_a_human_readable_explanation() {
        let alert = community_alert(IncidentType::MissingChild, Severity::High, "Douala");
        let subscription = subscription(vec![SubscriptionRule::Severity {
            operator: Comparison::GreaterThanOrEqual,
            value: Severity::Medium,
        }]);

        let decision = evaluate_subscription(&subscription, &alert);
        let explanation = decision.reasons[0].to_string();
        assert_eq!(explanation, "severity: alert >= Medium - match");
    }

    #[test]
    fn deduplicates_multiple_matching_subscriptions_for_the_same_consumer() {
        let alert = community_alert(IncidentType::MissingChild, Severity::High, "Douala");
        let consumer = ConsumerId::new();
        let matching_a = Subscription::new(
            SubscriptionId::new(),
            consumer,
            1,
            vec![SubscriptionRule::IncidentType(vec![
                IncidentType::MissingChild,
            ])],
        )
        .unwrap();
        let matching_b = Subscription::new(
            SubscriptionId::new(),
            consumer,
            1,
            vec![SubscriptionRule::Geography(GeoArea::new("Douala").unwrap())],
        )
        .unwrap();
        let non_matching = Subscription::new(
            SubscriptionId::new(),
            ConsumerId::new(),
            1,
            vec![SubscriptionRule::Geography(
                GeoArea::new("Yaounde").unwrap(),
            )],
        )
        .unwrap();

        let decisions = evaluate_subscriptions(
            &[matching_a.clone(), matching_b.clone(), non_matching],
            &alert,
        );
        let consumer_matches = deduplicate_by_consumer(&decisions);

        assert_eq!(consumer_matches.len(), 1);
        assert_eq!(consumer_matches[0].consumer_id, consumer);
        assert_eq!(
            consumer_matches[0].matching_subscriptions,
            vec![matching_a.id(), matching_b.id()]
        );
    }

    #[test]
    fn distinct_consumers_are_kept_separate() {
        let alert = community_alert(IncidentType::MissingChild, Severity::High, "Douala");
        let subscriptions: Vec<Subscription> = (0..3)
            .map(|_| {
                subscription(vec![SubscriptionRule::IncidentType(vec![
                    IncidentType::MissingChild,
                ])])
            })
            .collect();

        let decisions = evaluate_subscriptions(&subscriptions, &alert);
        let consumer_matches = deduplicate_by_consumer(&decisions);

        assert_eq!(consumer_matches.len(), 3);
        for consumer_match in &consumer_matches {
            assert_eq!(consumer_match.matching_subscriptions.len(), 1);
        }
    }

    #[test]
    fn comparison_database_value_round_trips_for_every_variant() {
        for operator in [
            Comparison::GreaterThan,
            Comparison::GreaterThanOrEqual,
            Comparison::Equal,
            Comparison::LessThanOrEqual,
            Comparison::LessThan,
        ] {
            assert_eq!(
                Comparison::from_database_value(operator.as_database_value()),
                Some(operator)
            );
        }
    }

    #[test]
    fn comparison_from_database_value_rejects_unknown_strings() {
        assert_eq!(Comparison::from_database_value("~="), None);
    }
}
