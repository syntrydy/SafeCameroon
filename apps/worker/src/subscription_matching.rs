//! One matching cycle: claim unpublished `ALERT_CREATED` outbox events, run
//! the subscription engine against each alert, and plan deliveries for every
//! matched consumer that has a configured delivery preference
//! (docs/SUBSCRIPTION_ENGINE.md). This is what actually connects alert
//! creation to the delivery engine — until now `evaluate_subscriptions`/
//! `plan_deliveries` were only exercised manually in tests. Candidate
//! filtering beyond a full subscription scan (section 7) is deferred until a
//! query pattern needs it, matching `PostgresSubscriptionRepository::list_all`.

use std::collections::HashMap;

use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_application::delivery_workflow::plan_deliveries;
use safe_cameroon_domain::{
    AlertId, ConsumerId, DeliveryPreference, RetryPolicy, deduplicate_by_consumer,
    evaluate_subscriptions,
};
use safe_cameroon_infrastructure::postgres::{
    PostgresAlertRepository, PostgresDeliveryPreferenceRepository, PostgresDeliveryRepository,
    PostgresOutboxRepository, PostgresSubscriptionRepository,
};
use uuid::Uuid;

const ALERT_CREATED_EVENT_TYPE: &str = "ALERT_CREATED";

/// Claims up to `batch_size` unpublished `ALERT_CREATED` events and matches
/// each one. Returns how many were claimed (0 means the caller should back
/// off before polling again), mirroring `dispatch::process_batch`. A
/// database error aborts the batch; per-alert matching never does — an alert
/// with no matching subscriptions, or matched consumers with no delivery
/// preference, is a normal outcome, not a failure. Each event is marked
/// published only right after its own matching succeeds — never up front at
/// claim time — so an error partway through a batch leaves every event from
/// that point on still unpublished (claimed, not silently dropped) rather
/// than permanently skipped.
pub async fn process_batch(
    outbox: &PostgresOutboxRepository,
    alerts: &PostgresAlertRepository,
    subscriptions: &PostgresSubscriptionRepository,
    delivery_preferences: &PostgresDeliveryPreferenceRepository,
    deliveries: &PostgresDeliveryRepository,
    batch_size: i64,
) -> Result<usize, sqlx::Error> {
    let claimed = outbox
        .claim_unpublished(ALERT_CREATED_EVENT_TYPE, batch_size)
        .await?;
    let count = claimed.len();
    for event in claimed {
        match_and_plan(
            alerts,
            subscriptions,
            delivery_preferences,
            deliveries,
            AlertId::from_uuid(event.aggregate_id),
        )
        .await?;
        outbox.mark_published(event.id).await?;
    }
    Ok(count)
}

/// `alert_id` is always the aggregate a real `ALERT_CREATED` outbox row was
/// written for in the same transaction as the alert itself
/// (`PostgresAlertRepository::create`), so a missing alert here would mean
/// data corruption, not a normal race; it is logged and skipped rather than
/// failing the whole batch over one row.
async fn match_and_plan(
    alerts: &PostgresAlertRepository,
    subscriptions: &PostgresSubscriptionRepository,
    delivery_preferences: &PostgresDeliveryPreferenceRepository,
    deliveries: &PostgresDeliveryRepository,
    alert_id: AlertId,
) -> Result<(), sqlx::Error> {
    let Some(alert) = alerts.find_by_id(alert_id).await? else {
        tracing::warn!(
            alert_id = %alert_id.as_uuid(),
            "ALERT_CREATED outbox event references a missing alert"
        );
        return Ok(());
    };

    let all_subscriptions = subscriptions.list_all().await?;
    let decisions = evaluate_subscriptions(&all_subscriptions, &alert);
    let consumer_matches = deduplicate_by_consumer(&decisions);
    if consumer_matches.is_empty() {
        return Ok(());
    }

    let mut preferences: HashMap<ConsumerId, DeliveryPreference> = HashMap::new();
    for consumer_match in &consumer_matches {
        if let Some(preference) = delivery_preferences
            .find_by_consumer(consumer_match.consumer_id)
            .await?
        {
            preferences.insert(consumer_match.consumer_id, preference);
        }
    }

    let planned = plan_deliveries(
        &alert,
        &consumer_matches,
        &preferences,
        RetryPolicy::standard(),
        Actor::Automated,
        Uuid::new_v4(),
    );
    deliveries.create_planned(&planned).await
}

