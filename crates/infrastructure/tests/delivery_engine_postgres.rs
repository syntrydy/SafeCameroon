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
    ConsumerMatch, DeliveryStatus, DeliveryStrategy, IncidentType, MatchedSubscription, ReportId,
    RetryPolicy, Severity, SubscriptionId, TargetGeography,
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
        "TRUNCATE report_extractions, attachments, webhook_replay_events, delivery_events, delivery_attempts, deliveries, alert_events, alert_fields, \
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
        prepare_anonymous_report("A child is missing.".into(), Uuid::new_v4(), None, None).unwrap();
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
        None,
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
        matching_subscriptions: vec![MatchedSubscription {
            subscription_id: SubscriptionId::new(),
            subscription_version: 1,
        }],
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
        matching_subscriptions: vec![MatchedSubscription {
            subscription_id: SubscriptionId::new(),
            subscription_version: 1,
        }],
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
        matching_subscriptions: vec![MatchedSubscription {
            subscription_id: SubscriptionId::new(),
            subscription_version: 1,
        }],
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

/// Plans and persists one delivery for a fresh consumer on `alert`, returning
/// its id.
async fn plan_and_persist_one(
    delivery_repository: &PostgresDeliveryRepository,
    alert: &safe_cameroon_domain::Alert,
    channel: ChannelType,
) -> safe_cameroon_domain::DeliveryId {
    let consumer_id = ConsumerId::new();
    let preference = safe_cameroon_domain::DeliveryPreference::new(
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

/// Plans and persists one consumer's `PrimaryFallback` deliveries across two
/// channels, returning `(tier_0_id, tier_1_id)`.
async fn plan_and_persist_fallback(
    delivery_repository: &PostgresDeliveryRepository,
    alert: &safe_cameroon_domain::Alert,
    primary: ChannelType,
    fallback: ChannelType,
) -> (
    safe_cameroon_domain::DeliveryId,
    safe_cameroon_domain::DeliveryId,
) {
    let consumer_id = ConsumerId::new();
    let preference = safe_cameroon_domain::DeliveryPreference::new(
        DeliveryStrategy::PrimaryFallback,
        vec![
            ChannelEndpoint::new(primary, "+237600000000").unwrap(),
            ChannelEndpoint::new(fallback, "+237600000001").unwrap(),
        ],
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
    let tier_0 = planned
        .iter()
        .find(|p| p.delivery.tier() == 0)
        .expect("a tier-0 delivery must be planned")
        .delivery
        .id();
    let tier_1 = planned
        .iter()
        .find(|p| p.delivery.tier() == 1)
        .expect("a tier-1 delivery must be planned")
        .delivery
        .id();
    (tier_0, tier_1)
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_fallback_tier_is_not_claimed_while_its_primary_tier_is_still_pending() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let delivery_repository = PostgresDeliveryRepository::new(pool.clone());

    let (tier_0_id, _tier_1_id) = plan_and_persist_fallback(
        &delivery_repository,
        &alert,
        ChannelType::Sms,
        ChannelType::Email,
    )
    .await;

    let claimed = delivery_repository.claim_next(10).await.unwrap();

    assert_eq!(
        claimed.len(),
        1,
        "only the still-pending tier-0 delivery may be claimed"
    );
    assert_eq!(claimed[0].id(), tier_0_id);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_fallback_tier_stays_blocked_once_its_primary_tier_succeeds() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let delivery_repository = PostgresDeliveryRepository::new(pool.clone());

    let (tier_0_id, _tier_1_id) = plan_and_persist_fallback(
        &delivery_repository,
        &alert,
        ChannelType::Sms,
        ChannelType::Email,
    )
    .await;

    let mut tier_0 = delivery_repository
        .find_by_id(tier_0_id)
        .await
        .unwrap()
        .unwrap();
    let start = start_delivery_attempt(&mut tier_0, Actor::Automated, Uuid::new_v4()).unwrap();
    delivery_repository
        .apply_transition(&tier_0, &start)
        .await
        .unwrap();
    let succeeded =
        record_delivery_success(&mut tier_0, None, Actor::Automated, Uuid::new_v4()).unwrap();
    delivery_repository
        .apply_attempt_transition(&tier_0, &succeeded)
        .await
        .unwrap();

    let claimed = delivery_repository.claim_next(10).await.unwrap();
    assert!(
        claimed.is_empty(),
        "a successful primary leaves nothing to fall back from, so the fallback must never fire"
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_fallback_tier_becomes_claimable_once_every_primary_tier_delivery_fails_permanently() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let delivery_repository = PostgresDeliveryRepository::new(pool.clone());

    let (tier_0_id, tier_1_id) = plan_and_persist_fallback(
        &delivery_repository,
        &alert,
        ChannelType::Sms,
        ChannelType::Email,
    )
    .await;

    let mut tier_0 = delivery_repository
        .find_by_id(tier_0_id)
        .await
        .unwrap()
        .unwrap();
    let start = start_delivery_attempt(&mut tier_0, Actor::Automated, Uuid::new_v4()).unwrap();
    delivery_repository
        .apply_transition(&tier_0, &start)
        .await
        .unwrap();
    let failed = record_delivery_failure(
        &mut tier_0,
        false,
        "invalid phone number",
        Actor::Automated,
        Uuid::new_v4(),
    )
    .unwrap();
    delivery_repository
        .apply_attempt_transition(&tier_0, &failed)
        .await
        .unwrap();
    assert_eq!(tier_0.status(), DeliveryStatus::FailedPermanently);

    let claimed = delivery_repository.claim_next(10).await.unwrap();
    assert_eq!(
        claimed.len(),
        1,
        "the fallback becomes claimable once its only primary tier has failed permanently"
    );
    assert_eq!(claimed[0].id(), tier_1_id);
    assert_eq!(claimed[0].status(), DeliveryStatus::Sending);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn claim_next_only_dequeues_queued_or_retrying_deliveries_and_starts_their_attempt() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let delivery_repository = PostgresDeliveryRepository::new(pool.clone());

    let queued_id = plan_and_persist_one(&delivery_repository, &alert, ChannelType::WhatsApp).await;
    let sent_id = plan_and_persist_one(&delivery_repository, &alert, ChannelType::Sms).await;

    // Drive the second delivery all the way to Sent, so it must not be
    // re-claimed alongside the still-Queued one.
    let mut sent_delivery = delivery_repository
        .find_by_id(sent_id)
        .await
        .unwrap()
        .unwrap();
    let start =
        start_delivery_attempt(&mut sent_delivery, Actor::Automated, Uuid::new_v4()).unwrap();
    delivery_repository
        .apply_transition(&sent_delivery, &start)
        .await
        .unwrap();
    let succeeded =
        record_delivery_success(&mut sent_delivery, None, Actor::Automated, Uuid::new_v4())
            .unwrap();
    delivery_repository
        .apply_attempt_transition(&sent_delivery, &succeeded)
        .await
        .unwrap();

    let claimed = delivery_repository.claim_next(10).await.unwrap();

    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].id(), queued_id);
    assert_eq!(claimed[0].status(), DeliveryStatus::Sending);
    assert_eq!(claimed[0].attempt_count(), 1);

    let (event_count,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM delivery_events WHERE delivery_id = $1 AND event_type = 'DELIVERY_STARTED'",
    )
    .bind(queued_id.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_count, 1);

    // A second claim finds nothing left to dequeue.
    let claimed_again = delivery_repository.claim_next(10).await.unwrap();
    assert!(claimed_again.is_empty());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn claim_next_never_lets_two_concurrent_workers_claim_the_same_delivery() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let setup_repository = PostgresDeliveryRepository::new(pool.clone());

    let mut delivery_ids = Vec::new();
    for _ in 0..3 {
        delivery_ids
            .push(plan_and_persist_one(&setup_repository, &alert, ChannelType::WhatsApp).await);
    }

    // A separate, multi-connection pool to the same database: genuine
    // concurrent workers each need their own connection to take out
    // independent row locks.
    let database_url = env::var("TEST_DATABASE_URL").unwrap();
    let concurrent_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("test database must be reachable");

    let mut workers = Vec::new();
    for _ in 0..5 {
        let repository = PostgresDeliveryRepository::new(concurrent_pool.clone());
        workers.push(tokio::spawn(async move {
            repository.claim_next(1).await.unwrap()
        }));
    }

    let mut claimed_ids = Vec::new();
    for worker in workers {
        claimed_ids.extend(worker.await.unwrap().into_iter().map(|d| d.id()));
    }

    claimed_ids.sort_by_key(|id| id.as_uuid());
    let mut expected = delivery_ids;
    expected.sort_by_key(|id| id.as_uuid());
    assert_eq!(
        claimed_ids, expected,
        "every delivery must be claimed exactly once across all concurrent workers"
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_retrying_delivery_is_not_reclaimed_until_its_backoff_elapses() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let delivery_repository = PostgresDeliveryRepository::new(pool.clone());
    let delivery_id =
        plan_and_persist_one(&delivery_repository, &alert, ChannelType::WhatsApp).await;

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
        true,
        "provider timeout",
        Actor::Automated,
        Uuid::new_v4(),
    )
    .unwrap();
    delivery_repository
        .apply_attempt_transition(&delivery, &failed)
        .await
        .unwrap();
    assert_eq!(delivery.status(), DeliveryStatus::Retrying);

    let claimed_too_soon = delivery_repository.claim_next(10).await.unwrap();
    assert!(
        claimed_too_soon.is_empty(),
        "a Retrying delivery must not be reclaimed before its backoff elapses"
    );

    sqlx::query(
        "UPDATE deliveries SET next_attempt_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(delivery_id.as_uuid())
    .execute(&pool)
    .await
    .unwrap();

    let claimed_after_backoff = delivery_repository.claim_next(10).await.unwrap();
    assert_eq!(claimed_after_backoff.len(), 1);
    assert_eq!(claimed_after_backoff[0].id(), delivery_id);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_deliverys_matched_subscription_versions_survive_a_round_trip() {
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
    let matched = vec![
        MatchedSubscription {
            subscription_id: SubscriptionId::new(),
            subscription_version: 3,
        },
        MatchedSubscription {
            subscription_id: SubscriptionId::new(),
            subscription_version: 1,
        },
    ];
    let consumer_matches = vec![ConsumerMatch {
        consumer_id,
        matching_subscriptions: matched.clone(),
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

    let loaded = delivery_repository
        .find_by_id(delivery_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.matching_subscriptions(), matched.as_slice());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn find_by_alert_id_returns_every_delivery_planned_for_that_alert_only() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let other_alert = verified_alert(&pool).await;
    let delivery_repository = PostgresDeliveryRepository::new(pool.clone());

    let whatsapp_id =
        plan_and_persist_one(&delivery_repository, &alert, ChannelType::WhatsApp).await;
    let sms_id = plan_and_persist_one(&delivery_repository, &alert, ChannelType::Sms).await;
    plan_and_persist_one(&delivery_repository, &other_alert, ChannelType::Email).await;

    let found = delivery_repository
        .find_by_alert_id(alert.id())
        .await
        .unwrap();
    let mut found_ids: Vec<_> = found.iter().map(|delivery| delivery.id()).collect();
    found_ids.sort_by_key(|id| id.as_uuid());
    let mut expected_ids = vec![whatsapp_id, sms_id];
    expected_ids.sort_by_key(|id| id.as_uuid());
    assert_eq!(found_ids, expected_ids);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn find_by_alert_id_returns_empty_for_an_alert_with_no_deliveries() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let delivery_repository = PostgresDeliveryRepository::new(pool);

    let found = delivery_repository
        .find_by_alert_id(alert.id())
        .await
        .unwrap();
    assert!(found.is_empty());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn find_attempts_returns_the_full_attempt_history_in_order() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let delivery_repository = PostgresDeliveryRepository::new(pool);
    let delivery_id =
        plan_and_persist_one(&delivery_repository, &alert, ChannelType::WhatsApp).await;

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
        true,
        "provider timeout",
        Actor::Automated,
        Uuid::new_v4(),
    )
    .unwrap();
    delivery_repository
        .apply_attempt_transition(&delivery, &failed)
        .await
        .unwrap();

    let start = start_delivery_attempt(&mut delivery, Actor::Automated, Uuid::new_v4()).unwrap();
    delivery_repository
        .apply_transition(&delivery, &start)
        .await
        .unwrap();
    let succeeded = record_delivery_success(
        &mut delivery,
        Some("provider-msg-1".into()),
        Actor::Automated,
        Uuid::new_v4(),
    )
    .unwrap();
    delivery_repository
        .apply_attempt_transition(&delivery, &succeeded)
        .await
        .unwrap();

    let attempts = delivery_repository
        .find_attempts(delivery_id)
        .await
        .unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].attempt_number, 1);
    assert_eq!(
        attempts[0].outcome,
        safe_cameroon_domain::DeliveryAttemptOutcome::Failed {
            retryable: true,
            reason: "provider timeout".into(),
        }
    );
    assert_eq!(attempts[1].attempt_number, 2);
    assert_eq!(
        attempts[1].outcome,
        safe_cameroon_domain::DeliveryAttemptOutcome::Sent {
            provider_message_id: Some("provider-msg-1".into()),
        }
    );
}
