//! Organizations and reviewer roles (prompt 09/10; docs/OPEN_QUESTIONS.md
//! governance section). Two of that section's three open questions have a
//! resolution that does not require guessing real institutional policy:
//!
//! - "Which organizations may issue community/public alerts?"
//! - "Which authority can verify a case for each incident class?"
//!
//! are answered by making verification/alert-issuance authority an explicit
//! property of each `Organization` record — granted per real organization by
//! a `Role::PlatformAdmin` when it is onboarded — rather than a rule this
//! codebase bakes in (e.g. "police may verify, NGOs may not"). The third
//! question, case ownership across multiple organizations, stays open: case
//! visibility/review remains flat (any identified reviewer), since real
//! multi-org case routing/locking is a separate, larger workflow feature
//! this module does not attempt.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::{AlertVisibility, ConsumerId, IncidentType, OrganizationId};

/// A reviewer's standing in the platform. `PlatformAdmin` is org-independent
/// (organization membership does not apply to it); `OrgAdmin` and `Member`
/// each belong to exactly one organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Role {
    PlatformAdmin,
    OrgAdmin,
    Member,
}

impl Role {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::PlatformAdmin => "PLATFORM_ADMIN",
            Self::OrgAdmin => "ORG_ADMIN",
            Self::Member => "MEMBER",
        }
    }

    pub fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "PLATFORM_ADMIN" => Some(Self::PlatformAdmin),
            "ORG_ADMIN" => Some(Self::OrgAdmin),
            "MEMBER" => Some(Self::Member),
            _ => None,
        }
    }
}

/// A reviewer's resolved role and (unless `PlatformAdmin`) organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Membership {
    pub role: Role,
    pub organization_id: Option<OrganizationId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyOrganizationName;

impl fmt::Display for EmptyOrganizationName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "organization name cannot be blank")
    }
}

impl std::error::Error for EmptyOrganizationName {}

/// A real-world partner (police, NGO, association, school, municipality).
/// `verified_incident_types`/`verified_alert_visibilities` are the trust
/// grants a `PlatformAdmin` sets explicitly when onboarding the
/// organization — empty by default, so a newly created organization can
/// verify nothing and issue nothing until deliberately trusted.
///
/// `consumer_id` links this reviewer institution to the
/// [`Consumer`](crate::Consumer) alerts are actually delivered to — a
/// separate identity on purpose (docs/DOMAIN_MODEL.md section 9: a consumer
/// is "an organization... or a citizen", not a reviewer institution), so an
/// org admin's own subscription/delivery-preference management reuses that
/// existing pipeline instead of a second one living on `Organization`
/// itself. `None` until the application layer creates and links one (every
/// organization created going forward gets one automatically; organizations
/// that predate this field do not, until backfilled).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Organization {
    id: OrganizationId,
    name: String,
    description: Option<String>,
    location: Option<String>,
    verified_incident_types: Vec<IncidentType>,
    verified_alert_visibilities: Vec<AlertVisibility>,
    consumer_id: Option<ConsumerId>,
    /// Soft-deactivate flag (migration 0026) -- `true` for every
    /// organization created before this field existed, and for every newly
    /// created one. `false` stops the organization's own trust grants,
    /// membership grants, and consumer/delivery management from being
    /// usable (`crates/application/src/authorization.rs`), without deleting
    /// the organization, its memberships, or its subscriptions.
    is_active: bool,
}

