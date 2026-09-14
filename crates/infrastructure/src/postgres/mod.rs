//! PostgreSQL adapters, one module per aggregate.

pub mod alerts;
pub mod attachments;
pub mod cases;
pub mod deliveries;
pub mod delivery_preferences;
pub mod outbox;
pub mod reports;
pub mod subscriptions;

pub use alerts::{AlertCancelOutcome, PostgresAlertRepository};
pub use attachments::PostgresAttachmentRepository;
pub use cases::{CaseCreationOutcome, CaseLinkOutcome, CaseReviewOutcome, PostgresCaseRepository};
pub use deliveries::{DeliveryTransitionOutcome, PostgresDeliveryRepository};
pub use delivery_preferences::PostgresDeliveryPreferenceRepository;
pub use outbox::{ClaimedOutboxEvent, PostgresOutboxRepository};
pub use reports::{PostgresReportRepository, SubmissionResult};
pub use subscriptions::PostgresSubscriptionRepository;
