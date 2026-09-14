use std::env;

use safe_cameroon_domain::{
    ChannelEndpoint, ChannelType, Comparison, ConsumerId, DeliveryPreference, DeliveryStrategy,
    GeoArea, IncidentType, Severity, Subscription, SubscriptionId, SubscriptionRule,
};
use safe_cameroon_infrastructure::postgres::{
    PostgresDeliveryPreferenceRepository, PostgresSubscriptionRepository,
};
use sqlx::{PgPool, postgres::PgPoolOptions};

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
    sqlx::query("TRUNCATE consumer_delivery_preferences, subscriptions")
        .execute(&pool)
        .await
        .expect("test tables must be reset");
    pool
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_subscription_with_every_rule_type_round_trips_through_postgres() {
    let pool = test_pool().await;
    let repository = PostgresSubscriptionRepository::new(pool);

    let consumer_id = ConsumerId::new();
    let subscription = Subscription::new(
        SubscriptionId::new(),
        consumer_id,
        3,
        vec![
            SubscriptionRule::IncidentType(vec![
                IncidentType::MissingChild,
                IncidentType::OtherProtectionIncident,
            ]),
            SubscriptionRule::Severity {
                operator: Comparison::GreaterThanOrEqual,
                value: Severity::High,
            },
            SubscriptionRule::EventType(vec![safe_cameroon_domain::CaseEventType::CaseVerified]),
            SubscriptionRule::Geography(GeoArea::new("Douala - Bonamoussadi").unwrap()),
        ],
    )
    .unwrap();

    repository.create(&subscription).await.unwrap();

    let loaded = repository
        .find_by_id(subscription.id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded, subscription);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn find_by_consumer_returns_only_that_consumers_subscriptions() {
    let pool = test_pool().await;
    let repository = PostgresSubscriptionRepository::new(pool);

    let consumer_a = ConsumerId::new();
    let consumer_b = ConsumerId::new();
    let rule = || SubscriptionRule::IncidentType(vec![IncidentType::MissingChild]);

    let a1 = Subscription::new(SubscriptionId::new(), consumer_a, 1, vec![rule()]).unwrap();
    let a2 = Subscription::new(SubscriptionId::new(), consumer_a, 1, vec![rule()]).unwrap();
    let b1 = Subscription::new(SubscriptionId::new(), consumer_b, 1, vec![rule()]).unwrap();

    for subscription in [&a1, &a2, &b1] {
        repository.create(subscription).await.unwrap();
    }

    let found = repository.find_by_consumer(consumer_a).await.unwrap();
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|s| s.consumer_id() == consumer_a));
    assert!(found.iter().any(|s| s.id() == a1.id()));
    assert!(found.iter().any(|s| s.id() == a2.id()));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn find_by_id_returns_none_for_an_unknown_subscription() {
    let pool = test_pool().await;
    let repository = PostgresSubscriptionRepository::new(pool);
    assert_eq!(
        repository.find_by_id(SubscriptionId::new()).await.unwrap(),
        None
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_delivery_preference_round_trips_and_can_be_replaced() {
    let pool = test_pool().await;
    let repository = PostgresDeliveryPreferenceRepository::new(pool);
    let consumer_id = ConsumerId::new();

    let initial = DeliveryPreference::new(
        DeliveryStrategy::PrimaryFallback,
        vec![
            ChannelEndpoint::new(ChannelType::WhatsApp, "+237600000000").unwrap(),
            ChannelEndpoint::new(ChannelType::Sms, "+237600000001").unwrap(),
        ],
    )
    .unwrap();
    repository.upsert(consumer_id, &initial).await.unwrap();

    let loaded = repository
        .find_by_consumer(consumer_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded, initial);

    let replacement = DeliveryPreference::new(
        DeliveryStrategy::All,
        vec![ChannelEndpoint::new(ChannelType::Email, "someone@example.test").unwrap()],
    )
    .unwrap();
    repository.upsert(consumer_id, &replacement).await.unwrap();

    let reloaded = repository
        .find_by_consumer(consumer_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reloaded, replacement);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn find_by_consumer_returns_none_when_no_preference_is_set() {
    let pool = test_pool().await;
    let repository = PostgresDeliveryPreferenceRepository::new(pool);
    assert_eq!(
        repository
            .find_by_consumer(ConsumerId::new())
            .await
            .unwrap(),
        None
    );
}
