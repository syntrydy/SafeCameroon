//! Alert workflow use cases: turning a verified case into an alert through an
//! explicit policy, and cancelling one. Reuses [`Actor`](crate::case_workflow::Actor)
//! from the case workflow so authorization has one vocabulary across both.

use core::fmt;

use safe_cameroon_domain::{
    Alert, AlertCreationError, AlertEvent, AlertFieldValue, AlertPolicy, AlertTransitionError,
    AlertVisibility, AuditEventId, Case, OutboxEventId, Severity, TargetGeography,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::case_workflow::Actor;

/// A non-reviewer actor attempted to create or cancel an alert that requires
/// an identified reviewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlertAuthorizationError {
    pub visibility: AlertVisibility,
}

impl fmt::Display for AlertAuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "only a reviewer may act on a {:?} alert",
            self.visibility
        )
    }
}

impl std::error::Error for AlertAuthorizationError {}

/// A community/public alert reaches an uncontrolled audience, so AI or system
/// automation may never create or cancel one directly (AGENTS.md: "AI may
/// not ... directly send public alerts"). Internal/partner alerts stay
/// within the organization and may be raised automatically.
fn authorize_alert_action(
    actor: Actor,
    visibility: AlertVisibility,
) -> Result<(), AlertAuthorizationError> {
    if visibility.admits_internal_only_fields() {
        return Ok(());
    }
    match actor {
        Actor::Reviewer(_) => Ok(()),
        Actor::Automated => Err(AlertAuthorizationError { visibility }),
    }
}

/// Cancelling any alert (including internal/partner ones, which a consumer
/// may already have acted on) requires an identified reviewer.
fn authorize_alert_cancellation(
    actor: Actor,
    visibility: AlertVisibility,
) -> Result<(), AlertAuthorizationError> {
    match actor {
        Actor::Reviewer(_) => Ok(()),
        Actor::Automated => Err(AlertAuthorizationError { visibility }),
    }
}

