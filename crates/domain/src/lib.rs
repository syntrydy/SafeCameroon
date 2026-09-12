//! Business concepts and rules for SafeCameroon.
//!
//! This crate deliberately has no HTTP, database, storage, or provider dependencies.

pub mod case;
pub mod ids;

pub use case::{Case, CaseEvent, CaseEventType, CaseStatus, IncidentType, TransitionError};
pub use ids::{CaseId, ReportId};
