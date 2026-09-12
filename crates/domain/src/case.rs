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
}
