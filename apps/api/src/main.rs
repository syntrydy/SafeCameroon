mod alerts;
mod cases;
mod error;
mod health;
mod reports;
mod reviewer;
mod state;
mod webhooks;

use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};
use safe_cameroon_application::webhook::WebhookVerifierRegistry;
use safe_cameroon_domain::ChannelType;
use safe_cameroon_infrastructure::postgres::{
    PostgresAlertRepository, PostgresCaseRepository, PostgresDeliveryRepository,
    PostgresReportRepository,
};
use safe_cameroon_infrastructure::webhook::{
    HmacSignedWebhookVerifier, PostgresWebhookReplayGuard,
};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::state::AppState;

/// The only webhook provider registered per channel today (every mock/
/// sandbox channel emits the same payload shape); a real provider
/// integration adds its own name here rather than replacing this one.
const SANDBOX_WEBHOOK_PROVIDER: &str = "sandbox";

fn build_state(pool: PgPool, webhook_secret: Vec<u8>) -> AppState {
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
        webhook_verifiers: Arc::new(webhook_verifiers),
        webhook_replay_guard: Arc::new(PostgresWebhookReplayGuard::new(pool)),
    }
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/v1/reports", post(reports::create_anonymous_report))
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
            "/v1/webhooks/{channel}/{provider}",
            post(webhooks::receive_webhook),
        )
        .with_state(state)
}

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be configured");
    let webhook_secret = std::env::var("WEBHOOK_SHARED_SECRET")
        .expect("WEBHOOK_SHARED_SECRET must be configured")
        .into_bytes();
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("database connection must succeed");

    let app = build_router(build_state(pool, webhook_secret));

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
    use safe_cameroon_application::delivery_workflow::{plan_deliveries, start_delivery_attempt};
    use safe_cameroon_application::prepare_anonymous_report;
    use safe_cameroon_domain::{
        AlertField, AlertFieldValue, AlertPolicy, CaseStatus, ChannelEndpoint, ConsumerId,
        ConsumerMatch, DeliveryPreference, DeliveryStatus, DeliveryStrategy, IncidentType,
        ReportId, RetryPolicy, Severity, SubscriptionId, TargetGeography,
    };
    use serde_json::json;
    use sha2::Sha256;
    use tower::ServiceExt;
    use uuid::Uuid;

    type HmacSha256 = Hmac<Sha256>;
    const TEST_SECRET: &[u8] = b"test-webhook-secret";

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
            "TRUNCATE webhook_replay_events, delivery_events, delivery_attempts, deliveries, \
             alert_events, alert_fields, alerts, case_events, case_reports, cases, \
             outbox_events, audit_events, reports, reporters",
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
            matching_subscriptions: vec![SubscriptionId::new()],
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
        let app = build_router(build_state(pool.clone(), TEST_SECRET.to_vec()));

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
        let app = build_router(build_state(pool.clone(), TEST_SECRET.to_vec()));

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
        let app = build_router(build_state(pool.clone(), TEST_SECRET.to_vec()));

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
        let app = build_router(build_state(pool, TEST_SECRET.to_vec()));

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
}
