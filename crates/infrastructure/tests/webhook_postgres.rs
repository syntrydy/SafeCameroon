use std::collections::HashMap;
use std::env;

use safe_cameroon_application::alert_workflow::create_alert_from_case;
use safe_cameroon_application::case_workflow::{Actor, create_case_from_report, review_case};
use safe_cameroon_application::delivery_workflow::{
    WebhookAppliedTransition, apply_webhook_event, plan_deliveries, record_delivery_success,
    start_delivery_attempt,
};
use safe_cameroon_application::prepare_anonymous_report;
use safe_cameroon_application::webhook::{
    ProviderDeliveryStatus, WebhookEvent, WebhookReplayGuard,
};
use safe_cameroon_domain::{
    AlertField, AlertFieldValue, AlertPolicy, CaseStatus, ChannelEndpoint, ChannelType, ConsumerId,
    ConsumerMatch, DeliveryPreference, DeliveryStatus, DeliveryStrategy, IncidentType,
    MatchedSubscription, ReportId, RetryPolicy, Severity, SubscriptionId, TargetGeography,
};
use safe_cameroon_infrastructure::postgres::{
    PostgresAlertRepository, PostgresCaseRepository, PostgresDeliveryRepository,
    PostgresReportRepository,
};
use safe_cameroon_infrastructure::webhook::PostgresWebhookReplayGuard;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

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

    MIGRATOR.run(&pool).await.expect("migrations must apply");
    sqlx::query(
        "TRUNCATE attachments, webhook_replay_events, delivery_events, delivery_attempts, deliveries, \
         alert_events, alert_fields, alerts, case_events, case_reports, cases, outbox_events, \
         audit_events, reports, reporters",
    )
    .execute(&pool)
    .await
    .expect("test tables must be reset");
    pool
}

async fn verified_alert(pool: &PgPool) -> safe_cameroon_domain::Alert {
    let report_repository = PostgresReportRepository::new(pool.clone());
    let submission =
        prepare_anonymous_report("A child is missing.".into(), Uuid::new_v4(), None).unwrap();
    report_repository
        .submit_anonymous(&submission)
        .await
        .unwrap();

    let case_repository = PostgresCaseRepository::new(pool.clone());
    let creation = create_case_from_report(
        IncidentType::MissingChild,
        ReportId::from_uuid(submission.report.id.as_uuid()),
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
    );
    case_repository.create(&creation).await.unwrap();

    let mut case = case_repository
        .find_by_id(creation.case.id())
        .await
        .unwrap()
        .unwrap();
    for target in [CaseStatus::UnderReview, CaseStatus::Verified] {
        let review = review_case(
            &mut case,
            Actor::Reviewer(Uuid::new_v4()),
            target,
            Uuid::new_v4(),
        )
        .unwrap();
        case_repository.apply_review(&case, &review).await.unwrap();
    }
    let case = case_repository
        .find_by_id(case.id())
        .await
        .unwrap()
        .unwrap();

    let alert_repository = PostgresAlertRepository::new(pool.clone());
    let policy = AlertPolicy::missing_child_community_v1();
    let alert_creation = create_alert_from_case(
        &case,
        &policy,
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
    alert_repository.create(&alert_creation).await.unwrap();
    alert_creation.alert
}

async fn plan_and_persist_one(
    delivery_repository: &PostgresDeliveryRepository,
    alert: &safe_cameroon_domain::Alert,
    channel: ChannelType,
) -> safe_cameroon_domain::DeliveryId {
    let consumer_id = ConsumerId::new();
    let preference = DeliveryPreference::new(
        DeliveryStrategy::All,
        vec![ChannelEndpoint::new(channel, "+237600000000").unwrap()],
    )
    .unwrap();
    let mut preferences = HashMap::new();
    preferences.insert(consumer_id, preference);
    let consumer_matches = vec![ConsumerMatch {
        consumer_id,
        matching_subscriptions: vec![MatchedSubscription {
            subscription_id: SubscriptionId::new(),
            subscription_version: 1,
        }],
    }];
    let planned = plan_deliveries(
        alert,
        &consumer_matches,
        &preferences,
        RetryPolicy::standard(),
        Actor::Automated,
        Uuid::new_v4(),
    );
    delivery_repository.create_planned(&planned).await.unwrap();
    planned[0].delivery.id()
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn finds_a_delivery_by_the_provider_message_id_it_was_sent_with() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let deliveries = PostgresDeliveryRepository::new(pool.clone());
    let delivery_id = plan_and_persist_one(&deliveries, &alert, ChannelType::WhatsApp).await;

    let mut delivery = deliveries.find_by_id(delivery_id).await.unwrap().unwrap();
    let started = start_delivery_attempt(&mut delivery, Actor::Automated, Uuid::new_v4()).unwrap();
    deliveries
        .apply_transition(&delivery, &started)
        .await
        .unwrap();
    let succeeded = record_delivery_success(
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

    let found = deliveries
        .find_by_provider_message_id("wa-provider-msg-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.id(), delivery_id);

    assert!(
        deliveries
            .find_by_provider_message_id("no-such-id")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_verified_webhook_event_is_applied_and_a_replayed_one_is_rejected_by_the_guard() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let deliveries = PostgresDeliveryRepository::new(pool.clone());
    let delivery_id = plan_and_persist_one(&deliveries, &alert, ChannelType::WhatsApp).await;

    let mut delivery = deliveries.find_by_id(delivery_id).await.unwrap().unwrap();
    let started = start_delivery_attempt(&mut delivery, Actor::Automated, Uuid::new_v4()).unwrap();
    deliveries
        .apply_transition(&delivery, &started)
        .await
        .unwrap();
    let succeeded = record_delivery_success(
        &mut delivery,
        Some("wa-provider-msg-2".into()),
        Actor::Automated,
        Uuid::new_v4(),
    )
    .unwrap();
    deliveries
        .apply_attempt_transition(&delivery, &succeeded)
        .await
        .unwrap();

    let replay_guard = PostgresWebhookReplayGuard::new(pool.clone());
    let event = WebhookEvent {
        provider_event_id: "evt-1".into(),
        provider_message_id: "wa-provider-msg-2".into(),
        status: ProviderDeliveryStatus::Delivered,
    };

    assert!(
        replay_guard
            .record_if_new(ChannelType::WhatsApp, &event.provider_event_id)
            .await
            .unwrap(),
        "the first occurrence of this event id must be accepted"
    );

    let mut delivery = deliveries
        .find_by_provider_message_id(&event.provider_message_id)
        .await
        .unwrap()
        .unwrap();
    let applied =
        apply_webhook_event(&mut delivery, &event, Actor::Automated, Uuid::new_v4()).unwrap();
    let WebhookAppliedTransition::Delivered(transition) = applied else {
        panic!("expected a Delivered transition");
    };
    deliveries
        .apply_transition(&delivery, &transition)
        .await
        .unwrap();

    let reloaded = deliveries.find_by_id(delivery_id).await.unwrap().unwrap();
    assert_eq!(reloaded.status(), DeliveryStatus::Delivered);

    // A replay of the same provider event id must be rejected by the guard
    // before it ever reaches delivery_workflow again.
    assert!(
        !replay_guard
            .record_if_new(ChannelType::WhatsApp, &event.provider_event_id)
            .await
            .unwrap(),
        "a replayed event id must be rejected"
    );
}
