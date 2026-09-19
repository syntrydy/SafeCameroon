//! Resend (https://resend.com) adapter for invite emails -- a separate,
//! purpose-built client from `channels::resend::ResendEmailChannel`
//! (alert delivery to citizens/consumers, fixed subject, retry-aware
//! `ChannelError`) because an invite email has a different subject/body
//! per role/organization and is always best-effort (see `InviteMailer`'s
//! module doc comment): failures are logged, never retried or surfaced.
//!
//! Sends both `html` (branded to match the console/citizen apps' dark,
//! emerald-accented look, with the same footer tagline/copyright line as
//! `apps/console/src/components/Footer.tsx`) and a plain-text fallback --
//! Resend/every mail client picks whichever it can render.

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
    /// Per-deployment product name (mirrors `apps/console`'s `BRAND_NAME`,
    /// which reads `VITE_BRAND_NAME` client-side) -- this is the
    /// server-side equivalent, since the email is composed here, not in
    /// the console app.
    brand_name: String,
    /// Link the email's "Sign in" button points to; omitted entirely (no
    /// button) when not configured, mirroring how the citizen/console
    /// apps' own footers only show their cross-link when its URL is set.
    console_url: Option<String>,
}

impl ResendInviteMailer {
    pub fn new(
        api_key: String,
        from_address: String,
        brand_name: String,
        console_url: Option<String>,
    ) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("reqwest client with only a timeout must build"),
            api_key,
            from_address,
            brand_name,
            console_url,
        }
    }
}

#[derive(Serialize)]
struct SendEmailRequest<'a> {
    from: &'a str,
    to: [&'a str; 1],
    subject: &'a str,
    text: &'a str,
    html: &'a str,
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

/// Minimal escape for the handful of values that ever reach the HTML
/// template (`brand_name` is server-configured, but `organization_name`
/// is a platform admin's free-text input) -- not a general-purpose
/// sanitizer, just enough to stop it from breaking out of the markup.
fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn added_as_line(role: Role, organization_name: Option<&str>, brand_name: &str) -> String {
    match organization_name {
        Some(name) => format!(
            "You've been added to {brand_name} as a {} for {name}.",
            role_label(role)
        ),
        None => format!(
            "You've been added to {brand_name} as a {}.",
            role_label(role)
        ),
    }
}

fn invite_text(
    role: Role,
    organization_name: Option<&str>,
    brand_name: &str,
    console_url: Option<&str>,
) -> String {
    let mut body = format!(
        "{}\n\nSign in with this exact email address (Google sign-in) to get started.",
        added_as_line(role, organization_name, brand_name),
    );
    if let Some(url) = console_url {
        body.push_str(&format!("\n\n{url}"));
    }
    body.push_str(&format!(
        "\n\n---\n{brand_name} \u{b7} Privacy-first civic-protection platform"
    ));
    body
}

/// No datetime crate in this workspace's direct dependencies (`chrono`/
/// `time` are only transitive) -- not worth adding one for a single footer
/// line, so this averages 365.25 days/year from the Unix epoch. Accurate
/// to within a day of a real calendar year boundary, which is all a
/// copyright footer needs.
fn current_year() -> u64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    1970 + secs / 31_557_600
}

