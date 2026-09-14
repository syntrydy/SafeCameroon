use std::env;

use safe_cameroon_infrastructure::postgres::{CreateReviewerOutcome, PostgresReviewerRepository};
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
    sqlx::query("TRUNCATE reviewers")
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
