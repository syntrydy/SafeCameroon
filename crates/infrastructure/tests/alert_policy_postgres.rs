use std::env;

use safe_cameroon_application::alert_workflow::{cancel_alert, create_alert_from_case};
use safe_cameroon_application::case_workflow::{Actor, create_case_from_report, review_case};
use safe_cameroon_application::prepare_anonymous_report;
use safe_cameroon_domain::{
    AlertField, AlertFieldValue, AlertPolicy, AlertStatus, AlertVisibility, CaseStatus,
    IncidentType, ReportId, TargetGeography,
};
use safe_cameroon_infrastructure::postgres::{
    AlertCancelOutcome, AlertCreationOutcome, AlertFilter, PostgresAlertRepository,
    PostgresCaseRepository, PostgresReportRepository,
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
        None,
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
        None,
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

fn internal_policy() -> AlertPolicy {
    AlertPolicy::new(
        safe_cameroon_domain::AlertPolicyId::new("INTERNAL_TEST"),
        1,
        IncidentType::MissingChild,
        AlertVisibility::Internal,
        safe_cameroon_domain::CaseEventType::CaseVerified,
        vec![AlertField::IncidentCategory],
    )
    .unwrap()
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn list_filters_by_status_and_visibility_most_recently_created_first() {
    let pool = test_pool().await;
    let alert_repository = PostgresAlertRepository::new(pool.clone());

    let (_repo, community_case) = verified_case(&pool).await;
    let community_creation = create_alert_from_case(
        &community_case,
        &AlertPolicy::missing_child_community_v1(),
        safe_cameroon_domain::Severity::High,
        TargetGeography::new("Douala - Bonamoussadi").unwrap(),
        safe_fields(),
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
        None,
    )
    .unwrap();
    alert_repository.create(&community_creation).await.unwrap();

    let (_repo, internal_case) = verified_case(&pool).await;
    let internal_creation = create_alert_from_case(
        &internal_case,
        &internal_policy(),
        safe_cameroon_domain::Severity::High,
        TargetGeography::new("Douala - Akwa").unwrap(),
        vec![AlertFieldValue {
            field: AlertField::IncidentCategory,
            value: "MISSING_CHILD".into(),
        }],
        Actor::Automated,
        Uuid::new_v4(),
        None,
    )
    .unwrap();
    alert_repository.create(&internal_creation).await.unwrap();

    let all = alert_repository
        .list(&AlertFilter::default(), 10, 0)
        .await
        .unwrap();
    assert_eq!(all.len(), 2);
    // Most recently created first.
    assert_eq!(all[0].id(), internal_creation.alert.id());
    assert_eq!(all[1].id(), community_creation.alert.id());

    let community_only = alert_repository
        .list(
            &AlertFilter {
                status: None,
                visibility: Some(AlertVisibility::Community),
            },
            10,
            0,
        )
        .await
        .unwrap();
    assert_eq!(community_only.len(), 1);
    assert_eq!(community_only[0].id(), community_creation.alert.id());
    assert_eq!(
        community_only[0].field_value(AlertField::ApproximateAge),
        Some("8 years old")
    );

    let active_only = alert_repository
        .list(
            &AlertFilter {
                status: Some(AlertStatus::Active),
                visibility: None,
            },
            10,
            0,
        )
        .await
        .unwrap();
    assert_eq!(active_only.len(), 2);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn list_respects_limit_and_offset() {
    let pool = test_pool().await;
    let alert_repository = PostgresAlertRepository::new(pool.clone());

    for _ in 0..5 {
        let (_repo, case) = verified_case(&pool).await;
        let creation = create_alert_from_case(
            &case,
            &AlertPolicy::missing_child_community_v1(),
            safe_cameroon_domain::Severity::High,
            TargetGeography::new("Douala - Bonamoussadi").unwrap(),
            safe_fields(),
            Actor::Reviewer(Uuid::new_v4()),
            Uuid::new_v4(),
            None,
        )
        .unwrap();
        alert_repository.create(&creation).await.unwrap();
    }

    let page1 = alert_repository
        .list(&AlertFilter::default(), 2, 0)
        .await
        .unwrap();
    let page2 = alert_repository
        .list(&AlertFilter::default(), 2, 2)
        .await
        .unwrap();
    assert_eq!(page1.len(), 2);
    assert_eq!(page2.len(), 2);
    let page1_ids: Vec<Uuid> = page1.iter().map(|alert| alert.id().as_uuid()).collect();
    let page2_ids: Vec<Uuid> = page2.iter().map(|alert| alert.id().as_uuid()).collect();
    assert!(page1_ids.iter().all(|id| !page2_ids.contains(id)));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_retried_alert_creation_with_the_same_idempotency_key_is_rejected_as_a_duplicate() {
    let pool = test_pool().await;
    let (_case_repository, case) = verified_case(&pool).await;
    let alert_repository = PostgresAlertRepository::new(pool.clone());
    let policy = AlertPolicy::missing_child_community_v1();

    let first = create_alert_from_case(
        &case,
        &policy,
        safe_cameroon_domain::Severity::High,
        TargetGeography::new("Douala - Bonamoussadi").unwrap(),
        safe_fields(),
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
        Some("client-retry-key-1"),
    )
    .unwrap();
    assert_eq!(
        alert_repository.create(&first).await.unwrap(),
        AlertCreationOutcome::Created
    );

    let retry = create_alert_from_case(
        &case,
        &policy,
        safe_cameroon_domain::Severity::High,
        TargetGeography::new("Douala - Bonamoussadi").unwrap(),
        safe_fields(),
        Actor::Reviewer(Uuid::new_v4()),
        Uuid::new_v4(),
        Some("client-retry-key-1"),
    )
    .unwrap();
    assert_eq!(
        alert_repository.create(&retry).await.unwrap(),
        AlertCreationOutcome::Duplicate {
            alert_id: first.alert.id().as_uuid()
        }
    );

    let (alert_count,): (i64,) = sqlx::query_as("SELECT count(*) FROM alerts WHERE case_id = $1")
        .bind(case.id().as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(alert_count, 1, "the retried alert must not persist");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn alert_creations_without_an_idempotency_key_are_never_treated_as_duplicates() {
    let pool = test_pool().await;
    let (_case_repository, case) = verified_case(&pool).await;
    let alert_repository = PostgresAlertRepository::new(pool.clone());
    let policy = AlertPolicy::missing_child_community_v1();

    for _ in 0..2 {
        let creation = create_alert_from_case(
            &case,
            &policy,
            safe_cameroon_domain::Severity::High,
            TargetGeography::new("Douala - Bonamoussadi").unwrap(),
            safe_fields(),
            Actor::Reviewer(Uuid::new_v4()),
            Uuid::new_v4(),
            None,
        )
        .unwrap();
        assert_eq!(
            alert_repository.create(&creation).await.unwrap(),
            AlertCreationOutcome::Created
        );
    }
}
