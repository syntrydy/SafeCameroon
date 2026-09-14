use std::env;

use safe_cameroon_application::rate_limit::{RateLimitScope, RateLimiter};
use safe_cameroon_infrastructure::postgres::PostgresRateLimiter;
use sqlx::{PgPool, postgres::PgPoolOptions};

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
    sqlx::query("TRUNCATE rate_limit_windows")
        .execute(&pool)
        .await
        .expect("test tables must be reset");
    pool
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn allows_requests_up_to_the_scopes_limit_then_blocks() {
    let pool = test_pool().await;
    let limiter = PostgresRateLimiter::new(pool);
    let scope = RateLimitScope::ReviewerLoginAttempt;

    for attempt in 1..=scope.limit() {
        assert!(
            limiter
                .record_and_check(scope, "someone@example.test")
                .await
                .unwrap(),
            "attempt {attempt} should still be within the limit"
        );
    }
    assert!(
        !limiter
            .record_and_check(scope, "someone@example.test")
            .await
            .unwrap(),
        "the next attempt must be refused"
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn distinct_sources_and_scopes_are_tracked_independently() {
    let pool = test_pool().await;
    let limiter = PostgresRateLimiter::new(pool);

    for _ in 0..RateLimitScope::ReviewerLoginAttempt.limit() {
        limiter
            .record_and_check(RateLimitScope::ReviewerLoginAttempt, "a@example.test")
            .await
            .unwrap();
    }
    assert!(
        !limiter
            .record_and_check(RateLimitScope::ReviewerLoginAttempt, "a@example.test")
            .await
            .unwrap()
    );
    assert!(
        limiter
            .record_and_check(RateLimitScope::ReviewerLoginAttempt, "b@example.test")
            .await
            .unwrap(),
        "a different source key must have its own budget"
    );
    assert!(
        limiter
            .record_and_check(RateLimitScope::AnonymousReportSubmission, "a@example.test")
            .await
            .unwrap(),
        "a different scope must have its own budget for the same source key"
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn concurrent_requests_from_the_same_source_never_exceed_the_limit() {
    let pool = test_pool().await;
    let database_url = env::var("TEST_DATABASE_URL").unwrap();
    let concurrent_pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("test database must be reachable");
    sqlx::query("TRUNCATE rate_limit_windows")
        .execute(&pool)
        .await
        .unwrap();

    let scope = RateLimitScope::ReviewerLoginAttempt;
    let mut workers = Vec::new();
    for _ in 0..20 {
        let limiter = PostgresRateLimiter::new(concurrent_pool.clone());
        workers.push(tokio::spawn(async move {
            limiter
                .record_and_check(scope, "contested@example.test")
                .await
                .unwrap()
        }));
    }

    let mut allowed = 0;
    for worker in workers {
        if worker.await.unwrap() {
            allowed += 1;
        }
    }
    assert_eq!(
        allowed,
        scope.limit(),
        "exactly the scope's limit of requests must be allowed across all concurrent callers"
    );
}
