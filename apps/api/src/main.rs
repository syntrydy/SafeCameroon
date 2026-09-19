mod alerts;
mod attachments;
mod audit_events;
mod auth;
mod cases;
mod citizen_subscriptions;
mod consumers;
mod deliveries;
mod error;
mod extractions;
mod health;
mod idempotency;
mod organizations;
mod rate_limit;
mod reports;
mod request_id;
mod reviewer;
mod source_key;
mod state;
mod subscriptions;
mod webhooks;

use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, HeaderValue, Method};
use axum::{
    Router,
    routing::{get, post, put},
};
use safe_cameroon_application::ai_extraction::ReportExtractor;
use safe_cameroon_application::attachment_workflow::AttachmentStorage;
use safe_cameroon_application::audio_transcription::AudioTranscriber;
use safe_cameroon_application::google_identity::GoogleIdentityVerifier;
use safe_cameroon_application::invite_mailer::InviteMailer;
use safe_cameroon_application::webhook::WebhookVerifierRegistry;
use safe_cameroon_domain::ChannelType;
use safe_cameroon_infrastructure::ai::{
    DisabledExtractor, DisabledTranscriber, OpenRouterExtractor, OpenRouterTranscriber,
};
use safe_cameroon_infrastructure::auth::ReviewerSessionTokenIssuer;
use safe_cameroon_infrastructure::google_identity::GoogleTokenInfoVerifier;
use safe_cameroon_infrastructure::invite_mailer::{DisabledInviteMailer, ResendInviteMailer};
use safe_cameroon_infrastructure::postgres::{
    PostgresAlertRepository, PostgresAttachmentRepository, PostgresAuditEventRepository,
    PostgresCaseRepository, PostgresConsumerRepository, PostgresDeliveryPreferenceRepository,
    PostgresDeliveryRepository, PostgresOrganizationRepository, PostgresRateLimiter,
    PostgresReportExtractionRepository, PostgresReportRepository, PostgresReviewerRepository,
    PostgresSubscriptionRepository,
};
use safe_cameroon_infrastructure::storage::{HmacSignedAttachmentStorage, R2AttachmentStorage};
use safe_cameroon_infrastructure::webhook::{
    HmacSignedWebhookVerifier, PostgresWebhookReplayGuard,
};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::state::AppState;

/// The only webhook provider registered per channel today (every mock/
/// sandbox channel emits the same payload shape); a real provider
/// integration adds its own name here rather than replacing this one.
const SANDBOX_WEBHOOK_PROVIDER: &str = "sandbox";

/// docs/OBSERVABILITY.md section 2: "every request/job/event should carry a
/// correlation/request ID." `SetRequestIdLayer` generates one when the
/// caller didn't send it; [`request_id::request_id_from_headers`] is what
/// every handler reads it back through.
pub(crate) const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Real R2 when all four `R2_*` vars are configured; otherwise the mock/
/// sandbox adapter (docs/DEPLOYMENT.md: R2 is the intended real provider —
/// this is what makes it actually real once credentials exist, while local
/// dev and the test suite, which never set `R2_*`, keep working unchanged
/// against the mock).
fn attachment_storage_from_env() -> Arc<dyn AttachmentStorage> {
    let r2_vars = [
        std::env::var("R2_ACCOUNT_ID"),
        std::env::var("R2_BUCKET_NAME"),
        std::env::var("R2_ACCESS_KEY_ID"),
        std::env::var("R2_SECRET_ACCESS_KEY"),
    ];
    if let [
        Ok(account_id),
        Ok(bucket),
        Ok(access_key_id),
        Ok(secret_access_key),
    ] = r2_vars
    {
        tracing::info!("attachment storage: Cloudflare R2");
        return Arc::new(R2AttachmentStorage::new(
            account_id,
            bucket,
            access_key_id,
            secret_access_key,
        ));
    }

    tracing::info!("attachment storage: mock/sandbox (R2_* vars not fully configured)");
    let attachment_storage_secret = std::env::var("ATTACHMENT_STORAGE_SECRET")
        .expect("ATTACHMENT_STORAGE_SECRET must be configured when R2_* vars are not set")
        .into_bytes();
    let attachment_storage_base_url = std::env::var("ATTACHMENT_STORAGE_BASE_URL")
        .unwrap_or_else(|_| "https://storage.sandbox.local".to_owned());
    Arc::new(HmacSignedAttachmentStorage::new(
        attachment_storage_base_url,
        attachment_storage_secret,
    ))
}

/// A real [`OpenRouterExtractor`] when `OPENROUTER_API_KEY` is configured;
/// otherwise a [`DisabledExtractor`] that fails clearly on use rather than
/// refusing to boot -- AI extraction is an optional feature a deployment
/// may not have set up yet (docs/OPEN_QUESTIONS.md: what data may go to an
/// external AI provider is itself an open question), unlike the secrets
/// this API always requires.
fn report_extractor_from_env() -> Arc<dyn ReportExtractor> {
    match std::env::var("OPENROUTER_API_KEY") {
        Ok(api_key) => {
            let model = std::env::var("OPENROUTER_MODEL")
                .unwrap_or_else(|_| "openai/gpt-4o-mini".to_owned());
            tracing::info!(model, "AI report extraction: OpenRouter");
            Arc::new(OpenRouterExtractor::new(api_key, model))
        }
        Err(_) => {
            tracing::info!("AI report extraction: disabled (OPENROUTER_API_KEY not configured)");
            Arc::new(DisabledExtractor)
        }
    }
}

/// A real [`OpenRouterTranscriber`] when `OPENROUTER_API_KEY` is configured;
/// otherwise a [`DisabledTranscriber`] (issue #160) -- same "optional
/// feature, don't refuse to boot" stance as [`report_extractor_from_env`].
/// `OPENROUTER_MODEL` is the extraction model; transcription uses its own
/// `VOICE_TRANSCRIPTION_MODEL` since not every model accepts audio input.
fn audio_transcriber_from_env() -> Arc<dyn AudioTranscriber> {
    match std::env::var("OPENROUTER_API_KEY") {
        Ok(api_key) => {
            let model = std::env::var("VOICE_TRANSCRIPTION_MODEL")
                .unwrap_or_else(|_| "google/gemini-2.5-flash".to_owned());
            tracing::info!(model, "Voice transcription: OpenRouter");
            Arc::new(OpenRouterTranscriber::new(api_key, model))
        }
        Err(_) => {
            tracing::info!("Voice transcription: disabled (OPENROUTER_API_KEY not configured)");
            Arc::new(DisabledTranscriber)
        }
    }
}

/// The operator kill-switch for the whole voice-report feature (issue
/// #160), independent of whether a real transcriber is configured --
/// on by default, so a deployment that never sets this still gets the
/// feature (gated in turn by `OPENROUTER_API_KEY` presence above). Only an
/// explicit `"false"` or `"0"` turns it off.
fn voice_reports_enabled_from_env() -> bool {
    match std::env::var("VOICE_REPORTS_ENABLED") {
        Ok(value) => !matches!(value.trim().to_ascii_lowercase().as_str(), "false" | "0"),
        Err(_) => true,
    }
}

