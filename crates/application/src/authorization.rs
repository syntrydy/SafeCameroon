//! A capability-oriented authorization vocabulary (prompt 09: "implement a
//! policy-oriented authorization layer"; docs/SECURITY_PRIVACY.md section 3
//! lists these exact capability names), plus the organization/role-aware
//! checks that used to be blocked on open product questions
//! (docs/OPEN_QUESTIONS.md). Two of those questions — "which authority can
//! verify a case for each incident class" and "which organizations may
//! issue community/public alerts" — are answered by
//! [`authorize_case_verification`] and [`authorize_alert_issuance`]
//! consulting each real organization's own trust grants
//! ([`safe_cameroon_domain::Organization`]) rather than a rule baked into
//! this codebase. The third ("who owns a case when multiple organizations
//! receive it") stays open: case visibility/review below is still flat, any
//! identified reviewer, since real multi-org case routing/locking is a
//! separate, larger workflow feature.
//!
//! [`case_workflow`](crate::case_workflow) and
//! [`alert_workflow`](crate::alert_workflow) already encode finer-grained,
//! tested authorization rules than a flat capability check would (e.g. an
//! automated actor may start a case review but never verify it; it may
//! raise an internal alert but never a public one) — this module does not
//! replace those. `authorize` is for resources, like attachments, that do
//! not yet have their own bespoke rule.

use core::fmt;

use crate::case_workflow::Actor;
use safe_cameroon_domain::{
    AlertVisibility, IncidentType, Membership, Organization, OrganizationId, Role,
};

/// The closed set of privileged actions across the platform
/// (docs/SECURITY_PRIVACY.md section 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    Report,
    ViewCase,
    ReviewReport,
    VerifyCase,
    CreateAlert,
    ManageSubscriptions,
    ViewAudit,
    ManageChannelProvider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizationDenied {
    pub actor: Actor,
    pub capability: Capability,
}

impl fmt::Display for AuthorizationDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} is not authorized to exercise {:?}",
            self.actor, self.capability
        )
    }
}

impl std::error::Error for AuthorizationDenied {}

/// `Capability::Report` always succeeds — anonymous reporting requires no
/// actor at all (AGENTS.md: "do not require authentication for anonymous
/// reporting"). Every other capability listed here requires only an
/// identified reviewer, regardless of role/organization — these are the
/// capabilities this module deliberately keeps flat (see module doc
/// comment); membership grants, case verification, and alert issuance have
/// their own, stricter functions below.
pub fn authorize(actor: Actor, capability: Capability) -> Result<(), AuthorizationDenied> {
    if capability == Capability::Report {
        return Ok(());
    }
    match actor {
        Actor::Reviewer(_) => Ok(()),
        Actor::Automated => Err(AuthorizationDenied { actor, capability }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MembershipGrantDenied;

impl fmt::Display for MembershipGrantDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "granter is not authorized to register a reviewer with this role/organization"
        )
    }
}

impl std::error::Error for MembershipGrantDenied {}

/// Whether `granter` (the already-authenticated reviewer making the
/// request, or `None` for an unauthenticated/automated caller) may register
/// a new reviewer with `target_role` in `target_organization_id`.
///
/// The very first reviewer in a fresh deployment must be creatable without
/// an existing authenticated reviewer to authorize it (there is no
/// bootstrapping mechanism otherwise) — the caller is responsible for then
/// granting that bootstrapping reviewer `Role::PlatformAdmin` with no
/// organization, regardless of what was requested; this function only
/// decides whether the registration attempt itself may proceed.
///
/// A `PlatformAdmin` may register anyone into any organization (or another
/// `PlatformAdmin`). An `OrgAdmin` may register only a `Member` into their
/// own organization — deliberately not another `OrgAdmin` or
/// `PlatformAdmin`, so an org admin can staff their own team but can never
/// escalate anyone's privileges beyond their own. A plain `Member`, or no
/// membership at all, may never register anyone.
pub fn authorize_membership_grant(
    granter: Option<Membership>,
    existing_reviewer_count: u64,
    target_role: Role,
    target_organization_id: Option<safe_cameroon_domain::OrganizationId>,
) -> Result<(), MembershipGrantDenied> {
    if existing_reviewer_count == 0 {
        return Ok(());
    }
    match granter {
        Some(Membership {
            role: Role::PlatformAdmin,
            ..
        }) => Ok(()),
        Some(Membership {
            role: Role::OrgAdmin,
            organization_id: Some(granter_org),
        }) if target_role == Role::Member && target_organization_id == Some(granter_org) => Ok(()),
        _ => Err(MembershipGrantDenied),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidMembershipShape;

impl fmt::Display for InvalidMembershipShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PlatformAdmin must have no organization; OrgAdmin and Member must each have one"
        )
    }
}

