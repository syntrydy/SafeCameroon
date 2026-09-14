//! Durable, Postgres-backed [`RateLimiter`]: a fixed-window counter,
//! incremented with an atomic `INSERT ... ON CONFLICT DO UPDATE ...
//! RETURNING`, so concurrent requests from the same source race-safely
//! share one counter instead of each reading-then-writing a stale count.

use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use safe_cameroon_application::rate_limit::{RateLimitError, RateLimitScope, RateLimiter};
use sqlx::PgPool;

#[derive(Clone)]
pub struct PostgresRateLimiter {
    pool: PgPool,
}

impl PostgresRateLimiter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RateLimiter for PostgresRateLimiter {
    async fn record_and_check(
        &self,
        scope: RateLimitScope,
        source_key: &str,
    ) -> Result<bool, RateLimitError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_secs() as i64;
        let window_seconds = scope.window().as_secs() as i64;
        let window_start = now - (now % window_seconds);

        let (request_count,): (i32,) = sqlx::query_as(
            r#"
            INSERT INTO rate_limit_windows (scope, source_key, window_start_epoch_seconds, request_count)
            VALUES ($1, $2, $3, 1)
            ON CONFLICT (scope, source_key, window_start_epoch_seconds)
            DO UPDATE SET request_count = rate_limit_windows.request_count + 1
            RETURNING request_count
            "#,
        )
        .bind(scope.as_database_value())
        .bind(source_key)
        .bind(window_start)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| RateLimitError {
            reason: error.to_string(),
        })?;

        Ok((request_count as u32) <= scope.limit())
    }
}
