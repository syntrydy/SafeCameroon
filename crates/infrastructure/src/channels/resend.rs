//! Resend (https://resend.com) adapter for the email channel -- the first
//! real (non-mock) email provider, replacing `EmailChannel` when
//! `RESEND_API_KEY` is configured (`apps/worker/src/main.rs`).
//!
//! Sends both `html` (branded to match the console/citizen apps' dark,
//! emerald-accented look, with the same card/footer shell as
//! `invite_mailer::resend::ResendInviteMailer`'s invite email) and a
//! plain-text fallback -- Resend/every mail client picks whichever it can
//! render. `message.body` (`crate::application::channel::build_outbound_message`)
//! always starts with `"[{severity}] {target_geography}"` on its own line;
//! this is the one place that line is parsed back apart, so it can render
//! as a colored severity pill instead of literal brackets.

use std::time::Duration;

use async_trait::async_trait;
use safe_cameroon_application::channel::{
    Channel, ChannelError, ChannelSendOutcome, EndpointValidationError, OutboundMessage,
};
use safe_cameroon_domain::ChannelType;
use serde::{Deserialize, Serialize};

use super::looks_like_email;

const RESEND_URL: &str = "https://api.resend.com/emails";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ResendEmailChannel {
    http: reqwest::Client,
    api_key: String,
    /// The verified sender identity Resend requires, e.g.
    /// `"Sentinel Alerts <alerts@example.org>"` -- there is no safe
    /// default, since it must match a domain verified in the Resend
    /// account this API key belongs to.
    from_address: String,
    /// Per-deployment product name (mirrors `ResendInviteMailer`'s
    /// `brand_name`) -- this channel's own copy, since Railway environment
    /// variables are per-service and this adapter lives in `apps/worker`,
    /// not `apps/api` (where the invite mailer's copy is read).
    brand_name: String,
}

impl ResendEmailChannel {
    pub fn new(api_key: String, from_address: String, brand_name: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("reqwest client with only a timeout must build"),
            api_key,
            from_address,
            brand_name,
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
    id: String,
}

fn validate(address: &str) -> Result<(), EndpointValidationError> {
    if looks_like_email(address) {
        Ok(())
    } else {
        Err(EndpointValidationError {
            channel: ChannelType::Email,
            reason: "expected a well-formed email address".into(),
        })
    }
}

/// Minimal escape for the values that reach the HTML template -- not a
/// general-purpose sanitizer, just enough to stop alert content (a
/// reviewer's own free-text description, ultimately) from breaking out of
/// the markup.
fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Splits `build_outbound_message`'s fixed `"[severity] geography"` first
/// line from the rest of the body (description/contact/reference, if
/// present).
fn split_header(body: &str) -> (&str, &str) {
    match body.split_once('\n') {
        Some((header, rest)) => (header, rest.trim_start_matches('\n')),
        None => (body, ""),
    }
}

/// Pulls the bracketed severity label and the geography that follows it out
/// of a header line, e.g. `"[High] Douala, Bamenda"` ->
/// `(Some("High"), "Douala, Bamenda")`. Falls back to `(None, header)` for
/// any header that doesn't match the exact shape `build_outbound_message`
/// always produces, so a future change to that format degrades to plain
/// text rather than panicking or silently dropping content.
fn parse_header(header: &str) -> (Option<&str>, &str) {
    if let Some(rest) = header.strip_prefix('[') {
        if let Some((label, after)) = rest.split_once("] ") {
            return (Some(label), after);
        }
    }
    (None, header)
}

/// The pitch deck's own severity palette (sev-low/med/high/crit), reused
/// here so an alert email visually agrees with the console. An unrecognized
/// label (only possible if `severity_label` changes without this file being
/// updated too) falls back to a neutral slate rather than failing to render.
fn severity_color(label: &str) -> &'static str {
    match label {
        "Low" => "#60a5fa",
        "Medium" => "#f59e0b",
        "High" => "#fb923c",
        "Critical" => "#f87171",
        _ => "#94a3b8",
    }
}

/// No datetime crate in this workspace's direct dependencies -- not worth
/// adding one for a single footer line, so this averages 365.25 days/year
/// from the Unix epoch (mirrors `invite_mailer::resend`'s copy).
fn current_year() -> u64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    1970 + secs / 31_557_600
}

