//! Resend (https://resend.com) adapter for invite emails -- a separate,
//! purpose-built client from `channels::resend::ResendEmailChannel`
//! (alert delivery to citizens/consumers, fixed subject, retry-aware
//! `ChannelError`) because an invite email has a different subject/body
//! per role/organization and is always best-effort (see `InviteMailer`'s
//! module doc comment): failures are logged, never retried or surfaced.

use std::time::Duration;

use async_trait::async_trait;
use safe_cameroon_application::invite_mailer::{InviteMailError, InviteMailer};
use safe_cameroon_domain::Role;
use serde::{Deserialize, Serialize};

const RESEND_URL: &str = "https://api.resend.com/emails";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ResendInviteMailer {
    http: reqwest::Client,
    api_key: String,
    from_address: String,
}

impl ResendInviteMailer {
    pub fn new(api_key: String, from_address: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("reqwest client with only a timeout must build"),
            api_key,
            from_address,
        }
    }
}

#[derive(Serialize)]
struct SendEmailRequest<'a> {
    from: &'a str,
    to: [&'a str; 1],
    subject: &'a str,
    text: &'a str,
}

#[derive(Deserialize)]
struct SendEmailResponse {
    #[allow(dead_code)]
    id: String,
}

fn role_label(role: Role) -> &'static str {
    match role {
        Role::PlatformAdmin => "platform admin",
        Role::OrgAdmin => "organization admin",
        Role::Member => "member",
    }
}

fn invite_body(role: Role, organization_name: Option<&str>) -> String {
    match organization_name {
        Some(name) => format!(
            "You've been added to Sentinel as a {} for {name}.\n\n\
             Sign in at the Sentinel portal with this exact email address (Google sign-in) to get started.",
            role_label(role),
        ),
        None => format!(
            "You've been added to Sentinel as a {}.\n\n\
             Sign in at the Sentinel portal with this exact email address (Google sign-in) to get started.",
            role_label(role),
        ),
    }
}

#[async_trait]
impl InviteMailer for ResendInviteMailer {
    async fn send_invite(
        &self,
        to: &str,
        role: Role,
        organization_name: Option<&str>,
    ) -> Result<(), InviteMailError> {
        let body = invite_body(role, organization_name);
        let request = SendEmailRequest {
            from: &self.from_address,
            to: [to],
            subject: "You've been invited to Sentinel",
            text: &body,
        };

        let response = self
            .http
            .post(RESEND_URL)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|error| InviteMailError {
                message: format!("could not reach Resend: {error}"),
            })?;

        let status = response.status();
        if status.is_success() {
            let _parsed: SendEmailResponse =
                response.json().await.map_err(|error| InviteMailError {
                    message: format!("unexpected response from Resend: {error}"),
                })?;
            return Ok(());
        }
        Err(InviteMailError {
            message: format!("Resend rejected the invite email ({status})"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_invite_body_names_the_organization_when_present() {
        let body = invite_body(Role::OrgAdmin, Some("Douala Police"));
        assert!(body.contains("organization admin"));
        assert!(body.contains("Douala Police"));
    }

    #[test]
    fn the_invite_body_omits_organization_for_a_platform_admin() {
        let body = invite_body(Role::PlatformAdmin, None);
        assert!(body.contains("platform admin"));
        assert!(!body.contains("for "));
    }
}
