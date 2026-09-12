//! Case workflow use cases: report-to-case creation, linking further reports,
//! and reviewer-authorized status transitions.

use core::fmt;

use safe_cameroon_domain::{
    AuditEventId, Case, CaseEvent, CaseId, CaseStatus, DuplicateReportLink, IncidentType,
    OutboxEventId, ReportId, TransitionError,
};
use serde_json::json;
use uuid::Uuid;

/// Who is performing a case-workflow action. AI/automated triage is represented
/// distinctly from a reviewer so authorization can refuse it independently of
/// the domain's state machine (AGENTS.md: "AI may not ... bypass authorization").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    Reviewer(Uuid),
    Automated,
}

impl Actor {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::Reviewer(_) => "REVIEWER",
            Self::Automated => "AUTOMATED",
        }
    }

    pub fn actor_id(self) -> Option<Uuid> {
        match self {
            Self::Reviewer(id) => Some(id),
            Self::Automated => None,
        }
    }
}

/// A non-reviewer actor attempted a transition that only a reviewer may make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizationError {
    pub attempted_status: CaseStatus,
}

impl fmt::Display for AuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "only a reviewer may transition a case to {:?}",
            self.attempted_status
        )
    }
}

impl std::error::Error for AuthorizationError {}

/// Only starting review may happen without an identified reviewer (for example,
/// automated triage queuing a freshly reported case). Every other transition
/// changes the case's verification state and requires a reviewer.
fn authorize_case_transition(actor: Actor, to: CaseStatus) -> Result<(), AuthorizationError> {
    match (actor, to) {
        (_, CaseStatus::UnderReview) => Ok(()),
        (Actor::Reviewer(_), _) => Ok(()),
        (Actor::Automated, attempted_status) => Err(AuthorizationError { attempted_status }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseReviewError {
    NotAuthorized(AuthorizationError),
    InvalidTransition(TransitionError),
}

impl fmt::Display for CaseReviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAuthorized(error) => error.fmt(f),
            Self::InvalidTransition(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CaseReviewError {}

/// Persists as a `cases` row plus the first `case_reports` link, in the same
/// transaction as its `CASE_CREATED` event, audit event, and outbox event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseCreation {
    pub case: Case,
    pub event: CaseEvent,
    pub actor: Actor,
    pub audit_event_id: AuditEventId,
    pub outbox_event_id: OutboxEventId,
    pub request_id: Uuid,
}

pub fn create_case_from_report(
    incident_type: IncidentType,
    report_id: ReportId,
    actor: Actor,
    request_id: Uuid,
) -> CaseCreation {
    let (case, event) = Case::create(incident_type, report_id);
    CaseCreation {
        case,
        event,
        actor,
        audit_event_id: AuditEventId::new(),
        outbox_event_id: OutboxEventId::new(),
        request_id,
    }
}

pub fn case_created_event_payload(creation: &CaseCreation) -> serde_json::Value {
    json!({
        "case_id": creation.case.id(),
        "incident_type": creation.case.incident_type(),
        "report_id": creation.case.report_ids().first(),
    })
}

/// Links an additional report as further evidence for an existing case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseReportLink {
    pub event: CaseEvent,
    pub report_id: ReportId,
    pub actor: Actor,
    pub audit_event_id: AuditEventId,
    pub outbox_event_id: OutboxEventId,
    pub request_id: Uuid,
}

pub fn link_report_to_case(
    case: &mut Case,
    report_id: ReportId,
    actor: Actor,
    request_id: Uuid,
) -> Result<CaseReportLink, DuplicateReportLink> {
    let event = case.link_report(report_id)?;
    Ok(CaseReportLink {
        event,
        report_id,
        actor,
        audit_event_id: AuditEventId::new(),
        outbox_event_id: OutboxEventId::new(),
        request_id,
    })
}

pub fn case_report_linked_event_payload(
    case_id: CaseId,
    link: &CaseReportLink,
) -> serde_json::Value {
    json!({
        "case_id": case_id,
        "report_id": link.report_id,
    })
}

/// A reviewer-authorized (or, for `UNDER_REVIEW` only, automated) status change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseReview {
    pub event: CaseEvent,
    pub actor: Actor,
    pub audit_event_id: AuditEventId,
    pub outbox_event_id: OutboxEventId,
    pub request_id: Uuid,
}

/// AI-suggested classifications may inform a reviewer but can never themselves
/// supply `actor`; callers construct `Actor` from an authenticated reviewer
/// identity, never from AI/model output (AGENTS.md AI rules).
pub fn review_case(
    case: &mut Case,
    actor: Actor,
    to: CaseStatus,
    request_id: Uuid,
) -> Result<CaseReview, CaseReviewError> {
    authorize_case_transition(actor, to).map_err(CaseReviewError::NotAuthorized)?;
    let event = case
        .transition_to(to)
        .map_err(CaseReviewError::InvalidTransition)?;
    Ok(CaseReview {
        event,
        actor,
        audit_event_id: AuditEventId::new(),
        outbox_event_id: OutboxEventId::new(),
        request_id,
    })
}

pub fn case_status_changed_event_payload(case: &Case, review: &CaseReview) -> serde_json::Value {
    json!({
        "case_id": case.id(),
        "status": case.status(),
        "aggregate_version": review.event.aggregate_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use safe_cameroon_domain::ReportId;

    #[test]
    fn creates_a_case_from_a_report() {
        let report_id = ReportId::new();
        let creation = create_case_from_report(
            IncidentType::MissingChild,
            report_id,
            Actor::Reviewer(Uuid::new_v4()),
            Uuid::new_v4(),
        );

        assert_eq!(creation.case.report_ids(), [report_id]);
        assert_eq!(creation.case.status(), CaseStatus::Reported);
        let payload = case_created_event_payload(&creation);
        assert_eq!(payload["report_id"], json!(report_id));
    }

    #[test]
    fn links_a_second_report_to_a_case() {
        let (mut case, _) = Case::create(IncidentType::MissingChild, ReportId::new());
        let second_report = ReportId::new();

        let link = link_report_to_case(
            &mut case,
            second_report,
            Actor::Reviewer(Uuid::new_v4()),
            Uuid::new_v4(),
        )
        .unwrap();

        assert_eq!(link.report_id, second_report);
        assert_eq!(case.report_ids(), [case.report_ids()[0], second_report]);
    }

    #[test]
    fn automated_actor_can_start_review_but_not_verify() {
        let (mut case, _) = Case::create(IncidentType::MissingChild, ReportId::new());
        let request_id = Uuid::new_v4();

        let started = review_case(
            &mut case,
            Actor::Automated,
            CaseStatus::UnderReview,
            request_id,
        );
        assert!(started.is_ok());

        let error = review_case(
            &mut case,
            Actor::Automated,
            CaseStatus::Verified,
            request_id,
        )
        .unwrap_err();
        assert_eq!(
            error,
            CaseReviewError::NotAuthorized(AuthorizationError {
                attempted_status: CaseStatus::Verified
            })
        );
        assert_eq!(case.status(), CaseStatus::UnderReview);
    }

    #[test]
    fn reviewer_can_verify_a_case_under_review() {
        let (mut case, _) = Case::create(IncidentType::MissingChild, ReportId::new());
        let request_id = Uuid::new_v4();
        review_case(
            &mut case,
            Actor::Automated,
            CaseStatus::UnderReview,
            request_id,
        )
        .unwrap();

        let review = review_case(
            &mut case,
            Actor::Reviewer(Uuid::new_v4()),
            CaseStatus::Verified,
            request_id,
        )
        .unwrap();

        assert_eq!(case.status(), CaseStatus::Verified);
        let payload = case_status_changed_event_payload(&case, &review);
        assert_eq!(payload["status"], json!(CaseStatus::Verified));
    }

    #[test]
    fn invalid_transition_is_reported_even_for_a_reviewer() {
        let (mut case, _) = Case::create(IncidentType::MissingChild, ReportId::new());
        let error = review_case(
            &mut case,
            Actor::Reviewer(Uuid::new_v4()),
            CaseStatus::Active,
            Uuid::new_v4(),
        )
        .unwrap_err();

        assert!(matches!(error, CaseReviewError::InvalidTransition(_)));
    }
}
