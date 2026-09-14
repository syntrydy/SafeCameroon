use std::sync::Arc;

use safe_cameroon_application::attachment_workflow::AttachmentStorage;
use safe_cameroon_application::rate_limit::RateLimiter;
use safe_cameroon_application::webhook::{WebhookReplayGuard, WebhookVerifierRegistry};
use safe_cameroon_infrastructure::auth::ReviewerSessionTokenIssuer;
use safe_cameroon_infrastructure::postgres::{
    PostgresAlertRepository, PostgresAttachmentRepository, PostgresAuditEventRepository,
    PostgresCaseRepository, PostgresDeliveryPreferenceRepository, PostgresDeliveryRepository,
    PostgresReportRepository, PostgresReviewerRepository, PostgresSubscriptionRepository,
};

#[derive(Clone)]
pub struct AppState {
    pub reports: PostgresReportRepository,
    pub cases: PostgresCaseRepository,
    pub alerts: PostgresAlertRepository,
    pub deliveries: PostgresDeliveryRepository,
    pub attachments: PostgresAttachmentRepository,
    pub audit_events: PostgresAuditEventRepository,
    pub subscriptions: PostgresSubscriptionRepository,
    pub delivery_preferences: PostgresDeliveryPreferenceRepository,
    pub reviewers: PostgresReviewerRepository,
    pub reviewer_session_tokens: ReviewerSessionTokenIssuer,
    pub rate_limiter: Arc<dyn RateLimiter>,
    pub attachment_storage: Arc<dyn AttachmentStorage>,
    pub webhook_verifiers: Arc<WebhookVerifierRegistry>,
    pub webhook_replay_guard: Arc<dyn WebhookReplayGuard>,
}
