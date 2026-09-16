use safe_cameroon_domain::{Consumer, ConsumerId, ConsumerType};
use sqlx::PgPool;
use uuid::Uuid;

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

    /// Most recently registered first (`consumers_created_at_idx`),
    /// optionally narrowed by type. Previously there was no way to find a
    /// consumer without already knowing its id — a reviewer had to record
    /// the id returned from `create` themselves.
    pub async fn list(
        &self,
        consumer_type: Option<ConsumerType>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Consumer>, sqlx::Error> {
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            r#"
            SELECT id, name, consumer_type::text
            FROM consumers
            WHERE ($1::consumer_type IS NULL OR consumer_type = $1::consumer_type)
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(consumer_type.map(ConsumerType::as_database_value))
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, name, consumer_type)| {
                Consumer::reconstitute(
                    ConsumerId::from_uuid(id),
                    name,
                    ConsumerType::from_database_value(&consumer_type)
                        .expect("consumers.consumer_type is constrained by the consumer_type enum"),
                )
            })
            .collect())
    }
}
