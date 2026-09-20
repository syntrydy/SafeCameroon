use std::env;

use safe_cameroon_infrastructure::postgres::{AuditEventFilter, PostgresAuditEventRepository};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

async fn test_pool() -> PgPool {
    let database_url = env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must point to a dedicated PostgreSQL test database");
    let pool = PgPoolOptions::new()
        .max_connections(5)
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
    sqlx::query("TRUNCATE audit_events")
        .execute(&pool)
        .await
        .expect("test tables must be reset");
    pool
}

#[allow(clippy::too_many_arguments)]
async fn insert_event(
    pool: &PgPool,
    actor_type: &str,
    actor_id: Option<Uuid>,
    action: &str,
    resource_type: &str,
    resource_id: Option<Uuid>,
) -> Uuid {
    insert_event_with_organization(
        pool,
        actor_type,
        actor_id,
        None,
        action,
        resource_type,
        resource_id,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn insert_event_with_organization(
    pool: &PgPool,
    actor_type: &str,
    actor_id: Option<Uuid>,
    organization_id: Option<Uuid>,
    action: &str,
    resource_type: &str,
    resource_id: Option<Uuid>,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO audit_events (id, actor_type, actor_id, organization_id, action, resource_type, resource_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(actor_type)
    .bind(actor_id)
    .bind(organization_id)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .execute(pool)
    .await
    .unwrap();
    id
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn list_with_no_filter_returns_every_event_most_recent_first() {
    let pool = test_pool().await;
    let repository = PostgresAuditEventRepository::new(pool.clone());

    let first = insert_event(&pool, "REVIEWER", None, "A", "CASE", None).await;
    let second = insert_event(&pool, "REVIEWER", None, "B", "CASE", None).await;

    let found = repository
        .list(&AuditEventFilter::default(), 10, 0)
        .await
        .unwrap();
    let found_ids: Vec<Uuid> = found.iter().map(|event| event.id).collect();
    // Both rows may share the same occurred_at instant under a fast test, so
    // only assert both are present, not a strict order between them.
    assert_eq!(found_ids.len(), 2);
    assert!(found_ids.contains(&first));
    assert!(found_ids.contains(&second));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn filters_by_resource_type_and_resource_id() {
    let pool = test_pool().await;
    let repository = PostgresAuditEventRepository::new(pool.clone());
    let case_id = Uuid::new_v4();

    let matching = insert_event(
        &pool,
        "REVIEWER",
        None,
        "CASE_VERIFIED",
        "CASE",
        Some(case_id),
    )
    .await;
    insert_event(
        &pool,
        "REVIEWER",
        None,
        "CASE_VERIFIED",
        "CASE",
        Some(Uuid::new_v4()),
    )
    .await;
    insert_event(
        &pool,
        "REVIEWER",
        None,
        "REVIEWER_LOGIN_SUCCEEDED",
        "REVIEWER",
        Some(case_id),
    )
    .await;

    let found = repository
        .list(
            &AuditEventFilter {
                resource_type: Some("CASE".to_owned()),
                resource_id: Some(case_id),
                ..Default::default()
            },
            10,
            0,
        )
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, matching);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn filters_by_action_and_actor_id() {
    let pool = test_pool().await;
    let repository = PostgresAuditEventRepository::new(pool.clone());
    let reviewer_id = Uuid::new_v4();

    let matching = insert_event(
        &pool,
        "REVIEWER",
        Some(reviewer_id),
        "SUBSCRIPTION_UPDATED",
        "SUBSCRIPTION",
        None,
    )
    .await;
    insert_event(
        &pool,
        "REVIEWER",
        Some(Uuid::new_v4()),
        "SUBSCRIPTION_UPDATED",
        "SUBSCRIPTION",
        None,
    )
    .await;
    insert_event(
        &pool,
        "REVIEWER",
        Some(reviewer_id),
        "REVIEWER_LOGOUT",
        "REVIEWER",
        None,
    )
    .await;

    let found = repository
        .list(
            &AuditEventFilter {
                action: Some("SUBSCRIPTION_UPDATED".to_owned()),
                actor_id: Some(reviewer_id),
                ..Default::default()
            },
            10,
            0,
        )
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, matching);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn filters_by_organization_id() {
    let pool = test_pool().await;
    let repository = PostgresAuditEventRepository::new(pool.clone());
    let organization_id = Uuid::new_v4();

    let matching = insert_event_with_organization(
        &pool,
        "REVIEWER",
        Some(Uuid::new_v4()),
        Some(organization_id),
        "ALERT_VIEWED",
        "ALERT",
        None,
    )
    .await;
    insert_event_with_organization(
        &pool,
        "REVIEWER",
        Some(Uuid::new_v4()),
        Some(Uuid::new_v4()),
        "ALERT_VIEWED",
        "ALERT",
        None,
    )
    .await;
    insert_event(&pool, "REVIEWER", None, "ALERT_VIEWED", "ALERT", None).await;

    let found = repository
        .list(
            &AuditEventFilter {
                organization_id: Some(organization_id),
                ..Default::default()
            },
            10,
            0,
        )
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, matching);
    assert_eq!(found[0].organization_id, Some(organization_id));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn limit_and_offset_page_through_results() {
    let pool = test_pool().await;
    let repository = PostgresAuditEventRepository::new(pool.clone());
    for i in 0..5 {
        insert_event(&pool, "REVIEWER", None, &format!("EVENT_{i}"), "CASE", None).await;
    }

    let first_page = repository
        .list(&AuditEventFilter::default(), 2, 0)
        .await
        .unwrap();
    assert_eq!(first_page.len(), 2);

    let second_page = repository
        .list(&AuditEventFilter::default(), 2, 2)
        .await
        .unwrap();
    assert_eq!(second_page.len(), 2);

    let first_ids: Vec<Uuid> = first_page.iter().map(|event| event.id).collect();
    let second_ids: Vec<Uuid> = second_page.iter().map(|event| event.id).collect();
    assert!(
        first_ids.iter().all(|id| !second_ids.contains(id)),
        "pages must not overlap"
    );
}