/// A real [`ResendInviteMailer`] when `RESEND_API_KEY`/`RESEND_FROM_ADDRESS`
/// are configured; otherwise a [`DisabledInviteMailer`] that silently does
/// nothing (invite emails are best-effort -- see `InviteMailer`'s module
/// doc comment) rather than refusing to boot. Reuses the same two vars as
/// `apps/worker`'s citizen-alert email channel, but must be configured
/// separately for this service (`apps/api`) since they're per-service
/// Railway variables, not shared.
fn invite_mailer_from_env() -> Arc<dyn InviteMailer> {
    match (
        std::env::var("RESEND_API_KEY"),
        std::env::var("RESEND_FROM_ADDRESS"),
    ) {
        (Ok(api_key), Ok(from_address)) => {
            tracing::info!("Invite emails: Resend");
            Arc::new(ResendInviteMailer::new(api_key, from_address))
        }
        _ => {
            tracing::info!(
                "Invite emails: disabled (RESEND_API_KEY/RESEND_FROM_ADDRESS not configured)"
            );
            Arc::new(DisabledInviteMailer)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_state(
    pool: PgPool,
    webhook_secret: Vec<u8>,
    attachment_storage: Arc<dyn AttachmentStorage>,
    reviewer_session_secret: Vec<u8>,
    google_identity_verifier: Arc<dyn GoogleIdentityVerifier>,
    report_extractor: Arc<dyn ReportExtractor>,
    audio_transcriber: Arc<dyn AudioTranscriber>,
    voice_reports_enabled: bool,
    invite_mailer: Arc<dyn InviteMailer>,
) -> AppState {
    let mut webhook_verifiers = WebhookVerifierRegistry::new();
    for channel in [ChannelType::WhatsApp, ChannelType::Sms, ChannelType::Email] {
        webhook_verifiers.register(
            SANDBOX_WEBHOOK_PROVIDER,
            Arc::new(HmacSignedWebhookVerifier::new(
                channel,
                webhook_secret.clone(),
            )),
        );
    }

    AppState {
        reports: PostgresReportRepository::new(pool.clone()),
        cases: PostgresCaseRepository::new(pool.clone()),
        alerts: PostgresAlertRepository::new(pool.clone()),
        deliveries: PostgresDeliveryRepository::new(pool.clone()),
        attachments: PostgresAttachmentRepository::new(pool.clone()),
        audit_events: PostgresAuditEventRepository::new(pool.clone()),
        consumers: PostgresConsumerRepository::new(pool.clone()),
        subscriptions: PostgresSubscriptionRepository::new(pool.clone()),
        delivery_preferences: PostgresDeliveryPreferenceRepository::new(pool.clone()),
        organizations: PostgresOrganizationRepository::new(pool.clone()),
        reviewers: PostgresReviewerRepository::new(pool.clone()),
        reviewer_session_tokens: ReviewerSessionTokenIssuer::new(reviewer_session_secret),
        google_identity_verifier,
        rate_limiter: Arc::new(PostgresRateLimiter::new(pool.clone())),
        attachment_storage,
        webhook_verifiers: Arc::new(webhook_verifiers),
        webhook_replay_guard: Arc::new(PostgresWebhookReplayGuard::new(pool.clone())),
        extractions: PostgresReportExtractionRepository::new(pool),
        report_extractor,
        audio_transcriber,
        voice_reports_enabled,
        invite_mailer,
    }
}

/// The console runs on a different origin than the API in every deployed
/// environment (docs/DEPLOYMENT.md), so browsers enforce CORS on every
/// request between them. `CORS_ALLOWED_ORIGINS` is a comma-separated
/// allow-list of exact origins (e.g. `https://console.example.com`); unset
/// or empty means no browser origin is allowed, which is safe by default
/// but must be configured explicitly per environment.
fn cors_layer() -> CorsLayer {
    let origins: Vec<HeaderValue> = std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| {
            origin.parse().unwrap_or_else(|_| {
                panic!("CORS_ALLOWED_ORIGINS contains an invalid origin: {origin}")
            })
        })
        .collect();

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::OPTIONS])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            HeaderName::from_static("idempotency-key"),
            HeaderName::from_static("management-token"),
        ])
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/v1/auth/register", post(auth::register))
        .route("/v1/auth/google", post(auth::google_login))
        .route("/v1/auth/logout", post(auth::logout))
        .route("/v1/audit-events", get(audit_events::list_audit_events))
        .route(
            "/v1/reports",
            get(reports::list_reports).post(reports::create_anonymous_report),
        )
        .route("/v1/reports/{id}", get(reports::get_report))
        .route(
            "/v1/reports/{id}/review",
            post(reports::start_report_review),
        )
        .route(
            "/v1/reports/transcribe-audio",
            post(reports::transcribe_audio).layer(DefaultBodyLimit::max(reports::MAX_AUDIO_BYTES)),
        )
        .route(
            "/v1/reports/{report_id}/attachments",
            get(attachments::list_attachments_for_report).post(attachments::create_attachment),
        )
        .route(
            "/v1/attachments/{id}/download-url",
            get(attachments::create_download_url),
        )
        .route(
            "/v1/reports/{id}/extractions",
            get(extractions::list_extractions).post(extractions::create_extraction),
        )
        .route("/v1/cases", get(cases::list_cases).post(cases::create_case))
        .route("/v1/cases/{id}", get(cases::get_case))
        .route("/v1/cases/{id}/reports", post(cases::link_report))
        .route(
            "/v1/cases/{id}/events",
            get(cases::list_case_events).post(cases::create_case_event),
        )
        .route("/v1/cases/{id}/verify", post(cases::verify_case))
        .route("/v1/cases/{id}/resolve", post(cases::resolve_case))
        .route("/v1/cases/{id}/alerts", post(alerts::create_alert))
        .route("/v1/alerts", get(alerts::list_alerts))
        .route("/v1/alerts/{id}", get(alerts::get_alert))
        .route("/v1/alerts/{id}/cancel", post(alerts::cancel))
        .route(
            "/v1/alerts/{id}/deliveries",
            get(deliveries::list_deliveries_for_alert),
        )
        .route("/v1/deliveries/{id}", get(deliveries::get_delivery))
        .route(
            "/v1/subscriptions",
            post(subscriptions::create_subscription),
        )
        .route(
            "/v1/subscriptions/{id}",
            put(subscriptions::update_subscription),
        )
        .route(
            "/v1/citizen-subscriptions",
            post(citizen_subscriptions::create_citizen_subscription),
        )
        .route(
            "/v1/citizen-subscriptions/{id}",
            get(citizen_subscriptions::get_citizen_subscription)
                .put(citizen_subscriptions::update_citizen_subscription),
        )
        .route(
            "/v1/citizen-subscriptions/{id}/cancel",
            post(citizen_subscriptions::cancel_citizen_subscription),
        )
        .route(
            "/v1/consumers",
            get(consumers::list_consumers).post(consumers::register_consumer),
        )
        .route("/v1/consumers/{id}", get(consumers::get_consumer))
        .route(
            "/v1/organizations",
            get(organizations::list_organizations).post(organizations::create_organization),
        )
        .route(
            "/v1/organizations/{id}",
            get(organizations::get_organization),
        )
        .route(
            "/v1/organizations/{id}/trust",
            put(organizations::set_trust_grants),
        )
        .route(
            "/v1/organizations/{id}/profile",
            put(organizations::update_organization_profile),
        )
        .route(
            "/v1/organizations/{id}/deactivate",
            post(organizations::deactivate_organization),
        )
        .route(
            "/v1/organizations/{id}/reactivate",
            post(organizations::reactivate_organization),
        )
        .route(
            "/v1/organizations/{id}/members",
            get(organizations::list_members),
        )
        .route(
            "/v1/consumers/{consumer_id}/subscriptions",
            get(subscriptions::list_subscriptions_for_consumer),
        )
        .route(
            "/v1/consumers/{consumer_id}/delivery-preference",
            get(subscriptions::get_delivery_preference).put(subscriptions::set_delivery_preference),
        )
        .route(
            "/v1/webhooks/{channel}/{provider}",
            post(webhooks::receive_webhook),
        )
        .with_state(state)
        // Runs outermost-to-innermost on the request, innermost-to-outermost
        // on the response, so listing SetRequestId last means it sees the
        // request first: the id is already on the request's headers by the
        // time TraceLayer builds its span or any handler runs. CORS is
        // listed first so it wraps everything else, including preflight
        // OPTIONS requests that never reach a route handler.
        .layer(cors_layer())
        .layer(PropagateRequestIdLayer::new(REQUEST_ID_HEADER))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(SetRequestIdLayer::new(REQUEST_ID_HEADER, MakeRequestUuid))
}

