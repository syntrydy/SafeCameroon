//! Persistence for [`Subscription`] (docs/SUBSCRIPTION_ENGINE.md). Rules are
//! stored as a JSONB array rather than a normalized table, since the closed
//! Rust rule set in `crates/domain/src/subscription.rs` is the single source
//! of truth for what a valid rule looks like — every row here is written by
//! this repository from an already-validated `Subscription`, so read-back
//! only ever `.expect()`s the shape it itself wrote (mirrors how
//! `PostgresDeliveryRepository` trusts its own enum columns).

use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_domain::{
    AlertVisibility, CaseEventType, Comparison, ConsumerId, GeoArea, IncidentType, Severity,
    Subscription, SubscriptionId, SubscriptionRule,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

fn rule_to_json(rule: &SubscriptionRule) -> Value {
    match rule {
        SubscriptionRule::IncidentType(values) => json!({
            "rule": "INCIDENT_TYPE",
            "values": values,
        }),
        SubscriptionRule::Severity { operator, value } => json!({
            "rule": "SEVERITY",
            "operator": operator.as_database_value(),
            "value": value,
        }),
        SubscriptionRule::EventType(values) => json!({
            "rule": "EVENT_TYPE",
            "values": values,
        }),
        SubscriptionRule::Geography(areas) => json!({
            "rule": "GEOGRAPHY",
            "areas": areas.iter().map(GeoArea::as_str).collect::<Vec<_>>(),
        }),
        SubscriptionRule::Visibility(values) => json!({
            "rule": "VISIBILITY",
            "values": values,
        }),
    }
}

fn rules_to_json(rules: &[SubscriptionRule]) -> Value {
    Value::Array(rules.iter().map(rule_to_json).collect())
}

fn rule_from_json(value: &Value) -> SubscriptionRule {
    let rule = value["rule"]
        .as_str()
        .expect("subscriptions.rules entries are written by this repository with a \"rule\" tag");
    match rule {
        "INCIDENT_TYPE" => SubscriptionRule::IncidentType(
            serde_json::from_value::<Vec<IncidentType>>(value["values"].clone())
                .expect("subscriptions.rules INCIDENT_TYPE values are written by this repository"),
        ),
        "SEVERITY" => SubscriptionRule::Severity {
            operator: Comparison::from_database_value(
                value["operator"]
                    .as_str()
                    .expect("subscriptions.rules SEVERITY entries carry an \"operator\" string"),
            )
            .expect(
                "subscriptions.rules SEVERITY operator is constrained to Comparison::as_database_value's output",
            ),
            value: serde_json::from_value::<Severity>(value["value"].clone())
                .expect("subscriptions.rules SEVERITY value is written by this repository"),
        },
        "EVENT_TYPE" => SubscriptionRule::EventType(
            serde_json::from_value::<Vec<CaseEventType>>(value["values"].clone())
                .expect("subscriptions.rules EVENT_TYPE values are written by this repository"),
        ),
        "GEOGRAPHY" => {
            // Rows written before multi-area support carried a single
            // "area" string; rows written since carry an "areas" array.
            // Reading both keeps old, already-persisted subscriptions
            // working without a backfill migration.
            let area_names: Vec<String> = if let Some(areas) = value["areas"].as_array() {
                areas
                    .iter()
                    .map(|area| {
                        area.as_str()
                            .expect(
                                "subscriptions.rules GEOGRAPHY areas entries are strings",
                            )
                            .to_owned()
                    })
                    .collect()
            } else {
                vec![
                    value["area"]
                        .as_str()
                        .expect(
                            "subscriptions.rules GEOGRAPHY entries carry an \"area\" or \"areas\" field",
                        )
                        .to_owned(),
                ]
            };
            SubscriptionRule::Geography(
                area_names
                    .into_iter()
                    .map(|name| {
                        GeoArea::new(name).expect(
                            "subscriptions.rules GEOGRAPHY areas are written non-blank by this repository",
                        )
                    })
                    .collect(),
            )
        }
        "VISIBILITY" => SubscriptionRule::Visibility(
            serde_json::from_value::<Vec<AlertVisibility>>(value["values"].clone())
                .expect("subscriptions.rules VISIBILITY values are written by this repository"),
        ),
        other => panic!("unknown subscription rule tag {other:?} in subscriptions.rules"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionUpdateOutcome {
    Updated,
    /// The in-memory subscription was mutated from a stale read; the caller
    /// must reload it and retry rather than silently overwrite a concurrent
    /// change (mirrors `PostgresAlertRepository::cancel`).
    Conflict,
}

type SubscriptionRow = (Uuid, Uuid, i32, Value, String);

fn subscription_from_row(row: SubscriptionRow) -> Subscription {
    let (id, consumer_id, version, rules, created_at) = row;
    let rules = rules
        .as_array()
        .expect("subscriptions.rules is constrained to be a JSON array by the CHECK constraint")
        .iter()
        .map(rule_from_json)
        .collect();
    Subscription::new(
        SubscriptionId::from_uuid(id),
        ConsumerId::from_uuid(consumer_id),
        version as u32,
        rules,
    )
    .expect("subscriptions.rules is constrained to be non-empty by the CHECK constraint")
    .with_created_at(created_at)
}

#[derive(Clone)]
pub struct PostgresSubscriptionRepository {
    pool: PgPool,
}

impl PostgresSubscriptionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, subscription: &Subscription) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO subscriptions (id, consumer_id, version, rules)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(subscription.id().as_uuid())
        .bind(subscription.consumer_id().as_uuid())
        .bind(subscription.version() as i32)
        .bind(rules_to_json(subscription.rules()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(
        &self,
        id: SubscriptionId,
    ) -> Result<Option<Subscription>, sqlx::Error> {
        let row: Option<SubscriptionRow> = sqlx::query_as(
            "SELECT id, consumer_id, version, rules, created_at::text FROM subscriptions WHERE id = $1",
        )
        .bind(id.as_uuid())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(subscription_from_row))
    }

    pub async fn find_by_consumer(
        &self,
        consumer_id: ConsumerId,
    ) -> Result<Vec<Subscription>, sqlx::Error> {
        let rows: Vec<SubscriptionRow> = sqlx::query_as(
            "SELECT id, consumer_id, version, rules, created_at::text FROM subscriptions \
             WHERE consumer_id = $1 ORDER BY created_at",
        )
        .bind(consumer_id.as_uuid())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(subscription_from_row).collect())
    }

    /// Every subscription in the system, with no candidate filtering
    /// (docs/SUBSCRIPTION_ENGINE.md section 7 defers indexed/geographic
    /// filtering until an actual query pattern needs it — "do not
    /// overengineer the first release", AGENTS.md). The caller runs
    /// `safe_cameroon_domain::evaluate_subscriptions` against the result.
    pub async fn list_all(&self) -> Result<Vec<Subscription>, sqlx::Error> {
        let rows: Vec<SubscriptionRow> = sqlx::query_as(
            "SELECT id, consumer_id, version, rules, created_at::text FROM subscriptions \
             ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(subscription_from_row).collect())
    }

    /// A paginated, most-recently-created-first counterpart to `list_all`
    /// for the admin-facing `GET /v1/subscriptions` listing
    /// (apps/api/src/subscriptions.rs::list_all_subscriptions) -- kept
    /// separate so `list_all`'s unfiltered/unpaginated contract for the
    /// worker's matching engine never changes.
    pub async fn list_all_page(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Subscription>, sqlx::Error> {
        let rows: Vec<SubscriptionRow> = sqlx::query_as(
            "SELECT id, consumer_id, version, rules, created_at::text FROM subscriptions \
             ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(subscription_from_row).collect())
    }

    /// `subscription` must already reflect the desired new state (i.e. the
    /// caller already called `Subscription::update_rules` on it); this
    /// applies it with optimistic concurrency on `version` and records an
    /// audit event, both in one transaction. `create` does not audit itself
    /// yet — a separate, deliberate follow-up (see #22/#29) rather than
    /// changing that widely-used method's signature here.
    pub async fn update(
        &self,
        subscription: &Subscription,
        actor: Actor,
        request_id: Uuid,
    ) -> Result<SubscriptionUpdateOutcome, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let expected_previous_version = subscription.version() as i32 - 1;
        let result = sqlx::query(
            r#"
            UPDATE subscriptions
            SET version = $1, rules = $2, updated_at = now()
            WHERE id = $3 AND version = $4
            "#,
        )
        .bind(subscription.version() as i32)
        .bind(rules_to_json(subscription.rules()))
        .bind(subscription.id().as_uuid())
        .bind(expected_previous_version)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() != 1 {
            transaction.rollback().await?;
            return Ok(SubscriptionUpdateOutcome::Conflict);
        }

        sqlx::query(
            r#"
            INSERT INTO audit_events (id, actor_type, actor_id, action, resource_type, resource_id, request_id)
            VALUES ($1, $2, $3, 'SUBSCRIPTION_UPDATED', 'SUBSCRIPTION', $4, $5)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(actor.as_database_value())
        .bind(actor.actor_id())
        .bind(subscription.id().as_uuid())
        .bind(request_id)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(SubscriptionUpdateOutcome::Updated)
    }

    /// Used only by self-service cancellation today
    /// (`apps/api/src/citizen_subscriptions.rs`) -- reviewer-managed
    /// subscriptions have no cancel/delete endpoint yet, matching
    /// `create`'s own "not yet audited" precedent rather than adding an
    /// audit event to a method no caller needs one from.
    pub async fn delete(&self, id: SubscriptionId) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM subscriptions WHERE id = $1")
            .bind(id.as_uuid())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
