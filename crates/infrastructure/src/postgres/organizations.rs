//! Persistence for organizations and reviewer role/organization membership
//! (migration 0021). One repository serves both concerns, mirroring
//! `PostgresReviewerRepository` also owning session revocation: they are
//! two facets of the same "who is this reviewer, and what may they do"
//! question.

use safe_cameroon_domain::{
    AlertVisibility, IncidentType, Membership, Organization, OrganizationId, Role,
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
        sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
            .bind(organization.id().as_uuid())
            .bind(organization.name())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Option<Organization>, sqlx::Error> {
        let row: Option<(String, Vec<String>, Vec<String>)> = sqlx::query_as(
            "SELECT name, verified_incident_types, verified_alert_visibilities \
             FROM organizations WHERE id = $1",
        )
        .bind(organization_id.as_uuid())
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(name, incident_types, visibilities)| {
            Organization::reconstitute(
                organization_id,
                name,
                parse_incident_types(incident_types),
                parse_alert_visibilities(visibilities),
            )
        }))
    }

    /// Most recently created first, mirroring every other list endpoint.
    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Organization>, sqlx::Error> {
        let rows: Vec<(Uuid, String, Vec<String>, Vec<String>)> = sqlx::query_as(
            "SELECT id, name, verified_incident_types, verified_alert_visibilities \
             FROM organizations ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, name, incident_types, visibilities)| {
                Organization::reconstitute(
                    OrganizationId::from_uuid(id),
                    name,
                    parse_incident_types(incident_types),
                    parse_alert_visibilities(visibilities),
                )
            })
            .collect())
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
