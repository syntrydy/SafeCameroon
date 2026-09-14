//! Reading back the generic `audit_events` table (docs/SECURITY_PRIVACY.md
//! section 6). Every other repository in this crate only ever *writes* to
//! this table, through its own small private helper (`insert_audit_event`
//! in `alerts.rs`/`cases.rs`/`deliveries.rs`, `record_audit_event` in
//! `reviewers.rs`) — this is the first (and only) one that reads it, since
//! nothing about auditing one specific resource needs a cross-resource
//! query, but viewing the trail (`Capability::ViewAudit`) does.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEventRecord {
    pub id: Uuid,
    pub actor_type: String,
    pub actor_id: Option<Uuid>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub request_id: Option<Uuid>,
    pub metadata: Value,
    /// Postgres's default text rendering of the underlying `TIMESTAMPTZ`
    /// (e.g. `"2026-01-15 10:30:00.123456+00"`) — kept as `String` rather
    /// than decoded into a Rust date/time type, so this crate does not need
    /// sqlx's `time`/`chrono` feature, consistent with how every other
    /// timestamp in this codebase is handled (epoch-second arithmetic in
    /// `ReviewerSessionTokenIssuer`/`PostgresRateLimiter`, `now()` done
    /// entirely in SQL elsewhere).
    pub occurred_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct AuditEventFilter {
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub action: Option<String>,
    pub actor_id: Option<Uuid>,
}

#[derive(Clone)]
pub struct PostgresAuditEventRepository {
    pool: PgPool,
}

impl PostgresAuditEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Most recent first. `limit`/`offset` are taken as given — the API
    /// layer is responsible for clamping `limit` to a sane maximum before
    /// it ever reaches here.
    pub async fn list(
        &self,
        filter: &AuditEventFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditEventRecord>, sqlx::Error> {
        #[allow(clippy::type_complexity)]
        let rows: Vec<(
            Uuid,
            String,
            Option<Uuid>,
            String,
            String,
            Option<Uuid>,
            Option<Uuid>,
            Value,
            String,
        )> = sqlx::query_as(
            r#"
            SELECT id, actor_type, actor_id, action, resource_type, resource_id, request_id,
                   metadata, occurred_at::text
            FROM audit_events
            WHERE ($1::text IS NULL OR resource_type = $1)
              AND ($2::uuid IS NULL OR resource_id = $2)
              AND ($3::text IS NULL OR action = $3)
              AND ($4::uuid IS NULL OR actor_id = $4)
            ORDER BY occurred_at DESC, id DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(&filter.resource_type)
        .bind(filter.resource_id)
        .bind(&filter.action)
        .bind(filter.actor_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    actor_type,
                    actor_id,
                    action,
                    resource_type,
                    resource_id,
                    request_id,
                    metadata,
                    occurred_at,
                )| AuditEventRecord {
                    id,
                    actor_type,
                    actor_id,
                    action,
                    resource_type,
                    resource_id,
                    request_id,
                    metadata,
                    occurred_at,
                },
            )
            .collect())
    }
}
