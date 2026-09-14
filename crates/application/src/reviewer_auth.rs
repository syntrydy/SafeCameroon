//! Reviewer account credentials (prompt 09, docs/SECURITY_PRIVACY.md section
//! 1: "authenticated platform access" is a distinct identity boundary from
//! anonymous reporting). Password hashing lives here because it is pure
//! computation with no I/O; issuing/verifying the session token handed back
//! after a successful login needs an HMAC secret and lives in
//! `safe_cameroon_infrastructure`, mirroring how `HmacSignedAttachmentStorage`
//! owns signing for short-lived attachment URLs.

use core::fmt;

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};

const MIN_PASSWORD_LENGTH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordError {
    TooShort,
}

impl fmt::Display for PasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(
                f,
                "password must be at least {MIN_PASSWORD_LENGTH} characters"
            ),
        }
    }
}

impl std::error::Error for PasswordError {}

/// Argon2id with a freshly generated random salt (the OWASP-recommended
/// default), encoded as a self-describing PHC string so the algorithm/salt/
/// parameters travel with the hash — no separate salt column needed.
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(PasswordError::TooShort);
    }
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("hashing with a freshly generated salt never fails");
    Ok(hash.to_string())
}

/// Never panics on a malformed `hash` — treats it as a verification failure,
/// since `hash` ultimately comes from a database row a caller does not fully
/// control the shape of.
pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hashed_password_verifies_against_the_same_password() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
    }

    #[test]
    fn a_hashed_password_does_not_verify_against_a_different_password() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(!verify_password("wrong password entirely", &hash));
    }

    #[test]
    fn hashing_the_same_password_twice_produces_different_hashes() {
        let first = hash_password("correct horse battery staple").unwrap();
        let second = hash_password("correct horse battery staple").unwrap();
        assert_ne!(
            first, second,
            "a fresh random salt must be used for every hash"
        );
    }

    #[test]
    fn a_password_shorter_than_the_minimum_is_rejected() {
        assert_eq!(hash_password("short1"), Err(PasswordError::TooShort));
    }

    #[test]
    fn verify_password_rejects_a_malformed_hash_instead_of_panicking() {
        assert!(!verify_password("anything", "not a real password hash"));
    }
}
