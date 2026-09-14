//! Business concepts and rules for SafeCameroon.
//!
//! This crate deliberately has no HTTP, database, storage, or provider dependencies.

pub mod alert;
pub mod attachment;
pub mod case;
pub mod delivery;
pub mod ids;
pub mod report;
pub mod subscription;

pub use alert::{
    Alert, AlertCreationError, AlertEvent, AlertEventType, AlertField, AlertFieldValue,
    AlertPolicy, AlertPolicyError, AlertPolicyId, AlertStatus, AlertTransitionError,
    AlertVisibility, EmptyTargetGeography, Severity, TargetGeography,
};
pub use attachment::{
    Attachment, AttachmentContentType, AttachmentError, MAX_ATTACHMENT_SIZE_BYTES, StorageProvider,
};
pub use case::{
    Case, CaseEvent, CaseEventType, CaseStatus, DuplicateReportLink, IncidentType, TransitionError,
};
pub use delivery::{
    ChannelEndpoint, ChannelType, Delivery, DeliveryAttempt, DeliveryAttemptOutcome, DeliveryEvent,
    DeliveryEventType, DeliveryIdempotencyKey, DeliveryPreference, DeliveryPreferenceError,
    DeliveryStatus, DeliveryStrategy, DeliveryTransitionError, EmptyChannelEndpointAddress,
    InvalidRetryPolicy, RetryPolicy, plan_deliveries,
};
pub use ids::{
    AlertEventId, AlertId, AttachmentId, AuditEventId, CaseEventId, CaseId, ConsumerId,
    DeliveryAttemptId, DeliveryEventId, DeliveryId, OutboxEventId, ReportId, SubscriptionId,
};
pub use report::{AnonymousReport, ReportSourceChannel};
pub use subscription::{
    Comparison, ConsumerMatch, EmptyGeoArea, EmptySubscriptionRules, GeoArea, MatchDecision,
    MatchReason, MatchedSubscription, Subscription, SubscriptionRule, deduplicate_by_consumer,
    evaluate_subscription, evaluate_subscriptions,
};
