use std::env;

use safe_cameroon_domain::{AlertVisibility, IncidentType, Organization, OrganizationId, Role};
use safe_cameroon_infrastructure::postgres::{
    PostgresOrganizationRepository, PostgresReviewerRepository,
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
    // alerts FK-references organizations (migration 0027) -- must be
    // truncated together, matching apps/api/src/main.rs's test_pool.
    sqlx::query(
        "TRUNCATE delivery_events, delivery_attempts, deliveries, alert_events, alert_fields, \
         alerts, reviewer_organization_memberships, organizations, reviewers",
    )
    .execute(&pool)
    .await
    .expect("test tables must be reset");
    pool
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_freshly_created_organization_round_trips_with_no_trust_grants() {
    let pool = test_pool().await;
    let repository = PostgresOrganizationRepository::new(pool);
    let organization = Organization::new("Douala Police").unwrap();

    repository.create(&organization).await.unwrap();

    let loaded = repository
        .find_by_id(organization.id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.name(), "Douala Police");
    assert!(loaded.verified_incident_types().is_empty());
    assert!(loaded.verified_alert_visibilities().is_empty());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn find_by_id_returns_none_for_an_unknown_organization() {
    let pool = test_pool().await;
    let repository = PostgresOrganizationRepository::new(pool);

    assert!(
        repository
            .find_by_id(OrganizationId::from_uuid(Uuid::new_v4()))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn set_trust_grants_replaces_both_lists_wholesale() {
    let pool = test_pool().await;
    let repository = PostgresOrganizationRepository::new(pool);
    let organization = Organization::new("Douala Police").unwrap();
    repository.create(&organization).await.unwrap();

    repository
        .set_trust_grants(
            organization.id(),
            &[IncidentType::MissingChild],
            &[AlertVisibility::Community, AlertVisibility::Public],
        )
        .await
        .unwrap();

    let loaded = repository
        .find_by_id(organization.id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        loaded.verified_incident_types(),
        [IncidentType::MissingChild]
    );
    assert_eq!(
        loaded.verified_alert_visibilities(),
        [AlertVisibility::Community, AlertVisibility::Public]
    );

    // A second call replaces, rather than appends to, the first.
    repository
        .set_trust_grants(organization.id(), &[], &[])
        .await
        .unwrap();
    let cleared = repository
        .find_by_id(organization.id())
        .await
        .unwrap()
        .unwrap();
    assert!(cleared.verified_incident_types().is_empty());
    assert!(cleared.verified_alert_visibilities().is_empty());
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn list_returns_every_organization_most_recently_created_first() {
    let pool = test_pool().await;
    let repository = PostgresOrganizationRepository::new(pool);
    let first = Organization::new("Douala Police").unwrap();
    repository.create(&first).await.unwrap();
    let second = Organization::new("Yaounde NGO").unwrap();
    repository.create(&second).await.unwrap();

    let listed = repository.list(50, 0).await.unwrap();

    assert_eq!(
        listed.iter().map(Organization::id).collect::<Vec<_>>(),
        vec![second.id(), first.id()]
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_platform_admin_membership_has_no_organization() {
    let pool = test_pool().await;
    let reviewers = PostgresReviewerRepository::new(pool.clone());
    let organizations = PostgresOrganizationRepository::new(pool);
    let reviewer_id = Uuid::new_v4();
    reviewers
        .create(reviewer_id, "admin@example.test")
        .await
        .unwrap();

    organizations
        .add_membership(reviewer_id, Role::PlatformAdmin, None)
        .await
        .unwrap();

    let membership = organizations
        .find_membership(reviewer_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(membership.role, Role::PlatformAdmin);
    assert_eq!(membership.organization_id, None);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn a_member_membership_carries_its_organization() {
    let pool = test_pool().await;
    let reviewers = PostgresReviewerRepository::new(pool.clone());
    let organizations = PostgresOrganizationRepository::new(pool);
    let reviewer_id = Uuid::new_v4();
    reviewers
        .create(reviewer_id, "member@example.test")
        .await
        .unwrap();
    let organization = Organization::new("Douala Police").unwrap();
    organizations.create(&organization).await.unwrap();

    organizations
        .add_membership(reviewer_id, Role::Member, Some(organization.id()))
        .await
        .unwrap();

    let membership = organizations
        .find_membership(reviewer_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(membership.role, Role::Member);
    assert_eq!(membership.organization_id, Some(organization.id()));
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn find_membership_returns_none_for_a_reviewer_with_no_membership_row() {
    let pool = test_pool().await;
    let organizations = PostgresOrganizationRepository::new(pool);

    assert!(
        organizations
            .find_membership(Uuid::new_v4())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
async fn list_members_returns_only_that_organizations_reviewers_with_email_and_role() {
    let pool = test_pool().await;
    let reviewers = PostgresReviewerRepository::new(pool.clone());
    let organizations = PostgresOrganizationRepository::new(pool);
    let organization = Organization::new("Douala Police").unwrap();
    organizations.create(&organization).await.unwrap();
    let other_organization = Organization::new("Yaounde NGO").unwrap();
    organizations.create(&other_organization).await.unwrap();

    let member_id = Uuid::new_v4();
    reviewers
        .create(member_id, "member@example.test")
        .await
        .unwrap();
    organizations
        .add_membership(member_id, Role::Member, Some(organization.id()))
        .await
        .unwrap();

    let elsewhere_id = Uuid::new_v4();
    reviewers
        .create(elsewhere_id, "elsewhere@example.test")
        .await
        .unwrap();
    organizations
        .add_membership(elsewhere_id, Role::Member, Some(other_organization.id()))
        .await
        .unwrap();

    let members = organizations.list_members(organization.id()).await.unwrap();

    assert_eq!(members.len(), 1);
    assert_eq!(members[0].reviewer_id, member_id);
    assert_eq!(members[0].email, "member@example.test");
    assert_eq!(members[0].role, Role::Member);
}
