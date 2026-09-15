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

    /// Claims up to `limit` unclaimed, unpublished rows of `event_type`,
    /// marking them *claimed* — not published — in the same transaction
    /// (mirrors `PostgresDeliveryRepository::claim_next`'s
    /// `FOR UPDATE SKIP LOCKED` pattern, so concurrent workers never
    /// double-claim the same row). The caller must call [`mark_published`]
    /// on each event only once it has actually finished processing that
    /// event; marking a whole batch published up front here would mean a
    /// later event in the batch is silently treated as done the moment an
    /// earlier one fails, even though it was never processed.
    ///
    /// [`mark_published`]: Self::mark_published
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
            WHERE event_type = $1 AND published_at IS NULL AND claimed_at IS NULL
            ORDER BY created_at
            LIMIT $2
            FOR UPDATE SKIP LOCKED
            "#,
        )
        .bind(event_type)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await?;

        mark_claimed(&mut tx, rows.iter().map(|(id, _, _)| *id)).await?;
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

    /// Marks one previously claimed event published — call this only after
    /// its processing has actually succeeded. Claiming here means "handed
    /// to a consumer"; published means "processed", not "delivered
    /// externally" — there is no external publisher yet, only in-process
    /// worker steps.
    pub async fn mark_published(&self, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE outbox_events SET published_at = now() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

async fn mark_claimed(
    transaction: &mut Transaction<'_, Postgres>,
    ids: impl Iterator<Item = Uuid>,
) -> Result<(), sqlx::Error> {
    let ids: Vec<Uuid> = ids.collect();
    if ids.is_empty() {
        return Ok(());
    }
    sqlx::query("UPDATE outbox_events SET claimed_at = now() WHERE id = ANY($1)")
        .bind(&ids)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}