#[tokio::main]
async fn main() {
    // Optional: a real deployment sets env vars directly and has no .env
    // file, so a missing one is not an error (docs/DEPLOYMENT.md).
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be configured");
    let webhook_secret = std::env::var("WEBHOOK_SHARED_SECRET")
        .expect("WEBHOOK_SHARED_SECRET must be configured")
        .into_bytes();
    let attachment_storage = attachment_storage_from_env();
    let reviewer_session_secret = std::env::var("REVIEWER_SESSION_SECRET")
        .expect("REVIEWER_SESSION_SECRET must be configured")
        .into_bytes();
    let google_oauth_client_id =
        std::env::var("GOOGLE_OAUTH_CLIENT_ID").expect("GOOGLE_OAUTH_CLIENT_ID must be configured");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("database connection must succeed");

    let app = build_router(build_state(
        pool,
        webhook_secret,
        attachment_storage,
        reviewer_session_secret,
        Arc::new(GoogleTokenInfoVerifier::new(google_oauth_client_id)),
        report_extractor_from_env(),
        audio_transcriber_from_env(),
        voice_reports_enabled_from_env(),
        invite_mailer_from_env(),
    ));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("API listener must bind");
    axum::serve(listener, app)
        .await
        .expect("API server must run");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use hmac::{Hmac, Mac};
    use safe_cameroon_application::ai_extraction::FakeReportExtractor;
    use safe_cameroon_application::alert_workflow::create_alert_from_case;
    use safe_cameroon_application::audio_transcription::FakeAudioTranscriber;
    use safe_cameroon_application::case_workflow::{Actor, create_case_from_report, review_case};
    use safe_cameroon_application::channel::{Channel, build_outbound_message};
    use safe_cameroon_application::delivery_workflow::{plan_deliveries, start_delivery_attempt};
    use safe_cameroon_application::google_identity::{
        FakeGoogleIdentityVerifier, INVALID_GOOGLE_TOKEN,
    };
    use safe_cameroon_application::invite_mailer::{FakeInviteMailer, InviteMailer};
    use safe_cameroon_application::prepare_anonymous_report;
    use safe_cameroon_domain::{
        AlertField, AlertFieldValue, AlertPolicy, CaseStatus, ChannelEndpoint, ConsumerId,
        ConsumerMatch, DeliveryPreference, DeliveryStatus, DeliveryStrategy, ExtractedReportFields,
        IncidentType, MatchedSubscription, ReportId, RetryPolicy, Severity, SubscriptionId,
        TargetGeography, deduplicate_by_consumer, evaluate_subscriptions,
    };
    use safe_cameroon_infrastructure::channels::WhatsAppChannel;
    use serde_json::json;
    use sha2::Sha256;
    use tower::ServiceExt;
    use uuid::Uuid;

    type HmacSha256 = Hmac<Sha256>;
    const TEST_SECRET: &[u8] = b"test-webhook-secret";

    fn test_state(pool: PgPool) -> AppState {
        test_state_with_voice_flag(pool, true)
    }

    /// Lets a test toggle the `VOICE_REPORTS_ENABLED` kill-switch
    /// (issue #160) without every other `test_state(pool)` call site
    /// having to pass a value it doesn't care about.
    fn test_state_with_voice_flag(pool: PgPool, voice_reports_enabled: bool) -> AppState {
        test_state_with_invite_mailer(
            pool,
            voice_reports_enabled,
            Arc::new(FakeInviteMailer::default()),
        )
    }

    /// Lets a test inspect what `send_invite` was called with (e.g.
    /// `registering_an_org_admin_sends_an_invite_email` below), without
    /// every other call site having to pass one.
    fn test_state_with_invite_mailer(
        pool: PgPool,
        voice_reports_enabled: bool,
        invite_mailer: Arc<dyn InviteMailer>,
    ) -> AppState {
        build_state(
            pool,
            TEST_SECRET.to_vec(),
            Arc::new(HmacSignedAttachmentStorage::new(
                "https://storage.example",
                b"test-attachment-storage-secret".to_vec(),
            )),
            b"test-reviewer-session-secret".to_vec(),
            Arc::new(FakeGoogleIdentityVerifier),
            Arc::new(FakeReportExtractor {
                result: Ok(ExtractedReportFields::new(
                    Some("a young girl in a blue school uniform".into()),
                    Some("about 8 years old".into()),
                    None,
                    Some("Douala".into()),
                    None,
                    None,
                    None,
                )),
            }),
            Arc::new(FakeAudioTranscriber {
                result: Ok("My daughter has not come home from school.".into()),
            }),
            voice_reports_enabled,
            invite_mailer,
        )
    }

    async fn test_pool() -> PgPool {
        let database_url = env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must point to a dedicated PostgreSQL test database");
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("test database must be reachable");

        let (database_name,): (String,) = sqlx::query_as("SELECT current_database()")
            .fetch_one(&pool)
            .await
            .expect("test database name must be readable");
        assert!(
            database_name.to_ascii_lowercase().contains("test"),
            "TEST_DATABASE_URL must target a database with 'test' in its name"
        );

        static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");
        MIGRATOR.run(&pool).await.expect("migrations must apply");
        sqlx::query(
            "TRUNCATE report_extractions, attachments, webhook_replay_events, delivery_events, delivery_attempts, \
             deliveries, alert_events, alert_fields, alerts, case_events, case_reports, cases, \
             outbox_events, audit_events, reports, reporters, consumer_delivery_preferences, \
             subscriptions, consumers, reviewer_organization_memberships, organizations, \
             reviewers, rate_limit_windows",
        )
        .execute(&pool)
        .await
        .expect("test tables must be reset");
        pool
    }

    async fn seeded_delivery(pool: &PgPool) -> safe_cameroon_domain::DeliveryId {
        let reports = PostgresReportRepository::new(pool.clone());
        let submission =
            prepare_anonymous_report("A child is missing.".into(), Uuid::new_v4(), None, None)
                .unwrap();
        reports.submit_anonymous(&submission).await.unwrap();

        let cases = PostgresCaseRepository::new(pool.clone());
        let creation = create_case_from_report(
            IncidentType::MissingChild,
            ReportId::from_uuid(submission.report.id.as_uuid()),
            Actor::Reviewer(Uuid::new_v4()),
            Uuid::new_v4(),
        );
        cases.create(&creation).await.unwrap();
        let mut case = cases.find_by_id(creation.case.id()).await.unwrap().unwrap();
        for target in [CaseStatus::UnderReview, CaseStatus::Verified] {
            let review = review_case(
                &mut case,
                Actor::Reviewer(Uuid::new_v4()),
                target,
                Uuid::new_v4(),
            )
            .unwrap();
            cases.apply_review(&case, &review).await.unwrap();
        }
        let case = cases.find_by_id(case.id()).await.unwrap().unwrap();

        let alerts = PostgresAlertRepository::new(pool.clone());
        let alert_creation = create_alert_from_case(
            &case,
            &AlertPolicy::missing_child_community_v1(),
            Severity::High,
            TargetGeography::new("Douala").unwrap(),
            vec![AlertFieldValue {
                field: AlertField::IncidentCategory,
                value: "MISSING_CHILD".into(),
            }],
            Actor::Reviewer(Uuid::new_v4()),
            Uuid::new_v4(),
            None,
        )
        .unwrap();
        alerts.create(&alert_creation).await.unwrap();

        let deliveries = PostgresDeliveryRepository::new(pool.clone());
        let consumer_id = ConsumerId::new();
        let preference = DeliveryPreference::new(
            DeliveryStrategy::All,
            vec![ChannelEndpoint::new(ChannelType::WhatsApp, "+237600000000").unwrap()],
        )
        .unwrap();
        let mut preferences = std::collections::HashMap::new();
        preferences.insert(consumer_id, preference);
        let consumer_matches = vec![ConsumerMatch {
            consumer_id,
            matching_subscriptions: vec![MatchedSubscription {
                subscription_id: SubscriptionId::new(),
                subscription_version: 1,
            }],
        }];
        let planned = plan_deliveries(
            &alert_creation.alert,
            &consumer_matches,
            &preferences,
            RetryPolicy::standard(),
            Actor::Automated,
            Uuid::new_v4(),
        );
        deliveries.create_planned(&planned).await.unwrap();
        let delivery_id = planned[0].delivery.id();

        let mut delivery = deliveries.find_by_id(delivery_id).await.unwrap().unwrap();
        let started =
            start_delivery_attempt(&mut delivery, Actor::Automated, Uuid::new_v4()).unwrap();
        deliveries
            .apply_transition(&delivery, &started)
            .await
            .unwrap();
        let succeeded = safe_cameroon_application::delivery_workflow::record_delivery_success(
            &mut delivery,
            Some("wa-provider-msg-1".into()),
            Actor::Automated,
            Uuid::new_v4(),
        )
        .unwrap();
        deliveries
            .apply_attempt_transition(&delivery, &succeeded)
            .await
            .unwrap();

        delivery_id
    }

    fn sign(body: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(TEST_SECRET).unwrap();
        mac.update(body);
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn listing_an_alerts_deliveries_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let delivery_id = seeded_delivery(&pool).await;
        let deliveries = PostgresDeliveryRepository::new(pool.clone());
        let alert_id = deliveries
            .find_by_id(delivery_id)
            .await
            .unwrap()
            .unwrap()
            .alert_id()
            .as_uuid();
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/alerts/{alert_id}/deliveries"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_lists_an_alerts_deliveries() {
        let pool = test_pool().await;
        let delivery_id = seeded_delivery(&pool).await;
        let deliveries = PostgresDeliveryRepository::new(pool.clone());
        let alert_id = deliveries
            .find_by_id(delivery_id)
            .await
            .unwrap()
            .unwrap()
            .alert_id()
            .as_uuid();
        let app = build_router(test_state(pool));
        let token = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/alerts/{alert_id}/deliveries"))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        let listed = body.as_array().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["delivery_id"], json!(delivery_id.as_uuid()));
        assert_eq!(listed[0]["channel"], json!("WHATSAPP"));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn listing_alerts_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/alerts")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_lists_and_filters_alerts() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"content": "A child has not returned from school."}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let report_id = json_body(response).await["report_id"]
            .as_str()
            .unwrap()
            .to_owned();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/cases")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(
                        json!({"report_id": report_id, "incident_type": "MISSING_CHILD"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let case_id = json_body(response).await["case_id"]
            .as_str()
            .unwrap()
            .to_owned();

        for target in ["UNDER_REVIEW"] {
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/v1/cases/{case_id}/events"))
                        .header("content-type", "application/json")
                        .header("Authorization", format!("Bearer {reviewer}"))
                        .body(Body::from(json!({"to": target}).to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
        }
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/verify"))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/alerts"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(
                        json!({
                            "policy_id": "MISSING_CHILD_COMMUNITY",
                            "severity": "HIGH",
                            "target_geography": "Douala - Bonamoussadi",
                            "fields": [
                                {"field": "INCIDENT_CATEGORY", "value": "MISSING_CHILD"}
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let alert_id = json_body(response).await["alert_id"]
            .as_str()
            .unwrap()
            .to_owned();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/alerts")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let listed = json_body(response).await;
        let listed = listed.as_array().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["alert_id"], json!(alert_id));
        assert_eq!(listed[0]["visibility"], json!("COMMUNITY"));
        assert_eq!(listed[0]["status"], json!("ACTIVE"));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/alerts?visibility=COMMUNITY&status=ACTIVE")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await.as_array().unwrap().len(), 1);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/alerts?visibility=INTERNAL")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(json_body(response).await.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn getting_a_single_alert_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/alerts/{}", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_gets_a_single_alert_by_id() {
        let pool = test_pool().await;
        let delivery_id = seeded_delivery(&pool).await;
        let app = build_router(test_state(pool.clone()));
        let reviewer = login_reviewer(app.clone()).await;

        let deliveries = PostgresDeliveryRepository::new(pool.clone());
        let delivery = deliveries.find_by_id(delivery_id).await.unwrap().unwrap();
        let alert_id = delivery.alert_id();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/alerts/{}", alert_id.as_uuid()))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            json_body(response).await["alert_id"],
            json!(alert_id.as_uuid())
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn retrying_alert_creation_with_the_same_idempotency_key_returns_a_conflict() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool.clone()));
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"content": "A child has not returned from school."}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let report_id = json_body(response).await["report_id"]
            .as_str()
            .unwrap()
            .to_owned();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/cases")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(
                        json!({"report_id": report_id, "incident_type": "MISSING_CHILD"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let case_id = json_body(response).await["case_id"]
            .as_str()
            .unwrap()
            .to_owned();
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/events"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(json!({"to": "UNDER_REVIEW"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let verify_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/verify"))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(verify_response.status(), StatusCode::OK);

        let alert_body = json!({
            "policy_id": "MISSING_CHILD_COMMUNITY",
            "severity": "HIGH",
            "target_geography": "Douala - Bonamoussadi",
            "fields": [
                {"field": "INCIDENT_CATEGORY", "value": "MISSING_CHILD"}
            ]
        })
        .to_string();

        let first = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/alerts"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .header("Idempotency-Key", "alert-retry-key-1")
                    .body(Body::from(alert_body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::CREATED);

        let retry = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/alerts"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .header("Idempotency-Key", "alert-retry-key-1")
                    .body(Body::from(alert_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(retry.status(), StatusCode::CONFLICT);
        assert_eq!(
            json_body(retry).await["error"]["code"],
            json!("IDEMPOTENCY_KEY_REUSED")
        );

        let (alert_count,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM alerts WHERE case_id = $1")
                .bind(Uuid::parse_str(&case_id).unwrap())
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(alert_count, 1, "the retried alert must not persist");
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_gets_a_deliverys_detail_including_its_attempt_history() {
        let pool = test_pool().await;
        let delivery_id = seeded_delivery(&pool).await;
        let app = build_router(test_state(pool));
        let token = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/deliveries/{}", delivery_id.as_uuid()))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["delivery_id"], json!(delivery_id.as_uuid()));
        assert_eq!(body["status"], json!("SENT"));
        let attempts = body["attempts"].as_array().unwrap();
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0]["attempt_number"], json!(1));
        assert_eq!(attempts[0]["outcome"], json!("SENT"));
        assert_eq!(
            attempts[0]["provider_message_id"],
            json!("wa-provider-msg-1")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn getting_an_unknown_delivery_returns_not_found() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let token = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/deliveries/{}", Uuid::new_v4()))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_correctly_signed_webhook_marks_the_delivery_delivered() {
        let pool = test_pool().await;
        let delivery_id = seeded_delivery(&pool).await;
        let app = build_router(test_state(pool.clone()));

        let body = json!({
            "event_id": "evt-1",
            "message_id": "wa-provider-msg-1",
            "status": "DELIVERED",
        })
        .to_string();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/webhooks/whatsapp/sandbox")
                    .header("x-signature", sign(body.as_bytes()))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let deliveries = PostgresDeliveryRepository::new(pool);
        let delivery = deliveries.find_by_id(delivery_id).await.unwrap().unwrap();
        assert_eq!(delivery.status(), DeliveryStatus::Delivered);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_replayed_webhook_event_is_accepted_but_applied_only_once() {
        let pool = test_pool().await;
        let delivery_id = seeded_delivery(&pool).await;
        let app = build_router(test_state(pool.clone()));

        let body = json!({
            "event_id": "evt-2",
            "message_id": "wa-provider-msg-1",
            "status": "DELIVERED",
        })
        .to_string();
        let signature = sign(body.as_bytes());

        for _ in 0..2 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/webhooks/whatsapp/sandbox")
                        .header("x-signature", signature.clone())
                        .header("content-type", "application/json")
                        .body(Body::from(body.clone()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }

        let deliveries = PostgresDeliveryRepository::new(pool.clone());
        let delivery = deliveries.find_by_id(delivery_id).await.unwrap().unwrap();
        assert_eq!(delivery.status(), DeliveryStatus::Delivered);

        let (event_count,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM delivery_events WHERE delivery_id = $1 AND event_type = 'DELIVERY_DELIVERED'",
        )
        .bind(delivery_id.as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(event_count, 1, "a replayed event must not be applied twice");
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn an_incorrectly_signed_webhook_is_rejected_and_does_not_change_the_delivery() {
        let pool = test_pool().await;
        let delivery_id = seeded_delivery(&pool).await;
        let app = build_router(test_state(pool.clone()));

        let body = json!({
            "event_id": "evt-3",
            "message_id": "wa-provider-msg-1",
            "status": "DELIVERED",
        })
        .to_string();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/webhooks/whatsapp/sandbox")
                    .header("x-signature", "0000")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let deliveries = PostgresDeliveryRepository::new(pool);
        let delivery = deliveries.find_by_id(delivery_id).await.unwrap().unwrap();
        assert_eq!(delivery.status(), DeliveryStatus::Sent);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn an_unknown_provider_is_rejected_with_not_found() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/webhooks/whatsapp/unknown-provider")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    async fn seeded_report(pool: &PgPool) -> ReportId {
        let reports = PostgresReportRepository::new(pool.clone());
        let submission =
            prepare_anonymous_report("A child is missing.".into(), Uuid::new_v4(), None, None)
                .unwrap();
        reports.submit_anonymous(&submission).await.unwrap();
        submission.report.id
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn creates_an_attachment_upload_url_without_any_actor() {
        let pool = test_pool().await;
        let report_id = seeded_report(&pool).await;
        let app = build_router(test_state(pool));

        let body = json!({
            "content_type": "image/jpeg",
            "size_bytes": 2048,
            "checksum": "deadbeef",
        })
        .to_string();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/reports/{}/attachments", report_id.as_uuid()))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(
            json["upload_url"]
                .as_str()
                .unwrap()
                .contains(json["object_key"].as_str().unwrap())
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_requests_and_lists_a_reports_extraction() {
        let pool = test_pool().await;
        let report_id = seeded_report(&pool).await;
        let app = build_router(test_state(pool));
        let token = login_reviewer(app.clone()).await;

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/reports/{}/extractions", report_id.as_uuid()))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let created = json_body(create_response).await;
        assert_eq!(created["provider"], json!("FAKE"));
        assert_eq!(created["fields"]["place"], json!("Douala"));

        let list_response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/reports/{}/extractions", report_id.as_uuid()))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_response.status(), StatusCode::OK);
        let listed = json_body(list_response).await;
        assert_eq!(listed.as_array().unwrap().len(), 1);
        assert_eq!(listed[0]["fields"]["place"], json!("Douala"));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn requesting_an_extraction_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let report_id = seeded_report(&pool).await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/reports/{}/extractions", report_id.as_uuid()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn requesting_an_extraction_for_an_unknown_report_returns_not_found() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let token = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/reports/{}/extractions", Uuid::new_v4()))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn exceeding_the_attachment_upload_rate_limit_returns_too_many_requests() {
        let pool = test_pool().await;
        let report_id = seeded_report(&pool).await;
        let app = build_router(test_state(pool));
        let request_upload = |app: Router| {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/reports/{}/attachments", report_id.as_uuid()))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"content_type": "image/jpeg", "size_bytes": 2048, "checksum": "deadbeef"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
        };

        for attempt_number in 1..=30 {
            let response = request_upload(app.clone()).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::CREATED,
                "attempt {attempt_number} should still be within the limit"
            );
        }

        let throttled = request_upload(app).await.unwrap();
        assert_eq!(throttled.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            json_body(throttled).await["error"]["code"],
            json!("RATE_LIMIT_EXCEEDED")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn rejects_an_attachment_for_an_unknown_report() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let body = json!({
            "content_type": "image/jpeg",
            "size_bytes": 2048,
            "checksum": "deadbeef",
        })
        .to_string();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/reports/{}/attachments", Uuid::new_v4()))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn listing_attachments_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let report_id = seeded_report(&pool).await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/reports/{}/attachments", report_id.as_uuid()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_lists_a_reports_attachments_in_upload_order() {
        let pool = test_pool().await;
        let report_id = seeded_report(&pool).await;
        let attachments = PostgresAttachmentRepository::new(pool.clone());
        let first = safe_cameroon_domain::Attachment::new(
            report_id,
            safe_cameroon_domain::StorageProvider::R2,
            "attachments/report-1/key-1",
            safe_cameroon_domain::AttachmentContentType::ImageJpeg,
            2048,
            "deadbeef",
        )
        .unwrap();
        attachments.create(&first).await.unwrap();
        let second = safe_cameroon_domain::Attachment::new(
            report_id,
            safe_cameroon_domain::StorageProvider::R2,
            "attachments/report-1/key-2",
            safe_cameroon_domain::AttachmentContentType::ApplicationPdf,
            4096,
            "cafef00d",
        )
        .unwrap();
        attachments.create(&second).await.unwrap();

        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/reports/{}/attachments", report_id.as_uuid()))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let listed = json_body(response).await;
        let listed = listed.as_array().unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0]["attachment_id"], json!(first.id().as_uuid()));
        assert_eq!(listed[0]["object_key"], json!("attachments/report-1/key-1"));
        assert_eq!(listed[0]["content_type"], json!("image/jpeg"));
        assert_eq!(listed[0]["size_bytes"], json!(2048));
        assert_eq!(listed[0]["checksum"], json!("deadbeef"));
        assert_eq!(listed[1]["attachment_id"], json!(second.id().as_uuid()));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_download_url_requires_an_identified_reviewer_and_is_audited() {
        let pool = test_pool().await;
        let report_id = seeded_report(&pool).await;
        let attachments = PostgresAttachmentRepository::new(pool.clone());
        let attachment = safe_cameroon_domain::Attachment::new(
            report_id,
            safe_cameroon_domain::StorageProvider::R2,
            "attachments/report-1/key-1",
            safe_cameroon_domain::AttachmentContentType::ImageJpeg,
            2048,
            "deadbeef",
        )
        .unwrap();
        attachments.create(&attachment).await.unwrap();
        let app = build_router(test_state(pool.clone()));

        let unauthorized = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/v1/attachments/{}/download-url",
                        attachment.id().as_uuid()
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::FORBIDDEN);

        let token = login_reviewer(app.clone()).await;
        let authorized = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/v1/attachments/{}/download-url",
                        attachment.id().as_uuid()
                    ))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(authorized.status(), StatusCode::OK);

        let (count,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM audit_events WHERE resource_id = $1 \
             AND action = 'ATTACHMENT_DOWNLOAD_URL_ISSUED'",
        )
        .bind(attachment.id().as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_client_supplied_request_id_is_echoed_back_and_used_as_the_audit_request_id() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool.clone()));
        let client_request_id = Uuid::new_v4();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports")
                    .header("content-type", "application/json")
                    .header("x-request-id", client_request_id.to_string())
                    .body(Body::from(
                        json!({"content": "A child is missing."}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response
                .headers()
                .get("x-request-id")
                .and_then(|value| value.to_str().ok()),
            Some(client_request_id.to_string().as_str())
        );

        let (count,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM audit_events WHERE request_id = $1 AND action = 'REPORT_SUBMITTED'",
        )
        .bind(client_request_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            count, 1,
            "the client-supplied x-request-id must be the same id recorded on the audit event"
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_missing_request_id_is_generated_and_echoed_back() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let generated = response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok());
        assert!(generated.is_some_and(|value| Uuid::parse_str(value).is_ok()));
    }

    async fn json_body(response: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    /// Registers a fresh reviewer and signs in with `FakeGoogleIdentityVerifier`
    /// (which treats the id token as the already-verified email), returning
    /// a bearer session token. Relies on the reviewers table being empty at
    /// the start of each test (`test_pool`'s `TRUNCATE`), so registration is
    /// always the deployment's bootstrapping first-reviewer case and needs
    /// no authorization of its own.
    async fn login_reviewer(app: Router) -> String {
        let email = format!("reviewer-{}@example.test", Uuid::new_v4());

        let register_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"email": email}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            register_response.status(),
            StatusCode::CREATED,
            "reviewer registration must succeed in test setup"
        );

        let login_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/google")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"id_token": email}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            login_response.status(),
            StatusCode::OK,
            "reviewer login must succeed in test setup"
        );
        json_body(login_response).await["token"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    /// Creates an organization as `granter_token` (must be a `PlatformAdmin`)
    /// and returns its id.
    async fn create_organization(app: Router, granter_token: &str, name: &str) -> String {
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/organizations")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {granter_token}"))
                    .body(Body::from(json!({"name": name}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::CREATED,
            "organization creation must succeed in test setup"
        );
        json_body(response).await["organization_id"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    /// Like `create_organization`, but also returns the id of the consumer
    /// auto-linked to it (migration 0025) — used by tests that manage that
    /// consumer's subscriptions/delivery preference.
    async fn create_organization_with_consumer(
        app: Router,
        granter_token: &str,
        name: &str,
    ) -> (String, String) {
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/organizations")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {granter_token}"))
                    .body(Body::from(json!({"name": name}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::CREATED,
            "organization creation must succeed in test setup"
        );
        let body = json_body(response).await;
        (
            body["organization_id"].as_str().unwrap().to_owned(),
            body["consumer_id"]
                .as_str()
                .expect("a newly created organization is linked to a consumer")
                .to_owned(),
        )
    }

    /// Registers a fresh reviewer with `role`/`organization_id` (as
    /// `granter_token`) and logs them in, returning their session token.
    async fn register_and_login(
        app: Router,
        granter_token: &str,
        role: &str,
        organization_id: Option<&str>,
    ) -> String {
        let email = format!("reviewer-{}@example.test", Uuid::new_v4());
        let mut body = serde_json::json!({"email": email, "role": role});
        if let Some(organization_id) = organization_id {
            body["organization_id"] = json!(organization_id);
        }

        let register_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {granter_token}"))
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            register_response.status(),
            StatusCode::CREATED,
            "reviewer registration must succeed in test setup"
        );

        let login_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/google")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"id_token": email}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login_response.status(), StatusCode::OK);
        json_body(login_response).await["token"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_second_reviewer_registration_without_authentication_is_forbidden() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        // Bootstraps the first reviewer, consuming the "no reviewers yet" window.
        login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": "second@example.test", "role": "PLATFORM_ADMIN"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn an_authenticated_reviewer_may_register_a_second_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let token = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::from(
                        json!({"email": "second@example.test", "role": "PLATFORM_ADMIN"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn registering_an_already_used_email_is_rejected() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        // Bootstraps the first reviewer, consuming the "no reviewers yet"
        // window, so both registrations below go through the same
        // authenticated reviewer.
        let token = login_reviewer(app.clone()).await;
        let email = "duplicate@example.test";
        let register = |app: Router, token: String| {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::from(
                        json!({"email": email, "role": "PLATFORM_ADMIN"}).to_string(),
                    ))
                    .unwrap(),
            )
        };

        let first = register(app.clone(), token.clone()).await.unwrap();
        assert_eq!(first.status(), StatusCode::CREATED);

        let second = register(app, token).await.unwrap();
        assert_eq!(second.status(), StatusCode::CONFLICT);
        assert_eq!(
            json_body(second).await["error"]["code"],
            json!("EMAIL_ALREADY_REGISTERED")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn google_login_rejects_a_token_google_does_not_verify() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/google")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"id_token": INVALID_GOOGLE_TOKEN}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("INVALID_GOOGLE_TOKEN")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn google_login_rejects_a_verified_email_that_is_not_a_registered_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/google")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"id_token": "nobody@example.test"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("REVIEWER_NOT_REGISTERED")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn google_login_response_carries_the_reviewers_normalized_email() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let email = format!("Reviewer-{}@Example.Test", Uuid::new_v4());

        let register_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"email": email}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(register_response.status(), StatusCode::CREATED);

        let login_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/google")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"id_token": email}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login_response.status(), StatusCode::OK);
        let body = json_body(login_response).await;
        assert_eq!(body["email"], json!(email.to_lowercase()));
        assert_eq!(
            body["role"],
            json!("PLATFORM_ADMIN"),
            "the bootstrapping first reviewer must be a platform admin"
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn google_login_response_carries_a_non_admin_reviewers_granted_role() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let admin_token = login_reviewer(app.clone()).await;
        let organization_id = create_organization(app.clone(), &admin_token, "Douala Police").await;
        let email = format!("member-{}@example.test", Uuid::new_v4());

        let register_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {admin_token}"))
                    .body(Body::from(
                        json!({"email": email, "role": "MEMBER", "organization_id": organization_id})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(register_response.status(), StatusCode::CREATED);

        let login_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/google")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"id_token": email}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login_response.status(), StatusCode::OK);
        let body = json_body(login_response).await;
        assert_eq!(body["role"], json!("MEMBER"));
        assert_eq!(body["organization_id"], json!(organization_id));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn logout_requires_an_authenticated_session() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/logout")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("NOT_AUTHENTICATED")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn logout_revokes_the_session_and_the_old_token_stops_working() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let token = login_reviewer(app.clone()).await;

        let protected_request = |app: Router, token: &str| {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/subscriptions")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::from(
                        json!({
                            "consumer_id": Uuid::new_v4(),
                            "rules": [{"rule": "INCIDENT_TYPE", "values": ["MISSING_CHILD"]}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
        };

        let before_logout = protected_request(app.clone(), &token).await.unwrap();
        assert_eq!(before_logout.status(), StatusCode::CREATED);

        let logout_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/logout")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(logout_response.status(), StatusCode::NO_CONTENT);

        let after_logout = protected_request(app, &token).await.unwrap();
        assert_eq!(after_logout.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            json_body(after_logout).await["error"]["code"],
            json!("INVALID_OR_EXPIRED_SESSION")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn register_login_and_logout_each_write_an_audit_event() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool.clone()));
        let email = "audited@example.test";

        let register_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"email": email}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let reviewer_id = json_body(register_response)
            .await
            .get("reviewer_id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();

        let (register_audit_count,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM audit_events WHERE action = 'REVIEWER_REGISTERED' AND resource_id = $1",
        )
        .bind(Uuid::parse_str(&reviewer_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(register_audit_count, 1);

        // A Google-verified identity that isn't a registered reviewer is the
        // only way `google_login` fails after Google itself has vouched for
        // the account, so this (not a "wrong password" for `email`) is what
        // exercises REVIEWER_LOGIN_FAILED now.
        let unregistered_email = "not-a-reviewer@example.test";
        let failed_login = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/google")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"id_token": unregistered_email}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(failed_login.status(), StatusCode::FORBIDDEN);

        let (login_failed_count,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM audit_events WHERE action = 'REVIEWER_LOGIN_FAILED' \
             AND resource_id IS NULL AND metadata->>'email' = $1",
        )
        .bind(unregistered_email)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(login_failed_count, 1);

        let login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/google")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"id_token": email}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login_response.status(), StatusCode::OK);
        let token = json_body(login_response).await["token"]
            .as_str()
            .unwrap()
            .to_owned();

        let (login_succeeded_count,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM audit_events WHERE action = 'REVIEWER_LOGIN_SUCCEEDED' AND resource_id = $1",
        )
        .bind(Uuid::parse_str(&reviewer_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(login_succeeded_count, 1);

        let logout_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/logout")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(logout_response.status(), StatusCode::NO_CONTENT);

        let (logout_count,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM audit_events WHERE action = 'REVIEWER_LOGOUT' AND resource_id = $1",
        )
        .bind(Uuid::parse_str(&reviewer_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(logout_count, 1);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn listing_audit_events_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/audit-events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_lists_audit_events_filtered_by_resource_type_and_action() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        // `login_reviewer` itself writes a REVIEWER_REGISTERED and a
        // REVIEWER_LOGIN_SUCCEEDED audit event for the account it creates.
        let token = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/audit-events?resource_type=REVIEWER&action=REVIEWER_REGISTERED")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let events = json_body(response).await;
        let events = events.as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["action"], json!("REVIEWER_REGISTERED"));
        assert_eq!(events[0]["resource_type"], json!("REVIEWER"));
        assert!(events[0]["occurred_at"].as_str().is_some());
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn listing_audit_events_respects_the_limit_query_parameter() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        // Bootstraps one reviewer (REVIEWER_REGISTERED + REVIEWER_LOGIN_SUCCEEDED),
        // then registers a second using the first's token (another
        // REVIEWER_REGISTERED) — at least 3 audit events exist by now.
        let token = login_reviewer(app.clone()).await;
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::from(
                        json!({"email": "second@example.test", "role": "PLATFORM_ADMIN"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/audit-events?limit=1")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let events = json_body(response).await;
        assert_eq!(events.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_garbage_bearer_token_is_rejected_rather_than_treated_as_automated() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/subscriptions")
                    .header("content-type", "application/json")
                    .header("Authorization", "Bearer not-a-real-token")
                    .body(Body::from(
                        json!({
                            "consumer_id": Uuid::new_v4(),
                            "rules": [{"rule": "INCIDENT_TYPE", "values": ["MISSING_CHILD"]}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("INVALID_OR_EXPIRED_SESSION")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn exceeding_the_login_rate_limit_returns_too_many_requests() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        // Keyed by source IP (unset here, so every attempt shares the
        // "unknown" bucket) rather than the attempted email: the rate limit
        // has to apply before Google is ever asked to verify the token.
        let attempt = |app: Router| {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/google")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"id_token": INVALID_GOOGLE_TOKEN}).to_string(),
                    ))
                    .unwrap(),
            )
        };

        for attempt_number in 1..=5 {
            let response = attempt(app.clone()).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "attempt {attempt_number} should be a normal credential failure, not throttled yet"
            );
        }

        let throttled = attempt(app).await.unwrap();
        assert_eq!(throttled.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            json_body(throttled).await["error"]["code"],
            json!("RATE_LIMIT_EXCEEDED")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn listing_reports_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/reports")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_lists_and_filters_reports() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"content": "My child has not returned from school."}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let report_id = json_body(response).await["report_id"]
            .as_str()
            .unwrap()
            .to_owned();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/reports")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let listed = json_body(response).await;
        let listed = listed.as_array().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["report_id"], json!(report_id));
        assert_eq!(
            listed[0]["raw_content"],
            json!("My child has not returned from school.")
        );
        assert_eq!(listed[0]["status"], json!("RECEIVED"));
        assert_eq!(listed[0]["source_channel"], json!("WEB"));
        assert!(listed[0]["received_at"].as_str().is_some());

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/reports?status=UNDER_REVIEW")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(json_body(response).await.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_gets_a_single_report_by_id() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"content": "My child has not returned from school."}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let report_id = json_body(response).await["report_id"]
            .as_str()
            .unwrap()
            .to_owned();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/reports/{report_id}"))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["report_id"], json!(report_id));
        assert_eq!(
            body["raw_content"],
            json!("My child has not returned from school.")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_starts_reviewing_a_received_report() {
        let pool = test_pool().await;
        let report_id = seeded_report(&pool).await;
        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/reports/{}/review", report_id.as_uuid()))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["status"], json!("UNDER_REVIEW"));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn starting_review_twice_returns_conflict() {
        let pool = test_pool().await;
        let report_id = seeded_report(&pool).await;
        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;

        let review = |app: Router, report_id: ReportId, reviewer: &str| {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/reports/{}/review", report_id.as_uuid()))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
        };

        let first = review(app.clone(), report_id, &reviewer).await.unwrap();
        assert_eq!(first.status(), StatusCode::OK);

        let second = review(app, report_id, &reviewer).await.unwrap();
        assert_eq!(second.status(), StatusCode::CONFLICT);
        let body = json_body(second).await;
        assert_eq!(body["error"]["code"], json!("REPORT_NOT_RECEIVED"));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn starting_review_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/reports/{}/review", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reporters_incident_type_guess_is_stored_and_surfaced_to_reviewers() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "content": "Someone is being harassed near the market.",
                            "incident_type": "OTHER_PROTECTION_INCIDENT",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let report_id = json_body(response).await["report_id"]
            .as_str()
            .unwrap()
            .to_owned();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/reports/{report_id}"))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(
            body["reported_incident_type"],
            json!("OTHER_PROTECTION_INCIDENT")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn getting_an_unknown_report_returns_not_found() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/reports/{}", Uuid::new_v4()))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("REPORT_NOT_FOUND")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn getting_a_report_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/reports/{}", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn exceeding_the_anonymous_report_rate_limit_returns_too_many_requests() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let submit = |app: Router| {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"content": "A child is missing."}).to_string(),
                    ))
                    .unwrap(),
            )
        };

        for attempt_number in 1..=10 {
            let response = submit(app.clone()).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::CREATED,
                "attempt {attempt_number} should still be within the limit"
            );
        }

        let throttled = submit(app).await.unwrap();
        assert_eq!(throttled.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            json_body(throttled).await["error"]["code"],
            json!("RATE_LIMIT_EXCEEDED")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn transcribes_audio_and_returns_the_transcript() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports/transcribe-audio")
                    .header("content-type", "audio/webm;codecs=opus")
                    .body(Body::from(vec![0u8; 128]))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            json_body(response).await["transcript"],
            json!("My daughter has not come home from school.")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn transcription_is_unavailable_when_the_voice_reports_flag_is_disabled() {
        let pool = test_pool().await;
        let app = build_router(test_state_with_voice_flag(pool, false));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports/transcribe-audio")
                    .header("content-type", "audio/webm;codecs=opus")
                    .body(Body::from(vec![0u8; 128]))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("VOICE_REPORTS_DISABLED")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn an_empty_audio_body_is_rejected() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports/transcribe-audio")
                    .header("content-type", "audio/webm;codecs=opus")
                    .body(Body::from(Vec::<u8>::new()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("EMPTY_AUDIO")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn exceeding_the_voice_transcription_rate_limit_returns_too_many_requests() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let transcribe = |app: Router| {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports/transcribe-audio")
                    .header("content-type", "audio/webm;codecs=opus")
                    .body(Body::from(vec![0u8; 128]))
                    .unwrap(),
            )
        };

        for attempt_number in 1..=5 {
            let response = transcribe(app.clone()).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "attempt {attempt_number} should still be within the limit"
            );
        }

        let throttled = transcribe(app).await.unwrap();
        assert_eq!(throttled.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            json_body(throttled).await["error"]["code"],
            json!("RATE_LIMIT_EXCEEDED")
        );
    }

    /// docs/TESTING.md section 8, Scenario A ("missing child"), driven end to
    /// end through the real HTTP surface wherever one exists: anonymous
    /// report -> case review -> verified case -> community alert ->
    /// subscription match -> a real channel adapter's delivery -> the
    /// provider's delivered callback -> case resolution. There is no HTTP
    /// endpoint for planning/dispatching deliveries yet (that is
    /// `apps/worker`'s job, exercised directly here against the same
    /// production repositories/adapters `apps/worker` uses) and AI analysis
    /// is not implemented anywhere in this codebase, so both are skipped
    /// rather than faked.
    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn full_missing_child_scenario_from_anonymous_report_to_resolution() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool.clone()));
        let reviewer = login_reviewer(app.clone()).await;

        // 1. Anonymous report - no actor required.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"content": "My child has not returned from school."}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let report_id = json_body(response).await["report_id"]
            .as_str()
            .unwrap()
            .to_owned();

        // 2. A reviewer opens a case from the report.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/cases")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(
                        json!({"report_id": report_id, "incident_type": "MISSING_CHILD"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let case_body = json_body(response).await;
        assert_eq!(case_body["status"], json!("REPORTED"));
        let case_id = case_body["case_id"].as_str().unwrap().to_owned();

        // 3. Review begins, then the case is verified.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/events"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(json!({"to": "UNDER_REVIEW"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/verify"))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await["status"], json!("VERIFIED"));

        // 4. A community alert is raised from the verified case.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/alerts"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(
                        json!({
                            "policy_id": "MISSING_CHILD_COMMUNITY",
                            "severity": "HIGH",
                            "target_geography": "Douala - Bonamoussadi",
                            "fields": [
                                {"field": "INCIDENT_CATEGORY", "value": "MISSING_CHILD"},
                                {"field": "APPROXIMATE_AGE", "value": "8 years old"}
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let alert_body = json_body(response).await;
        assert_eq!(alert_body["visibility"], json!("COMMUNITY"));
        let alert_id = alert_body["alert_id"].as_str().unwrap().to_owned();

        // 5. Case becomes actively worked while the alert is delivered.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/events"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(json!({"to": "ACTIVE"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // 6. A reviewer registers a matching subscription and delivery
        // preference through the real HTTP endpoints (rather than
        // constructing domain objects in memory), then subscription
        // matching and delivery planning run against what was actually
        // persisted — the same domain/application functions
        // apps/worker's subscription-matching step calls, since apps/api
        // does not depend on the apps/worker binary crate.
        let consumer_id = Uuid::new_v4();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/subscriptions")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(
                        json!({
                            "consumer_id": consumer_id,
                            "rules": [
                                {"rule": "INCIDENT_TYPE", "values": ["MISSING_CHILD"]},
                                {"rule": "SEVERITY", "operator": "GREATER_THAN_OR_EQUAL", "value": "MEDIUM"},
                                {"rule": "GEOGRAPHY", "areas": ["Douala"]}
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/v1/consumers/{consumer_id}/delivery-preference"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(
                        json!({
                            "strategy": "ALL",
                            "channels": [{"channel": "WHATSAPP", "address": "+237600000000"}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let alerts = PostgresAlertRepository::new(pool.clone());
        let alert = alerts
            .find_by_id(safe_cameroon_domain::AlertId::from_uuid(
                Uuid::parse_str(&alert_id).unwrap(),
            ))
            .await
            .unwrap()
            .unwrap();

        let subscriptions = PostgresSubscriptionRepository::new(pool.clone());
        let decisions = evaluate_subscriptions(&subscriptions.list_all().await.unwrap(), &alert);
        let consumer_matches = deduplicate_by_consumer(&decisions);
        assert_eq!(
            consumer_matches.len(),
            1,
            "the persisted subscription must match this alert"
        );
        assert_eq!(consumer_matches[0].consumer_id.as_uuid(), consumer_id);

        let delivery_preferences = PostgresDeliveryPreferenceRepository::new(pool.clone());
        let mut preferences = std::collections::HashMap::new();
        preferences.insert(
            ConsumerId::from_uuid(consumer_id),
            delivery_preferences
                .find_by_consumer(ConsumerId::from_uuid(consumer_id))
                .await
                .unwrap()
                .unwrap(),
        );

        let planned = plan_deliveries(
            &alert,
            &consumer_matches,
            &preferences,
            RetryPolicy::standard(),
            Actor::Automated,
            Uuid::new_v4(),
        );
        assert_eq!(planned.len(), 1);

        let deliveries = PostgresDeliveryRepository::new(pool.clone());
        deliveries.create_planned(&planned).await.unwrap();

        // 7. Dispatch, against the real WhatsApp mock/sandbox adapter
        // apps/worker registers.
        let claimed = deliveries.claim_next(1).await.unwrap();
        assert_eq!(claimed.len(), 1);
        let mut delivery = claimed.into_iter().next().unwrap();
        let message = build_outbound_message(&alert, &delivery);
        let send_outcome = WhatsAppChannel.send(message).await.unwrap();
        let succeeded = safe_cameroon_application::delivery_workflow::record_delivery_success(
            &mut delivery,
            send_outcome.provider_message_id.clone(),
            Actor::Automated,
            Uuid::new_v4(),
        )
        .unwrap();
        deliveries
            .apply_attempt_transition(&delivery, &succeeded)
            .await
            .unwrap();
        assert_eq!(delivery.status(), DeliveryStatus::Sent);
        let provider_message_id = send_outcome.provider_message_id.unwrap();

        // 8. The provider's delivered callback arrives.
        let webhook_body = json!({
            "event_id": Uuid::new_v4().to_string(),
            "message_id": provider_message_id,
            "status": "DELIVERED",
        })
        .to_string();
        let mut mac = HmacSha256::new_from_slice(TEST_SECRET).unwrap();
        mac.update(webhook_body.as_bytes());
        let signature: String = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/webhooks/whatsapp/sandbox")
                    .header("content-type", "application/json")
                    .header("x-signature", signature)
                    .body(Body::from(webhook_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let reloaded_delivery = deliveries.find_by_id(delivery.id()).await.unwrap().unwrap();
        assert_eq!(reloaded_delivery.status(), DeliveryStatus::Delivered);

        // 9. The child is found; the case is resolved.
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/resolve"))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await["status"], json!("RESOLVED"));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn listing_case_events_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/cases/{}/events", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_lists_a_cases_full_event_history_in_order() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"content": "My child has not returned from school."}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let report_id = json_body(response).await["report_id"]
            .as_str()
            .unwrap()
            .to_owned();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/cases")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(
                        json!({"report_id": report_id, "incident_type": "MISSING_CHILD"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let case_id = json_body(response).await["case_id"]
            .as_str()
            .unwrap()
            .to_owned();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/events"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(json!({"to": "UNDER_REVIEW"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/cases/{case_id}/events"))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let events = json_body(response).await;
        let events = events.as_array().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["event_type"], json!("CASE_CREATED"));
        assert_eq!(events[0]["aggregate_version"], json!(1));
        assert_eq!(events[0]["case_id"], json!(case_id));
        assert!(events[0]["occurred_at"].as_str().is_some());
        assert_eq!(events[1]["event_type"], json!("CASE_UNDER_REVIEW"));
        assert_eq!(events[1]["aggregate_version"], json!(2));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn listing_cases_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/cases")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_lists_and_filters_cases() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;

        async fn create_case(
            app: Router,
            reviewer: &str,
            content: &str,
            incident_type: &str,
        ) -> String {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/reports")
                        .header("content-type", "application/json")
                        .body(Body::from(json!({"content": content}).to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            let report_id = json_body(response).await["report_id"]
                .as_str()
                .unwrap()
                .to_owned();

            let response = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/cases")
                        .header("content-type", "application/json")
                        .header("Authorization", format!("Bearer {reviewer}"))
                        .body(Body::from(
                            json!({"report_id": report_id, "incident_type": incident_type})
                                .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            json_body(response).await["case_id"]
                .as_str()
                .unwrap()
                .to_owned()
        }

        let missing_child_case = create_case(
            app.clone(),
            &reviewer,
            "A child has not returned from school.",
            "MISSING_CHILD",
        )
        .await;
        let other_case = create_case(
            app.clone(),
            &reviewer,
            "An unrelated protection incident.",
            "OTHER_PROTECTION_INCIDENT",
        )
        .await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/cases")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let all_cases = json_body(response).await;
        let all_ids: Vec<String> = all_cases
            .as_array()
            .unwrap()
            .iter()
            .map(|case| case["case_id"].as_str().unwrap().to_owned())
            .collect();
        assert!(all_ids.contains(&missing_child_case));
        assert!(all_ids.contains(&other_case));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/cases?incident_type=MISSING_CHILD&status=REPORTED")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let filtered = json_body(response).await;
        let filtered = filtered.as_array().unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0]["case_id"], json!(missing_child_case));
        assert_eq!(filtered[0]["incident_type"], json!("MISSING_CHILD"));
        assert_eq!(filtered[0]["status"], json!("REPORTED"));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn getting_a_single_case_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/cases/{}", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_gets_a_single_case_by_id() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"content": "A child has not returned from school."}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let report_id = json_body(response).await["report_id"]
            .as_str()
            .unwrap()
            .to_owned();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/cases")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(
                        json!({"report_id": report_id, "incident_type": "MISSING_CHILD"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let case_id = json_body(response).await["case_id"]
            .as_str()
            .unwrap()
            .to_owned();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/cases/{case_id}"))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await["case_id"], json!(case_id));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn registering_a_consumer_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/consumers")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"name": "Douala Police", "consumer_type": "ORGANIZATION"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_registers_and_reads_back_a_consumer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/consumers")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(
                        json!({"name": "Douala Police", "consumer_type": "ORGANIZATION"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = json_body(response).await;
        assert_eq!(body["name"], json!("Douala Police"));
        assert_eq!(body["consumer_type"], json!("ORGANIZATION"));
        let consumer_id = body["consumer_id"].as_str().unwrap().to_owned();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/consumers/{consumer_id}"))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["consumer_id"], json!(consumer_id));
        assert_eq!(body["name"], json!("Douala Police"));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn registering_a_consumer_with_a_blank_name_is_rejected() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/consumers")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(
                        json!({"name": "   ", "consumer_type": "CITIZEN"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("INVALID_CONSUMER_NAME")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn getting_an_unknown_consumer_returns_not_found() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/consumers/{}", Uuid::new_v4()))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_lists_and_filters_consumers() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let reviewer = login_reviewer(app.clone()).await;
        let register = |app: Router, name: &'static str, consumer_type: &'static str| {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/consumers")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(
                        json!({"name": name, "consumer_type": consumer_type}).to_string(),
                    ))
                    .unwrap(),
            )
        };
        register(app.clone(), "Douala Police", "ORGANIZATION")
            .await
            .unwrap();
        register(app.clone(), "Amina N.", "CITIZEN").await.unwrap();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/consumers")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await.as_array().unwrap().len(), 2);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/consumers?consumer_type=CITIZEN")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let listed = json_body(response).await;
        let listed = listed.as_array().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["name"], json!("Amina N."));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn listing_consumers_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/consumers")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn creating_an_organization_requires_a_platform_admin() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let platform_admin = login_reviewer(app.clone()).await;
        let org_id = create_organization(app.clone(), &platform_admin, "Douala Police").await;
        let member =
            register_and_login(app.clone(), &platform_admin, "MEMBER", Some(&org_id)).await;

        let unauthenticated = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/organizations")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"name": "Yaounde NGO"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthenticated.status(), StatusCode::FORBIDDEN);

        let as_member = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/organizations")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {member}"))
                    .body(Body::from(json!({"name": "Yaounde NGO"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(as_member.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn creating_an_organization_auto_links_a_consumer_usable_for_notifications() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let platform_admin = login_reviewer(app.clone()).await;

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/organizations")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::from(json!({"name": "Douala Police"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let body = json_body(create_response).await;
        let consumer_id = body["consumer_id"]
            .as_str()
            .expect("a newly created organization is linked to a consumer")
            .to_owned();

        // The linked consumer is a real, usable consumer: its own alert
        // subscription and delivery preference can be set through the
        // existing endpoints, exactly as for any other consumer.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/v1/consumers/{consumer_id}/delivery-preference"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::from(
                        json!({
                            "strategy": "ALL",
                            "channels": [{"channel": "WHATSAPP", "address": "+237600000000"}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/subscriptions")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::from(
                        json!({
                            "consumer_id": consumer_id,
                            "rules": [{"rule": "INCIDENT_TYPE", "values": ["MISSING_CHILD"]}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn an_org_admin_may_not_manage_another_organizations_consumer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let platform_admin = login_reviewer(app.clone()).await;

        let (org_a, consumer_a) =
            create_organization_with_consumer(app.clone(), &platform_admin, "Douala Police").await;
        let (_org_b, consumer_b) =
            create_organization_with_consumer(app.clone(), &platform_admin, "Yaounde NGO").await;
        let org_a_admin =
            register_and_login(app.clone(), &platform_admin, "ORG_ADMIN", Some(&org_a)).await;

        fn set_delivery_preference_request(consumer_id: &str, token: &str) -> Request<Body> {
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/consumers/{consumer_id}/delivery-preference"))
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::from(
                    json!({
                        "strategy": "ALL",
                        "channels": [{"channel": "WHATSAPP", "address": "+237600000000"}]
                    })
                    .to_string(),
                ))
                .unwrap()
        }

        // Org A's admin may manage org A's own consumer...
        let response = app
            .clone()
            .oneshot(set_delivery_preference_request(&consumer_a, &org_a_admin))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // ...but not org B's.
        let response = app
            .clone()
            .oneshot(set_delivery_preference_request(&consumer_b, &org_a_admin))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/consumers/{consumer_b}/delivery-preference"))
                    .header("Authorization", format!("Bearer {org_a_admin}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/subscriptions")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {org_a_admin}"))
                    .body(Body::from(
                        json!({
                            "consumer_id": consumer_b,
                            "rules": [{"rule": "INCIDENT_TYPE", "values": ["MISSING_CHILD"]}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        // The platform admin may still manage org B's consumer regardless.
        let response = app
            .oneshot(set_delivery_preference_request(
                &consumer_b,
                &platform_admin,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_platform_admin_creates_an_organization_and_sets_its_trust_grants() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let platform_admin = login_reviewer(app.clone()).await;
        let org_id = create_organization(app.clone(), &platform_admin, "Douala Police").await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/v1/organizations/{org_id}/trust"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::from(
                        json!({
                            "verified_incident_types": ["MISSING_CHILD"],
                            "verified_alert_visibilities": ["COMMUNITY"]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["verified_incident_types"], json!(["MISSING_CHILD"]));
        assert_eq!(body["verified_alert_visibilities"], json!(["COMMUNITY"]));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/organizations/{org_id}"))
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            json_body(response).await["verified_incident_types"],
            json!(["MISSING_CHILD"])
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn creating_an_organization_accepts_an_optional_description_and_location() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let platform_admin = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/organizations")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::from(
                        json!({
                            "name": "Douala Police",
                            "description": "Municipal police unit",
                            "location": "Douala, Cameroon"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = json_body(response).await;
        assert_eq!(body["description"], json!("Municipal police unit"));
        assert_eq!(body["location"], json!("Douala, Cameroon"));
        assert_eq!(body["is_active"], json!(true));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_platform_admin_updates_an_organizations_profile() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let platform_admin = login_reviewer(app.clone()).await;
        let org_id = create_organization(app.clone(), &platform_admin, "Douala Police").await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/v1/organizations/{org_id}/profile"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::from(
                        json!({
                            "description": "Municipal police unit",
                            "location": "Douala, Cameroon"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["description"], json!("Municipal police unit"));
        assert_eq!(body["location"], json!("Douala, Cameroon"));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_platform_admin_deactivates_and_reactivates_an_organization() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let platform_admin = login_reviewer(app.clone()).await;
        let org_id = create_organization(app.clone(), &platform_admin, "Douala Police").await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/organizations/{org_id}/deactivate"))
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await["is_active"], json!(false));

        // A deactivated organization cannot register new members.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::from(
                        json!({
                            "email": "member@douala-police.example",
                            "role": "MEMBER",
                            "organization_id": org_id
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("ORGANIZATION_INACTIVE")
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/organizations/{org_id}/reactivate"))
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await["is_active"], json!(true));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn registering_an_org_admin_sends_an_invite_email() {
        let pool = test_pool().await;
        let invite_mailer = Arc::new(FakeInviteMailer::default());
        let app = build_router(test_state_with_invite_mailer(
            pool,
            true,
            invite_mailer.clone(),
        ));
        let platform_admin = login_reviewer(app.clone()).await;
        let org_id = create_organization(app.clone(), &platform_admin, "Douala Police").await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::from(
                        json!({
                            "email": "admin@douala-police.example",
                            "role": "ORG_ADMIN",
                            "organization_id": org_id
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let sent = invite_mailer.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, "admin@douala-police.example");
        assert_eq!(sent[0].2, Some("Douala Police".to_owned()));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn an_org_admin_may_register_a_member_into_their_own_organization_but_no_further() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let platform_admin = login_reviewer(app.clone()).await;
        let org_id = create_organization(app.clone(), &platform_admin, "Douala Police").await;
        let other_org_id = create_organization(app.clone(), &platform_admin, "Yaounde NGO").await;
        let org_admin =
            register_and_login(app.clone(), &platform_admin, "ORG_ADMIN", Some(&org_id)).await;

        // May register a Member into their own organization.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {org_admin}"))
                    .body(Body::from(
                        json!({"email": "member@example.test", "role": "MEMBER", "organization_id": org_id})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        // May not register a Member into a different organization.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {org_admin}"))
                    .body(Body::from(
                        json!({"email": "elsewhere@example.test", "role": "MEMBER", "organization_id": other_org_id})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        // May not register another OrgAdmin, even into their own organization.
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {org_admin}"))
                    .body(Body::from(
                        json!({"email": "co-admin@example.test", "role": "ORG_ADMIN", "organization_id": org_id})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn verifying_a_case_requires_the_organizations_incident_type_trust() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let platform_admin = login_reviewer(app.clone()).await;
        let org_id = create_organization(app.clone(), &platform_admin, "Douala Police").await;
        let member =
            register_and_login(app.clone(), &platform_admin, "MEMBER", Some(&org_id)).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"content": "My child has not returned from school."}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let report_id = json_body(response).await["report_id"]
            .as_str()
            .unwrap()
            .to_owned();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/cases")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {member}"))
                    .body(Body::from(
                        json!({"report_id": report_id, "incident_type": "MISSING_CHILD"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let case_id = json_body(response).await["case_id"]
            .as_str()
            .unwrap()
            .to_owned();

        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/events"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {member}"))
                    .body(Body::from(json!({"to": "UNDER_REVIEW"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // The organization has not been trusted for MISSING_CHILD yet.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/verify"))
                    .header("Authorization", format!("Bearer {member}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("ORGANIZATION_NOT_TRUSTED_FOR_INCIDENT_TYPE")
        );

        // Once the platform admin grants that trust, verification succeeds.
        app.clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/v1/organizations/{org_id}/trust"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::from(
                        json!({"verified_incident_types": ["MISSING_CHILD"], "verified_alert_visibilities": []})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/verify"))
                    .header("Authorization", format!("Bearer {member}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await["status"], json!("VERIFIED"));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn creating_a_community_alert_requires_the_organizations_visibility_trust() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let platform_admin = login_reviewer(app.clone()).await;
        let org_id = create_organization(app.clone(), &platform_admin, "Douala Police").await;
        app.clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/v1/organizations/{org_id}/trust"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::from(
                        json!({"verified_incident_types": ["MISSING_CHILD"], "verified_alert_visibilities": []})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let member =
            register_and_login(app.clone(), &platform_admin, "MEMBER", Some(&org_id)).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"content": "My child has not returned from school."}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let report_id = json_body(response).await["report_id"]
            .as_str()
            .unwrap()
            .to_owned();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/cases")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {member}"))
                    .body(Body::from(
                        json!({"report_id": report_id, "incident_type": "MISSING_CHILD"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let case_id = json_body(response).await["case_id"]
            .as_str()
            .unwrap()
            .to_owned();

        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/events"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {member}"))
                    .body(Body::from(json!({"to": "UNDER_REVIEW"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/cases/{case_id}/verify"))
                    .header("Authorization", format!("Bearer {member}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let create_alert_request = || {
            Request::builder()
                .method("POST")
                .uri(format!("/v1/cases/{case_id}/alerts"))
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {member}"))
                .body(Body::from(
                    json!({
                        "policy_id": "MISSING_CHILD_COMMUNITY",
                        "severity": "HIGH",
                        "target_geography": "Douala",
                        "fields": []
                    })
                    .to_string(),
                ))
                .unwrap()
        };

        // The organization is trusted to verify MISSING_CHILD but not yet to
        // issue COMMUNITY alerts.
        let response = app.clone().oneshot(create_alert_request()).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("ORGANIZATION_NOT_TRUSTED_FOR_ALERT_VISIBILITY")
        );

        app.clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/v1/organizations/{org_id}/trust"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::from(
                        json!({"verified_incident_types": ["MISSING_CHILD"], "verified_alert_visibilities": ["COMMUNITY"]})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response = app.oneshot(create_alert_request()).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_platform_admin_lists_an_organizations_members() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let platform_admin = login_reviewer(app.clone()).await;
        let org_id = create_organization(app.clone(), &platform_admin, "Douala Police").await;
        register_and_login(app.clone(), &platform_admin, "MEMBER", Some(&org_id)).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/organizations/{org_id}/members"))
                    .header("Authorization", format!("Bearer {platform_admin}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let members = json_body(response).await;
        let members = members.as_array().unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0]["role"], json!("MEMBER"));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn an_org_admin_manages_their_own_organization_but_not_another() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let platform_admin = login_reviewer(app.clone()).await;
        let douala_org = create_organization(app.clone(), &platform_admin, "Douala Police").await;
        let yaounde_org =
            create_organization(app.clone(), &platform_admin, "Yaounde Association").await;
        let douala_org_admin =
            register_and_login(app.clone(), &platform_admin, "ORG_ADMIN", Some(&douala_org)).await;
        register_and_login(app.clone(), &platform_admin, "MEMBER", Some(&douala_org)).await;

        // The org admin may view their own organization's detail...
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/organizations/{douala_org}"))
                    .header("Authorization", format!("Bearer {douala_org_admin}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // ...and its member list...
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/organizations/{douala_org}/members"))
                    .header("Authorization", format!("Bearer {douala_org_admin}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let members = json_body(response).await;
        assert_eq!(members.as_array().unwrap().len(), 2);

        // ...but not another organization's detail or members.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/organizations/{yaounde_org}"))
                    .header("Authorization", format!("Bearer {douala_org_admin}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/organizations/{yaounde_org}/members"))
                    .header("Authorization", format!("Bearer {douala_org_admin}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_plain_member_may_not_view_organization_detail_or_members() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let platform_admin = login_reviewer(app.clone()).await;
        let org_id = create_organization(app.clone(), &platform_admin, "Douala Police").await;
        let member =
            register_and_login(app.clone(), &platform_admin, "MEMBER", Some(&org_id)).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/organizations/{org_id}"))
                    .header("Authorization", format!("Bearer {member}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/organizations/{org_id}/members"))
                    .header("Authorization", format!("Bearer {member}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn creating_a_subscription_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/subscriptions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "consumer_id": Uuid::new_v4(),
                            "rules": [{"rule": "INCIDENT_TYPE", "values": ["MISSING_CHILD"]}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_creates_a_subscription_and_lists_it_for_its_consumer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let consumer_id = Uuid::new_v4();
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/subscriptions")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(
                        json!({
                            "consumer_id": consumer_id,
                            "rules": [
                                {"rule": "INCIDENT_TYPE", "values": ["MISSING_CHILD"]},
                                {"rule": "SEVERITY", "operator": "GREATER_THAN_OR_EQUAL", "value": "HIGH"},
                                {"rule": "GEOGRAPHY", "areas": ["Douala"]}
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = json_body(response).await;
        assert_eq!(body["consumer_id"], json!(consumer_id));
        assert_eq!(body["version"], json!(1));
        assert_eq!(body["rules"].as_array().unwrap().len(), 3);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/consumers/{consumer_id}/subscriptions"))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let listed = json_body(response).await;
        assert_eq!(listed.as_array().unwrap().len(), 1);
        assert_eq!(listed[0]["subscription_id"], body["subscription_id"]);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn creating_a_subscription_with_no_rules_is_rejected() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let token = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/subscriptions")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::from(
                        json!({"consumer_id": Uuid::new_v4(), "rules": []}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("EMPTY_SUBSCRIPTION_RULES")
        );
    }

    fn push_subscription_json(endpoint: &str) -> serde_json::Value {
        json!({
            "endpoint": endpoint,
            "keys": {"p256dh": "test-p256dh-key", "auth": "test-auth-secret"}
        })
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_citizen_creates_reads_updates_and_cancels_a_push_subscription() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/citizen-subscriptions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "incident_types": ["MISSING_CHILD"],
                            "minimum_severity": "HIGH",
                            "geography": ["Douala"],
                            "push_subscription": push_subscription_json("https://push.example/a"),
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let created = json_body(create_response).await;
        let subscription_id = created["subscription_id"].as_str().unwrap().to_owned();
        let token = created["management_token"].as_str().unwrap().to_owned();

        // No token at all is rejected the same way as a wrong one.
        let unauthorized = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/citizen-subscriptions/{subscription_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::NOT_FOUND);

        let get_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/citizen-subscriptions/{subscription_id}"))
                    .header("Management-Token", token.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get_response.status(), StatusCode::OK);
        let fetched = json_body(get_response).await;
        assert_eq!(fetched["incident_types"], json!(["MISSING_CHILD"]));
        assert_eq!(fetched["minimum_severity"], json!("HIGH"));
        assert_eq!(fetched["geography"], json!(["Douala"]));

        let update_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/v1/citizen-subscriptions/{subscription_id}"))
                    .header("content-type", "application/json")
                    .header("Management-Token", token.clone())
                    .body(Body::from(
                        json!({
                            "incident_types": ["MISSING_CHILD", "OTHER_PROTECTION_INCIDENT"],
                            "minimum_severity": "CRITICAL",
                            "geography": ["Yaounde"],
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(update_response.status(), StatusCode::OK);
        let updated = json_body(update_response).await;
        assert_eq!(updated["minimum_severity"], json!("CRITICAL"));
        assert_eq!(updated["geography"], json!(["Yaounde"]));

        let cancel_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/v1/citizen-subscriptions/{subscription_id}/cancel"
                    ))
                    .header("Management-Token", token.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cancel_response.status(), StatusCode::NO_CONTENT);

        // Cancelled -- even the right token no longer finds it.
        let after_cancel = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/citizen-subscriptions/{subscription_id}"))
                    .header("Management-Token", token)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(after_cancel.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_wrong_management_token_is_rejected_the_same_way_as_no_such_subscription() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/citizen-subscriptions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "incident_types": ["MISSING_CHILD"],
                            "minimum_severity": "HIGH",
                            "geography": ["Douala"],
                            "push_subscription": push_subscription_json("https://push.example/b"),
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let subscription_id = json_body(create_response).await["subscription_id"]
            .as_str()
            .unwrap()
            .to_owned();

        let wrong_token_response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/citizen-subscriptions/{subscription_id}"))
                    .header("Management-Token", "not-the-real-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(wrong_token_response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            json_body(wrong_token_response).await["error"]["code"],
            json!("CITIZEN_SUBSCRIPTION_NOT_FOUND")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn creating_a_citizen_subscription_with_no_incident_types_is_rejected() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/citizen-subscriptions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "incident_types": [],
                            "minimum_severity": "HIGH",
                            "geography": ["Douala"],
                            "push_subscription": push_subscription_json("https://push.example/c"),
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("EMPTY_INCIDENT_TYPES")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn exceeding_the_citizen_subscription_rate_limit_returns_too_many_requests() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let submit = |app: Router, endpoint: String| {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/citizen-subscriptions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "incident_types": ["MISSING_CHILD"],
                            "minimum_severity": "HIGH",
                            "geography": ["Douala"],
                            "push_subscription": push_subscription_json(&endpoint),
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
        };

        for attempt_number in 1..=10 {
            let response = submit(
                app.clone(),
                format!("https://push.example/limit-{attempt_number}"),
            )
            .await
            .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::CREATED,
                "attempt {attempt_number} should still be within the limit"
            );
        }

        let throttled = submit(app, "https://push.example/limit-11".into())
            .await
            .unwrap();
        assert_eq!(throttled.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            json_body(throttled).await["error"]["code"],
            json!("RATE_LIMIT_EXCEEDED")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_updates_a_subscriptions_rules_and_its_version_increments() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let token = login_reviewer(app.clone()).await;

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/subscriptions")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::from(
                        json!({
                            "consumer_id": Uuid::new_v4(),
                            "rules": [{"rule": "INCIDENT_TYPE", "values": ["MISSING_CHILD"]}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let created = json_body(create_response).await;
        let subscription_id = created["subscription_id"].as_str().unwrap().to_owned();
        assert_eq!(created["version"], json!(1));

        let update_response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/v1/subscriptions/{subscription_id}"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::from(
                        json!({
                            "rules": [{"rule": "GEOGRAPHY", "areas": ["Douala"]}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(update_response.status(), StatusCode::OK);
        let updated = json_body(update_response).await;
        assert_eq!(updated["subscription_id"], created["subscription_id"]);
        assert_eq!(updated["version"], json!(2));
        assert_eq!(
            updated["rules"],
            json!([{"rule": "GEOGRAPHY", "areas": ["Douala"]}])
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn updating_an_unknown_subscription_returns_not_found() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let token = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/v1/subscriptions/{}", Uuid::new_v4()))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::from(
                        json!({"rules": [{"rule": "GEOGRAPHY", "areas": ["Douala"]}]}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn updating_a_subscription_requires_an_identified_reviewer() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/v1/subscriptions/{}", Uuid::new_v4()))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"rules": [{"rule": "GEOGRAPHY", "areas": ["Douala"]}]}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_reviewer_sets_and_reads_back_a_consumers_delivery_preference() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let consumer_id = Uuid::new_v4();
        let reviewer = login_reviewer(app.clone()).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/v1/consumers/{consumer_id}/delivery-preference"))
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::from(
                        json!({
                            "strategy": "PRIMARY_FALLBACK",
                            "channels": [
                                {"channel": "WHATSAPP", "address": "+237600000000"},
                                {"channel": "SMS", "address": "+237600000001"}
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["strategy"], json!("PRIMARY_FALLBACK"));
        assert_eq!(body["channels"].as_array().unwrap().len(), 2);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/consumers/{consumer_id}/delivery-preference"))
                    .header("Authorization", format!("Bearer {reviewer}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await, body);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn reading_an_unset_delivery_preference_returns_not_found() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let token = login_reviewer(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/v1/consumers/{}/delivery-preference",
                        Uuid::new_v4()
                    ))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
