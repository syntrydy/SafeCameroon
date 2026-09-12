//! PostgreSQL adapters, one module per aggregate.

pub mod alerts;
pub mod cases;
pub mod reports;

pub use alerts::{AlertCancelOutcome, PostgresAlertRepository};
pub use cases::{CaseCreationOutcome, CaseLinkOutcome, CaseReviewOutcome, PostgresCaseRepository};
pub use reports::{PostgresReportRepository, SubmissionResult};
