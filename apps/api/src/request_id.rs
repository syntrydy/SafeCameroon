//! One correlation id per request (docs/OBSERVABILITY.md section 2), shared
//! between the `x-request-id` header `SetRequestIdLayer` guarantees is
//! present (generating one when the caller didn't send it) and the
//! `request_id` every handler already threads into `ApiError`, audit
//! events, and domain workflows. Reading it back here — rather than each
//! handler minting its own unrelated `Uuid::new_v4()` — is what makes an
//! HTTP access log line, an audit row, and a domain event all
//! correlatable by the same id.

use axum::http::HeaderMap;
use uuid::Uuid;

use crate::REQUEST_ID_HEADER;

/// Falls back to a fresh id only if the header is missing or unparsable,
/// which should not happen once `SetRequestIdLayer` is wired in — kept as a
/// defensive default rather than a panic.
pub fn request_id_from_headers(headers: &HeaderMap) -> Uuid {
    headers
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value.trim()).ok())
        .unwrap_or_else(Uuid::new_v4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_valid_request_id_header() {
        let id = Uuid::new_v4();
        let mut headers = HeaderMap::new();
        headers.insert(REQUEST_ID_HEADER, id.to_string().parse().unwrap());
        assert_eq!(request_id_from_headers(&headers), id);
    }

    #[test]
    fn falls_back_to_a_fresh_id_when_the_header_is_missing_or_invalid() {
        assert!(request_id_from_headers(&HeaderMap::new()) != Uuid::nil());

        let mut headers = HeaderMap::new();
        headers.insert(REQUEST_ID_HEADER, "not-a-uuid".parse().unwrap());
        assert!(request_id_from_headers(&headers) != Uuid::nil());
    }
}
