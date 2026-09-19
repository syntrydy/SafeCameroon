use std::sync::Arc;

use safe_cameroon_application::ai_extraction::ReportExtractor;
use safe_cameroon_application::attachment_workflow::AttachmentStorage;
use safe_cameroon_application::audio_transcription::AudioTranscriber;
use safe_cameroon_application::google_identity::GoogleIdentityVerifier;
use safe_cameroon_application::invite_mailer::InviteMailer;
use safe_cameroon_application::rate_limit::RateLimiter;
use safe_cameroon_application::webhook::{WebhookReplayGuard, WebhookVerifierRegistry};
use safe_cameroon_infrastructure::auth::ReviewerSessionTokenIssuer;
use safe_cameroon_infrastructure::postgres::{
    PostgresAlertRepository, PostgresAttachmentRepository, PostgresAuditEventRepository,
    PostgresCaseRepository, PostgresConsumerRepository, PostgresDeliveryPreferenceRepository,
    PostgresDeliveryRepository, PostgresOrganizationRepository, PostgresReportExtractionRepository,
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
    pub consumers: PostgresConsumerRepository,
    pub subscriptions: PostgresSubscriptionRepository,
    pub delivery_preferences: PostgresDeliveryPreferenceRepository,
    pub organizations: PostgresOrganizationRepository,
    pub reviewers: PostgresReviewerRepository,
    pub reviewer_session_tokens: ReviewerSessionTokenIssuer,
    pub google_identity_verifier: Arc<dyn GoogleIdentityVerifier>,
    pub rate_limiter: Arc<dyn RateLimiter>,
    pub attachment_storage: Arc<dyn AttachmentStorage>,
    pub webhook_verifiers: Arc<WebhookVerifierRegistry>,
    pub webhook_replay_guard: Arc<dyn WebhookReplayGuard>,
    pub extractions: PostgresReportExtractionRepository,
    pub report_extractor: Arc<dyn ReportExtractor>,
    pub audio_transcriber: Arc<dyn AudioTranscriber>,
    /// Operator kill-switch (`VOICE_REPORTS_ENABLED`, on by default) --
    /// distinct from whether a real transcriber is configured (issue #160).
    pub voice_reports_enabled: bool,
    pub invite_mailer: Arc<dyn InviteMailer>,
}
