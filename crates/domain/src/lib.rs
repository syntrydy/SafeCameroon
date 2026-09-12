//! Business concepts and rules for SafeCameroon.
//!
//! This crate deliberately has no HTTP, database, storage, or provider dependencies.

pub mod alert;
pub mod case;
pub mod ids;
pub mod report;

pub use alert::{
    Alert, AlertCreationError, AlertEvent, AlertEventType, AlertField, AlertFieldValue,
    AlertPolicy, AlertPolicyError, AlertPolicyId, AlertStatus, AlertTransitionError,
    AlertVisibility, EmptyTargetGeography, TargetGeography,
};
pub use case::{
    Case, CaseEvent, CaseEventType, CaseStatus, DuplicateReportLink, IncidentType, TransitionError,
};
pub use ids::{AlertEventId, AlertId, AuditEventId, CaseEventId, CaseId, OutboxEventId, ReportId};
pub use report::{AnonymousReport, ReportSourceChannel};
