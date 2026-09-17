use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_application::delivery_workflow::{
    DeliveryAttemptTransition, DeliveryTransition, PlannedDelivery, delivery_attempt_event_payload,
    delivery_requested_event_payload, delivery_transition_event_payload, start_delivery_attempt,
};
use safe_cameroon_domain::{
    AlertId, ChannelType, ConsumerId, Delivery, DeliveryAttempt, DeliveryAttemptId,
    DeliveryAttemptOutcome, DeliveryId, DeliveryStatus, MatchedSubscription, RetryPolicy,
    SubscriptionId,
};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// `matching_subscriptions` is stored as a JSONB array of
/// `{"subscription_id": ..., "subscription_version": ...}` objects rather
/// than a plain `UUID[]`, so the rule version a delivery actually matched
/// under survives a later edit to that subscription
/// (docs/SUBSCRIPTION_ENGINE.md section 11) — mirrors how
/// `subscriptions.rules`/`consumer_delivery_preferences.channels` already
/// store closed Rust types as JSONB.
fn matched_subscription_to_json(matched: &MatchedSubscription) -> Value {
    json!({
        "subscription_id": matched.subscription_id.as_uuid(),
        "subscription_version": matched.subscription_version,
    })
}

fn matched_subscriptions_to_json(matched: &[MatchedSubscription]) -> Value {
    Value::Array(matched.iter().map(matched_subscription_to_json).collect())
}

fn matched_subscription_from_json(value: &Value) -> MatchedSubscription {
    let subscription_id = value["subscription_id"].as_str().expect(
        "deliveries.matching_subscriptions entries are written by this repository with a \"subscription_id\" string",
    );
    let subscription_id = Uuid::parse_str(subscription_id)
        .expect("deliveries.matching_subscriptions subscription_id is written as a valid UUID");
    let subscription_version = value["subscription_version"].as_u64().expect(
        "deliveries.matching_subscriptions entries are written by this repository with a \"subscription_version\" number",
    );
    MatchedSubscription {
        subscription_id: SubscriptionId::from_uuid(subscription_id),
        subscription_version: subscription_version as u32,
    }
}

/// Column shape shared by every query that reads a full delivery row.
#[allow(clippy::type_complexity)]
type DeliveryRow = (
    Uuid,   // alert_id
    Uuid,   // consumer_id
    String, // channel
    String, // endpoint_address
    i16,    // tier
    Value,  // matching_subscriptions
    String, // status
    i32,    // attempt_count
    i32,    // max_attempts
    i64,    // aggregate_version
);

