mod alerts;
mod attachments;
mod auth;
mod cases;
mod error;
mod health;
mod rate_limit;
mod reports;
mod request_id;
mod reviewer;
mod source_key;
mod state;
mod subscriptions;
mod webhooks;

use std::sync::Arc;

use axum::http::HeaderName;
use axum::{
    Router,
    routing::{get, post, put},
};
use safe_cameroon_application::webhook::WebhookVerifierRegistry;
use safe_cameroon_domain::ChannelType;
use safe_cameroon_infrastructure::auth::ReviewerSessionTokenIssuer;
use safe_cameroon_infrastructure::postgres::{
    PostgresAlertRepository, PostgresAttachmentRepository, PostgresCaseRepository,
    PostgresDeliveryPreferenceRepository, PostgresDeliveryRepository, PostgresRateLimiter,
    PostgresReportRepository, PostgresReviewerRepository, PostgresSubscriptionRepository,
};
use safe_cameroon_infrastructure::storage::HmacSignedAttachmentStorage;
use safe_cameroon_infrastructure::webhook::{
    HmacSignedWebhookVerifier, PostgresWebhookReplayGuard,
};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
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

fn build_state(
    pool: PgPool,
    webhook_secret: Vec<u8>,
    attachment_storage_secret: Vec<u8>,
    attachment_storage_base_url: String,
    reviewer_session_secret: Vec<u8>,
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
        subscriptions: PostgresSubscriptionRepository::new(pool.clone()),
        delivery_preferences: PostgresDeliveryPreferenceRepository::new(pool.clone()),
        reviewers: PostgresReviewerRepository::new(pool.clone()),
        reviewer_session_tokens: ReviewerSessionTokenIssuer::new(reviewer_session_secret),
        rate_limiter: Arc::new(PostgresRateLimiter::new(pool.clone())),
        attachment_storage: Arc::new(HmacSignedAttachmentStorage::new(
            attachment_storage_base_url,
            attachment_storage_secret,
        )),
        webhook_verifiers: Arc::new(webhook_verifiers),
        webhook_replay_guard: Arc::new(PostgresWebhookReplayGuard::new(pool)),
    }
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/v1/auth/register", post(auth::register))
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/logout", post(auth::logout))
        .route("/v1/reports", post(reports::create_anonymous_report))
        .route(
            "/v1/reports/{report_id}/attachments",
            post(attachments::create_attachment),
        )
        .route(
            "/v1/attachments/{id}/download-url",
            get(attachments::create_download_url),
        )
        .route("/v1/cases", post(cases::create_case))
        .route("/v1/cases/{id}", get(cases::get_case))
        .route("/v1/cases/{id}/reports", post(cases::link_report))
        .route("/v1/cases/{id}/events", post(cases::create_case_event))
        .route("/v1/cases/{id}/verify", post(cases::verify_case))
        .route("/v1/cases/{id}/resolve", post(cases::resolve_case))
        .route("/v1/cases/{id}/alerts", post(alerts::create_alert))
        .route("/v1/alerts/{id}", get(alerts::get_alert))
        .route("/v1/alerts/{id}/cancel", post(alerts::cancel))
        .route(
            "/v1/subscriptions",
            post(subscriptions::create_subscription),
        )
        .route(
            "/v1/subscriptions/{id}",
            put(subscriptions::update_subscription),
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
        // time TraceLayer builds its span or any handler runs.
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
    let attachment_storage_secret = std::env::var("ATTACHMENT_STORAGE_SECRET")
        .expect("ATTACHMENT_STORAGE_SECRET must be configured")
        .into_bytes();
    let attachment_storage_base_url = std::env::var("ATTACHMENT_STORAGE_BASE_URL")
        .unwrap_or_else(|_| "https://storage.sandbox.local".to_owned());
    let reviewer_session_secret = std::env::var("REVIEWER_SESSION_SECRET")
        .expect("REVIEWER_SESSION_SECRET must be configured")
        .into_bytes();
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("database connection must succeed");

    let app = build_router(build_state(
        pool,
        webhook_secret,
        attachment_storage_secret,
        attachment_storage_base_url,
        reviewer_session_secret,
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
    use safe_cameroon_application::alert_workflow::create_alert_from_case;
    use safe_cameroon_application::case_workflow::{Actor, create_case_from_report, review_case};
    use safe_cameroon_application::channel::{Channel, build_outbound_message};
    use safe_cameroon_application::delivery_workflow::{plan_deliveries, start_delivery_attempt};
    use safe_cameroon_application::prepare_anonymous_report;
    use safe_cameroon_domain::{
        AlertField, AlertFieldValue, AlertPolicy, CaseStatus, ChannelEndpoint, ConsumerId,
        ConsumerMatch, DeliveryPreference, DeliveryStatus, DeliveryStrategy, IncidentType,
        MatchedSubscription, ReportId, RetryPolicy, Severity, SubscriptionId, TargetGeography,
        deduplicate_by_consumer, evaluate_subscriptions,
    };
    use safe_cameroon_infrastructure::channels::WhatsAppChannel;
    use serde_json::json;
    use sha2::Sha256;
    use tower::ServiceExt;
    use uuid::Uuid;

    type HmacSha256 = Hmac<Sha256>;
    const TEST_SECRET: &[u8] = b"test-webhook-secret";

    fn test_state(pool: PgPool) -> AppState {
        build_state(
            pool,
            TEST_SECRET.to_vec(),
            b"test-attachment-storage-secret".to_vec(),
            "https://storage.example".to_owned(),
            b"test-reviewer-session-secret".to_vec(),
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
            "TRUNCATE attachments, webhook_replay_events, delivery_events, delivery_attempts, \
             deliveries, alert_events, alert_fields, alerts, case_events, case_reports, cases, \
             outbox_events, audit_events, reports, reporters, consumer_delivery_preferences, \
             subscriptions, reviewers, rate_limit_windows",
        )
        .execute(&pool)
        .await
        .expect("test tables must be reset");
        pool
    }

    async fn seeded_delivery(pool: &PgPool) -> safe_cameroon_domain::DeliveryId {
        let reports = PostgresReportRepository::new(pool.clone());
        let submission =
            prepare_anonymous_report("A child is missing.".into(), Uuid::new_v4(), None).unwrap();
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
            prepare_anonymous_report("A child is missing.".into(), Uuid::new_v4(), None).unwrap();
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

    /// Registers a fresh reviewer and logs in, returning a bearer session
    /// token. Relies on the reviewers table being empty at the start of each
    /// test (`test_pool`'s `TRUNCATE`), so registration is always the
    /// deployment's bootstrapping first-reviewer case and needs no
    /// authorization of its own.
    async fn login_reviewer(app: Router) -> String {
        let email = format!("reviewer-{}@example.test", Uuid::new_v4());
        let password = "correct-horse-battery-staple";

        let register_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": email, "password": password}).to_string(),
                    ))
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
                    .uri("/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": email, "password": password}).to_string(),
                    ))
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
                        json!({"email": "second@example.test", "password": "correct-horse-battery-staple"})
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
                        json!({"email": "second@example.test", "password": "correct-horse-battery-staple"})
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
                        json!({"email": email, "password": "correct-horse-battery-staple"})
                            .to_string(),
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
    async fn registering_with_a_short_password_is_rejected() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": "short@example.test", "password": "short1"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("WEAK_PASSWORD")
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn login_rejects_a_wrong_password_and_an_unknown_email_identically() {
        let pool = test_pool().await;
        let app = build_router(test_state(pool));
        let email = "reviewer@example.test";

        let register = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": email, "password": "correct-horse-battery-staple"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(register.status(), StatusCode::CREATED);

        let wrong_password = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": email, "password": "not the right password"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(wrong_password.status(), StatusCode::UNAUTHORIZED);
        let wrong_password_code = json_body(wrong_password).await["error"]["code"].clone();

        let unknown_email = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": "nobody@example.test", "password": "whatever-password"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unknown_email.status(), StatusCode::UNAUTHORIZED);
        let unknown_email_code = json_body(unknown_email).await["error"]["code"].clone();

        assert_eq!(wrong_password_code, json!("INVALID_CREDENTIALS"));
        assert_eq!(
            wrong_password_code, unknown_email_code,
            "the two failure cases must be indistinguishable to the caller"
        );
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
        let password = "correct-horse-battery-staple";

        let register_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": email, "password": password}).to_string(),
                    ))
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

        let failed_login = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": email, "password": "wrong password"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(failed_login.status(), StatusCode::UNAUTHORIZED);

        let (login_failed_count,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM audit_events WHERE action = 'REVIEWER_LOGIN_FAILED' \
             AND resource_id = $1 AND metadata->>'email' = $2",
        )
        .bind(Uuid::parse_str(&reviewer_id).unwrap())
        .bind(email)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(login_failed_count, 1);

        let login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": email, "password": password}).to_string(),
                    ))
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
        let email = "throttle-target@example.test";
        let attempt = |app: Router| {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": email, "password": "wrong password"}).to_string(),
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
                                {"rule": "GEOGRAPHY", "area": "Douala"}
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
                                {"rule": "GEOGRAPHY", "area": "Douala"}
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
                            "rules": [{"rule": "GEOGRAPHY", "area": "Douala"}]
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
            json!([{"rule": "GEOGRAPHY", "area": "Douala"}])
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
                        json!({"rules": [{"rule": "GEOGRAPHY", "area": "Douala"}]}).to_string(),
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
                        json!({"rules": [{"rule": "GEOGRAPHY", "area": "Douala"}]}).to_string(),
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
