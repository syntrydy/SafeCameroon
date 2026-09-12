//! A capability-oriented authorization vocabulary (prompt 09: "implement a
//! policy-oriented authorization layer"; docs/SECURITY_PRIVACY.md section 3
//! lists these exact capability names). This module is deliberately small:
//! a real organization/role model that grants capabilities more precisely
//! than "any identified reviewer" is blocked on open product questions
//! (docs/OPEN_QUESTIONS.md — "which authority can verify a case for each
//! incident class", "who owns a case when multiple organizations receive
//! it", etc.) that should not be guessed here.
//!
//! [`case_workflow`](crate::case_workflow) and
//! [`alert_workflow`](crate::alert_workflow) already encode finer-grained,
//! tested authorization rules than a flat capability check would (e.g. an
//! automated actor may start a case review but never verify it; it may
//! raise an internal alert but never a public one) — this module does not
//! replace those. It is for new resources, like attachments, that do not
//! yet have their own bespoke rule.

use core::fmt;

use crate::case_workflow::Actor;

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
    ManageOrganization,
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
/// reporting"). Every other capability requires an identified reviewer;
/// this is intentionally the only rule until organization/role scoping is
/// defined, per this module's doc comment.
pub fn authorize(actor: Actor, capability: Capability) -> Result<(), AuthorizationDenied> {
    if capability == Capability::Report {
        return Ok(());
    }
    match actor {
        Actor::Reviewer(_) => Ok(()),
        Actor::Automated => Err(AuthorizationDenied { actor, capability }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn reporting_never_requires_an_actor_check() {
        assert!(authorize(Actor::Automated, Capability::Report).is_ok());
    }

    #[test]
    fn an_identified_reviewer_may_exercise_any_capability() {
        let reviewer = Actor::Reviewer(Uuid::new_v4());
        for capability in [
            Capability::ViewCase,
            Capability::ReviewReport,
            Capability::VerifyCase,
            Capability::CreateAlert,
            Capability::ManageSubscriptions,
            Capability::ViewAudit,
            Capability::ManageOrganization,
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
}
