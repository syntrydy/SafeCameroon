//! Persistence for organizations and reviewer role/organization membership
//! (migration 0021). One repository serves both concerns, mirroring
//! `PostgresReviewerRepository` also owning session revocation: they are
//! two facets of the same "who is this reviewer, and what may they do"
//! question.

use safe_cameroon_domain::{
    AlertVisibility, ConsumerId, IncidentType, Membership, Organization, OrganizationId, Role,
};
use sqlx::PgPool;
use uuid::Uuid;

fn parse_incident_types(values: Vec<String>) -> Vec<IncidentType> {
    values
        .iter()
        .map(|value| {
            IncidentType::from_database_value(value).expect(
                "organizations.verified_incident_types only ever holds valid IncidentType values",
            )
        })
        .collect()
}

fn parse_alert_visibilities(values: Vec<String>) -> Vec<AlertVisibility> {
    values
        .iter()
        .map(|value| {
            AlertVisibility::from_database_value(value)
                .expect("organizations.verified_alert_visibilities only ever holds valid AlertVisibility values")
        })
        .collect()
}

#[allow(clippy::type_complexity)]
type OrganizationRow = (
    String,
    Option<String>,
    Option<String>,
    Vec<String>,
    Vec<String>,
    Option<Uuid>,
    bool,
);
#[allow(clippy::type_complexity)]
type OrganizationListRow = (
    Uuid,
    String,
    Option<String>,
    Option<String>,
    Vec<String>,
    Vec<String>,
    Option<Uuid>,
    bool,
);

pub struct MembershipRecord {
    pub reviewer_id: Uuid,
    pub email: String,
    pub role: Role,
}

#[derive(Clone)]
pub struct PostgresOrganizationRepository {
    pool: PgPool,
}

