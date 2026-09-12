use core::fmt;

use serde::{Deserialize, Serialize};

use crate::{CaseId, ReportId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IncidentType {
    MissingChild,
    OtherProtectionIncident,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CaseStatus {
    Reported,
    UnderReview,
    Verified,
    Active,
    Resolved,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CaseEventType {
    CaseCreated,
    CaseReportLinked,
    CaseUnderReview,
    CaseVerified,
    CaseActivated,
    CaseResolved,
    CaseCancelled,
    CaseRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    id: CaseId,
    incident_type: IncidentType,
    status: CaseStatus,
    report_ids: Vec<ReportId>,
    version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseEvent {
    pub case_id: CaseId,
    pub event_type: CaseEventType,
    pub aggregate_version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionError {
    pub from: CaseStatus,
    pub to: CaseStatus,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "case cannot transition from {:?} to {:?}",
            self.from, self.to
        )
    }
}

impl std::error::Error for TransitionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateReportLink {
    pub case_id: CaseId,
    pub report_id: ReportId,
}

impl fmt::Display for DuplicateReportLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "report {:?} is already linked to case {:?}",
            self.report_id, self.case_id
        )
    }
}

impl std::error::Error for DuplicateReportLink {}

impl Case {
    pub fn create(incident_type: IncidentType, first_report: ReportId) -> (Self, CaseEvent) {
        let case_ = Self {
            id: CaseId::new(),
            incident_type,
            status: CaseStatus::Reported,
            report_ids: vec![first_report],
            version: 1,
        };
        let event = case_.event(CaseEventType::CaseCreated);
        (case_, event)
    }

    /// Rebuilds a case aggregate from persisted state. Infrastructure adapters use
    /// this to reload a case before applying a use case; it performs no validation
    /// because the stored state is assumed to already satisfy domain invariants.
    pub fn reconstitute(
        id: CaseId,
        incident_type: IncidentType,
        status: CaseStatus,
        report_ids: Vec<ReportId>,
        version: u64,
    ) -> Self {
        Self {
            id,
            incident_type,
            status,
            report_ids,
            version,
        }
    }

    /// Links an additional report as further evidence for this case. Linking does
    /// not by itself change verification status; a reviewer still decides whether
    /// the additional report changes the case outcome.
    pub fn link_report(&mut self, report_id: ReportId) -> Result<CaseEvent, DuplicateReportLink> {
        if self.report_ids.contains(&report_id) {
            return Err(DuplicateReportLink {
                case_id: self.id,
                report_id,
            });
        }
        self.report_ids.push(report_id);
        self.version += 1;
        Ok(self.event(CaseEventType::CaseReportLinked))
    }

    pub fn id(&self) -> CaseId {
        self.id
    }
    pub fn incident_type(&self) -> IncidentType {
        self.incident_type
    }
    pub fn status(&self) -> CaseStatus {
        self.status
    }
    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn report_ids(&self) -> &[ReportId] {
        &self.report_ids
    }

    pub fn transition_to(&mut self, next: CaseStatus) -> Result<CaseEvent, TransitionError> {
        if !is_allowed_transition(self.status, next) {
            return Err(TransitionError {
                from: self.status,
                to: next,
            });
        }

        self.status = next;
        self.version += 1;
        Ok(self.event(event_for(next)))
    }

    fn event(&self, event_type: CaseEventType) -> CaseEvent {
        CaseEvent {
            case_id: self.id,
            event_type,
            aggregate_version: self.version,
        }
    }
}

fn is_allowed_transition(from: CaseStatus, to: CaseStatus) -> bool {
    matches!(
        (from, to),
        (CaseStatus::Reported, CaseStatus::UnderReview)
            | (CaseStatus::UnderReview, CaseStatus::Verified)
            | (CaseStatus::UnderReview, CaseStatus::Rejected)
            | (CaseStatus::Verified, CaseStatus::Active)
            | (CaseStatus::Verified, CaseStatus::Cancelled)
            | (CaseStatus::Active, CaseStatus::Resolved)
            | (CaseStatus::Active, CaseStatus::Cancelled)
    )
}

