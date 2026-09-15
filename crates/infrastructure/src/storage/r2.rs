//! Real Cloudflare R2 [`AttachmentStorage`] adapter (docs/DEPLOYMENT.md),
//! replacing the mock/sandbox `HmacSignedAttachmentStorage` for production
//! use. R2 is API-compatible with S3's SigV4 signing scheme — Cloudflare's
//! own docs recommend signing against R2 exactly as you would against S3 —
//! so this presigns PUT/GET URLs against R2's S3-compatible endpoint
//! (`https://<account_id>.r2.cloudflarestorage.com`) using `aws-sigv4`, the
//! same crate the real AWS SDKs use internally, rather than hand-rolling
//! SigV4 canonicalization: it has enough edge cases (single vs. double
//! percent-encoding, URI path normalization, the `UNSIGNED-PAYLOAD` body
//! hash placeholder) that a bespoke implementation would be a real risk to
//! get subtly wrong for a security-relevant operation.
//!
//! Settings mirror the well-known "presign an S3 request" recipe: query-
//! param signature location, an unsigned body (the client performs the
//! PUT/GET later, not us), single percent-encoding, and no URI path
//! normalization — S3 (and R2) reject normalized paths in some cases.

use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use aws_credential_types::Credentials;
use aws_sigv4::http_request::{
    PayloadChecksumKind, SignableBody, SignableRequest, SignatureLocation, SigningSettings,
    UriPathNormalizationMode, sign,
};
use aws_sigv4::http_request::{PercentEncodingMode, SigningParams as HttpSigningParams};
use aws_sigv4::sign::v4;
use safe_cameroon_application::attachment_workflow::{AttachmentStorage, SignedUrl, StorageError};
use safe_cameroon_domain::{AttachmentContentType, StorageProvider};

const DEFAULT_UPLOAD_TTL: Duration = Duration::from_secs(15 * 60);
const DEFAULT_DOWNLOAD_TTL: Duration = Duration::from_secs(5 * 60);
/// R2 has no regions; Cloudflare's S3-compatibility docs say to sign with
/// this literal value.
const REGION: &str = "auto";
const SERVICE: &str = "s3";

pub struct R2AttachmentStorage {
    endpoint: String,
    bucket: String,
    access_key_id: String,
    secret_access_key: String,
    upload_ttl: Duration,
    download_ttl: Duration,
}

impl R2AttachmentStorage {
    pub fn new(
        account_id: impl Into<String>,
        bucket: impl Into<String>,
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: format!("https://{}.r2.cloudflarestorage.com", account_id.into()),
            bucket: bucket.into(),
            access_key_id: access_key_id.into(),
            secret_access_key: secret_access_key.into(),
            upload_ttl: DEFAULT_UPLOAD_TTL,
            download_ttl: DEFAULT_DOWNLOAD_TTL,
        }
    }

    fn presign(
        &self,
        method: &str,
        object_key: &str,
        ttl: Duration,
    ) -> Result<SignedUrl, StorageError> {
        let uri = format!("{}/{}/{}", self.endpoint, self.bucket, object_key);
        let identity = Credentials::new(
            self.access_key_id.clone(),
            self.secret_access_key.clone(),
            None,
            None,
            "r2-attachment-storage",
        )
        .into();

        // `SigningSettings` is `#[non_exhaustive]`, so it can't be built with
        // a struct literal outside its own crate — start from the default
        // and override only what S3-compatible presigning needs.
        let mut settings = SigningSettings::default();
        settings.percent_encoding_mode = PercentEncodingMode::Single;
        settings.payload_checksum_kind = PayloadChecksumKind::NoHeader;
        settings.signature_location = SignatureLocation::QueryParams;
        settings.expires_in = Some(ttl);
        settings.uri_path_normalization_mode = UriPathNormalizationMode::Disabled;

        let signing_params: HttpSigningParams<'_> = v4::SigningParams::builder()
            .identity(&identity)
            .region(REGION)
            .name(SERVICE)
            .time(SystemTime::now())
            .settings(settings)
            .build()
            .map_err(|error| StorageError {
                reason: error.to_string(),
            })?
            .into();

        let signable_request = SignableRequest::new(
            method,
            &uri,
            std::iter::empty(),
            SignableBody::UnsignedPayload,
        )
        .map_err(|error| StorageError {
            reason: error.to_string(),
        })?;

        let (signing_instructions, _signature) = sign(signable_request, &signing_params)
            .map_err(|error| StorageError {
                reason: error.to_string(),
            })?
            .into_parts();

        let mut request = http::Request::builder()
            .method(method)
            .uri(&uri)
            .body(())
            .map_err(|error| StorageError {
                reason: error.to_string(),
            })?;
        signing_instructions.apply_to_request_http1x(&mut request);

        Ok(SignedUrl {
            url: request.uri().to_string(),
            expires_in: ttl,
        })
    }
}

