use std::sync::Arc;

use safe_cameroon_application::webhook::{WebhookReplayGuard, WebhookVerifierRegistry};
use safe_cameroon_infrastructure::postgres::{
    PostgresAlertRepository, PostgresCaseRepository, PostgresDeliveryRepository,
    PostgresReportRepository,
};

#[derive(Clone)]
pub struct AppState {
    pub reports: PostgresReportRepository,
    pub cases: PostgresCaseRepository,
    pub alerts: PostgresAlertRepository,
    pub deliveries: PostgresDeliveryRepository,
    pub webhook_verifiers: Arc<WebhookVerifierRegistry>,
    pub webhook_replay_guard: Arc<dyn WebhookReplayGuard>,
}
