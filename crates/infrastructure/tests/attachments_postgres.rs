use std::env;

use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_application::prepare_anonymous_report;
use safe_cameroon_domain::{Attachment, AttachmentContentType, AttachmentId, StorageProvider};
use safe_cameroon_infrastructure::postgres::{
    PostgresAttachmentRepository, PostgresReportRepository,
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
        "TRUNCATE attachments, webhook_replay_events, delivery_events, delivery_attempts, \
         deliveries, alert_events, alert_fields, alerts, case_events, case_reports, cases, \
         outbox_events, audit_events, reports, reporters",
    )
    .execute(&pool)
    .await
    .expect("test tables must be reset");
    pool
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn persists_and_reconstitutes_an_attachment() {
    let pool = test_pool().await;
    let reports = PostgresReportRepository::new(pool.clone());
    let submission =
        prepare_anonymous_report("A child is missing.".into(), Uuid::new_v4(), None).unwrap();
    reports.submit_anonymous(&submission).await.unwrap();

    let attachments = PostgresAttachmentRepository::new(pool.clone());
    let attachment = Attachment::new(
        submission.report.id,
        StorageProvider::R2,
        "attachments/report-1/key-1",
        AttachmentContentType::ImagePng,
        4096,
        "abc123",
    )
    .unwrap();
    attachments.create(&attachment).await.unwrap();

    let loaded = attachments
        .find_by_id(attachment.id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.report_id(), submission.report.id);
    assert_eq!(loaded.object_key(), "attachments/report-1/key-1");
    assert_eq!(loaded.content_type(), AttachmentContentType::ImagePng);
    assert_eq!(loaded.size_bytes(), 4096);
    assert_eq!(loaded.checksum(), "abc123");

    assert!(
        attachments
            .find_by_id(AttachmentId::new())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn records_an_audit_event_for_download_access() {
    let pool = test_pool().await;
    let reports = PostgresReportRepository::new(pool.clone());
    let submission =
        prepare_anonymous_report("A child is missing.".into(), Uuid::new_v4(), None).unwrap();
    reports.submit_anonymous(&submission).await.unwrap();

    let attachments = PostgresAttachmentRepository::new(pool.clone());
    let attachment = Attachment::new(
        submission.report.id,
        StorageProvider::R2,
        "attachments/report-1/key-2",
        AttachmentContentType::ImagePng,
        4096,
        "abc123",
    )
    .unwrap();
    attachments.create(&attachment).await.unwrap();

    let reviewer = Actor::Reviewer(Uuid::new_v4());
    let request_id = Uuid::new_v4();
    attachments
        .record_download_access(attachment.id(), reviewer, request_id)
        .await
        .unwrap();

    let (count,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM audit_events WHERE resource_id = $1 \
         AND action = 'ATTACHMENT_DOWNLOAD_URL_ISSUED' AND request_id = $2",
    )
    .bind(attachment.id().as_uuid())
    .bind(request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}
