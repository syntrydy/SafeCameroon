//! Shared `Idempotency-Key` header parsing/validation, used by every
//! endpoint that accepts one (`POST /v1/reports`, `POST
//! /v1/cases/{id}/alerts`) so the 8-256 character rule lives in one place
//! rather than being copied at each call site.

use axum::http::HeaderMap;
use uuid::Uuid;

use crate::error::ApiError;

const IDEMPOTENCY_KEY_HEADER: &str = "Idempotency-Key";
const MIN_LENGTH: usize = 8;
const MAX_LENGTH: usize = 256;

fn invalid_idempotency_key(request_id: Uuid) -> ApiError {
    ApiError {
        status: axum::http::StatusCode::BAD_REQUEST,
        code: "INVALID_IDEMPOTENCY_KEY",
        message: "Idempotency-Key must be between 8 and 256 characters.",
        request_id,
    }
}

/// `None` when the header is absent or blank; `Err` when present but outside
/// the allowed length.
pub fn idempotency_key_from_headers(
    headers: &HeaderMap,
    request_id: Uuid,
) -> Result<Option<&str>, ApiError> {
    let key = headers
        .get(IDEMPOTENCY_KEY_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(key) = key {
        if !(MIN_LENGTH..=MAX_LENGTH).contains(&key.len()) {
            return Err(invalid_idempotency_key(request_id));
        }
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_header_is_none() {
        assert_eq!(
            idempotency_key_from_headers(&HeaderMap::new(), Uuid::new_v4()).unwrap(),
            None
        );
    }

    #[test]
    fn a_blank_header_is_none() {
        let mut headers = HeaderMap::new();
        headers.insert(IDEMPOTENCY_KEY_HEADER, "   ".parse().unwrap());
        assert_eq!(
            idempotency_key_from_headers(&headers, Uuid::new_v4()).unwrap(),
            None
        );
    }

    #[test]
    fn a_valid_key_is_trimmed_and_returned() {
        let mut headers = HeaderMap::new();
        headers.insert(IDEMPOTENCY_KEY_HEADER, "  retry-key-123  ".parse().unwrap());
        assert_eq!(
            idempotency_key_from_headers(&headers, Uuid::new_v4()).unwrap(),
            Some("retry-key-123")
        );
    }

    #[test]
    fn a_key_shorter_than_eight_characters_is_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(IDEMPOTENCY_KEY_HEADER, "short".parse().unwrap());
        let error = idempotency_key_from_headers(&headers, Uuid::new_v4()).unwrap_err();
        assert_eq!(error.code, "INVALID_IDEMPOTENCY_KEY");
    }

    #[test]
    fn a_key_longer_than_256_characters_is_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(IDEMPOTENCY_KEY_HEADER, "a".repeat(257).parse().unwrap());
        let error = idempotency_key_from_headers(&headers, Uuid::new_v4()).unwrap_err();
        assert_eq!(error.code, "INVALID_IDEMPOTENCY_KEY");
    }
}
