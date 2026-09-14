//! PostgreSQL adapters, one module per aggregate.

pub mod alerts;
pub mod attachments;
pub mod cases;
pub mod deliveries;
pub mod delivery_preferences;
pub mod outbox;
pub mod rate_limits;
pub mod reports;
pub mod reviewers;
pub mod subscriptions;

pub use alerts::{AlertCancelOutcome, PostgresAlertRepository};
pub use attachments::PostgresAttachmentRepository;
pub use cases::{CaseCreationOutcome, CaseLinkOutcome, CaseReviewOutcome, PostgresCaseRepository};
pub use deliveries::{DeliveryTransitionOutcome, PostgresDeliveryRepository};
pub use delivery_preferences::PostgresDeliveryPreferenceRepository;
pub use outbox::{ClaimedOutboxEvent, PostgresOutboxRepository};
pub use rate_limits::PostgresRateLimiter;
pub use reports::{PostgresReportRepository, SubmissionResult};
pub use reviewers::{CreateReviewerOutcome, PostgresReviewerRepository, ReviewerRecord};
pub use subscriptions::{PostgresSubscriptionRepository, SubscriptionUpdateOutcome};
