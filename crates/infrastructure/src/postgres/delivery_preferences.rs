//! Persistence for a consumer's [`DeliveryPreference`]
//! (docs/SUBSCRIPTION_ENGINE.md section 9). One row per consumer — a
//! consumer has one set of contact endpoints and one fan-out strategy for
//! every alert it receives (mirrors `crates/domain/src/delivery.rs`'s doc
//! comment). Setting a new preference replaces the prior one outright; there
//! is no versioning requirement here, unlike subscriptions.

use safe_cameroon_domain::{ChannelEndpoint, ChannelType, ConsumerId, DeliveryPreference};
use serde_json::{Value, json};
use sqlx::PgPool;

fn channel_endpoint_to_json(endpoint: &ChannelEndpoint) -> Value {
    json!({
        "channel": endpoint.channel().as_database_value(),
        "address": endpoint.address(),
    })
}

fn channels_to_json(channels: &[ChannelEndpoint]) -> Value {
    Value::Array(channels.iter().map(channel_endpoint_to_json).collect())
}

fn channel_endpoint_from_json(value: &Value) -> ChannelEndpoint {
    let channel = ChannelType::from_database_value(
        value["channel"].as_str().expect(
            "consumer_delivery_preferences.channels entries are written by this repository with a \"channel\" string",
        ),
    )
    .expect(
        "consumer_delivery_preferences.channels channel values are constrained to ChannelType::as_database_value's output",
    );
    let address = value["address"].as_str().expect(
        "consumer_delivery_preferences.channels entries are written by this repository with an \"address\" string",
    );
    ChannelEndpoint::new(channel, address).expect(
        "consumer_delivery_preferences rows are written by this repository from an already-validated ChannelEndpoint",
    )
}

#[derive(Clone)]
pub struct PostgresDeliveryPreferenceRepository {
    pool: PgPool,
}

impl PostgresDeliveryPreferenceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Replaces `consumer_id`'s delivery preference outright.
    pub async fn upsert(
        &self,
        consumer_id: ConsumerId,
        preference: &DeliveryPreference,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO consumer_delivery_preferences (consumer_id, strategy, channels)
            VALUES ($1, $2::delivery_strategy, $3)
            ON CONFLICT (consumer_id) DO UPDATE
            SET strategy = EXCLUDED.strategy, channels = EXCLUDED.channels, updated_at = now()
            "#,
        )
        .bind(consumer_id.as_uuid())
        .bind(preference.strategy().as_database_value())
        .bind(channels_to_json(preference.channels()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_consumer(
        &self,
        consumer_id: ConsumerId,
    ) -> Result<Option<DeliveryPreference>, sqlx::Error> {
        let row: Option<(String, Value)> = sqlx::query_as(
            "SELECT strategy::text, channels FROM consumer_delivery_preferences WHERE consumer_id = $1",
        )
        .bind(consumer_id.as_uuid())
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(strategy, channels)| {
            let strategy = safe_cameroon_domain::DeliveryStrategy::from_database_value(&strategy)
                .expect(
                    "consumer_delivery_preferences.strategy is constrained by the delivery_strategy enum",
                );
            let channels = channels
                .as_array()
                .expect(
                    "consumer_delivery_preferences.channels is constrained to be a JSON array by the CHECK constraint",
                )
                .iter()
                .map(channel_endpoint_from_json)
                .collect();
            DeliveryPreference::new(strategy, channels).expect(
                "consumer_delivery_preferences rows are written by this repository from an already-validated DeliveryPreference",
            )
        }))
    }
}
