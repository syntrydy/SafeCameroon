use safe_cameroon_domain::{Consumer, ConsumerId, ConsumerType};
use sqlx::PgPool;

#[derive(Clone)]
pub struct PostgresConsumerRepository {
    pool: PgPool,
}

impl PostgresConsumerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, consumer: &Consumer) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO consumers (id, name, consumer_type) VALUES ($1, $2, $3::consumer_type)",
        )
        .bind(consumer.id().as_uuid())
        .bind(consumer.name())
        .bind(consumer.consumer_type().as_database_value())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(
        &self,
        consumer_id: ConsumerId,
    ) -> Result<Option<Consumer>, sqlx::Error> {
        let row: Option<(String, String)> =
            sqlx::query_as("SELECT name, consumer_type::text FROM consumers WHERE id = $1")
                .bind(consumer_id.as_uuid())
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.map(|(name, consumer_type)| {
            Consumer::reconstitute(
                consumer_id,
                name,
                ConsumerType::from_database_value(&consumer_type)
                    .expect("consumers.consumer_type is constrained by the consumer_type enum"),
            )
        }))
    }
}