fn invite_html(
    role: Role,
    organization_name: Option<&str>,
    brand_name: &str,
    console_url: Option<&str>,
) -> String {
    let brand_name_escaped = escape_html(brand_name);
    let added_as_line_escaped = escape_html(&added_as_line(role, organization_name, brand_name));
    let year = current_year();
    let button = match console_url {
        Some(url) => format!(
            r#"<tr><td style="padding:0 32px 32px 32px;">
                 <a href="{url}" style="display:inline-block;background:linear-gradient(135deg,#059669,#047857);color:#ffffff;text-decoration:none;font-size:14px;font-weight:600;padding:12px 22px;border-radius:10px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">Sign in to the portal</a>
               </td></tr>"#
        ),
        None => String::new(),
    };
    format!(
        r#"<!DOCTYPE html>
<html>
  <body style="margin:0;padding:0;background-color:#020617;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">
    <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background-color:#020617;padding:32px 16px;">
      <tr>
        <td align="center">
          <table role="presentation" width="480" cellpadding="0" cellspacing="0" style="max-width:480px;width:100%;background-color:#0f172a;border:1px solid rgba(255,255,255,0.08);border-radius:16px;overflow:hidden;">
            <tr>
              <td style="padding:28px 32px 0 32px;">
                <table role="presentation" cellpadding="0" cellspacing="0">
                  <tr>
                    <td style="width:36px;height:36px;background:linear-gradient(135deg,#34d399,#059669);border-radius:10px;text-align:center;vertical-align:middle;font-size:18px;line-height:36px;">&#128737;</td>
                    <td style="padding-left:10px;color:#ffffff;font-size:16px;font-weight:700;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">{brand_name_escaped}</td>
                  </tr>
                </table>
              </td>
            </tr>
            <tr>
              <td style="padding:24px 32px 8px 32px;color:#ffffff;font-size:18px;font-weight:600;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">You've been invited</td>
            </tr>
            <tr>
              <td style="padding:0 32px 24px 32px;color:#cbd5e1;font-size:14px;line-height:22px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">
                {added_as_line_escaped}<br/><br/>
                Sign in with this exact email address (Google sign-in) to get started.
              </td>
            </tr>
            {button}
            <tr>
              <td style="padding:20px 32px;border-top:1px solid rgba(255,255,255,0.06);text-align:center;color:#64748b;font-size:12px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">
                {brand_name_escaped} &middot; Privacy-first civic-protection platform<br/>
                &copy; {year} {brand_name_escaped}
              </td>
            </tr>
          </table>
        </td>
      </tr>
    </table>
  </body>
</html>"#
    )
}

#[async_trait]
impl InviteMailer for ResendInviteMailer {
    async fn send_invite(
        &self,
        to: &str,
        role: Role,
        organization_name: Option<&str>,
    ) -> Result<(), InviteMailError> {
        let text = invite_text(
            role,
            organization_name,
            &self.brand_name,
            self.console_url.as_deref(),
        );
        let html = invite_html(
            role,
            organization_name,
            &self.brand_name,
            self.console_url.as_deref(),
        );
        let subject = format!("You've been invited to {}", self.brand_name);
        let request = SendEmailRequest {
            from: &self.from_address,
            to: [to],
            subject: &subject,
            text: &text,
            html: &html,
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
    fn the_invite_text_names_the_organization_and_includes_the_footer() {
        let body = invite_text(Role::OrgAdmin, Some("Douala Police"), "Sentinel", None);
        assert!(body.contains("organization admin"));
        assert!(body.contains("Douala Police"));
        assert!(body.contains("Privacy-first civic-protection platform"));
    }

    #[test]
    fn the_invite_text_omits_organization_for_a_platform_admin() {
        let body = invite_text(Role::PlatformAdmin, None, "Sentinel", None);
        assert!(body.contains("platform admin"));
        assert!(!body.contains("for "));
    }

    #[test]
    fn the_invite_text_includes_the_console_url_when_configured() {
        let body = invite_text(
            Role::Member,
            None,
            "Sentinel",
            Some("https://portal.example.org"),
        );
        assert!(body.contains("https://portal.example.org"));
    }

    #[test]
    fn the_invite_html_escapes_the_organization_name() {
        let html = invite_html(
            Role::Member,
            Some("<script>alert(1)</script>"),
            "Sentinel",
            None,
        );
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn the_invite_html_includes_the_button_only_when_a_console_url_is_configured() {
        let with_url = invite_html(
            Role::Member,
            None,
            "Sentinel",
            Some("https://portal.example.org"),
        );
        assert!(with_url.contains("https://portal.example.org"));
        assert!(with_url.contains("Sign in to the portal"));

        let without_url = invite_html(Role::Member, None, "Sentinel", None);
        assert!(!without_url.contains("Sign in to the portal"));
    }

    #[test]
    fn the_invite_html_includes_the_footer_tagline() {
        let html = invite_html(Role::Member, None, "Sentinel", None);
        assert!(html.contains("Privacy-first civic-protection platform"));
        assert!(html.contains("Sentinel"));
    }
}
