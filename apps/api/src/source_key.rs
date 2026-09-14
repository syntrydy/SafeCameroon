//! The per-source identity `RateLimiter` buckets are keyed by
//! (`crates/application/src/rate_limit.rs`). docs/DEPLOYMENT.md's starting
//! topology puts Cloudflare in front of the API, which sets
//! `X-Forwarded-For` to the real client address — there is no
//! `ConnectInfo`/raw TCP peer address use here, since behind that edge the
//! raw peer would always be Cloudflare's own address, not the caller's.

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
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(UNKNOWN_SOURCE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_first_address_from_x_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Forwarded-For",
            "203.0.113.7, 10.0.0.1, 10.0.0.2".parse().unwrap(),
        );
        assert_eq!(source_key_from_headers(&headers), "203.0.113.7");
    }

    #[test]
    fn trims_whitespace_around_the_first_address() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Forwarded-For",
            "  203.0.113.7  , 10.0.0.1".parse().unwrap(),
        );
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
