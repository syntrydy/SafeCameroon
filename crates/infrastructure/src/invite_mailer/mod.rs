//! [`InviteMailer`](safe_cameroon_application::invite_mailer::InviteMailer)
//! adapters, one module per provider (mirrors `ai`, `channels`).

pub mod disabled;
pub mod resend;

pub use disabled::DisabledInviteMailer;
pub use resend::ResendInviteMailer;
