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

    /// Registers a consumer that manages itself rather than being
    /// reviewer-managed (`apps/api/src/citizen_subscriptions.rs`): only the
    /// SHA-256 digest of its management token is ever persisted, mirroring
    /// how report reference codes are hashed.
    pub async fn create_with_management_token(
        &self,
        consumer: &Consumer,
        management_token_hash: &[u8],
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO consumers (id, name, consumer_type, management_token_hash) \
             VALUES ($1, $2, $3::consumer_type, $4)",
        )
        .bind(consumer.id().as_uuid())
        .bind(consumer.name())
        .bind(consumer.consumer_type().as_database_value())
        .bind(management_token_hash)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// `None` when the consumer doesn't exist or is reviewer-managed (no
    /// token was ever set) -- the caller (application layer) is responsible
    /// for the constant-time comparison against a presented token's hash.
    pub async fn management_token_hash(
        &self,
        consumer_id: ConsumerId,
    ) -> Result<Option<Vec<u8>>, sqlx::Error> {
        let row: Option<(Option<Vec<u8>>,)> =
            sqlx::query_as("SELECT management_token_hash FROM consumers WHERE id = $1")
                .bind(consumer_id.as_uuid())
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|(hash,)| hash))
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