#[async_trait]
impl AttachmentStorage for R2AttachmentStorage {
    fn storage_provider(&self) -> StorageProvider {
        StorageProvider::R2
    }

    async fn create_upload_url(
        &self,
        object_key: &str,
        _content_type: AttachmentContentType,
    ) -> Result<SignedUrl, StorageError> {
        self.presign("PUT", object_key, self.upload_ttl)
    }

    async fn create_download_url(&self, object_key: &str) -> Result<SignedUrl, StorageError> {
        self.presign("GET", object_key, self.download_ttl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query_params(url: &str) -> std::collections::HashMap<String, String> {
        url.split_once('?')
            .unwrap()
            .1
            .split('&')
            .map(|pair| {
                let (key, value) = pair.split_once('=').unwrap();
                (urlencoding_decode(key), urlencoding_decode(value))
            })
            .collect()
    }

    /// Query params are percent-encoded (e.g. `:` in the credential scope
    /// becomes `%2F` for `/`); a tiny decoder is enough to assert on the
    /// decoded values without pulling in a dependency just for tests.
    fn urlencoding_decode(value: &str) -> String {
        let mut bytes = Vec::with_capacity(value.len());
        let mut chars = value.chars();
        while let Some(c) = chars.next() {
            if c == '%' {
                let hi = chars.next().unwrap();
                let lo = chars.next().unwrap();
                let byte = u8::from_str_radix(&format!("{hi}{lo}"), 16).unwrap();
                bytes.push(byte);
            } else {
                bytes.push(c as u8);
            }
        }
        String::from_utf8(bytes).unwrap()
    }

    fn storage() -> R2AttachmentStorage {
        R2AttachmentStorage::new(
            "test-account",
            "media",
            "test-access-key",
            "test-secret-key",
        )
    }

    #[tokio::test]
    async fn an_upload_url_is_a_presigned_put_against_the_accounts_r2_endpoint() {
        let storage = storage();
        let signed = storage
            .create_upload_url(
                "attachments/report-1/key-1",
                AttachmentContentType::ImageJpeg,
            )
            .await
            .unwrap();

        assert!(signed.url.starts_with(
            "https://test-account.r2.cloudflarestorage.com/media/attachments/report-1/key-1?"
        ));
        let params = query_params(&signed.url);
        assert_eq!(params.get("X-Amz-Algorithm").unwrap(), "AWS4-HMAC-SHA256");
        assert!(
            params
                .get("X-Amz-Credential")
                .unwrap()
                .contains("test-access-key")
        );
        assert!(
            params
                .get("X-Amz-Credential")
                .unwrap()
                .contains("auto/s3/aws4_request")
        );
        assert_eq!(
            params.get("X-Amz-Expires").unwrap(),
            &DEFAULT_UPLOAD_TTL.as_secs().to_string()
        );
        assert_eq!(params.get("X-Amz-SignedHeaders").unwrap(), "host");
        assert_eq!(params.get("X-Amz-Signature").unwrap().len(), 64);
    }

    #[tokio::test]
    async fn a_download_url_uses_a_shorter_ttl_than_an_upload_url() {
        let storage = storage();
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
        assert!(query_params(&download.url).contains_key("X-Amz-Signature"));
    }

    #[tokio::test]
    async fn storage_provider_is_r2() {
        assert_eq!(storage().storage_provider(), StorageProvider::R2);
    }
}
