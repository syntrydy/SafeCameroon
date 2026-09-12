use std::env;

use safe_cameroon_application::case_workflow::{
    Actor, create_case_from_report, link_report_to_case, review_case,
};
use safe_cameroon_application::prepare_anonymous_report;
use safe_cameroon_domain::{CaseStatus, IncidentType};
use safe_cameroon_infrastructure::postgres::{
    CaseCreationOutcome, CaseLinkOutcome, CaseReviewOutcome, PostgresCaseRepository,
    PostgresReportRepository,
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
        "TRUNCATE attachments, webhook_replay_events, delivery_events, delivery_attempts, deliveries, alert_events, alert_fields, \
         alerts, case_events, case_reports, cases, outbox_events, audit_events, reports, reporters",
    )
    .execute(&pool)
    .await
    .expect("test tables must be reset");
    pool
}

/// Inserts a report directly so case-workflow tests have a real `reports.id`
/// to satisfy the `case_reports` foreign key.
async fn seed_report(pool: &PgPool) -> Uuid {
    let repository = PostgresReportRepository::new(pool.clone());
    let submission =
        prepare_anonymous_report("A child is missing.".into(), Uuid::new_v4(), None).unwrap();
    repository.submit_anonymous(&submission).await.unwrap();
    submission.report.id.as_uuid()
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn creates_a_case_and_marks_the_source_report_linked() {
    let pool = test_pool().await;
    let report_id = seed_report(&pool).await;
    let repository = PostgresCaseRepository::new(pool.clone());

    let creation = create_case_from_report(
        IncidentType::MissingChild,
        safe_cameroon_domain::ReportId::from_uuid(report_id),
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
    );
    let case_id = creation.case.id();

    assert_eq!(
        repository.create(&creation).await.unwrap(),
        CaseCreationOutcome::Created
    );

    let loaded = repository.find_by_id(case_id).await.unwrap().unwrap();
    assert_eq!(loaded.status(), CaseStatus::Reported);
    assert_eq!(loaded.report_ids(), [creation.case.report_ids()[0]]);

    let (report_status,): (String,) =
        sqlx::query_as("SELECT status::text FROM reports WHERE id = $1")
            .bind(report_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(report_status, "LINKED_TO_CASE");

    let (event_count,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM case_events WHERE case_id = $1 AND event_type = 'CASE_CREATED'",
    )
    .bind(case_id.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_count, 1);

    let (outbox_count,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM outbox_events WHERE aggregate_id = $1 AND event_type = 'CASE_CREATED'",
    )
    .bind(case_id.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(outbox_count, 1);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_report_cannot_be_linked_to_two_cases() {
    let pool = test_pool().await;
    let report_id = seed_report(&pool).await;
    let repository = PostgresCaseRepository::new(pool.clone());

    let first = create_case_from_report(
        IncidentType::MissingChild,
        safe_cameroon_domain::ReportId::from_uuid(report_id),
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
    );
    repository.create(&first).await.unwrap();

    let second = create_case_from_report(
        IncidentType::MissingChild,
        safe_cameroon_domain::ReportId::from_uuid(report_id),
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
    );
    assert_eq!(
        repository.create(&second).await.unwrap(),
        CaseCreationOutcome::ReportAlreadyLinked
    );

    let (case_count,): (i64,) = sqlx::query_as("SELECT count(*) FROM cases WHERE id = $1")
        .bind(second.case.id().as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(case_count, 0, "the second case must not persist");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn links_a_second_report_and_records_its_event() {
    let pool = test_pool().await;
    let first_report = seed_report(&pool).await;
    let second_report = seed_report(&pool).await;
    let repository = PostgresCaseRepository::new(pool.clone());

    let creation = create_case_from_report(
        IncidentType::MissingChild,
        safe_cameroon_domain::ReportId::from_uuid(first_report),
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
    );
    repository.create(&creation).await.unwrap();

    let mut case = repository
        .find_by_id(creation.case.id())
        .await
        .unwrap()
        .unwrap();
    let link = link_report_to_case(
        &mut case,
        safe_cameroon_domain::ReportId::from_uuid(second_report),
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
    )
    .unwrap();

    assert_eq!(
        repository.link_report(&case, &link).await.unwrap(),
        CaseLinkOutcome::Linked
    );

    let reloaded = repository.find_by_id(case.id()).await.unwrap().unwrap();
    assert_eq!(reloaded.report_ids().len(), 2);
    assert_eq!(reloaded.version(), 2);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_stale_case_version_is_rejected_as_a_conflict() {
    let pool = test_pool().await;
    let report_id = seed_report(&pool).await;
    let repository = PostgresCaseRepository::new(pool.clone());

    let creation = create_case_from_report(
        IncidentType::MissingChild,
        safe_cameroon_domain::ReportId::from_uuid(report_id),
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
    );
    repository.create(&creation).await.unwrap();

    // Two reviewers independently load the same case...
    let mut first_read = repository
        .find_by_id(creation.case.id())
        .await
        .unwrap()
        .unwrap();
    let mut second_read = repository
        .find_by_id(creation.case.id())
        .await
        .unwrap()
        .unwrap();

    // ...the first reviewer's review is applied...
    let first_review = review_case(
        &mut first_read,
        Actor::Automated,
        CaseStatus::UnderReview,
        Uuid::new_v4(),
    )
    .unwrap();
    assert_eq!(
        repository
            .apply_review(&first_read, &first_review)
            .await
            .unwrap(),
        CaseReviewOutcome::Applied
    );

    // ...and the second reviewer's stale in-memory case must be rejected.
    let second_review = review_case(
        &mut second_read,
        Actor::Automated,
        CaseStatus::UnderReview,
        Uuid::new_v4(),
    )
    .unwrap();
    assert_eq!(
        repository
            .apply_review(&second_read, &second_review)
            .await
            .unwrap(),
        CaseReviewOutcome::Conflict
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn review_lifecycle_persists_status_and_events() {
    let pool = test_pool().await;
    let report_id = seed_report(&pool).await;
    let repository = PostgresCaseRepository::new(pool.clone());

    let creation = create_case_from_report(
        IncidentType::MissingChild,
        safe_cameroon_domain::ReportId::from_uuid(report_id),
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
    );
    repository.create(&creation).await.unwrap();
    let mut case = repository
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
        assert_eq!(
            repository.apply_review(&case, &review).await.unwrap(),
            CaseReviewOutcome::Applied
        );
    }

    let reloaded = repository.find_by_id(case.id()).await.unwrap().unwrap();
    assert_eq!(reloaded.status(), CaseStatus::Verified);
    assert_eq!(reloaded.version(), 3);

    let (event_count,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM case_events WHERE case_id = $1")
            .bind(case.id().as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(event_count, 3, "created, under review, and verified");
}
