use std::env;

use safe_cameroon_application::alert_workflow::create_alert_from_case;
use safe_cameroon_application::case_workflow::{Actor, create_case_from_report, review_case};
use safe_cameroon_application::prepare_anonymous_report;
use safe_cameroon_domain::{
    AlertField, AlertFieldValue, AlertPolicy, CaseStatus, IncidentType, ReportId, Severity,
    TargetGeography,
};
use safe_cameroon_infrastructure::postgres::{
    PostgresAlertRepository, PostgresCaseRepository, PostgresOutboxRepository,
    PostgresReportRepository,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

async fn test_pool() -> PgPool {
    let database_url = env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must point to a dedicated PostgreSQL test database");
    let pool = PgPoolOptions::new()
        .max_connections(5)
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
         outbox_events, audit_events, reports, reporters",
    )
    .execute(&pool)
    .await
    .expect("test tables must be reset");
    pool
}

/// Creates a verified case and a community alert from it, persisting both —
/// this is what actually writes the `ALERT_CREATED` outbox row under test.
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

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn claiming_an_event_marks_it_claimed_but_not_yet_published() {
    let pool = test_pool().await;
    let alert = verified_alert(&pool).await;
    let outbox = PostgresOutboxRepository::new(pool.clone());

    let claimed = outbox.claim_unpublished("ALERT_CREATED", 10).await.unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].aggregate_id, alert.id().as_uuid());

    // Claiming alone must never mark an event published -- that would mean
    // an event that was handed to a worker but never actually processed
    // (the worker crashed, or an earlier event in the same batch failed) is
    // silently treated as done and never seen again.
    let (claimed_at_set, published_at_set): (bool, bool) = sqlx::query_as(
        "SELECT claimed_at IS NOT NULL, published_at IS NOT NULL FROM outbox_events WHERE id = $1",
    )
    .bind(claimed[0].id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(claimed_at_set, "claiming must set claimed_at");
    assert!(
        !published_at_set,
        "claiming alone must not set published_at"
    );

    outbox.mark_published(claimed[0].id).await.unwrap();
    let (is_published,): (bool,) =
        sqlx::query_as("SELECT published_at IS NOT NULL FROM outbox_events WHERE id = $1")
            .bind(claimed[0].id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        is_published,
        "mark_published must set published_at once processing succeeds"
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_claimed_event_is_never_returned_again() {
    let pool = test_pool().await;
    verified_alert(&pool).await;
    let outbox = PostgresOutboxRepository::new(pool.clone());

    let first = outbox.claim_unpublished("ALERT_CREATED", 10).await.unwrap();
    assert_eq!(first.len(), 1);

    let second = outbox.claim_unpublished("ALERT_CREATED", 10).await.unwrap();
    assert!(second.is_empty());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn only_events_of_the_requested_type_are_claimed() {
    let pool = test_pool().await;
    verified_alert(&pool).await;
    let outbox = PostgresOutboxRepository::new(pool.clone());

    let claimed = outbox
        .claim_unpublished("SOME_OTHER_EVENT_TYPE", 10)
        .await
        .unwrap();
    assert!(claimed.is_empty());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn two_concurrent_workers_never_claim_the_same_event() {
    let pool = test_pool().await;
    for _ in 0..3 {
        verified_alert(&pool).await;
    }

    let database_url = env::var("TEST_DATABASE_URL").unwrap();
    let concurrent_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("test database must be reachable");

    let mut workers = Vec::new();
    for _ in 0..5 {
        let outbox = PostgresOutboxRepository::new(concurrent_pool.clone());
        workers.push(tokio::spawn(async move {
            outbox.claim_unpublished("ALERT_CREATED", 1).await.unwrap()
        }));
    }

    let mut claimed_ids = Vec::new();
    for worker in workers {
        claimed_ids.extend(worker.await.unwrap().into_iter().map(|event| event.id));
    }
    claimed_ids.sort();
    claimed_ids.dedup();

    let (total,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM outbox_events WHERE event_type = 'ALERT_CREATED' AND claimed_at IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(total, 3);
    assert_eq!(
        claimed_ids.len(),
        3,
        "every event must be claimed exactly once across all concurrent workers"
    );
}
