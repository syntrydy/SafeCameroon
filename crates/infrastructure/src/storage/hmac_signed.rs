//! Mock/sandbox [`AttachmentStorage`] adapter (prompt 09; docs/DEPLOYMENT.md:
//! R2 is the intended real provider, but no credentials exist yet — mirrors
//! how the mock channel adapters in `crates/infrastructure/src/channels`
//! stand in for real WhatsApp/SMS/Email providers). Signs a URL with
//! HMAC-SHA256 over `object_key:expires_at`, the same real, working scheme
//! `HmacSignedWebhookVerifier` uses for inbound callbacks, applied here to
//! outbound short-lived access.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use hmac::{Hmac, Mac};
use safe_cameroon_application::attachment_workflow::{AttachmentStorage, SignedUrl, StorageError};
use safe_cameroon_domain::{AttachmentContentType, StorageProvider};
use sha2::Sha256;

use crate::hex;

type HmacSha256 = Hmac<Sha256>;

const DEFAULT_UPLOAD_TTL: Duration = Duration::from_secs(15 * 60);
const DEFAULT_DOWNLOAD_TTL: Duration = Duration::from_secs(5 * 60);

pub struct HmacSignedAttachmentStorage {
    base_url: String,
    secret: Vec<u8>,
    upload_ttl: Duration,
    download_ttl: Duration,
}

impl HmacSignedAttachmentStorage {
    pub fn new(base_url: impl Into<String>, secret: impl Into<Vec<u8>>) -> Self {
        Self {
            base_url: base_url.into(),
            secret: secret.into(),
            upload_ttl: DEFAULT_UPLOAD_TTL,
            download_ttl: DEFAULT_DOWNLOAD_TTL,
        }
    }

    fn sign(&self, object_key: &str, expires_at: u64) -> String {
        let mut mac = HmacSha256::new_from_slice(&self.secret)
            .expect("HMAC-SHA256 accepts a key of any length");
        mac.update(format!("{object_key}:{expires_at}").as_bytes());
        hex::encode(&mac.finalize().into_bytes())
    }

    /// Recomputes the signature for `(object_key, expires_at)` and compares
    /// it against `signature` in constant time, also rejecting an expired
    /// URL. Exists mainly to prove the scheme is real and testable; a real
    /// deployment's actual verification happens inside the storage
    /// provider, not this process.
    pub fn verify(&self, object_key: &str, expires_at: u64, signature: &str) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_secs();
        if expires_at < now {
            return false;
        }
        let Some(signature_bytes) = hex::decode(signature) else {
            return false;
        };
        let mut mac = HmacSha256::new_from_slice(&self.secret)
            .expect("HMAC-SHA256 accepts a key of any length");
        mac.update(format!("{object_key}:{expires_at}").as_bytes());
        mac.verify_slice(&signature_bytes).is_ok()
    }

    fn signed_url(&self, object_key: &str, ttl: Duration) -> SignedUrl {
        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_secs()
            + ttl.as_secs();
        let signature = self.sign(object_key, expires_at);
        SignedUrl {
            url: format!(
                "{}/{object_key}?expires={expires_at}&signature={signature}",
                self.base_url
            ),
            expires_in: ttl,
        }
    }
}

#[async_trait]
impl AttachmentStorage for HmacSignedAttachmentStorage {
    fn storage_provider(&self) -> StorageProvider {
        StorageProvider::R2
    }

    async fn create_upload_url(
        &self,
        object_key: &str,
        _content_type: AttachmentContentType,
    ) -> Result<SignedUrl, StorageError> {
        Ok(self.signed_url(object_key, self.upload_ttl))
    }

    async fn create_download_url(&self, object_key: &str) -> Result<SignedUrl, StorageError> {
        Ok(self.signed_url(object_key, self.download_ttl))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_query(url: &str) -> (u64, String) {
        let query = url.split_once('?').unwrap().1;
        let mut expires_at = 0;
        let mut signature = String::new();
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').unwrap();
            match key {
                "expires" => expires_at = value.parse().unwrap(),
                "signature" => signature = value.to_owned(),
                _ => {}
            }
        }
        (expires_at, signature)
    }

    #[tokio::test]
    async fn an_issued_upload_url_verifies_and_a_tampered_one_does_not() {
        let storage =
            HmacSignedAttachmentStorage::new("https://storage.example", b"secret".to_vec());
        let signed = storage
            .create_upload_url(
                "attachments/report-1/key-1",
                AttachmentContentType::ImageJpeg,
            )
            .await
            .unwrap();

        let (expires_at, signature) = parse_query(&signed.url);
        assert!(storage.verify("attachments/report-1/key-1", expires_at, &signature));
        assert!(!storage.verify("attachments/report-1/key-2", expires_at, &signature));
        assert!(!storage.verify(
            "attachments/report-1/key-1",
            expires_at,
            "0000000000000000000000000000000000000000000000000000000000000000"
        ));
    }

    #[tokio::test]
    async fn an_expired_url_does_not_verify() {
        let storage =
            HmacSignedAttachmentStorage::new("https://storage.example", b"secret".to_vec());
        let expired_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 1;
        let signature = storage.sign("attachments/report-1/key-1", expired_at);
        assert!(!storage.verify("attachments/report-1/key-1", expired_at, &signature));
    }

    #[tokio::test]
    async fn download_urls_use_a_shorter_ttl_than_upload_urls() {
        let storage =
            HmacSignedAttachmentStorage::new("https://storage.example", b"secret".to_vec());
        let upload = storage
            .create_upload_url(
                "attachments/report-1/key-1",
                AttachmentContentType::ImageJpeg,
            )
            .await
            .unwrap();
        let download = storage
            .create_download_url("attachments/report-1/key-1")
            .await
            .unwrap();
        assert!(download.expires_in < upload.expires_in);
    }
}
