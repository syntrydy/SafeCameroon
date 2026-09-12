use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_application::delivery_workflow::{
    DeliveryAttemptTransition, DeliveryTransition, PlannedDelivery, delivery_attempt_event_payload,
    delivery_requested_event_payload, delivery_transition_event_payload,
};
use safe_cameroon_domain::{
    ChannelType, ConsumerId, Delivery, DeliveryAttempt, DeliveryAttemptOutcome, DeliveryId,
    DeliveryStatus, RetryPolicy, SubscriptionId,
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryTransitionOutcome {
    Applied,
    /// The in-memory delivery was mutated from a stale read; the caller must
    /// reload it and retry rather than silently overwrite a concurrent
    /// change (the same conflict guard `PostgresAlertRepository::cancel` uses).
    Conflict,
}

#[derive(Clone)]
pub struct PostgresDeliveryRepository {
    pool: PgPool,
}

impl PostgresDeliveryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(
        &self,
        delivery_id: DeliveryId,
    ) -> Result<Option<Delivery>, sqlx::Error> {
        #[allow(clippy::type_complexity)]
        let Some(row): Option<(
            Uuid,
            Uuid,
            String,
            String,
            i16,
            Vec<Uuid>,
            String,
            i32,
            i32,
            i64,
        )> = sqlx::query_as(
            r#"
            SELECT alert_id, consumer_id, channel::text, endpoint_address, tier,
                   matching_subscription_ids, status::text, attempt_count, max_attempts,
                   aggregate_version
            FROM deliveries
            WHERE id = $1
            "#,
        )
        .bind(delivery_id.as_uuid())
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        let (
            alert_id,
            consumer_id,
            channel,
            endpoint_address,
            tier,
            matching_subscription_ids,
            status,
            attempt_count,
            max_attempts,
            aggregate_version,
        ) = row;

        Ok(Some(Delivery::reconstitute(
            delivery_id,
            safe_cameroon_domain::AlertId::from_uuid(alert_id),
            ConsumerId::from_uuid(consumer_id),
            ChannelType::from_database_value(&channel)
                .expect("deliveries.channel is constrained by the channel_type enum"),
            endpoint_address,
            tier as u8,
            matching_subscription_ids
                .into_iter()
                .map(SubscriptionId::from_uuid)
                .collect(),
            RetryPolicy::new(max_attempts as u32)
                .expect("deliveries.max_attempts is constrained to be positive"),
            DeliveryStatus::from_database_value(&status)
                .expect("deliveries.status is constrained by the delivery_status enum"),
            attempt_count as u32,
            aggregate_version as u64,
        )))
    }

    /// Persists newly planned deliveries plus their `DELIVERY_REQUESTED`
    /// events, audit records, and outbox events, one transaction per
    /// delivery. A delivery whose idempotency key already exists is skipped
    /// rather than erroring, so re-running planning for the same alert never
    /// creates duplicates (mirroring `PostgresReportRepository`).
    pub async fn create_planned(&self, planned: &[PlannedDelivery]) -> Result<(), sqlx::Error> {
        for item in planned {
            let mut transaction = self.pool.begin().await?;
            if !insert_delivery(&mut transaction, item).await? {
                transaction.rollback().await?;
                continue;
            }
            insert_delivery_event(
                &mut transaction,
                item.delivery.id(),
                &item.event,
                item.actor,
            )
            .await?;
            insert_audit_event(
                &mut transaction,
                item.audit_event_id.as_uuid(),
                item.actor,
                item.event.event_type.as_database_value(),
                item.delivery.id().as_uuid(),
                item.request_id,
            )
            .await?;
            insert_outbox_event(
                &mut transaction,
                item.outbox_event_id.as_uuid(),
                item.delivery.id().as_uuid(),
                item.event.event_type.as_database_value(),
                delivery_requested_event_payload(item),
            )
            .await?;
            transaction.commit().await?;
        }
        Ok(())
    }

    /// Applies a transition with no attempt outcome (`start_attempt`,
    /// `record_delivered`). `delivery` must be the aggregate *after* the
    /// workflow function mutated it in memory.
    pub async fn apply_transition(
        &self,
        delivery: &Delivery,
        transition: &DeliveryTransition,
    ) -> Result<DeliveryTransitionOutcome, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        if !advance_delivery(&mut tx, delivery).await? {
            tx.rollback().await?;
            return Ok(DeliveryTransitionOutcome::Conflict);
        }
        insert_delivery_event(&mut tx, delivery.id(), &transition.event, transition.actor).await?;
        insert_audit_event(
            &mut tx,
            transition.audit_event_id.as_uuid(),
            transition.actor,
            transition.event.event_type.as_database_value(),
            delivery.id().as_uuid(),
            transition.request_id,
        )
        .await?;
        insert_outbox_event(
            &mut tx,
            transition.outbox_event_id.as_uuid(),
            delivery.id().as_uuid(),
            transition.event.event_type.as_database_value(),
            delivery_transition_event_payload(delivery, transition),
        )
        .await?;
        tx.commit().await?;
        Ok(DeliveryTransitionOutcome::Applied)
    }

    /// Applies a transition that also records a [`DeliveryAttempt`]
    /// (`record_success`, `record_failure`).
    pub async fn apply_attempt_transition(
        &self,
        delivery: &Delivery,
        transition: &DeliveryAttemptTransition,
    ) -> Result<DeliveryTransitionOutcome, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        if !advance_delivery(&mut tx, delivery).await? {
            tx.rollback().await?;
            return Ok(DeliveryTransitionOutcome::Conflict);
        }
        insert_delivery_attempt(&mut tx, &transition.attempt).await?;
        insert_delivery_event(&mut tx, delivery.id(), &transition.event, transition.actor).await?;
        insert_audit_event(
            &mut tx,
            transition.audit_event_id.as_uuid(),
            transition.actor,
            transition.event.event_type.as_database_value(),
            delivery.id().as_uuid(),
            transition.request_id,
        )
        .await?;
        insert_outbox_event(
            &mut tx,
            transition.outbox_event_id.as_uuid(),
            delivery.id().as_uuid(),
            transition.event.event_type.as_database_value(),
            delivery_attempt_event_payload(delivery, transition),
        )
        .await?;
        tx.commit().await?;
        Ok(DeliveryTransitionOutcome::Applied)
    }
}

