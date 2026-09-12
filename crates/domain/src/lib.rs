//! Business concepts and rules for SafeCameroon.
//!
//! This crate deliberately has no HTTP, database, storage, or provider dependencies.

pub mod case;
pub mod ids;
pub mod report;

pub use case::{
    Case, CaseEvent, CaseEventType, CaseStatus, DuplicateReportLink, IncidentType, TransitionError,
};
pub use ids::{AuditEventId, CaseEventId, CaseId, OutboxEventId, ReportId};
pub use report::{AnonymousReport, ReportSourceChannel};
