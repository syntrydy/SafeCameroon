use std::env;

use safe_cameroon_domain::{Consumer, ConsumerId, ConsumerType};
use safe_cameroon_infrastructure::postgres::PostgresConsumerRepository;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

async fn test_pool() -> PgPool {
    let database_url = env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must point to a dedicated PostgreSQL test database");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("test database must be reachable");

    let (database_name,): (String,) = sqlx::query_as("SELECT current_database()")
        .fetch_one(&pool)
        .await
        .expect("test database name must be readable");
    assert!(
        database_name.to_ascii_lowercase().contains("test"),
        "TEST_DATABASE_URL must target a database with 'test' in its name"
    );

    MIGRATOR.run(&pool).await.expect("migrations must apply");
    // `organizations` FK-references `consumers` (migration 0025), so it must
    // be truncated in the same statement.
    sqlx::query("TRUNCATE organizations, consumers")
        .execute(&pool)
        .await
        .expect("test tables must be reset");
    pool
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn persists_and_reconstitutes_a_consumer() {
    let pool = test_pool().await;
    let repository = PostgresConsumerRepository::new(pool);
    let consumer = Consumer::new("Douala Police", ConsumerType::Organization).unwrap();

    repository.create(&consumer).await.unwrap();

    let loaded = repository.find_by_id(consumer.id()).await.unwrap().unwrap();
    assert_eq!(loaded.id(), consumer.id());
    assert_eq!(loaded.name(), "Douala Police");
    assert_eq!(loaded.consumer_type(), ConsumerType::Organization);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn find_by_id_returns_none_for_an_unknown_consumer() {
    let pool = test_pool().await;
    let repository = PostgresConsumerRepository::new(pool);

    assert!(
        repository
            .find_by_id(ConsumerId::from_uuid(Uuid::new_v4()))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_citizen_consumer_round_trips_too() {
    let pool = test_pool().await;
    let repository = PostgresConsumerRepository::new(pool);
    let consumer = Consumer::new("Amina N.", ConsumerType::Citizen).unwrap();

    repository.create(&consumer).await.unwrap();

    let loaded = repository.find_by_id(consumer.id()).await.unwrap().unwrap();
    assert_eq!(loaded.consumer_type(), ConsumerType::Citizen);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_self_service_consumers_management_token_hash_round_trips() {
    let pool = test_pool().await;
    let repository = PostgresConsumerRepository::new(pool);
    let consumer = Consumer::new("Citizen (self-subscribed)", ConsumerType::Citizen).unwrap();
    let hash = b"a-fake-32-byte-sha256-digest-1234".to_vec();

    repository
        .create_with_management_token(&consumer, &hash)
        .await
        .unwrap();

    let loaded = repository.find_by_id(consumer.id()).await.unwrap().unwrap();
    assert_eq!(loaded.id(), consumer.id());
    assert_eq!(
        repository
            .management_token_hash(consumer.id())
            .await
            .unwrap(),
        Some(hash)
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn management_token_hash_is_none_for_a_reviewer_managed_consumer() {
    let pool = test_pool().await;
    let repository = PostgresConsumerRepository::new(pool);
    let consumer = Consumer::new("Douala Police", ConsumerType::Organization).unwrap();
    repository.create(&consumer).await.unwrap();

    assert_eq!(
        repository
            .management_token_hash(consumer.id())
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn management_token_hash_is_none_for_an_unknown_consumer() {
    let pool = test_pool().await;
    let repository = PostgresConsumerRepository::new(pool);

    assert_eq!(
        repository
            .management_token_hash(ConsumerId::from_uuid(Uuid::new_v4()))
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn list_returns_every_consumer_most_recently_registered_first() {
    let pool = test_pool().await;
    let repository = PostgresConsumerRepository::new(pool);
    let first = Consumer::new("Douala Police", ConsumerType::Organization).unwrap();
    repository.create(&first).await.unwrap();
    let second = Consumer::new("Amina N.", ConsumerType::Citizen).unwrap();
    repository.create(&second).await.unwrap();

    let listed = repository.list(None, 50, 0).await.unwrap();

    assert_eq!(
        listed.iter().map(Consumer::id).collect::<Vec<_>>(),
        vec![second.id(), first.id()]
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn list_filters_by_consumer_type() {
    let pool = test_pool().await;
    let repository = PostgresConsumerRepository::new(pool);
    let organization = Consumer::new("Douala Police", ConsumerType::Organization).unwrap();
    repository.create(&organization).await.unwrap();
    let citizen = Consumer::new("Amina N.", ConsumerType::Citizen).unwrap();
    repository.create(&citizen).await.unwrap();

    let listed = repository
        .list(Some(ConsumerType::Citizen), 50, 0)
        .await
        .unwrap();

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id(), citizen.id());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn list_respects_limit_and_offset() {
    let pool = test_pool().await;
    let repository = PostgresConsumerRepository::new(pool);
    for name in ["Consumer A", "Consumer B", "Consumer C"] {
        let consumer = Consumer::new(name, ConsumerType::Organization).unwrap();
        repository.create(&consumer).await.unwrap();
    }

    let page = repository.list(None, 1, 1).await.unwrap();
    assert_eq!(page.len(), 1);
}