impl std::error::Error for InvalidMembershipShape {}

/// `PlatformAdmin` is org-independent; `OrgAdmin`/`Member` each require
/// exactly one organization. Checked independently of
/// [`authorize_membership_grant`] so a malformed request (e.g. `OrgAdmin`
/// with no organization) is rejected before authorization is even
/// evaluated.
pub fn validate_membership_shape(
    role: Role,
    organization_id: Option<safe_cameroon_domain::OrganizationId>,
) -> Result<(), InvalidMembershipShape> {
    let valid = match role {
        Role::PlatformAdmin => organization_id.is_none(),
        Role::OrgAdmin | Role::Member => organization_id.is_some(),
    };
    if valid {
        Ok(())
    } else {
        Err(InvalidMembershipShape)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaseVerificationNotAuthorized;

impl fmt::Display for CaseVerificationNotAuthorized {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "reviewer's organization is not trusted to verify this incident type"
        )
    }
}

impl std::error::Error for CaseVerificationNotAuthorized {}

/// A `PlatformAdmin` may verify any case (a deliberate break-glass
/// allowance, mirroring their unrestricted registration authority). An
/// `OrgAdmin`/`Member` may verify a case only if their organization has
/// been explicitly trusted (by a `PlatformAdmin`, at onboarding) to verify
/// that incident type.
pub fn authorize_case_verification(
    membership: Membership,
    organization: Option<&Organization>,
    incident_type: IncidentType,
) -> Result<(), CaseVerificationNotAuthorized> {
    if membership.role == Role::PlatformAdmin {
        return Ok(());
    }
    match organization {
        Some(organization) if organization.may_verify(incident_type) => Ok(()),
        _ => Err(CaseVerificationNotAuthorized),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlertIssuanceNotAuthorized;

impl fmt::Display for AlertIssuanceNotAuthorized {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "reviewer's organization is not trusted to issue alerts at this visibility"
        )
    }
}

impl std::error::Error for AlertIssuanceNotAuthorized {}

/// Only meaningful for a visibility that already requires an identified
/// reviewer (community/public — see `alert_workflow::authorize_alert_action`);
/// internal/partner alerts are unaffected by organization trust, matching
/// AGENTS.md: "internal/partner alerts... stay within the organization and
/// may be raised automatically."
pub fn authorize_alert_issuance(
    membership: Membership,
    organization: Option<&Organization>,
    visibility: AlertVisibility,
) -> Result<(), AlertIssuanceNotAuthorized> {
    if membership.role == Role::PlatformAdmin {
        return Ok(());
    }
    match organization {
        Some(organization) if organization.may_issue_alert(visibility) => Ok(()),
        _ => Err(AlertIssuanceNotAuthorized),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsumerManagementNotAuthorized;

impl fmt::Display for ConsumerManagementNotAuthorized {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "reviewer may not manage this consumer's subscriptions or delivery preference"
        )
    }
}

impl std::error::Error for ConsumerManagementNotAuthorized {}

