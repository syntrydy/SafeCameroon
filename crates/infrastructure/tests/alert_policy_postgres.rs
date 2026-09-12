use std::env;

use safe_cameroon_application::alert_workflow::{cancel_alert, create_alert_from_case};
use safe_cameroon_application::case_workflow::{Actor, create_case_from_report, review_case};
use safe_cameroon_application::prepare_anonymous_report;
use safe_cameroon_domain::{
    AlertField, AlertFieldValue, AlertPolicy, AlertVisibility, CaseStatus, IncidentType, ReportId,
    TargetGeography,
};
use safe_cameroon_infrastructure::postgres::{
    AlertCancelOutcome, PostgresAlertRepository, PostgresCaseRepository, PostgresReportRepository,
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
        "TRUNCATE webhook_replay_events, delivery_events, delivery_attempts, deliveries, alert_events, alert_fields, \
         alerts, case_events, case_reports, cases, outbox_events, audit_events, reports, reporters",
    )
    .execute(&pool)
    .await
    .expect("test tables must be reset");
    pool
}

/// Builds a verified case (with a real, persisted report/case) so alert
/// tests have a legitimate case to project from.
async fn verified_case(pool: &PgPool) -> (PostgresCaseRepository, safe_cameroon_domain::Case) {
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
    (case_repository, case)
}

fn safe_fields() -> Vec<AlertFieldValue> {
    vec![
        AlertFieldValue {
            field: AlertField::IncidentCategory,
            value: "MISSING_CHILD".into(),
        },
        AlertFieldValue {
            field: AlertField::ApproximateAge,
            value: "8 years old".into(),
        },
    ]
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn creates_a_community_alert_from_a_verified_case() {
    let pool = test_pool().await;
    let (_case_repository, case) = verified_case(&pool).await;
    let alert_repository = PostgresAlertRepository::new(pool.clone());
    let policy = AlertPolicy::missing_child_community_v1();

    let creation = create_alert_from_case(
        &case,
        &policy,
        safe_cameroon_domain::Severity::High,
        TargetGeography::new("Douala - Bonamoussadi").unwrap(),
        safe_fields(),
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
    )
    .unwrap();
    alert_repository.create(&creation).await.unwrap();

    let loaded = alert_repository
        .find_by_id(creation.alert.id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.visibility(), AlertVisibility::Community);
    assert_eq!(
        loaded.field_value(AlertField::ApproximateAge),
        Some("8 years old")
    );
    assert_eq!(loaded.field_value(AlertField::ReporterIdentity), None);

    let (field_count,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM alert_fields WHERE alert_id = $1")
            .bind(creation.alert.id().as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(field_count, 2);

    let (event_count,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM alert_events WHERE alert_id = $1 AND event_type = 'ALERT_CREATED'",
    )
    .bind(creation.alert.id().as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_count, 1);

    let (outbox_count,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM outbox_events WHERE aggregate_id = $1 AND event_type = 'ALERT_CREATED'",
    )
    .bind(creation.alert.id().as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(outbox_count, 1);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn cancels_an_alert_and_rejects_a_stale_retry() {
    let pool = test_pool().await;
    let (_case_repository, case) = verified_case(&pool).await;
    let alert_repository = PostgresAlertRepository::new(pool.clone());
    let policy = AlertPolicy::missing_child_community_v1();

    let creation = create_alert_from_case(
        &case,
        &policy,
        safe_cameroon_domain::Severity::High,
        TargetGeography::new("Douala").unwrap(),
        safe_fields(),
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
    )
    .unwrap();
    alert_repository.create(&creation).await.unwrap();

    let mut first_read = alert_repository
        .find_by_id(creation.alert.id())
        .await
        .unwrap()
        .unwrap();
    let mut second_read = alert_repository
        .find_by_id(creation.alert.id())
        .await
        .unwrap()
        .unwrap();

    let first_cancel = cancel_alert(
        &mut first_read,
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
    )
    .unwrap();
    assert_eq!(
        alert_repository
            .cancel(&first_read, &first_cancel)
            .await
            .unwrap(),
        AlertCancelOutcome::Cancelled
    );

    let second_cancel = cancel_alert(
        &mut second_read,
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
    )
    .unwrap();
    assert_eq!(
        alert_repository
            .cancel(&second_read, &second_cancel)
            .await
            .unwrap(),
        AlertCancelOutcome::Conflict
    );

    let reloaded = alert_repository
        .find_by_id(creation.alert.id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        reloaded.status(),
        safe_cameroon_domain::AlertStatus::Cancelled
    );
}