fn delivery_from_row(delivery_id: DeliveryId, row: DeliveryRow) -> Delivery {
    let (
        alert_id,
        consumer_id,
        channel,
        endpoint_address,
        tier,
        matching_subscriptions,
        status,
        attempt_count,
        max_attempts,
        aggregate_version,
    ) = row;

    Delivery::reconstitute(
        delivery_id,
        safe_cameroon_domain::AlertId::from_uuid(alert_id),
        ConsumerId::from_uuid(consumer_id),
        ChannelType::from_database_value(&channel)
            .expect("deliveries.channel is constrained by the channel_type enum"),
        endpoint_address,
        tier as u8,
        matching_subscriptions
            .as_array()
            .expect(
                "deliveries.matching_subscriptions is constrained to be a JSON array by the CHECK constraint",
            )
            .iter()
            .map(matched_subscription_from_json)
            .collect(),
        RetryPolicy::new(max_attempts as u32)
            .expect("deliveries.max_attempts is constrained to be positive"),
        DeliveryStatus::from_database_value(&status)
            .expect("deliveries.status is constrained by the delivery_status enum"),
        attempt_count as u32,
        aggregate_version as u64,
    )
}

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
        let row: Option<DeliveryRow> = sqlx::query_as(
            r#"
            SELECT alert_id, consumer_id, channel::text, endpoint_address, tier,
                   matching_subscriptions, status::text, attempt_count, max_attempts,
                   aggregate_version
            FROM deliveries
            WHERE id = $1
            "#,
        )
        .bind(delivery_id.as_uuid())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| delivery_from_row(delivery_id, row)))
    }

    /// Every delivery planned for `alert_id` (docs/OBSERVABILITY.md: "which
    /// deliveries were attempted for this alert?").
    pub async fn find_by_alert_id(&self, alert_id: AlertId) -> Result<Vec<Delivery>, sqlx::Error> {
        #[allow(clippy::type_complexity)]
        let rows: Vec<(
            Uuid,
            Uuid,
            Uuid,
            String,
            String,
            i16,
            Value,
            String,
            i32,
            i32,
            i64,
        )> = sqlx::query_as(
            r#"
                SELECT id, alert_id, consumer_id, channel::text, endpoint_address, tier,
                       matching_subscriptions, status::text, attempt_count, max_attempts,
                       aggregate_version
                FROM deliveries
                WHERE alert_id = $1
                ORDER BY created_at
                "#,
        )
        .bind(alert_id.as_uuid())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    alert_id,
                    consumer_id,
                    channel,
                    endpoint_address,
                    tier,
                    matching_subscriptions,
                    status,
                    attempt_count,
                    max_attempts,
                    aggregate_version,
                )| {
                    delivery_from_row(
                        DeliveryId::from_uuid(id),
                        (
                            alert_id,
                            consumer_id,
                            channel,
                            endpoint_address,
                            tier,
                            matching_subscriptions,
                            status,
                            attempt_count,
                            max_attempts,
                            aggregate_version,
                        ),
                    )
                },
            )
            .collect())
    }

    /// A delivery's full attempt history in order (docs/OBSERVABILITY.md:
    /// "why did a delivery fail?") — written on every attempt
    /// (`insert_delivery_attempt`) but, until now, never read back anywhere.
    pub async fn find_attempts(
        &self,
        delivery_id: DeliveryId,
    ) -> Result<Vec<DeliveryAttempt>, sqlx::Error> {
        #[allow(clippy::type_complexity)]
        let rows: Vec<(Uuid, i32, String, Option<String>, Option<bool>, Option<String>)> =
            sqlx::query_as(
                r#"
                SELECT id, attempt_number, outcome::text, provider_message_id, retryable, failure_reason
                FROM delivery_attempts
                WHERE delivery_id = $1
                ORDER BY attempt_number
                "#,
            )
            .bind(delivery_id.as_uuid())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(
                |(id, attempt_number, outcome, provider_message_id, retryable, failure_reason)| {
                    let outcome = match outcome.as_str() {
                        "SENT" => DeliveryAttemptOutcome::Sent {
                            provider_message_id,
                        },
                        "FAILED" => DeliveryAttemptOutcome::Failed {
                            retryable: retryable
                                .expect("delivery_attempts FAILED rows always carry retryable"),
                            reason: failure_reason
                                .expect("delivery_attempts FAILED rows always carry failure_reason"),
                        },
                        other => panic!(
                            "unknown delivery_attempts.outcome {other:?}, constrained by the delivery_attempt_outcome enum"
                        ),
                    };
                    DeliveryAttempt {
                        id: DeliveryAttemptId::from_uuid(id),
                        delivery_id,
                        attempt_number: attempt_number as u32,
                        outcome,
                    }
                },
            )
            .collect())
    }

    /// Looks up the delivery a provider webhook callback refers to
    /// (docs/API.md section 7). A delivery attempt only ever sets
    /// `provider_message_id` on a `SENT` outcome, so the most recent such
    /// attempt identifies the delivery unambiguously.
    pub async fn find_by_provider_message_id(
        &self,
        provider_message_id: &str,
    ) -> Result<Option<Delivery>, sqlx::Error> {
        let delivery_id: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT delivery_id
            FROM delivery_attempts
            WHERE provider_message_id = $1 AND outcome = 'SENT'
            ORDER BY attempt_number DESC
            LIMIT 1
            "#,
        )
        .bind(provider_message_id)
        .fetch_optional(&self.pool)
        .await?;

        match delivery_id {
            Some(id) => self.find_by_id(DeliveryId::from_uuid(id)).await,
            None => Ok(None),
        }
    }

    /// Dequeues up to `limit` deliveries ready to be sent (`QUEUED` or
    /// `RETRYING`) and starts each one's attempt, all in one transaction.
    /// `FOR UPDATE SKIP LOCKED` is what makes this safe under concurrent
    /// workers: a row already locked by another in-flight `claim_next` call
    /// is simply skipped rather than waited on or double-claimed
    /// (docs/EVENTS.md section 9, prompt 07: "test duplicate jobs and
    /// concurrent workers"). Every returned delivery is already `SENDING`
    /// with its `DELIVERY_STARTED` event/audit/outbox row persisted; the
    /// caller only needs to dispatch it to a channel and report the outcome.
    ///
    /// A tier-`N` delivery (`N` > 0, i.e. `PrimaryFallback`/`PriorityList`'s
    /// fallback tiers -- `DeliveryPreference::tiers`) is excluded until every
    /// lower-tier delivery for the same `alert_id`/`consumer_id` has reached
    /// `FAILED_PERMANENTLY`. If a lower tier is still in flight, retrying, or
    /// already succeeded, the fallback simply never becomes claimable (a
    /// successful primary makes the fallback row permanently moot, which is
    /// correct -- there is nothing left to fall back from). The `NOT EXISTS`
    /// subquery is not part of the outer `FROM`, so `FOR UPDATE OF d` locks
    /// only the rows this call may actually claim, never the lower-tier rows
    /// it merely reads to decide gating (avoiding needless lock contention
    /// with whatever is concurrently dispatching those tier-0 rows).
    pub async fn claim_next(&self, limit: i64) -> Result<Vec<Delivery>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        #[allow(clippy::type_complexity)]
        let rows: Vec<(
            Uuid,
            Uuid,
            Uuid,
            String,
            String,
            i16,
            Value,
            String,
            i32,
            i32,
            i64,
        )> = sqlx::query_as(
            r#"
            SELECT d.id, d.alert_id, d.consumer_id, d.channel::text, d.endpoint_address, d.tier,
                   d.matching_subscriptions, d.status::text, d.attempt_count, d.max_attempts,
                   d.aggregate_version
            FROM deliveries d
            WHERE d.status IN ('QUEUED', 'RETRYING')
              AND (d.next_attempt_at IS NULL OR d.next_attempt_at <= now())
              AND (
                d.tier = 0
                OR NOT EXISTS (
                  SELECT 1 FROM deliveries lower
                  WHERE lower.alert_id = d.alert_id
                    AND lower.consumer_id = d.consumer_id
                    AND lower.tier < d.tier
                    AND lower.status <> 'FAILED_PERMANENTLY'
                )
              )
            ORDER BY d.created_at
            LIMIT $1
            FOR UPDATE OF d SKIP LOCKED
            "#,
        )
        .bind(limit)
        .fetch_all(&mut *tx)
        .await?;

        let mut claimed = Vec::with_capacity(rows.len());
        for (
            id,
            alert_id,
            consumer_id,
            channel,
            endpoint_address,
            tier,
            matching_subscriptions,
            status,
            attempt_count,
            max_attempts,
            aggregate_version,
        ) in rows
        {
            let mut delivery = delivery_from_row(
                DeliveryId::from_uuid(id),
                (
                    alert_id,
                    consumer_id,
                    channel,
                    endpoint_address,
                    tier,
                    matching_subscriptions,
                    status,
                    attempt_count,
                    max_attempts,
                    aggregate_version,
                ),
            );
            // The WHERE clause above guarantees Queued/Retrying, and this
            // transaction holds the row's lock, so this can never fail.
            let transition =
                start_delivery_attempt(&mut delivery, Actor::Automated, Uuid::new_v4())
                    .expect("a claimed row is always Queued or Retrying");
            advance_delivery(&mut tx, &delivery)
                .await?
                .then_some(())
                .expect("a locked row is never modified concurrently");
            insert_delivery_event(&mut tx, delivery.id(), &transition.event, transition.actor)
                .await?;
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
                delivery_transition_event_payload(&delivery, &transition),
            )
            .await?;
            claimed.push(delivery);
        }
        tx.commit().await?;
        Ok(claimed)
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
    let matching_subscriptions = matched_subscriptions_to_json(delivery.matching_subscriptions());
    let result = sqlx::query(
        r#"
        INSERT INTO deliveries (
            id, alert_id, consumer_id, channel, endpoint_address, tier,
            idempotency_key_hash, matching_subscriptions, status, attempt_count,
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
    .bind(matching_subscriptions)
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
    // `Some(..)` only while `delivery` is Retrying (`Delivery::retry_backoff`);
    // computed as a plain seconds float and turned into a point in time
    // entirely in SQL (`now() + make_interval(...)`) so no date/time crate is
    // needed on the Rust side, mirroring `ReviewerSessionTokenIssuer`/
    // `PostgresRateLimiter`'s own epoch-second arithmetic.
    let backoff_seconds = delivery
        .retry_backoff()
        .map(|backoff| backoff.as_secs_f64());
    let result = sqlx::query(
        r#"
        UPDATE deliveries
        SET status = $1::delivery_status, attempt_count = $2, aggregate_version = $3, updated_at = now(),
            next_attempt_at = CASE WHEN $6::float8 IS NULL THEN NULL ELSE now() + make_interval(secs => $6) END
        WHERE id = $4 AND aggregate_version = $5
        "#,
    )
    .bind(delivery.status().as_database_value())
    .bind(delivery.attempt_count() as i32)
    .bind(delivery.version() as i64)
    .bind(delivery.id().as_uuid())
    .bind(expected_previous_version)
    .bind(backoff_seconds)
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