/// Whether `membership` may manage a consumer's subscriptions/delivery
/// preference (`apps/api/src/subscriptions.rs`). `Capability::ManageSubscriptions`
/// deliberately stays flat for consumers with no real owner (a standalone
/// consumer, or a citizen's self-managed one via management token) — but
/// once a consumer is linked to a reviewer `Organization`
/// (`Organization::consumer_id`), it *does* have a real owner, and letting
/// any identified reviewer manage another organization's actual
/// notification channel (docs/SECURITY_PRIVACY.md: least privilege) is the
/// gap this closes. A `PlatformAdmin` may still manage any consumer,
/// mirroring its unrestricted authority elsewhere in this module.
pub fn authorize_consumer_management(
    membership: Membership,
    owning_organization_id: Option<OrganizationId>,
) -> Result<(), ConsumerManagementNotAuthorized> {
    if membership.role == Role::PlatformAdmin {
        return Ok(());
    }
    match owning_organization_id {
        None => Ok(()),
        Some(owning_organization_id)
            if membership.organization_id == Some(owning_organization_id) =>
        {
            Ok(())
        }
        Some(_) => Err(ConsumerManagementNotAuthorized),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use safe_cameroon_domain::OrganizationId;
    use uuid::Uuid;

    #[test]
    fn reporting_never_requires_an_actor_check() {
        assert!(authorize(Actor::Automated, Capability::Report).is_ok());
    }

    #[test]
    fn an_identified_reviewer_may_exercise_any_flat_capability() {
        let reviewer = Actor::Reviewer(Uuid::new_v4());
        for capability in [
            Capability::ViewCase,
            Capability::ReviewReport,
            Capability::VerifyCase,
            Capability::CreateAlert,
            Capability::ManageSubscriptions,
            Capability::ViewAudit,
            Capability::ManageChannelProvider,
        ] {
            assert!(authorize(reviewer, capability).is_ok());
        }
    }

    #[test]
    fn an_automated_actor_may_not_exercise_privileged_capabilities() {
        let error = authorize(Actor::Automated, Capability::ViewCase).unwrap_err();
        assert_eq!(
            error,
            AuthorizationDenied {
                actor: Actor::Automated,
                capability: Capability::ViewCase
            }
        );
    }

    #[test]
    fn the_first_reviewer_may_register_without_being_authenticated() {
        assert!(authorize_membership_grant(None, 0, Role::PlatformAdmin, None).is_ok());
    }

    #[test]
    fn no_membership_may_not_register_a_further_reviewer() {
        assert_eq!(
            authorize_membership_grant(None, 1, Role::Member, Some(OrganizationId::new())),
            Err(MembershipGrantDenied)
        );
    }

    #[test]
    fn a_platform_admin_may_register_anyone_into_any_organization() {
        let granter = Membership {
            role: Role::PlatformAdmin,
            organization_id: None,
        };
        assert!(
            authorize_membership_grant(
                Some(granter),
                1,
                Role::OrgAdmin,
                Some(OrganizationId::new())
            )
            .is_ok()
        );
        assert!(authorize_membership_grant(Some(granter), 1, Role::PlatformAdmin, None).is_ok());
    }

    #[test]
    fn an_org_admin_may_register_only_a_member_into_their_own_organization() {
        let org = OrganizationId::new();
        let granter = Membership {
            role: Role::OrgAdmin,
            organization_id: Some(org),
        };
        assert!(authorize_membership_grant(Some(granter), 1, Role::Member, Some(org)).is_ok());

        let other_org = OrganizationId::new();
        assert_eq!(
            authorize_membership_grant(Some(granter), 1, Role::Member, Some(other_org)),
            Err(MembershipGrantDenied)
        );
        assert_eq!(
            authorize_membership_grant(Some(granter), 1, Role::OrgAdmin, Some(org)),
            Err(MembershipGrantDenied)
        );
    }

    #[test]
    fn a_plain_member_may_not_register_anyone() {
        let granter = Membership {
            role: Role::Member,
            organization_id: Some(OrganizationId::new()),
        };
        assert_eq!(
            authorize_membership_grant(Some(granter), 1, Role::Member, Some(OrganizationId::new())),
            Err(MembershipGrantDenied)
        );
    }

    #[test]
    fn platform_admin_must_have_no_organization() {
        assert!(validate_membership_shape(Role::PlatformAdmin, None).is_ok());
        assert_eq!(
            validate_membership_shape(Role::PlatformAdmin, Some(OrganizationId::new())),
            Err(InvalidMembershipShape)
        );
    }

    #[test]
    fn org_admin_and_member_must_have_an_organization() {
        assert!(validate_membership_shape(Role::OrgAdmin, Some(OrganizationId::new())).is_ok());
        assert_eq!(
            validate_membership_shape(Role::OrgAdmin, None),
            Err(InvalidMembershipShape)
        );
        assert!(validate_membership_shape(Role::Member, Some(OrganizationId::new())).is_ok());
        assert_eq!(
            validate_membership_shape(Role::Member, None),
            Err(InvalidMembershipShape)
        );
    }

    #[test]
    fn a_platform_admin_may_verify_any_case_regardless_of_organization_trust() {
        let membership = Membership {
            role: Role::PlatformAdmin,
            organization_id: None,
        };
        assert!(authorize_case_verification(membership, None, IncidentType::MissingChild).is_ok());
    }

    #[test]
    fn a_member_may_verify_only_incident_types_their_organization_is_trusted_for() {
        let org_id = OrganizationId::new();
        let trusted_org = Organization::reconstitute(
            org_id,
            "Douala Police".into(),
            vec![IncidentType::MissingChild],
            vec![],
            None,
        );
        let membership = Membership {
            role: Role::Member,
            organization_id: Some(org_id),
        };
        assert!(
            authorize_case_verification(membership, Some(&trusted_org), IncidentType::MissingChild)
                .is_ok()
        );
        assert_eq!(
            authorize_case_verification(
                membership,
                Some(&trusted_org),
                IncidentType::OtherProtectionIncident
            ),
            Err(CaseVerificationNotAuthorized)
        );
    }

    #[test]
    fn a_member_with_no_organization_may_not_verify_anything() {
        let membership = Membership {
            role: Role::Member,
            organization_id: None,
        };
        assert_eq!(
            authorize_case_verification(membership, None, IncidentType::MissingChild),
            Err(CaseVerificationNotAuthorized)
        );
    }

    #[test]
    fn a_platform_admin_may_issue_any_alert_visibility() {
        let membership = Membership {
            role: Role::PlatformAdmin,
            organization_id: None,
        };
        assert!(authorize_alert_issuance(membership, None, AlertVisibility::Community).is_ok());
    }

    #[test]
    fn a_member_may_issue_only_visibilities_their_organization_is_trusted_for() {
        let org_id = OrganizationId::new();
        let trusted_org = Organization::reconstitute(
            org_id,
            "Douala Police".into(),
            vec![],
            vec![AlertVisibility::Community],
            None,
        );
        let membership = Membership {
            role: Role::Member,
            organization_id: Some(org_id),
        };
        assert!(
            authorize_alert_issuance(membership, Some(&trusted_org), AlertVisibility::Community)
                .is_ok()
        );
        assert_eq!(
            authorize_alert_issuance(membership, Some(&trusted_org), AlertVisibility::Public),
            Err(AlertIssuanceNotAuthorized)
        );
    }

    #[test]
    fn a_platform_admin_may_manage_any_consumer() {
        let membership = Membership {
            role: Role::PlatformAdmin,
            organization_id: None,
        };
        assert!(authorize_consumer_management(membership, Some(OrganizationId::new())).is_ok());
        assert!(authorize_consumer_management(membership, None).is_ok());
    }

    #[test]
    fn a_consumer_with_no_owning_organization_is_manageable_by_any_reviewer() {
        let membership = Membership {
            role: Role::Member,
            organization_id: Some(OrganizationId::new()),
        };
        assert!(authorize_consumer_management(membership, None).is_ok());
    }

    #[test]
    fn a_member_may_manage_only_their_own_organizations_consumer() {
        let org_id = OrganizationId::new();
        let membership = Membership {
            role: Role::Member,
            organization_id: Some(org_id),
        };
        assert!(authorize_consumer_management(membership, Some(org_id)).is_ok());
        assert_eq!(
            authorize_consumer_management(membership, Some(OrganizationId::new())),
            Err(ConsumerManagementNotAuthorized)
        );
    }

    #[test]
    fn a_member_with_no_organization_may_not_manage_an_owned_consumer() {
        let membership = Membership {
            role: Role::Member,
            organization_id: None,
        };
        assert_eq!(
            authorize_consumer_management(membership, Some(OrganizationId::new())),
            Err(ConsumerManagementNotAuthorized)
        );
    }
}
