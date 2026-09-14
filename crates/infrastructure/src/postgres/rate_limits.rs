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

/// Well past the longest scope window today (1 hour,
/// `RateLimitScope::AnonymousReportSubmission`) — deliberately a single
/// generous cutoff applied to every scope rather than an exact per-scope
/// one, so adding a scope with a longer window later can't silently make
/// this delete live rows early (AGENTS.md: "do not overengineer the first
/// release").
const RETENTION: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

impl PostgresRateLimiter {
    /// Deletes window rows old enough that no scope's window could still be
    /// open for them — maintenance, not part of the `RateLimiter` port
    /// itself (nothing about checking/recording a limit needs this).
    /// Returns how many rows were removed.
    pub async fn delete_expired_windows(&self) -> Result<u64, sqlx::Error> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_secs() as i64;
        let cutoff = now - RETENTION.as_secs() as i64;

        let result =
            sqlx::query("DELETE FROM rate_limit_windows WHERE window_start_epoch_seconds < $1")
                .bind(cutoff)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }
}
