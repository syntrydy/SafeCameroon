//! [`Channel`](safe_cameroon_application::channel::Channel) adapters, one
//! module per provider (mirrors `postgres`, one module per aggregate).
//! `email`/`sms`/`whatsapp` are mock/sandbox implementations (prompt 08:
//! "first adapters may be mock/sandbox implementations if external
//! credentials are not available") — no vendor SDK or real network call yet,
//! but endpoint validation and provider-error encapsulation are real and
//! independently testable, so swapping in a real HTTP client later only
//! touches `send`. `push` is the first real (non-mock) adapter, since Web
//! Push needs no vendor account (docs/OPEN_QUESTIONS.md: unlike WhatsApp/SMS,
//! no provider choice is pending).

pub mod email;
pub mod push;
pub mod sms;
pub mod whatsapp;

pub use email::EmailChannel;
pub use push::WebPushChannel;
pub use sms::SmsChannel;
pub use whatsapp::WhatsAppChannel;

/// Shared E.164-ish shape check for the two phone-based channels. Real
/// deliverability (a live, WhatsApp-registered, or SMS-reachable number) can
/// only be confirmed by the provider itself; this only rejects addresses
/// that could never be valid.
fn looks_like_e164(address: &str) -> bool {
    let digits = match address.strip_prefix('+') {
        Some(rest) => rest,
        None => return false,
    };
    (8..=15).contains(&digits.len()) && digits.chars().all(|c| c.is_ascii_digit())
}
