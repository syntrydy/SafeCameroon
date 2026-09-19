//! Stands in for a real provider when no email credentials are configured
//! (`RESEND_API_KEY` unset) -- unlike `ai::disabled::DisabledExtractor`,
//! this never fails: an invite email is a best-effort side effect of
//! registration, not something a caller is waiting on, so a disabled
//! mailer silently does nothing rather than surfacing an error nobody
//! would see anyway (see `InviteMailer`'s module doc comment).

use async_trait::async_trait;
use safe_cameroon_application::invite_mailer::{InviteMailError, InviteMailer};
use safe_cameroon_domain::Role;

#[derive(Debug, Clone, Copy, Default)]
pub struct DisabledInviteMailer;

#[async_trait]
impl InviteMailer for DisabledInviteMailer {
    async fn send_invite(
        &self,
        _to: &str,
        _role: Role,
        _organization_name: Option<&str>,
    ) -> Result<(), InviteMailError> {
        Ok(())
    }
}
