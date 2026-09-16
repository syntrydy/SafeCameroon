//! PostgreSQL adapters, one module per aggregate.

pub mod alerts;
pub mod attachments;
pub mod audit_events;
pub mod cases;
pub mod consumers;
pub mod deliveries;
pub mod delivery_preferences;
pub mod organizations;
pub mod outbox;
pub mod rate_limits;
pub mod reports;
pub mod reviewers;
pub mod subscriptions;

pub use alerts::{AlertCancelOutcome, AlertCreationOutcome, AlertFilter, PostgresAlertRepository};
pub use attachments::PostgresAttachmentRepository;
pub use audit_events::{AuditEventFilter, AuditEventRecord, PostgresAuditEventRepository};
pub use cases::{
    CaseCreationOutcome, CaseEventRecord, CaseFilter, CaseLinkOutcome, CaseReviewOutcome,
    PostgresCaseRepository,
};
pub use consumers::PostgresConsumerRepository;
pub use deliveries::{DeliveryTransitionOutcome, PostgresDeliveryRepository};
pub use delivery_preferences::PostgresDeliveryPreferenceRepository;
pub use organizations::{MembershipRecord, PostgresOrganizationRepository};
pub use outbox::{ClaimedOutboxEvent, PostgresOutboxRepository};
pub use rate_limits::PostgresRateLimiter;
pub use reports::{PostgresReportRepository, ReportSummary, SubmissionResult};
pub use reviewers::{CreateReviewerOutcome, PostgresReviewerRepository};
pub use subscriptions::{PostgresSubscriptionRepository, SubscriptionUpdateOutcome};
