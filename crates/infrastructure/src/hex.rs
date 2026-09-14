//! Minimal hex encode/decode shared by every HMAC-signed token scheme in
//! this crate (`storage::hmac_signed`, `auth::session_token`) — small enough
//! that pulling in a crate for it would be the overengineering, not the
//! duplication it replaces.

pub(crate) fn encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn decode(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_then_decoding_round_trips() {
        let bytes = [0u8, 1, 255, 16, 128];
        assert_eq!(decode(&encode(&bytes)).unwrap(), bytes);
    }

    #[test]
    fn decode_rejects_odd_length_input() {
        assert_eq!(decode("abc"), None);
    }

    #[test]
    fn decode_rejects_non_hex_characters() {
        assert_eq!(decode("zz"), None);
    }
}