impl PostgresOrganizationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, organization: &Organization) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO organizations (id, name, description, location) VALUES ($1, $2, $3, $4)",
        )
        .bind(organization.id().as_uuid())
        .bind(organization.name())
        .bind(organization.description())
        .bind(organization.location())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Option<Organization>, sqlx::Error> {
        let row: Option<OrganizationRow> = sqlx::query_as(
            "SELECT name, description, location, verified_incident_types, \
             verified_alert_visibilities, consumer_id, is_active \
             FROM organizations WHERE id = $1",
        )
        .bind(organization_id.as_uuid())
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(
                name,
                description,
                location,
                incident_types,
                visibilities,
                consumer_id,
                is_active,
            )| {
                Organization::reconstitute(
                    organization_id,
                    name,
                    description,
                    location,
                    parse_incident_types(incident_types),
                    parse_alert_visibilities(visibilities),
                    consumer_id.map(ConsumerId::from_uuid),
                    is_active,
                )
            },
        ))
    }

    /// Most recently created first, mirroring every other list endpoint.
    /// Returns every organization regardless of `is_active`, so a platform
    /// admin can still find and reactivate a deactivated one.
    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Organization>, sqlx::Error> {
        let rows: Vec<OrganizationListRow> = sqlx::query_as(
            "SELECT id, name, description, location, verified_incident_types, \
             verified_alert_visibilities, consumer_id, is_active \
             FROM organizations ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    name,
                    description,
                    location,
                    incident_types,
                    visibilities,
                    consumer_id,
                    is_active,
                )| {
                    Organization::reconstitute(
                        OrganizationId::from_uuid(id),
                        name,
                        description,
                        location,
                        parse_incident_types(incident_types),
                        parse_alert_visibilities(visibilities),
                        consumer_id.map(ConsumerId::from_uuid),
                        is_active,
                    )
                },
            )
            .collect())
    }

    /// Updates the organization's description/location wholesale -- both
    /// nullable, so `None` clears the field rather than leaving it
    /// unchanged (matching `set_trust_grants`'s wholesale-replace
    /// semantics, not an incremental patch).
    pub async fn set_profile(
        &self,
        organization_id: OrganizationId,
        description: Option<&str>,
        location: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE organizations SET description = $2, location = $3 WHERE id = $1")
            .bind(organization_id.as_uuid())
            .bind(description)
            .bind(location)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Flips the soft-deactivate flag (migration 0026). Never deletes the
    /// organization or anything it owns -- see `Organization::is_active`'s
    /// doc comment.
    pub async fn set_active(
        &self,
        organization_id: OrganizationId,
        is_active: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE organizations SET is_active = $2 WHERE id = $1")
            .bind(organization_id.as_uuid())
            .bind(is_active)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Links this organization to the consumer alerts are actually
    /// delivered through (see `Organization::consumer_id`'s doc comment) —
    /// a one-time set at creation time
    /// (`apps/api/src/organizations.rs::create_organization`), not a
    /// wholesale replace like `set_trust_grants`.
    pub async fn set_consumer_id(
        &self,
        organization_id: OrganizationId,
        consumer_id: ConsumerId,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE organizations SET consumer_id = $2 WHERE id = $1")
            .bind(organization_id.as_uuid())
            .bind(consumer_id.as_uuid())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The organization, if any, that owns this consumer (`consumer_id` is
    /// `UNIQUE` on `organizations`, so at most one) — used to scope reviewer
    /// access to a consumer's subscriptions/delivery preference to their own
    /// organization (`authorize_consumer_management`).
    pub async fn find_organization_id_by_consumer_id(
        &self,
        consumer_id: ConsumerId,
    ) -> Result<Option<OrganizationId>, sqlx::Error> {
        let row: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM organizations WHERE consumer_id = $1")
                .bind(consumer_id.as_uuid())
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(id,)| OrganizationId::from_uuid(id)))
    }

    /// Replaces both trust grants wholesale — there is no incremental
    /// add/remove semantics to preserve, only "what is this organization
    /// currently trusted for."
    pub async fn set_trust_grants(
        &self,
        organization_id: OrganizationId,
        verified_incident_types: &[IncidentType],
        verified_alert_visibilities: &[AlertVisibility],
    ) -> Result<(), sqlx::Error> {
        let incident_types: Vec<&'static str> = verified_incident_types
            .iter()
            .map(|value| value.as_database_value())
            .collect();
        let visibilities: Vec<&'static str> = verified_alert_visibilities
            .iter()
            .map(|value| value.as_database_value())
            .collect();
        sqlx::query(
            "UPDATE organizations SET verified_incident_types = $2, verified_alert_visibilities = $3 \
             WHERE id = $1",
        )
        .bind(organization_id.as_uuid())
        .bind(&incident_types)
        .bind(&visibilities)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn add_membership(
        &self,
        reviewer_id: Uuid,
        role: Role,
        organization_id: Option<OrganizationId>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO reviewer_organization_memberships (reviewer_id, role, organization_id) \
             VALUES ($1, $2::reviewer_role, $3)",
        )
        .bind(reviewer_id)
        .bind(role.as_database_value())
        .bind(organization_id.map(OrganizationId::as_uuid))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_membership(
        &self,
        reviewer_id: Uuid,
    ) -> Result<Option<Membership>, sqlx::Error> {
        let row: Option<(String, Option<Uuid>)> = sqlx::query_as(
            "SELECT role::text, organization_id FROM reviewer_organization_memberships WHERE reviewer_id = $1",
        )
        .bind(reviewer_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(role, organization_id)| Membership {
            role: Role::from_database_value(&role).expect(
                "reviewer_organization_memberships.role is constrained by the reviewer_role enum",
            ),
            organization_id: organization_id.map(OrganizationId::from_uuid),
        }))
    }

    pub async fn list_members(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Vec<MembershipRecord>, sqlx::Error> {
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            "SELECT r.id, r.email, m.role::text \
             FROM reviewer_organization_memberships m \
             JOIN reviewers r ON r.id = m.reviewer_id \
             WHERE m.organization_id = $1 \
             ORDER BY r.email",
        )
        .bind(organization_id.as_uuid())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(reviewer_id, email, role)| MembershipRecord {
                reviewer_id,
                email,
                role: Role::from_database_value(&role)
                    .expect("reviewer_organization_memberships.role is constrained by the reviewer_role enum"),
            })
            .collect())
    }
}
