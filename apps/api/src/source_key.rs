//! The per-source identity `RateLimiter` buckets are keyed by
//! (`crates/application/src/rate_limit.rs`). The deployed topology
//! (docs/DEPLOYMENT.md) puts exactly one trusted proxy in front of the API
//! (Railway's own edge), which appends the real client address as the
//! *last* hop of `X-Forwarded-For` — there is no `ConnectInfo`/raw TCP peer
//! address use here, since behind that edge the raw peer would always be
//! the edge's own address, not the caller's.
//!
//! The *last* hop, specifically, not the first: every hop before it is
//! whatever the client itself sent, so a caller can put any address it
//! wants at the front of the header (`X-Forwarded-For: 1.2.3.4`) to get a
//! fresh rate-limit bucket on every request. Only the single value the
//! trusted edge itself appended cannot be forged this way. If a second
//! trusted hop (e.g. a CDN) is ever added in front of Railway, this needs
//! to change to "last minus one" — trusting exactly as many hops from the
//! end as there are trusted proxies, never more.

use axum::http::HeaderMap;

const FORWARDED_FOR_HEADER: &str = "X-Forwarded-For";

/// Every request sharing this bucket when no `X-Forwarded-For` header is
/// present (e.g. a direct connection with no edge/proxy in front at all) —
/// a degraded-but-nonzero level of protection rather than no rate limiting.
const UNKNOWN_SOURCE: &str = "unknown";

pub fn source_key_from_headers(headers: &HeaderMap) -> &str {
    headers
        .get(FORWARDED_FOR_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next_back())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(UNKNOWN_SOURCE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_last_address_from_x_forwarded_for() {
        // The last hop is the one the trusted edge itself appended; every
        // hop before it is client-controlled and therefore untrustworthy.
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Forwarded-For",
            "10.0.0.1, 10.0.0.2, 203.0.113.7".parse().unwrap(),
        );
        assert_eq!(source_key_from_headers(&headers), "203.0.113.7");
    }

    #[test]
    fn a_client_supplied_prefix_cannot_forge_a_fresh_bucket() {
        // Without trusting only the last hop, an attacker could send a
        // different fake first address on every request to dodge rate
        // limiting entirely.
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", "1.2.3.4, 203.0.113.7".parse().unwrap());
        assert_eq!(source_key_from_headers(&headers), "203.0.113.7");

        let mut headers_2 = HeaderMap::new();
        headers_2.insert("X-Forwarded-For", "9.9.9.9, 203.0.113.7".parse().unwrap());
        assert_eq!(
            source_key_from_headers(&headers),
            source_key_from_headers(&headers_2),
            "the attacker-controlled prefix must not change the bucket key"
        );
    }

    #[test]
    fn trims_whitespace_around_the_last_address() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Forwarded-For",
            "10.0.0.1,  203.0.113.7  ".parse().unwrap(),
        );
        assert_eq!(source_key_from_headers(&headers), "203.0.113.7");
    }

    #[test]
    fn a_single_address_with_no_proxy_hops_is_used_as_is() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", "203.0.113.7".parse().unwrap());
        assert_eq!(source_key_from_headers(&headers), "203.0.113.7");
    }

    #[test]
    fn falls_back_to_a_shared_bucket_when_the_header_is_missing_or_blank() {
        assert_eq!(source_key_from_headers(&HeaderMap::new()), UNKNOWN_SOURCE);

        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", "   ".parse().unwrap());
        assert_eq!(source_key_from_headers(&headers), UNKNOWN_SOURCE);
    }
}