/// Returns `false` (without inserting anything else) when a delivery with
/// the same idempotency key already exists.
async fn insert_delivery(
    transaction: &mut Transaction<'_, Postgres>,
    item: &PlannedDelivery,
) -> Result<bool, sqlx::Error> {
    let delivery = &item.delivery;
    let subscription_ids: Vec<Uuid> = delivery
        .matching_subscription_ids()
        .iter()
        .map(|id| id.as_uuid())
        .collect();
    let result = sqlx::query(
        r#"
        INSERT INTO deliveries (
            id, alert_id, consumer_id, channel, endpoint_address, tier,
            idempotency_key_hash, matching_subscription_ids, status, attempt_count,
            max_attempts, aggregate_version
        )
        VALUES (
            $1, $2, $3, $4::channel_type, $5, $6, $7, $8, $9::delivery_status, $10, $11, $12
        )
        ON CONFLICT (idempotency_key_hash) DO NOTHING
        "#,
    )
    .bind(delivery.id().as_uuid())
    .bind(delivery.alert_id().as_uuid())
    .bind(delivery.consumer_id().as_uuid())
    .bind(delivery.channel().as_database_value())
    .bind(delivery.endpoint_address())
    .bind(delivery.tier() as i16)
    .bind(&item.idempotency_key_hash)
    .bind(&subscription_ids)
    .bind(delivery.status().as_database_value())
    .bind(delivery.attempt_count() as i32)
    .bind(delivery.max_attempts() as i32)
    .bind(delivery.version() as i64)
    .execute(&mut **transaction)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Optimistically advances a delivery's `status`/`attempt_count`/
/// `aggregate_version` from `version - 1` to `version`. Returns `false` if no
/// row matched, meaning the delivery was modified concurrently since it was
/// loaded (mirrors `advance_case_version`).
async fn advance_delivery(
    transaction: &mut Transaction<'_, Postgres>,
    delivery: &Delivery,
) -> Result<bool, sqlx::Error> {
    let expected_previous_version = delivery.version() as i64 - 1;
    let result = sqlx::query(
        r#"
        UPDATE deliveries
        SET status = $1::delivery_status, attempt_count = $2, aggregate_version = $3, updated_at = now()
        WHERE id = $4 AND aggregate_version = $5
        "#,
    )
    .bind(delivery.status().as_database_value())
    .bind(delivery.attempt_count() as i32)
    .bind(delivery.version() as i64)
    .bind(delivery.id().as_uuid())
    .bind(expected_previous_version)
    .execute(&mut **transaction)
    .await?;
    Ok(result.rows_affected() == 1)
}

async fn insert_delivery_attempt(
    transaction: &mut Transaction<'_, Postgres>,
    attempt: &DeliveryAttempt,
) -> Result<(), sqlx::Error> {
    let (outcome, provider_message_id, retryable, failure_reason) = match &attempt.outcome {
        DeliveryAttemptOutcome::Sent {
            provider_message_id,
        } => ("SENT", provider_message_id.clone(), None, None),
        DeliveryAttemptOutcome::Failed { retryable, reason } => {
            ("FAILED", None, Some(*retryable), Some(reason.clone()))
        }
    };
    sqlx::query(
        r#"
        INSERT INTO delivery_attempts (
            id, delivery_id, attempt_number, outcome, provider_message_id, retryable, failure_reason
        )
        VALUES ($1, $2, $3, $4::delivery_attempt_outcome, $5, $6, $7)
        "#,
    )
    .bind(attempt.id.as_uuid())
    .bind(attempt.delivery_id.as_uuid())
    .bind(attempt.attempt_number as i32)
    .bind(outcome)
    .bind(provider_message_id)
    .bind(retryable)
    .bind(failure_reason)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_delivery_event(
    transaction: &mut Transaction<'_, Postgres>,
    delivery_id: DeliveryId,
    event: &safe_cameroon_domain::DeliveryEvent,
    actor: Actor,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO delivery_events (id, delivery_id, event_type, aggregate_version, actor_type, actor_id)
        VALUES ($1, $2, $3::delivery_event_type, $4, $5, $6)
        "#,
    )
    .bind(event.id.as_uuid())
    .bind(delivery_id.as_uuid())
    .bind(event.event_type.as_database_value())
    .bind(event.aggregate_version as i64)
    .bind(actor.as_database_value())
    .bind(actor.actor_id())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_audit_event(
    transaction: &mut Transaction<'_, Postgres>,
    audit_event_id: Uuid,
    actor: Actor,
    action: &str,
    delivery_id: Uuid,
    request_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO audit_events (id, actor_type, actor_id, action, resource_type, resource_id, request_id)
        VALUES ($1, $2, $3, $4, 'DELIVERY', $5, $6)
        "#,
    )
    .bind(audit_event_id)
    .bind(actor.as_database_value())
    .bind(actor.actor_id())
    .bind(action)
    .bind(delivery_id)
    .bind(request_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_outbox_event(
    transaction: &mut Transaction<'_, Postgres>,
    outbox_event_id: Uuid,
    delivery_id: Uuid,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO outbox_events (id, aggregate_type, aggregate_id, event_type, payload)
        VALUES ($1, 'DELIVERY', $2, $3, $4)
        "#,
    )
    .bind(outbox_event_id)
    .bind(delivery_id)
    .bind(event_type)
    .bind(payload)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