fn event_for(status: CaseStatus) -> CaseEventType {
    match status {
        CaseStatus::UnderReview => CaseEventType::CaseUnderReview,
        CaseStatus::Verified => CaseEventType::CaseVerified,
        CaseStatus::Active => CaseEventType::CaseActivated,
        CaseStatus::Resolved => CaseEventType::CaseResolved,
        CaseStatus::Cancelled => CaseEventType::CaseCancelled,
        CaseStatus::Rejected => CaseEventType::CaseRejected,
        CaseStatus::Reported => unreachable!("a case is created in REPORTED state"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_the_review_to_resolution_lifecycle() {
        let (mut case_, _) = Case::create(IncidentType::MissingChild, ReportId::new());
        assert_eq!(
            case_
                .transition_to(CaseStatus::UnderReview)
                .unwrap()
                .aggregate_version,
            2
        );
        assert_eq!(
            case_
                .transition_to(CaseStatus::Verified)
                .unwrap()
                .aggregate_version,
            3
        );
        assert_eq!(
            case_
                .transition_to(CaseStatus::Active)
                .unwrap()
                .aggregate_version,
            4
        );
        assert_eq!(
            case_
                .transition_to(CaseStatus::Resolved)
                .unwrap()
                .event_type,
            CaseEventType::CaseResolved
        );
    }

    #[test]
    fn cannot_publish_a_reported_case() {
        let (mut case_, _) = Case::create(IncidentType::MissingChild, ReportId::new());
        let error = case_.transition_to(CaseStatus::Active).unwrap_err();
        assert_eq!(error.from, CaseStatus::Reported);
        assert_eq!(case_.status(), CaseStatus::Reported);
    }

    #[test]
    fn terminal_cases_cannot_transition() {
        let (mut case_, _) = Case::create(IncidentType::MissingChild, ReportId::new());
        case_.transition_to(CaseStatus::UnderReview).unwrap();
        case_.transition_to(CaseStatus::Rejected).unwrap();
        assert!(case_.transition_to(CaseStatus::Verified).is_err());
    }

    #[test]
    fn links_a_new_report_and_rejects_a_duplicate() {
        let first_report = ReportId::new();
        let (mut case_, _) = Case::create(IncidentType::MissingChild, first_report);
        let second_report = ReportId::new();

        let event = case_.link_report(second_report).unwrap();
        assert_eq!(event.event_type, CaseEventType::CaseReportLinked);
        assert_eq!(event.aggregate_version, 2);
        assert_eq!(case_.report_ids(), [first_report, second_report]);

        let error = case_.link_report(second_report).unwrap_err();
        assert_eq!(error.case_id, case_.id());
        assert_eq!(error.report_id, second_report);
        assert_eq!(
            case_.report_ids().len(),
            2,
            "duplicate link must not be applied"
        );
    }

    #[test]
    fn reconstitutes_a_case_from_persisted_state() {
        let case_id = CaseId::new();
        let report_id = ReportId::new();
        let case_ = Case::reconstitute(
            case_id,
            IncidentType::MissingChild,
            CaseStatus::Verified,
            vec![report_id],
            3,
        );

        assert_eq!(case_.id(), case_id);
        assert_eq!(case_.status(), CaseStatus::Verified);
        assert_eq!(case_.version(), 3);
        assert_eq!(case_.report_ids(), [report_id]);
    }

    const ALL_STATUSES: [CaseStatus; 7] = [
        CaseStatus::Reported,
        CaseStatus::UnderReview,
        CaseStatus::Verified,
        CaseStatus::Active,
        CaseStatus::Resolved,
        CaseStatus::Cancelled,
        CaseStatus::Rejected,
    ];

    /// Mirrors `docs/ALERT_SAFETY.md`'s verification state machine. Kept as an
    /// independent table (rather than calling `is_allowed_transition`) so this
    /// test fails if the implementation's table silently drifts from the spec.
    fn is_spec_allowed_transition(from: CaseStatus, to: CaseStatus) -> bool {
        matches!(
            (from, to),
            (CaseStatus::Reported, CaseStatus::UnderReview)
                | (CaseStatus::UnderReview, CaseStatus::Verified)
                | (CaseStatus::UnderReview, CaseStatus::Rejected)
                | (CaseStatus::Verified, CaseStatus::Active)
                | (CaseStatus::Verified, CaseStatus::Cancelled)
                | (CaseStatus::Active, CaseStatus::Resolved)
                | (CaseStatus::Active, CaseStatus::Cancelled)
        )
    }

    #[test]
    fn decision_table_covers_every_status_pair() {
        for from in ALL_STATUSES {
            for to in ALL_STATUSES {
                let expected = is_spec_allowed_transition(from, to);
                let case_ = Case::reconstitute(
                    CaseId::new(),
                    IncidentType::MissingChild,
                    from,
                    vec![ReportId::new()],
                    1,
                );
                let mut case_ = case_;
                let outcome = case_.transition_to(to);

                assert_eq!(
                    outcome.is_ok(),
                    expected,
                    "transition {from:?} -> {to:?} expected allowed={expected}"
                );
                if expected {
                    assert_eq!(case_.status(), to);
                    assert_eq!(case_.version(), 2);
                } else {
                    assert_eq!(
                        case_.status(),
                        from,
                        "rejected transition must not mutate state"
                    );
                    assert_eq!(case_.version(), 1);
                }
            }
        }
    }

    #[test]
    fn terminal_statuses_have_no_outgoing_transitions() {
        for terminal in [
            CaseStatus::Resolved,
            CaseStatus::Cancelled,
            CaseStatus::Rejected,
        ] {
            for to in ALL_STATUSES {
                assert!(
                    !is_spec_allowed_transition(terminal, to),
                    "{terminal:?} must not transition to {to:?}"
                );
            }
        }
    }
}