/// The built-in alert policy templates known to this deployment. Prompt 05
/// starts with a single missing-child community template; more are added
/// here as they are approved, never invented ad hoc by a caller.
pub fn resolve_policy(policy_id: &str) -> Option<AlertPolicy> {
    match policy_id {
        "MISSING_CHILD_COMMUNITY" => Some(AlertPolicy::missing_child_community_v1()),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertCreationUseCaseError {
    NotAuthorized(AlertAuthorizationError),
    InvalidAlert(AlertCreationError),
}

impl fmt::Display for AlertCreationUseCaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAuthorized(error) => error.fmt(f),
            Self::InvalidAlert(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for AlertCreationUseCaseError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertCreation {
    pub alert: Alert,
    pub event: AlertEvent,
    pub actor: Actor,
    pub audit_event_id: AuditEventId,
    pub outbox_event_id: OutboxEventId,
    pub request_id: Uuid,
    /// Mirrors `AnonymousReportSubmission::idempotency_key_hash`: a retried
    /// `POST /v1/cases/{id}/alerts` request must produce one alert, not two
    /// (two alerts independently trigger subscription matching and produce
    /// duplicate, non-deduplicated deliveries — `DeliveryIdempotencyKey` is
    /// keyed on `alert_id`, so it cannot catch a duplicate *alert*).
    pub idempotency_key_hash: Option<Vec<u8>>,
}

#[allow(clippy::too_many_arguments)]
pub fn create_alert_from_case(
    case: &Case,
    policy: &AlertPolicy,
    severity: Severity,
    target_geography: TargetGeography,
    fields: Vec<AlertFieldValue>,
    actor: Actor,
    request_id: Uuid,
    idempotency_key: Option<&str>,
) -> Result<AlertCreation, AlertCreationUseCaseError> {
    authorize_alert_action(actor, policy.visibility())
        .map_err(AlertCreationUseCaseError::NotAuthorized)?;
    let (alert, event) = Alert::create_from_case(case, policy, severity, target_geography, fields)
        .map_err(AlertCreationUseCaseError::InvalidAlert)?;
    let idempotency_key_hash = idempotency_key.map(|key| Sha256::digest(key.as_bytes()).to_vec());
    Ok(AlertCreation {
        alert,
        event,
        actor,
        audit_event_id: AuditEventId::new(),
        outbox_event_id: OutboxEventId::new(),
        request_id,
        idempotency_key_hash,
    })
}

/// Deliberately carries only routing metadata, not the alert's field values:
/// a downstream consumer reads the actual content from the persisted alert,
/// keeping the outbox payload minimal (docs/DATA_GOVERNANCE.md).
pub fn alert_created_event_payload(creation: &AlertCreation) -> serde_json::Value {
    json!({
        "alert_id": creation.alert.id(),
        "case_id": creation.alert.case_id(),
        "policy_id": creation.alert.policy_id().as_str(),
        "policy_version": creation.alert.policy_version(),
        "visibility": creation.alert.visibility(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertCancellationError {
    NotAuthorized(AlertAuthorizationError),
    InvalidTransition(AlertTransitionError),
}

impl fmt::Display for AlertCancellationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAuthorized(error) => error.fmt(f),
            Self::InvalidTransition(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for AlertCancellationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertCancellation {
    pub event: AlertEvent,
    pub actor: Actor,
    pub audit_event_id: AuditEventId,
    pub outbox_event_id: OutboxEventId,
    pub request_id: Uuid,
}

pub fn cancel_alert(
    alert: &mut Alert,
    actor: Actor,
    request_id: Uuid,
) -> Result<AlertCancellation, AlertCancellationError> {
    authorize_alert_cancellation(actor, alert.visibility())
        .map_err(AlertCancellationError::NotAuthorized)?;
    let event = alert
        .cancel()
        .map_err(AlertCancellationError::InvalidTransition)?;
    Ok(AlertCancellation {
        event,
        actor,
        audit_event_id: AuditEventId::new(),
        outbox_event_id: OutboxEventId::new(),
        request_id,
    })
}

pub fn alert_cancelled_event_payload(
    alert: &Alert,
    cancellation: &AlertCancellation,
) -> serde_json::Value {
    json!({
        "alert_id": alert.id(),
        "case_id": alert.case_id(),
        "aggregate_version": cancellation.event.aggregate_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use safe_cameroon_domain::{AlertField, CaseStatus, IncidentType, ReportId};

    fn verified_case() -> Case {
        let (mut case, _) = Case::create(IncidentType::MissingChild, ReportId::new());
        case.transition_to(CaseStatus::UnderReview).unwrap();
        case.transition_to(CaseStatus::Verified).unwrap();
        case
    }

    fn safe_fields() -> Vec<AlertFieldValue> {
        vec![AlertFieldValue {
            field: AlertField::IncidentCategory,
            value: "MISSING_CHILD".into(),
        }]
    }

    #[test]
    fn resolves_the_known_missing_child_template_and_rejects_unknown_ids() {
        assert!(resolve_policy("MISSING_CHILD_COMMUNITY").is_some());
        assert!(resolve_policy("SOMETHING_ELSE").is_none());
    }

    #[test]
    fn automated_actor_cannot_create_a_community_alert() {
        let case = verified_case();
        let policy = AlertPolicy::missing_child_community_v1();
        let geography = TargetGeography::new("Douala").unwrap();

        let error = create_alert_from_case(
            &case,
            &policy,
            Severity::High,
            geography,
            safe_fields(),
            Actor::Automated,
            Uuid::new_v4(),
            None,
        )
        .unwrap_err();

        assert_eq!(
            error,
            AlertCreationUseCaseError::NotAuthorized(AlertAuthorizationError {
                visibility: AlertVisibility::Community
            })
        );
    }

    #[test]
    fn reviewer_can_create_a_community_alert() {
        let case = verified_case();
        let policy = AlertPolicy::missing_child_community_v1();
        let geography = TargetGeography::new("Douala").unwrap();

        let creation = create_alert_from_case(
            &case,
            &policy,
            Severity::High,
            geography,
            safe_fields(),
            Actor::Reviewer(Uuid::new_v4()),
            Uuid::new_v4(),
            None,
        )
        .unwrap();

        assert_eq!(creation.alert.visibility(), AlertVisibility::Community);
        let payload = alert_created_event_payload(&creation);
        assert_eq!(payload["policy_id"], json!("MISSING_CHILD_COMMUNITY"));
    }

    #[test]
    fn an_idempotency_key_is_hashed_into_the_creation() {
        let case = verified_case();
        let policy = AlertPolicy::missing_child_community_v1();
        let geography = TargetGeography::new("Douala").unwrap();

        let creation = create_alert_from_case(
            &case,
            &policy,
            Severity::High,
            geography,
            safe_fields(),
            Actor::Reviewer(Uuid::new_v4()),
            Uuid::new_v4(),
            Some("client-retry-key-1"),
        )
        .unwrap();

        assert_eq!(
            creation.idempotency_key_hash,
            Some(Sha256::digest(b"client-retry-key-1").to_vec())
        );
    }

    #[test]
    fn no_idempotency_key_means_no_hash() {
        let case = verified_case();
        let policy = AlertPolicy::missing_child_community_v1();
        let geography = TargetGeography::new("Douala").unwrap();

        let creation = create_alert_from_case(
            &case,
            &policy,
            Severity::High,
            geography,
            safe_fields(),
            Actor::Reviewer(Uuid::new_v4()),
            Uuid::new_v4(),
            None,
        )
        .unwrap();

        assert_eq!(creation.idempotency_key_hash, None);
    }

    #[test]
    fn automated_actor_can_create_an_internal_alert() {
        let case = verified_case();
        let policy = AlertPolicy::new(
            safe_cameroon_domain::AlertPolicyId::new("INTERNAL_TEST"),
            1,
            IncidentType::MissingChild,
            AlertVisibility::Internal,
            safe_cameroon_domain::CaseEventType::CaseVerified,
            vec![AlertField::IncidentCategory],
        )
        .unwrap();
        let geography = TargetGeography::new("Douala").unwrap();

        let creation = create_alert_from_case(
            &case,
            &policy,
            Severity::High,
            geography,
            safe_fields(),
            Actor::Automated,
            Uuid::new_v4(),
            None,
        )
        .unwrap();
        assert_eq!(creation.alert.visibility(), AlertVisibility::Internal);
    }

    #[test]
    fn automated_actor_cannot_cancel_even_an_internal_alert() {
        let case = verified_case();
        let policy = AlertPolicy::new(
            safe_cameroon_domain::AlertPolicyId::new("INTERNAL_TEST"),
            1,
            IncidentType::MissingChild,
            AlertVisibility::Internal,
            safe_cameroon_domain::CaseEventType::CaseVerified,
            vec![AlertField::IncidentCategory],
        )
        .unwrap();
        let geography = TargetGeography::new("Douala").unwrap();
        let creation = create_alert_from_case(
            &case,
            &policy,
            Severity::High,
            geography,
            safe_fields(),
            Actor::Automated,
            Uuid::new_v4(),
            None,
        )
        .unwrap();
        let mut alert = creation.alert;

        let error = cancel_alert(&mut alert, Actor::Automated, Uuid::new_v4()).unwrap_err();
        assert_eq!(
            error,
            AlertCancellationError::NotAuthorized(AlertAuthorizationError {
                visibility: AlertVisibility::Internal
            })
        );
    }

    #[test]
    fn reviewer_can_cancel_an_alert_but_not_twice() {
        let case = verified_case();
        let policy = AlertPolicy::missing_child_community_v1();
        let geography = TargetGeography::new("Douala").unwrap();
        let creation = create_alert_from_case(
            &case,
            &policy,
            Severity::High,
            geography,
            safe_fields(),
            Actor::Reviewer(Uuid::new_v4()),
            Uuid::new_v4(),
            None,
        )
        .unwrap();
        let mut alert = creation.alert;

        cancel_alert(&mut alert, Actor::Reviewer(Uuid::new_v4()), Uuid::new_v4()).unwrap();
        let error =
            cancel_alert(&mut alert, Actor::Reviewer(Uuid::new_v4()), Uuid::new_v4()).unwrap_err();
        assert!(matches!(
            error,
            AlertCancellationError::InvalidTransition(_)
        ));
    }
}
