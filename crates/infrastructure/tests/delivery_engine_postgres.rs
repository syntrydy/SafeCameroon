use std::collections::HashMap;
use std::env;

use safe_cameroon_application::alert_workflow::create_alert_from_case;
use safe_cameroon_application::case_workflow::{Actor, create_case_from_report, review_case};
use safe_cameroon_application::delivery_workflow::{
    plan_deliveries, record_delivery_failure, record_delivery_success, start_delivery_attempt,
};
use safe_cameroon_application::prepare_anonymous_report;
use safe_cameroon_domain::{
    AlertField, AlertFieldValue, AlertPolicy, CaseStatus, ChannelEndpoint, ChannelType, ConsumerId,
    ConsumerMatch, DeliveryStatus, DeliveryStrategy, IncidentType, ReportId, RetryPolicy, Severity,
    SubscriptionId, TargetGeography,
};
use safe_cameroon_infrastructure::postgres::{
    DeliveryTransitionOutcome, PostgresAlertRepository, PostgresCaseRepository,
    PostgresDeliveryRepository, PostgresReportRepository,
};
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
        "TRUNCATE delivery_events, delivery_attempts, deliveries, alert_events, alert_fields, \
         alerts, case_events, case_reports, cases, outbox_events, audit_events, reports, reporters",
    )
    .execute(&pool)
    .await
    .expect("test tables must be reset");
    pool
}

