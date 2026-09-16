use std::env;

use safe_cameroon_application::prepare_anonymous_report;
use safe_cameroon_domain::ReportStatus;
use safe_cameroon_infrastructure::postgres::{PostgresReportRepository, SubmissionResult};
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
        "TRUNCATE attachments, webhook_replay_events, delivery_events, delivery_attempts, deliveries, alert_events, alert_fields, \
         alerts, case_events, case_reports, cases, outbox_events, audit_events, reports, reporters",
    )
    .execute(&pool)
    .await
    .expect("test tables must be reset");
    pool
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn submission_persists_report_audit_event_and_outbox_event_together() {
    let pool = test_pool().await;
    let repository = PostgresReportRepository::new(pool.clone());
    let submission = prepare_anonymous_report(
        "A child is missing.".into(),
        Uuid::new_v4(),
        Some("report-retry-1"),
    )
    .unwrap();

    assert_eq!(
        repository.submit_anonymous(&submission).await.unwrap(),
        SubmissionResult::Created
    );

    let (content, token_hash): (String, Vec<u8>) =
        sqlx::query_as("SELECT raw_content, follow_up_token_hash FROM reports WHERE id = $1")
            .bind(submission.report.id.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(content, "A child is missing.");
    assert_eq!(token_hash, submission.reference_code_hash);

    let (audit_count,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM audit_events WHERE id = $1 AND action = 'REPORT_SUBMITTED'",
    )
    .bind(submission.audit_event_id.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 1);

    let (outbox_count,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM outbox_events WHERE id = $1 AND event_type = 'REPORT_SUBMITTED'",
    )
    .bind(submission.outbox_event_id.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(outbox_count, 1);

    let mutation = sqlx::query("UPDATE audit_events SET action = 'TAMPERED' WHERE id = $1")
        .bind(submission.audit_event_id.as_uuid())
        .execute(&pool)
        .await;
    assert!(mutation.is_err(), "database must reject audit mutation");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn audit_failure_rolls_back_the_report_insert() {
    let pool = test_pool().await;
    let repository = PostgresReportRepository::new(pool.clone());
    let first = prepare_anonymous_report("Initial report.".into(), Uuid::new_v4(), None).unwrap();
    repository.submit_anonymous(&first).await.unwrap();

    let mut conflicting =
        prepare_anonymous_report("Must be rolled back.".into(), Uuid::new_v4(), None).unwrap();
    conflicting.audit_event_id = first.audit_event_id;
    let report_id = conflicting.report.id.as_uuid();

    assert!(repository.submit_anonymous(&conflicting).await.is_err());

    let (report_count,): (i64,) = sqlx::query_as("SELECT count(*) FROM reports WHERE id = $1")
        .bind(report_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        report_count, 0,
        "a failed audit insert must roll back the report"
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn idempotency_key_prevents_a_duplicate_report() {
    let pool = test_pool().await;
    let repository = PostgresReportRepository::new(pool);
    let first = prepare_anonymous_report(
        "First attempt.".into(),
        Uuid::new_v4(),
        Some("same-retry-key"),
    )
    .unwrap();
    let second = prepare_anonymous_report(
        "Retry attempt.".into(),
        Uuid::new_v4(),
        Some("same-retry-key"),
    )
    .unwrap();

    assert_eq!(
        repository.submit_anonymous(&first).await.unwrap(),
        SubmissionResult::Created
    );
    assert_eq!(
        repository.submit_anonymous(&second).await.unwrap(),
        SubmissionResult::Duplicate {
            report_id: first.report.id.as_uuid()
        }
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn list_filters_by_status_most_recently_received_first() {
    let pool = test_pool().await;
    let repository = PostgresReportRepository::new(pool.clone());

    let first = prepare_anonymous_report("First report.".into(), Uuid::new_v4(), None).unwrap();
    repository.submit_anonymous(&first).await.unwrap();
    let second = prepare_anonymous_report("Second report.".into(), Uuid::new_v4(), None).unwrap();
    repository.submit_anonymous(&second).await.unwrap();

    sqlx::query("UPDATE reports SET status = 'UNDER_REVIEW' WHERE id = $1")
        .bind(second.report.id.as_uuid())
        .execute(&pool)
        .await
        .unwrap();

    let all = repository.list(None, 10, 0).await.unwrap();
    assert_eq!(all.len(), 2);
    // Most recently received first.
    assert_eq!(all[0].id, second.report.id.as_uuid());
    assert_eq!(all[0].status, ReportStatus::UnderReview);
    assert_eq!(all[0].raw_content, "Second report.");
    assert!(!all[0].received_at.is_empty());
    assert_eq!(all[1].id, first.report.id.as_uuid());
    assert_eq!(all[1].status, ReportStatus::Received);

    let received_only = repository
        .list(Some(ReportStatus::Received), 10, 0)
        .await
        .unwrap();
    assert_eq!(received_only.len(), 1);
    assert_eq!(received_only[0].id, first.report.id.as_uuid());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn list_respects_limit_and_offset() {
    let pool = test_pool().await;
    let repository = PostgresReportRepository::new(pool.clone());

    for index in 0..5 {
        let report =
            prepare_anonymous_report(format!("Report {index}"), Uuid::new_v4(), None).unwrap();
        repository.submit_anonymous(&report).await.unwrap();
    }

    let page1 = repository.list(None, 2, 0).await.unwrap();
    let page2 = repository.list(None, 2, 2).await.unwrap();
    assert_eq!(page1.len(), 2);
    assert_eq!(page2.len(), 2);
    let page1_ids: Vec<Uuid> = page1.iter().map(|report| report.id).collect();
    let page2_ids: Vec<Uuid> = page2.iter().map(|report| report.id).collect();
    assert!(page1_ids.iter().all(|id| !page2_ids.contains(id)));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn find_by_id_returns_the_matching_report() {
    let pool = test_pool().await;
    let repository = PostgresReportRepository::new(pool);
    let submission =
        prepare_anonymous_report("A child is missing.".into(), Uuid::new_v4(), None).unwrap();
    repository.submit_anonymous(&submission).await.unwrap();

    let found = repository
        .find_by_id(submission.report.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.id, submission.report.id.as_uuid());
    assert_eq!(found.raw_content, "A child is missing.");
    assert_eq!(found.status, ReportStatus::Received);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn find_by_id_returns_none_for_an_unknown_report() {
    let pool = test_pool().await;
    let repository = PostgresReportRepository::new(pool);

    assert!(
        repository
            .find_by_id(safe_cameroon_domain::ReportId::new())
            .await
            .unwrap()
            .is_none()
    );
}