impl Organization {
    pub fn new(name: impl Into<String>) -> Result<Self, EmptyOrganizationName> {
        let name = name.into();
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(EmptyOrganizationName);
        }
        Ok(Self {
            id: OrganizationId::new(),
            name: trimmed.to_owned(),
            description: None,
            location: None,
            verified_incident_types: Vec::new(),
            verified_alert_visibilities: Vec::new(),
            consumer_id: None,
            is_active: true,
        })
    }

    /// Sets the free-text profile fields at creation time
    /// (`apps/api/src/organizations.rs::create_organization`) -- kept
    /// separate from `new`'s required-name validation since a description
    /// and location are both optional and never invalidate an
    /// otherwise-valid organization.
    pub fn with_profile(mut self, description: Option<String>, location: Option<String>) -> Self {
        self.description = description;
        self.location = location;
        self
    }

    /// Rebuilds an organization from persisted state; infrastructure
    /// adapters use this rather than `new`, which always mints a fresh id,
    /// empty trust grants, no profile, and no linked consumer.
    #[allow(clippy::too_many_arguments)]
    pub fn reconstitute(
        id: OrganizationId,
        name: String,
        description: Option<String>,
        location: Option<String>,
        verified_incident_types: Vec<IncidentType>,
        verified_alert_visibilities: Vec<AlertVisibility>,
        consumer_id: Option<ConsumerId>,
        is_active: bool,
    ) -> Self {
        Self {
            id,
            name,
            description,
            location,
            verified_incident_types,
            verified_alert_visibilities,
            consumer_id,
            is_active,
        }
    }

    pub fn id(&self) -> OrganizationId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn verified_incident_types(&self) -> &[IncidentType] {
        &self.verified_incident_types
    }

    pub fn verified_alert_visibilities(&self) -> &[AlertVisibility] {
        &self.verified_alert_visibilities
    }

    pub fn consumer_id(&self) -> Option<ConsumerId> {
        self.consumer_id
    }

    pub fn may_verify(&self, incident_type: IncidentType) -> bool {
        self.verified_incident_types.contains(&incident_type)
    }

    pub fn may_issue_alert(&self, visibility: AlertVisibility) -> bool {
        self.verified_alert_visibilities.contains(&visibility)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_name_is_rejected() {
        assert_eq!(Organization::new("   ").unwrap_err(), EmptyOrganizationName);
    }

    #[test]
    fn a_valid_name_is_trimmed_and_a_fresh_id_is_minted() {
        let organization = Organization::new("  Douala Police  ").unwrap();
        assert_eq!(organization.name(), "Douala Police");
    }

    #[test]
    fn a_freshly_created_organization_has_no_linked_consumer() {
        let organization = Organization::new("Douala Police").unwrap();
        assert_eq!(organization.consumer_id(), None);
    }

    #[test]
    fn a_freshly_created_organization_has_no_profile_and_is_active() {
        let organization = Organization::new("Douala Police").unwrap();
        assert_eq!(organization.description(), None);
        assert_eq!(organization.location(), None);
        assert!(organization.is_active());
    }

    #[test]
    fn a_freshly_created_organization_is_trusted_for_nothing() {
        let organization = Organization::new("Douala Police").unwrap();
        assert!(!organization.may_verify(IncidentType::MissingChild));
        assert!(!organization.may_issue_alert(AlertVisibility::Community));
    }

    #[test]
    fn may_verify_checks_only_the_granted_incident_types() {
        let organization = Organization::reconstitute(
            OrganizationId::new(),
            "Douala Police".into(),
            None,
            None,
            vec![IncidentType::MissingChild],
            vec![],
            None,
            true,
        );
        assert!(organization.may_verify(IncidentType::MissingChild));
        assert!(!organization.may_verify(IncidentType::OtherProtectionIncident));
    }

    #[test]
    fn may_issue_alert_checks_only_the_granted_visibilities() {
        let organization = Organization::reconstitute(
            OrganizationId::new(),
            "Douala Police".into(),
            None,
            None,
            vec![],
            vec![AlertVisibility::Community],
            None,
            true,
        );
        assert!(organization.may_issue_alert(AlertVisibility::Community));
        assert!(!organization.may_issue_alert(AlertVisibility::Public));
    }

    #[test]
    fn role_database_value_round_trips() {
        for role in [Role::PlatformAdmin, Role::OrgAdmin, Role::Member] {
            assert_eq!(
                Role::from_database_value(role.as_database_value()),
                Some(role)
            );
        }
        assert_eq!(Role::from_database_value("NOT_A_ROLE"), None);
    }
}
