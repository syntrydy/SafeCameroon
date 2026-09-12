//! PostgreSQL adapters, one module per aggregate.

pub mod cases;
pub mod reports;

pub use cases::{CaseCreationOutcome, CaseLinkOutcome, CaseReviewOutcome, PostgresCaseRepository};
pub use reports::{PostgresReportRepository, SubmissionResult};
