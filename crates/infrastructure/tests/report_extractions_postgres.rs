use std::env;

use safe_cameroon_application::ai_extraction::ExtractionRecord;
use safe_cameroon_application::prepare_anonymous_report;
use safe_cameroon_domain::{ExtractedReportFields, ReportExtractionId, ReportId};
use safe_cameroon_infrastructure::postgres::{
    PostgresReportExtractionRepository, PostgresReportRepository, SubmissionResult,
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
        "TRUNCATE report_extractions, attachments, webhook_replay_events, delivery_events, \
         delivery_attempts, deliveries, alert_events, alert_fields, alerts, case_events, \
         case_reports, cases, outbox_events, audit_events, reports, reporters",
    )
    .execute(&pool)
    .await
    .expect("test tables must be reset");
    pool
}

async fn seeded_report(pool: &PgPool) -> ReportId {
    let submission =
        prepare_anonymous_report("A child is missing.".into(), Uuid::new_v4(), None).unwrap();
    let report_id = submission.report.id;
    assert_eq!(
        PostgresReportRepository::new(pool.clone())
            .submit_anonymous(&submission)
            .await
            .unwrap(),
        SubmissionResult::Created
    );
    report_id
}

fn sample_fields() -> ExtractedReportFields {
    ExtractedReportFields::new(
        Some("a young girl in a blue school uniform".into()),
        Some("about 8 years old".into()),
        Some("around 3pm today".into()),
        Some("Douala - Bonamoussadi".into()),
        None,
        None,
        None,
    )
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn persists_and_reads_back_an_extraction_with_full_provenance() {
    let pool = test_pool().await;
    let report_id = seeded_report(&pool).await;
    let repository = PostgresReportExtractionRepository::new(pool);
    let requested_by = Uuid::new_v4();

    let record = ExtractionRecord {
        id: ReportExtractionId::new(),
        report_id,
        requested_by,
        provider: "OPENROUTER".into(),
        model: "openai/gpt-4o-mini".into(),
        prompt_version: "v1".into(),
        fields: sample_fields(),
    };
    repository.create(&record).await.unwrap();

    let loaded = repository.find_by_report(report_id).await.unwrap();
    assert_eq!(loaded, vec![record]);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn find_by_report_orders_most_recent_first() {
    let pool = test_pool().await;
    let report_id = seeded_report(&pool).await;
    let repository = PostgresReportExtractionRepository::new(pool);

    let first = ExtractionRecord {
        id: ReportExtractionId::new(),
        report_id,
        requested_by: Uuid::new_v4(),
        provider: "OPENROUTER".into(),
        model: "openai/gpt-4o-mini".into(),
        prompt_version: "v1".into(),
        fields: sample_fields(),
    };
    repository.create(&first).await.unwrap();

    let second = ExtractionRecord {
        id: ReportExtractionId::new(),
        report_id,
        requested_by: Uuid::new_v4(),
        provider: "OPENROUTER".into(),
        model: "openai/gpt-4o-mini".into(),
        prompt_version: "v1".into(),
        fields: ExtractedReportFields::default(),
    };
    repository.create(&second).await.unwrap();

    let loaded = repository.find_by_report(report_id).await.unwrap();
    assert_eq!(loaded, vec![second, first]);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn find_by_report_returns_empty_for_a_report_with_no_extractions() {
    let pool = test_pool().await;
    let report_id = seeded_report(&pool).await;
    let repository = PostgresReportExtractionRepository::new(pool);

    assert_eq!(repository.find_by_report(report_id).await.unwrap(), vec![]);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_fully_empty_extraction_round_trips_too() {
    let pool = test_pool().await;
    let report_id = seeded_report(&pool).await;
    let repository = PostgresReportExtractionRepository::new(pool);

    let record = ExtractionRecord {
        id: ReportExtractionId::new(),
        report_id,
        requested_by: Uuid::new_v4(),
        provider: "OPENROUTER".into(),
        model: "openai/gpt-4o-mini".into(),
        prompt_version: "v1".into(),
        fields: ExtractedReportFields::default(),
    };
    repository.create(&record).await.unwrap();

    let loaded = repository.find_by_report(report_id).await.unwrap();
    assert_eq!(loaded, vec![record]);
}