/// Builds and persists a verified case, then a community alert from it, so
/// delivery tests have a legitimate alert to plan deliveries for.
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
    )
    .unwrap();
    alert_repository.create(&alert_creation).await.unwrap();
    alert_creation.alert
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn plans_and_persists_a_delivery_without_duplicating_it_on_replay() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let delivery_repository = PostgresDeliveryRepository::new(pool.clone());

    let consumer_id = ConsumerId::new();
    let preference = safe_cameroon_domain::DeliveryPreference::new(
        DeliveryStrategy::All,
        vec![ChannelEndpoint::new(ChannelType::WhatsApp, "+237600000000").unwrap()],
    )
    .unwrap();
    let mut preferences = HashMap::new();
    preferences.insert(consumer_id, preference);
    let consumer_matches = vec![ConsumerMatch {
        consumer_id,
        matching_subscriptions: vec![SubscriptionId::new()],
    }];

    let planned = plan_deliveries(
        &alert,
        &consumer_matches,
        &preferences,
        RetryPolicy::standard(),
        Actor::Automated,
        Uuid::new_v4(),
    );
    assert_eq!(planned.len(), 1);
    let delivery_id = planned[0].delivery.id();

    // A second planning pass for the same alert/consumer/channel/endpoint
    // (e.g. an at-least-once outbox consumer replaying DELIVERY_REQUESTED)
    // must not create a second row.
    delivery_repository.create_planned(&planned).await.unwrap();
    delivery_repository.create_planned(&planned).await.unwrap();

    let loaded = delivery_repository
        .find_by_id(delivery_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.status(), DeliveryStatus::Queued);
    assert_eq!(loaded.channel(), ChannelType::WhatsApp);

    let (delivery_count,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM deliveries WHERE alert_id = $1")
            .bind(alert.id().as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(delivery_count, 1);

    let (event_count,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM delivery_events WHERE delivery_id = $1 AND event_type = 'DELIVERY_REQUESTED'",
    )
    .bind(delivery_id.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_count, 1);

    let (outbox_count,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM outbox_events WHERE aggregate_id = $1 AND event_type = 'DELIVERY_REQUESTED'",
    )
    .bind(delivery_id.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(outbox_count, 1);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn drives_a_delivery_to_sent_and_rejects_a_concurrent_stale_transition() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let delivery_repository = PostgresDeliveryRepository::new(pool.clone());

    let consumer_id = ConsumerId::new();
    let preference = safe_cameroon_domain::DeliveryPreference::new(
        DeliveryStrategy::All,
        vec![ChannelEndpoint::new(ChannelType::WhatsApp, "+237600000000").unwrap()],
    )
    .unwrap();
    let mut preferences = HashMap::new();
    preferences.insert(consumer_id, preference);
    let consumer_matches = vec![ConsumerMatch {
        consumer_id,
        matching_subscriptions: vec![SubscriptionId::new()],
    }];
    let planned = plan_deliveries(
        &alert,
        &consumer_matches,
        &preferences,
        RetryPolicy::standard(),
        Actor::Automated,
        Uuid::new_v4(),
    );
    delivery_repository.create_planned(&planned).await.unwrap();
    let delivery_id = planned[0].delivery.id();

    // Two independent readers of the same row, simulating two worker
    // processes racing to pick up the same queued delivery.
    let mut first_worker = delivery_repository
        .find_by_id(delivery_id)
        .await
        .unwrap()
        .unwrap();
    let mut second_worker = delivery_repository
        .find_by_id(delivery_id)
        .await
        .unwrap()
        .unwrap();

    let first_start =
        start_delivery_attempt(&mut first_worker, Actor::Automated, Uuid::new_v4()).unwrap();
    assert_eq!(
        delivery_repository
            .apply_transition(&first_worker, &first_start)
            .await
            .unwrap(),
        DeliveryTransitionOutcome::Applied
    );

    let second_start =
        start_delivery_attempt(&mut second_worker, Actor::Automated, Uuid::new_v4()).unwrap();
    assert_eq!(
        delivery_repository
            .apply_transition(&second_worker, &second_start)
            .await
            .unwrap(),
        DeliveryTransitionOutcome::Conflict,
        "a second worker must not also start the same attempt"
    );

    let succeeded = record_delivery_success(
        &mut first_worker,
        Some("provider-msg-1".into()),
        Actor::Automated,
        Uuid::new_v4(),
    )
    .unwrap();
    assert_eq!(
        delivery_repository
            .apply_attempt_transition(&first_worker, &succeeded)
            .await
            .unwrap(),
        DeliveryTransitionOutcome::Applied
    );

    let reloaded = delivery_repository
        .find_by_id(delivery_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reloaded.status(), DeliveryStatus::Sent);
    assert_eq!(reloaded.attempt_count(), 1);

    let (attempt_count,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM delivery_attempts WHERE delivery_id = $1")
            .bind(delivery_id.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attempt_count, 1);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_non_retryable_failure_is_persisted_as_permanently_failed() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let delivery_repository = PostgresDeliveryRepository::new(pool.clone());

    let consumer_id = ConsumerId::new();
    let preference = safe_cameroon_domain::DeliveryPreference::new(
        DeliveryStrategy::All,
        vec![ChannelEndpoint::new(ChannelType::Sms, "+237600000000").unwrap()],
    )
    .unwrap();
    let mut preferences = HashMap::new();
    preferences.insert(consumer_id, preference);
    let consumer_matches = vec![ConsumerMatch {
        consumer_id,
        matching_subscriptions: vec![SubscriptionId::new()],
    }];
    let planned = plan_deliveries(
        &alert,
        &consumer_matches,
        &preferences,
        RetryPolicy::standard(),
        Actor::Automated,
        Uuid::new_v4(),
    );
    delivery_repository.create_planned(&planned).await.unwrap();
    let delivery_id = planned[0].delivery.id();

    let mut delivery = delivery_repository
        .find_by_id(delivery_id)
        .await
        .unwrap()
        .unwrap();
    let start = start_delivery_attempt(&mut delivery, Actor::Automated, Uuid::new_v4()).unwrap();
    delivery_repository
        .apply_transition(&delivery, &start)
        .await
        .unwrap();

    let failed = record_delivery_failure(
        &mut delivery,
        false,
        "invalid phone number",
        Actor::Automated,
        Uuid::new_v4(),
    )
    .unwrap();
    delivery_repository
        .apply_attempt_transition(&delivery, &failed)
        .await
        .unwrap();

    let reloaded = delivery_repository
        .find_by_id(delivery_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reloaded.status(), DeliveryStatus::FailedPermanently);

    let (retryable,): (Option<bool>,) = sqlx::query_as(
        "SELECT retryable FROM delivery_attempts WHERE delivery_id = $1 AND attempt_number = 1",
    )
    .bind(delivery_id.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(retryable, Some(false));
}
