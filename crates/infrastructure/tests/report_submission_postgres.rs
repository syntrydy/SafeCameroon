use std::env;

use safe_cameroon_application::prepare_anonymous_report;
use safe_cameroon_infrastructure::postgres::PostgresReportRepository;
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
    sqlx::query("TRUNCATE outbox_events, audit_events, reports, reporters")
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
    let submission =
        prepare_anonymous_report("A child is missing.".into(), Uuid::new_v4()).unwrap();

    repository.submit_anonymous(&submission).await.unwrap();

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
    let first = prepare_anonymous_report("Initial report.".into(), Uuid::new_v4()).unwrap();
    repository.submit_anonymous(&first).await.unwrap();

    let mut conflicting =
        prepare_anonymous_report("Must be rolled back.".into(), Uuid::new_v4()).unwrap();
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
