use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_application::reviewer_auth::SessionRevocationStore;
use safe_cameroon_infrastructure::postgres::{CreateReviewerOutcome, PostgresReviewerRepository};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

fn now_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

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
    sqlx::query("TRUNCATE reviewers, audit_events")
        .execute(&pool)
        .await
        .expect("test tables must be reset");
    pool
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn count_starts_at_zero_and_reflects_created_reviewers() {
    let pool = test_pool().await;
    let repository = PostgresReviewerRepository::new(pool);
    assert_eq!(repository.count().await.unwrap(), 0);

    repository
        .create(Uuid::new_v4(), "reviewer@example.test", "hash-1")
        .await
        .unwrap();
    assert_eq!(repository.count().await.unwrap(), 1);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_created_reviewer_can_be_found_by_email_case_insensitively() {
    let pool = test_pool().await;
    let repository = PostgresReviewerRepository::new(pool);
    let id = Uuid::new_v4();

    let outcome = repository
        .create(id, "Reviewer@Example.Test", "hashed-password")
        .await
        .unwrap();
    assert_eq!(outcome, CreateReviewerOutcome::Created);

    let found = repository
        .find_by_email("reviewer@example.test")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.id, id);
    assert_eq!(found.password_hash, "hashed-password");
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn registering_the_same_email_twice_is_rejected() {
    let pool = test_pool().await;
    let repository = PostgresReviewerRepository::new(pool);

    repository
        .create(Uuid::new_v4(), "duplicate@example.test", "hash-1")
        .await
        .unwrap();
    let outcome = repository
        .create(Uuid::new_v4(), "Duplicate@Example.Test", "hash-2")
        .await
        .unwrap();
    assert_eq!(outcome, CreateReviewerOutcome::EmailAlreadyRegistered);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn find_by_email_returns_none_for_an_unknown_email() {
    let pool = test_pool().await;
    let repository = PostgresReviewerRepository::new(pool);
    assert_eq!(
        repository
            .find_by_email("nobody@example.test")
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_reviewer_with_no_revocation_is_never_revoked() {
    let pool = test_pool().await;
    let repository = PostgresReviewerRepository::new(pool);
    let reviewer_id = Uuid::new_v4();
    repository
        .create(reviewer_id, "never-revoked@example.test", "hash-1")
        .await
        .unwrap();

    assert!(
        !repository
            .is_session_revoked(reviewer_id, now_epoch_seconds())
            .await
            .unwrap()
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn revoke_all_sessions_invalidates_tokens_issued_before_it_but_not_after() {
    let pool = test_pool().await;
    let repository = PostgresReviewerRepository::new(pool);
    let reviewer_id = Uuid::new_v4();
    repository
        .create(reviewer_id, "revoked@example.test", "hash-1")
        .await
        .unwrap();
    let issued_before = now_epoch_seconds() - 10;

    repository.revoke_all_sessions(reviewer_id).await.unwrap();
    let issued_after = now_epoch_seconds() + 10;

    assert!(
        repository
            .is_session_revoked(reviewer_id, issued_before)
            .await
            .unwrap()
    );
    assert!(
        !repository
            .is_session_revoked(reviewer_id, issued_after)
            .await
            .unwrap()
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn an_unknown_reviewer_id_is_treated_as_revoked() {
    let pool = test_pool().await;
    let repository = PostgresReviewerRepository::new(pool);
    assert!(
        repository
            .is_session_revoked(Uuid::new_v4(), now_epoch_seconds())
            .await
            .unwrap()
    );
}

async fn audit_row(
    pool: &PgPool,
    action: &str,
    request_id: Uuid,
) -> Option<(String, Option<Uuid>, Option<Uuid>, serde_json::Value)> {
    sqlx::query_as(
        "SELECT actor_type, actor_id, resource_id, metadata FROM audit_events \
         WHERE action = $1 AND request_id = $2",
    )
    .bind(action)
    .bind(request_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn record_registration_writes_an_audit_event_for_the_registering_actor() {
    let pool = test_pool().await;
    let repository = PostgresReviewerRepository::new(pool.clone());
    let reviewer_id = Uuid::new_v4();
    let registering_reviewer = Uuid::new_v4();
    let request_id = Uuid::new_v4();

    repository
        .record_registration(
            reviewer_id,
            Actor::Reviewer(registering_reviewer),
            request_id,
        )
        .await
        .unwrap();

    let (actor_type, actor_id, resource_id, _) =
        audit_row(&pool, "REVIEWER_REGISTERED", request_id)
            .await
            .unwrap();
    assert_eq!(actor_type, "REVIEWER");
    assert_eq!(actor_id, Some(registering_reviewer));
    assert_eq!(resource_id, Some(reviewer_id));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn record_login_success_writes_an_audit_event_for_the_reviewer_who_logged_in() {
    let pool = test_pool().await;
    let repository = PostgresReviewerRepository::new(pool.clone());
    let reviewer_id = Uuid::new_v4();
    let request_id = Uuid::new_v4();

    repository
        .record_login_success(reviewer_id, request_id)
        .await
        .unwrap();

    let (_, actor_id, resource_id, _) = audit_row(&pool, "REVIEWER_LOGIN_SUCCEEDED", request_id)
        .await
        .unwrap();
    assert_eq!(actor_id, Some(reviewer_id));
    assert_eq!(resource_id, Some(reviewer_id));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn record_login_failure_writes_an_audit_event_carrying_the_attempted_email() {
    let pool = test_pool().await;
    let repository = PostgresReviewerRepository::new(pool.clone());
    let request_id = Uuid::new_v4();

    repository
        .record_login_failure(None, "nobody@example.test", request_id)
        .await
        .unwrap();

    let (_, _, resource_id, metadata) = audit_row(&pool, "REVIEWER_LOGIN_FAILED", request_id)
        .await
        .unwrap();
    assert_eq!(resource_id, None);
    assert_eq!(metadata["email"], serde_json::json!("nobody@example.test"));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn record_login_failure_still_carries_the_reviewer_id_for_a_known_email() {
    let pool = test_pool().await;
    let repository = PostgresReviewerRepository::new(pool.clone());
    let reviewer_id = Uuid::new_v4();
    let request_id = Uuid::new_v4();

    repository
        .record_login_failure(Some(reviewer_id), "known@example.test", request_id)
        .await
        .unwrap();

    let (_, _, resource_id, _) = audit_row(&pool, "REVIEWER_LOGIN_FAILED", request_id)
        .await
        .unwrap();
    assert_eq!(resource_id, Some(reviewer_id));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn record_logout_writes_an_audit_event_for_the_reviewer_who_logged_out() {
    let pool = test_pool().await;
    let repository = PostgresReviewerRepository::new(pool.clone());
    let reviewer_id = Uuid::new_v4();
    let request_id = Uuid::new_v4();

    repository
        .record_logout(reviewer_id, request_id)
        .await
        .unwrap();

    let (_, actor_id, resource_id, _) = audit_row(&pool, "REVIEWER_LOGOUT", request_id)
        .await
        .unwrap();
    assert_eq!(actor_id, Some(reviewer_id));
    assert_eq!(resource_id, Some(reviewer_id));
}
