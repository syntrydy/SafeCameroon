//! Notifies a newly registered reviewer that they now have a role, so they
//! know to sign in (`apps/api/src/auth.rs::register`/`google_login`) --
//! reviewers authenticate via Google OAuth only, so there is no
//! password/magic-link to send: this is purely "someone told the platform
//! you have access; go sign in with this email." Best-effort by design --
//! a failed or disabled send never blocks registration itself, since the
//! authorization write (the membership row) is what actually matters.

use core::fmt;

use async_trait::async_trait;
use safe_cameroon_domain::Role;

/// A provider/network failure sending one invite email.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InviteMailError {
    pub message: String,
}

impl fmt::Display for InviteMailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for InviteMailError {}

/// `organization_name` is `None` for a `PlatformAdmin` (org-independent).
#[async_trait]
pub trait InviteMailer: Send + Sync {
    async fn send_invite(
        &self,
        to: &str,
        role: Role,
        organization_name: Option<&str>,
    ) -> Result<(), InviteMailError>;
}

/// Records calls instead of sending anything -- for tests.
#[derive(Default)]
pub struct FakeInviteMailer {
    pub sent: std::sync::Mutex<Vec<(String, Role, Option<String>)>>,
}

#[async_trait]
impl InviteMailer for FakeInviteMailer {
    async fn send_invite(
        &self,
        to: &str,
        role: Role,
        organization_name: Option<&str>,
    ) -> Result<(), InviteMailError> {
        self.sent
            .lock()
            .unwrap()
            .push((to.to_owned(), role, organization_name.map(str::to_owned)));
        Ok(())
    }
}
