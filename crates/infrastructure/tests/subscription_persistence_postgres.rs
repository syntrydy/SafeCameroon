use std::env;

use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_domain::{
    AlertVisibility, ChannelEndpoint, ChannelType, Comparison, ConsumerId, DeliveryPreference,
    DeliveryStrategy, GeoArea, IncidentType, Severity, Subscription, SubscriptionId,
    SubscriptionRule,
};
use safe_cameroon_infrastructure::postgres::{
    PostgresDeliveryPreferenceRepository, PostgresSubscriptionRepository, SubscriptionUpdateOutcome,
};
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
    sqlx::query("TRUNCATE consumer_delivery_preferences, subscriptions, audit_events")
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
            SubscriptionRule::Geography(vec![
                GeoArea::new("Douala - Bonamoussadi").unwrap(),
                GeoArea::new("Yaounde").unwrap(),
            ]),
            SubscriptionRule::Visibility(vec![AlertVisibility::Community, AlertVisibility::Public]),
        ],
    )
    .unwrap();

    repository.create(&subscription).await.unwrap();

    let loaded = repository
        .find_by_id(subscription.id())
        .await
        .unwrap()
        .unwrap();
    // Field-by-field rather than a whole-struct assert_eq!: `subscription`
    // is the in-memory, pre-persist value (created_at is still None), while
    // `loaded` carries the DB-assigned created_at, which this test cannot
    // predict.
    assert_eq!(loaded.id(), subscription.id());
    assert_eq!(loaded.consumer_id(), subscription.consumer_id());
    assert_eq!(loaded.version(), subscription.version());
    assert_eq!(loaded.rules(), subscription.rules());
    assert!(loaded.created_at().is_some());
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
async fn list_all_returns_every_subscription_regardless_of_consumer() {
    let pool = test_pool().await;
    let repository = PostgresSubscriptionRepository::new(pool);
    let rule = || SubscriptionRule::IncidentType(vec![IncidentType::MissingChild]);

    let a = Subscription::new(SubscriptionId::new(), ConsumerId::new(), 1, vec![rule()]).unwrap();
    let b = Subscription::new(SubscriptionId::new(), ConsumerId::new(), 1, vec![rule()]).unwrap();
    repository.create(&a).await.unwrap();
    repository.create(&b).await.unwrap();

    let all = repository.list_all().await.unwrap();
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|s| s.id() == a.id()));
    assert!(all.iter().any(|s| s.id() == b.id()));
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

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn updating_a_subscription_persists_the_new_rules_and_version_and_audits_it() {
    let pool = test_pool().await;
    let repository = PostgresSubscriptionRepository::new(pool.clone());
    let mut subscription = Subscription::new(
        SubscriptionId::new(),
        ConsumerId::new(),
        1,
        vec![SubscriptionRule::IncidentType(vec![
            IncidentType::MissingChild,
        ])],
    )
    .unwrap();
    repository.create(&subscription).await.unwrap();

    subscription
        .update_rules(vec![SubscriptionRule::Geography(vec![
            GeoArea::new("Douala").unwrap(),
        ])])
        .unwrap();
    let reviewer = Actor::Reviewer(Uuid::new_v4());
    let request_id = Uuid::new_v4();
    let outcome = repository
        .update(&subscription, reviewer, request_id)
        .await
        .unwrap();
    assert_eq!(outcome, SubscriptionUpdateOutcome::Updated);

    let reloaded = repository
        .find_by_id(subscription.id())
        .await
        .unwrap()
        .unwrap();
    // See the created_at comment in the round-trip test above.
    assert_eq!(reloaded.id(), subscription.id());
    assert_eq!(reloaded.consumer_id(), subscription.consumer_id());
    assert_eq!(reloaded.version(), subscription.version());
    assert_eq!(reloaded.rules(), subscription.rules());
    assert!(reloaded.created_at().is_some());

    let (count,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM audit_events WHERE resource_id = $1 AND action = 'SUBSCRIPTION_UPDATED' AND request_id = $2",
    )
    .bind(subscription.id().as_uuid())
    .bind(request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn updating_a_subscription_from_a_stale_read_is_a_conflict() {
    let pool = test_pool().await;
    let repository = PostgresSubscriptionRepository::new(pool);
    let subscription = Subscription::new(
        SubscriptionId::new(),
        ConsumerId::new(),
        1,
        vec![SubscriptionRule::IncidentType(vec![
            IncidentType::MissingChild,
        ])],
    )
    .unwrap();
    repository.create(&subscription).await.unwrap();

    let mut first_reader = subscription.clone();
    let mut second_reader = subscription.clone();
    first_reader
        .update_rules(vec![SubscriptionRule::Geography(vec![
            GeoArea::new("Douala").unwrap(),
        ])])
        .unwrap();
    second_reader
        .update_rules(vec![SubscriptionRule::Geography(vec![
            GeoArea::new("Yaounde").unwrap(),
        ])])
        .unwrap();

    let actor = Actor::Reviewer(Uuid::new_v4());
    assert_eq!(
        repository
            .update(&first_reader, actor, Uuid::new_v4())
            .await
            .unwrap(),
        SubscriptionUpdateOutcome::Updated
    );
    assert_eq!(
        repository
            .update(&second_reader, actor, Uuid::new_v4())
            .await
            .unwrap(),
        SubscriptionUpdateOutcome::Conflict,
        "a second writer from a stale read must not silently overwrite the first update"
    );
}
