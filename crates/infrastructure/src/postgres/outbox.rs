//! A generic consumer over `outbox_events` (docs/EVENTS.md, docs/DATABASE.md
//! section — the transactional-outbox table every aggregate already writes
//! into on creation/transition). Nothing has read from it until now; every
//! existing worker step claims straight from its own aggregate's table
//! instead (`PostgresDeliveryRepository::claim_next`). This is the first
//! actual outbox consumer, used to drive subscription matching off
//! `ALERT_CREATED` events (docs/SUBSCRIPTION_ENGINE.md) without coupling the
//! alert-creation HTTP handler to it synchronously.

use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedOutboxEvent {
    pub id: Uuid,
    pub aggregate_id: Uuid,
    pub payload: Value,
}

#[derive(Clone)]
pub struct PostgresOutboxRepository {
    pool: PgPool,
}

impl PostgresOutboxRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Claims up to `limit` unpublished rows of `event_type`, marking them
    /// published in the same transaction (mirrors
    /// `PostgresDeliveryRepository::claim_next`'s `FOR UPDATE SKIP LOCKED`
    /// pattern, so concurrent workers never double-claim the same row).
    /// Claiming here means "handed to a consumer", not "delivered
    /// externally" — there is no external publisher yet, only in-process
    /// worker steps.
    pub async fn claim_unpublished(
        &self,
        event_type: &str,
        limit: i64,
    ) -> Result<Vec<ClaimedOutboxEvent>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let rows: Vec<(Uuid, Uuid, Value)> = sqlx::query_as(
            r#"
            SELECT id, aggregate_id, payload
            FROM outbox_events
            WHERE event_type = $1 AND published_at IS NULL
            ORDER BY created_at
            LIMIT $2
            FOR UPDATE SKIP LOCKED
            "#,
        )
        .bind(event_type)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await?;

        mark_published(&mut tx, rows.iter().map(|(id, _, _)| *id)).await?;
        tx.commit().await?;

        Ok(rows
            .into_iter()
            .map(|(id, aggregate_id, payload)| ClaimedOutboxEvent {
                id,
                aggregate_id,
                payload,
            })
            .collect())
    }
}

async fn mark_published(
    transaction: &mut Transaction<'_, Postgres>,
    ids: impl Iterator<Item = Uuid>,
) -> Result<(), sqlx::Error> {
    let ids: Vec<Uuid> = ids.collect();
    if ids.is_empty() {
        return Ok(());
    }
    sqlx::query("UPDATE outbox_events SET published_at = now() WHERE id = ANY($1)")
        .bind(&ids)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}
