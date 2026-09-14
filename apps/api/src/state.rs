use std::sync::Arc;

use safe_cameroon_application::attachment_workflow::AttachmentStorage;
use safe_cameroon_application::webhook::{WebhookReplayGuard, WebhookVerifierRegistry};
use safe_cameroon_infrastructure::postgres::{
    PostgresAlertRepository, PostgresAttachmentRepository, PostgresCaseRepository,
    PostgresDeliveryPreferenceRepository, PostgresDeliveryRepository, PostgresReportRepository,
    PostgresSubscriptionRepository,
};

#[derive(Clone)]
pub struct AppState {
    pub reports: PostgresReportRepository,
    pub cases: PostgresCaseRepository,
    pub alerts: PostgresAlertRepository,
    pub deliveries: PostgresDeliveryRepository,
    pub attachments: PostgresAttachmentRepository,
    pub subscriptions: PostgresSubscriptionRepository,
    pub delivery_preferences: PostgresDeliveryPreferenceRepository,
    pub attachment_storage: Arc<dyn AttachmentStorage>,
    pub webhook_verifiers: Arc<WebhookVerifierRegistry>,
    pub webhook_replay_guard: Arc<dyn WebhookReplayGuard>,
}