#[cfg(test)]
mod tests {
    use std::env;

    use safe_cameroon_application::alert_workflow::create_alert_from_case;
    use safe_cameroon_application::case_workflow::{Actor, create_case_from_report, review_case};
    use safe_cameroon_application::prepare_anonymous_report;
    use safe_cameroon_domain::{
        AlertField, AlertFieldValue, AlertPolicy, CaseStatus, ChannelEndpoint, ChannelType,
        Comparison, DeliveryPreference, DeliveryStrategy, GeoArea, IncidentType, ReportId,
        Severity, Subscription, SubscriptionId, SubscriptionRule, TargetGeography,
    };
    use safe_cameroon_infrastructure::postgres::{
        PostgresAlertRepository, PostgresCaseRepository, PostgresDeliveryPreferenceRepository,
        PostgresDeliveryRepository, PostgresOutboxRepository, PostgresReportRepository,
        PostgresSubscriptionRepository,
    };
    use sqlx::{PgPool, postgres::PgPoolOptions};
    use uuid::Uuid;

    use crate::subscription_matching::process_batch;

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
            "TRUNCATE report_extractions, attachments, webhook_replay_events, delivery_events, delivery_attempts, \
             deliveries, alert_events, alert_fields, alerts, case_events, case_reports, cases, \
             outbox_events, audit_events, reports, reporters, consumer_delivery_preferences, \
             subscriptions",
        )
        .execute(&pool)
        .await
        .expect("test tables must be reset");
        pool
    }

    async fn verified_alert(pool: &PgPool) -> safe_cameroon_domain::Alert {
        let report_repository = PostgresReportRepository::new(pool.clone());
        let submission =
            prepare_anonymous_report("A child is missing.".into(), Uuid::new_v4(), None, None)
                .unwrap();
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

    struct Repositories {
        outbox: PostgresOutboxRepository,
        alerts: PostgresAlertRepository,
        subscriptions: PostgresSubscriptionRepository,
        delivery_preferences: PostgresDeliveryPreferenceRepository,
        deliveries: PostgresDeliveryRepository,
    }

    fn repositories(pool: &PgPool) -> Repositories {
        Repositories {
            outbox: PostgresOutboxRepository::new(pool.clone()),
            alerts: PostgresAlertRepository::new(pool.clone()),
            subscriptions: PostgresSubscriptionRepository::new(pool.clone()),
            delivery_preferences: PostgresDeliveryPreferenceRepository::new(pool.clone()),
            deliveries: PostgresDeliveryRepository::new(pool.clone()),
        }
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_matching_subscription_with_a_delivery_preference_gets_a_planned_delivery() {
        let pool = test_pool().await;
        let alert = verified_alert(&pool).await;
        let repos = repositories(&pool);

        let consumer_id = safe_cameroon_domain::ConsumerId::new();
        let subscription = Subscription::new(
            SubscriptionId::new(),
            consumer_id,
            1,
            vec![
                SubscriptionRule::IncidentType(vec![IncidentType::MissingChild]),
                SubscriptionRule::Severity {
                    operator: Comparison::GreaterThanOrEqual,
                    value: Severity::Medium,
                },
                SubscriptionRule::Geography(vec![GeoArea::new("Douala").unwrap()]),
            ],
        )
        .unwrap();
        repos.subscriptions.create(&subscription).await.unwrap();
        let preference = DeliveryPreference::new(
            DeliveryStrategy::All,
            vec![ChannelEndpoint::new(ChannelType::WhatsApp, "+237600000000").unwrap()],
        )
        .unwrap();
        repos
            .delivery_preferences
            .upsert(consumer_id, &preference)
            .await
            .unwrap();

        let processed = process_batch(
            &repos.outbox,
            &repos.alerts,
            &repos.subscriptions,
            &repos.delivery_preferences,
            &repos.deliveries,
            10,
        )
        .await
        .unwrap();
        assert_eq!(processed, 1);

        let (delivery_count,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM deliveries WHERE alert_id = $1 AND consumer_id = $2",
        )
        .bind(alert.id().as_uuid())
        .bind(consumer_id.as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(delivery_count, 1);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn successful_matching_marks_the_outbox_event_published_not_just_claimed() {
        let pool = test_pool().await;
        verified_alert(&pool).await;
        let repos = repositories(&pool);

        let processed = process_batch(
            &repos.outbox,
            &repos.alerts,
            &repos.subscriptions,
            &repos.delivery_preferences,
            &repos.deliveries,
            10,
        )
        .await
        .unwrap();
        assert_eq!(processed, 1);

        let (claimed_at_set, published_at_set): (bool, bool) = sqlx::query_as(
            "SELECT claimed_at IS NOT NULL, published_at IS NOT NULL FROM outbox_events \
             WHERE event_type = 'ALERT_CREATED'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(claimed_at_set);
        assert!(
            published_at_set,
            "a fully processed event must end up published, not just claimed"
        );
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_non_matching_subscription_produces_no_delivery() {
        let pool = test_pool().await;
        verified_alert(&pool).await;
        let repos = repositories(&pool);

        let consumer_id = safe_cameroon_domain::ConsumerId::new();
        let subscription = Subscription::new(
            SubscriptionId::new(),
            consumer_id,
            1,
            vec![SubscriptionRule::Geography(vec![
                GeoArea::new("Yaounde").unwrap(),
            ])],
        )
        .unwrap();
        repos.subscriptions.create(&subscription).await.unwrap();
        let preference = DeliveryPreference::new(
            DeliveryStrategy::All,
            vec![ChannelEndpoint::new(ChannelType::WhatsApp, "+237600000000").unwrap()],
        )
        .unwrap();
        repos
            .delivery_preferences
            .upsert(consumer_id, &preference)
            .await
            .unwrap();

        let processed = process_batch(
            &repos.outbox,
            &repos.alerts,
            &repos.subscriptions,
            &repos.delivery_preferences,
            &repos.deliveries,
            10,
        )
        .await
        .unwrap();
        assert_eq!(processed, 1, "the ALERT_CREATED event is still processed");

        let (delivery_count,): (i64,) = sqlx::query_as("SELECT count(*) FROM deliveries")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(delivery_count, 0);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_matching_consumer_without_a_delivery_preference_is_skipped() {
        let pool = test_pool().await;
        verified_alert(&pool).await;
        let repos = repositories(&pool);

        let subscription = Subscription::new(
            SubscriptionId::new(),
            safe_cameroon_domain::ConsumerId::new(),
            1,
            vec![SubscriptionRule::IncidentType(vec![
                IncidentType::MissingChild,
            ])],
        )
        .unwrap();
        repos.subscriptions.create(&subscription).await.unwrap();
        // No delivery preference is configured for this subscription's consumer.

        let processed = process_batch(
            &repos.outbox,
            &repos.alerts,
            &repos.subscriptions,
            &repos.delivery_preferences,
            &repos.deliveries,
            10,
        )
        .await
        .unwrap();
        assert_eq!(processed, 1);

        let (delivery_count,): (i64,) = sqlx::query_as("SELECT count(*) FROM deliveries")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(delivery_count, 0);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn an_alert_created_event_is_never_matched_twice_across_batches() {
        let pool = test_pool().await;
        verified_alert(&pool).await;
        let repos = repositories(&pool);

        assert_eq!(
            process_batch(
                &repos.outbox,
                &repos.alerts,
                &repos.subscriptions,
                &repos.delivery_preferences,
                &repos.deliveries,
                10,
            )
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            process_batch(
                &repos.outbox,
                &repos.alerts,
                &repos.subscriptions,
                &repos.delivery_preferences,
                &repos.deliveries,
                10,
            )
            .await
            .unwrap(),
            0,
            "an already-claimed ALERT_CREATED event must not be matched again"
        );
    }
}
