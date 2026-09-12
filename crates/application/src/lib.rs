//! Application use cases. Infrastructure implements ports introduced here.

use core::fmt;

use safe_cameroon_domain::{
    AnonymousReport, AuditEventId, Case, CaseEvent, CaseStatus, OutboxEventId, ReportId,
    ReportSourceChannel, TransitionError,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// A state transition is intentionally a use case, not an incidental data update.
pub fn transition_case(case_: &mut Case, next: CaseStatus) -> Result<CaseEvent, TransitionError> {
    case_.transition_to(next)
}

const MAX_REPORT_CONTENT_CHARS: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousReportSubmission {
    pub report: AnonymousReport,
    pub reference_code: String,
    pub reference_code_hash: Vec<u8>,
    pub idempotency_key_hash: Option<Vec<u8>>,
    pub audit_event_id: AuditEventId,
    pub outbox_event_id: OutboxEventId,
    pub request_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportValidationError {
    EmptyContent,
    ContentTooLong,
}

impl fmt::Display for ReportValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyContent => write!(f, "report content cannot be blank"),
            Self::ContentTooLong => write!(f, "report content exceeds the allowed length"),
        }
    }
}

impl std::error::Error for ReportValidationError {}

/// Builds a submission without persisting it. The reference code is an opaque,
/// UUID-v4 capability; only its SHA-256 digest is passed to persistence.
pub fn prepare_anonymous_report(
    raw_content: String,
    request_id: Uuid,
    idempotency_key: Option<&str>,
) -> Result<AnonymousReportSubmission, ReportValidationError> {
    let normalized_content = raw_content.trim().to_owned();
    if normalized_content.is_empty() {
        return Err(ReportValidationError::EmptyContent);
    }
    if normalized_content.chars().count() > MAX_REPORT_CONTENT_CHARS {
        return Err(ReportValidationError::ContentTooLong);
    }

    let token = Uuid::new_v4().to_string();
    let reference_code = format!("SC-{token}");
    let reference_code_hash = Sha256::digest(reference_code.as_bytes()).to_vec();
    let idempotency_key_hash = idempotency_key.map(|key| Sha256::digest(key.as_bytes()).to_vec());

    Ok(AnonymousReportSubmission {
        report: AnonymousReport {
            id: ReportId::new(),
            source_channel: ReportSourceChannel::Web,
            raw_content: normalized_content,
        },
        reference_code,
        reference_code_hash,
        idempotency_key_hash,
        audit_event_id: AuditEventId::new(),
        outbox_event_id: OutboxEventId::new(),
        request_id,
    })
}

pub fn report_submitted_event_payload(submission: &AnonymousReportSubmission) -> serde_json::Value {
    json!({
        "report_id": submission.report.id,
        "source_channel": submission.report.source_channel,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_an_anonymous_report_without_exposing_the_follow_up_secret() {
        let submission = prepare_anonymous_report(
            "  A child is missing.  ".into(),
            Uuid::new_v4(),
            Some("request-1"),
        )
        .unwrap();

        assert_eq!(submission.report.raw_content, "A child is missing.");
        assert!(submission.reference_code.starts_with("SC-"));
        assert_ne!(
            submission.reference_code.as_bytes(),
            submission.reference_code_hash.as_slice()
        );
        assert_eq!(submission.reference_code_hash.len(), 32);
    }

    #[test]
    fn rejects_blank_or_excessively_large_reports() {
        assert_eq!(
            prepare_anonymous_report(" \n ".into(), Uuid::new_v4(), None).unwrap_err(),
            ReportValidationError::EmptyContent
        );
        assert_eq!(
            prepare_anonymous_report(
                "a".repeat(MAX_REPORT_CONTENT_CHARS + 1),
                Uuid::new_v4(),
                None,
            )
            .unwrap_err(),
            ReportValidationError::ContentTooLong
        );
    }
}