fn alert_html(body: &str, brand_name: &str) -> String {
    let (header, rest) = split_header(body);
    let (severity_label, geography) = parse_header(header);
    let brand_name_escaped = escape_html(brand_name);
    let geography_escaped = escape_html(geography);
    let rest_html = escape_html(rest.trim()).replace('\n', "<br/>");
    let year = current_year();

    let severity_pill = match severity_label {
        Some(label) => {
            let color = severity_color(label);
            let label_escaped = escape_html(label);
            format!(
                r#"<span style="display:inline-block;background:{color}1a;color:{color};border:1px solid {color}55;font-size:11px;font-weight:700;letter-spacing:0.04em;text-transform:uppercase;padding:4px 10px;border-radius:999px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">{label_escaped}</span>"#
            )
        }
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
                    <td style="padding-left:10px;color:#ffffff;font-size:16px;font-weight:700;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">{brand_name_escaped} Alert</td>
                  </tr>
                </table>
              </td>
            </tr>
            <tr>
              <td style="padding:20px 32px 0 32px;">
                {severity_pill}<span style="display:inline-block;margin-left:8px;color:#94a3b8;font-size:13px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">{geography_escaped}</span>
              </td>
            </tr>
            <tr>
              <td style="padding:16px 32px 24px 32px;color:#cbd5e1;font-size:14px;line-height:22px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">
                {rest_html}
              </td>
            </tr>
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
impl Channel for ResendEmailChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Email
    }

    fn validate_endpoint(&self, endpoint_address: &str) -> Result<(), EndpointValidationError> {
        validate(endpoint_address)
    }

    async fn send(&self, message: OutboundMessage) -> Result<ChannelSendOutcome, ChannelError> {
        validate(&message.endpoint_address).map_err(|error| ChannelError {
            retryable: false,
            message: format!("Email: {}", error.reason),
        })?;

        let subject = format!("{} Alert", self.brand_name);
        let html = alert_html(&message.body, &self.brand_name);
        let request = SendEmailRequest {
            from: &self.from_address,
            to: [&message.endpoint_address],
            subject: &subject,
            text: &message.body,
            html: &html,
        };

        let response = self
            .http
            .post(RESEND_URL)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|error| ChannelError {
                retryable: true,
                message: format!("Email: could not reach Resend: {error}"),
            })?;

        let status = response.status();
        if status.is_success() {
            let parsed: SendEmailResponse =
                response.json().await.map_err(|error| ChannelError {
                    retryable: true,
                    message: format!("Email: unexpected response from Resend: {error}"),
                })?;
            return Ok(ChannelSendOutcome {
                provider_message_id: Some(parsed.id),
            });
        }

        // Resend's own retry guidance (docs/api-reference/errors): 409
        // (a same-day idempotency-key collision, harmless to retry with a
        // fresh key), 429 (rate limited), and 5xx are transient; everything
        // else (400-405, 422) is a malformed/rejected request that retrying
        // unchanged can never fix.
        let retryable = matches!(status.as_u16(), 409 | 429) || status.is_server_error();
        Err(ChannelError {
            retryable,
            message: format!("Email: Resend rejected the request ({status})"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel() -> ResendEmailChannel {
        ResendEmailChannel::new("key".into(), "alerts@example.org".into(), "Sentinel".into())
    }

    #[test]
    fn validates_well_formed_addresses_only() {
        let channel = channel();
        assert!(channel.validate_endpoint("ngo@example.cm").is_ok());
        assert!(channel.validate_endpoint("not-an-email").is_err());
    }

    #[tokio::test]
    async fn send_rejects_an_invalid_address_without_a_network_call() {
        let channel = channel();
        let error = channel
            .send(OutboundMessage {
                endpoint_address: "not-an-email".into(),
                body: "hello".into(),
            })
            .await
            .unwrap_err();
        assert!(!error.retryable);
    }

    #[test]
    fn parses_the_severity_and_geography_out_of_the_standard_header() {
        assert_eq!(
            parse_header("[High] Douala, Bamenda"),
            (Some("High"), "Douala, Bamenda")
        );
    }

    #[test]
    fn falls_back_to_the_whole_header_when_it_does_not_match_the_expected_shape() {
        assert_eq!(parse_header("no brackets here"), (None, "no brackets here"));
    }

    #[test]
    fn the_alert_html_renders_a_severity_pill_and_escapes_the_description() {
        let html = alert_html(
            "[Critical] Douala\n\n<script>alert(1)</script> is missing.\n\nContact: 117",
            "Sentinel",
        );
        assert!(html.contains("CRITICAL") || html.contains("Critical"));
        assert!(html.contains("#f87171")); // sev-crit
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("Douala"));
        assert!(html.contains("Contact: 117"));
    }

    #[test]
    fn the_alert_html_includes_the_footer_tagline_and_brand_name() {
        let html = alert_html("[Low] Yaounde\n\nAll clear.", "Sentinel");
        assert!(html.contains("Privacy-first civic-protection platform"));
        assert!(html.contains("Sentinel"));
    }
}
