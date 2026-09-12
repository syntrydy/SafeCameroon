//! Attachment upload preparation, behind a storage port
//! (docs/SECURITY_PRIVACY.md section 5: "access should use short-lived
//! authorization and signed URLs/tokens"; prompt 03's original, never-built
//! "storage port and R2 adapter placeholder"). Domain validation
//! ([`Attachment::new`]) always runs before any URL is issued.

use core::fmt;
use std::time::Duration;

use async_trait::async_trait;
use safe_cameroon_domain::{
    Attachment, AttachmentContentType, AttachmentError, ReportId, StorageProvider,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedUrl {
    pub url: String,
    pub expires_in: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageError {
    pub reason: String,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for StorageError {}

/// Issues short-lived, single-purpose URLs against private objects. An
/// adapter's own vendor SDK/HTTP client never crosses this boundary
/// (docs/CHANNELS.md-style separation, applied to storage).
#[async_trait]
pub trait AttachmentStorage: Send + Sync {
    fn storage_provider(&self) -> StorageProvider;

    /// A URL the caller may `PUT` the raw bytes to directly, so large files
    /// never transit our own service.
    async fn create_upload_url(
        &self,
        object_key: &str,
        content_type: AttachmentContentType,
    ) -> Result<SignedUrl, StorageError>;

    /// A URL to `GET` a private object's bytes.
    async fn create_download_url(&self, object_key: &str) -> Result<SignedUrl, StorageError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareAttachmentError {
    Invalid(AttachmentError),
    Storage(StorageError),
}

impl fmt::Display for PrepareAttachmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => error.fmt(f),
            Self::Storage(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for PrepareAttachmentError {}

#[derive(Debug)]
pub struct PreparedAttachmentUpload {
    pub attachment: Attachment,
    pub upload: SignedUrl,
}

/// Validates the declared metadata, mints an object key scoped to the
/// report, and asks `storage` for a short-lived upload URL to it. Never
/// called with an authorization check of its own — matches AGENTS.md's "do
/// not require authentication for anonymous reporting" (a report's
/// attachments are part of the same anonymous submission).
pub async fn prepare_attachment_upload(
    storage: &dyn AttachmentStorage,
    report_id: ReportId,
    content_type: AttachmentContentType,
    size_bytes: u64,
    checksum: String,
) -> Result<PreparedAttachmentUpload, PrepareAttachmentError> {
    let object_key = format!("attachments/{}/{}", report_id.as_uuid(), Uuid::new_v4());
    let attachment = Attachment::new(
        report_id,
        storage.storage_provider(),
        object_key.clone(),
        content_type,
        size_bytes,
        checksum,
    )
    .map_err(PrepareAttachmentError::Invalid)?;

    let upload = storage
        .create_upload_url(&object_key, content_type)
        .await
        .map_err(PrepareAttachmentError::Storage)?;

    Ok(PreparedAttachmentUpload { attachment, upload })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingStorage {
        requested_upload_keys: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl AttachmentStorage for RecordingStorage {
        fn storage_provider(&self) -> StorageProvider {
            StorageProvider::R2
        }

        async fn create_upload_url(
            &self,
            object_key: &str,
            _content_type: AttachmentContentType,
        ) -> Result<SignedUrl, StorageError> {
            self.requested_upload_keys
                .lock()
                .unwrap()
                .push(object_key.to_owned());
            Ok(SignedUrl {
                url: format!("https://storage.example/{object_key}"),
                expires_in: Duration::from_secs(300),
            })
        }

        async fn create_download_url(&self, object_key: &str) -> Result<SignedUrl, StorageError> {
            Ok(SignedUrl {
                url: format!("https://storage.example/{object_key}"),
                expires_in: Duration::from_secs(300),
            })
        }
    }

    struct AlwaysFailsStorage;

    #[async_trait]
    impl AttachmentStorage for AlwaysFailsStorage {
        fn storage_provider(&self) -> StorageProvider {
            StorageProvider::R2
        }

        async fn create_upload_url(
            &self,
            _object_key: &str,
            _content_type: AttachmentContentType,
        ) -> Result<SignedUrl, StorageError> {
            Err(StorageError {
                reason: "storage unavailable".into(),
            })
        }

        async fn create_download_url(&self, _object_key: &str) -> Result<SignedUrl, StorageError> {
            unreachable!("not exercised by these tests")
        }
    }

    #[tokio::test]
    async fn prepares_a_valid_attachment_and_requests_an_upload_url_for_its_object_key() {
        let storage = RecordingStorage::default();
        let report_id = ReportId::new();

        let prepared = prepare_attachment_upload(
            &storage,
            report_id,
            AttachmentContentType::ImageJpeg,
            2048,
            "deadbeef".into(),
        )
        .await
        .unwrap();

        assert_eq!(prepared.attachment.report_id(), report_id);
        assert_eq!(
            prepared.attachment.object_key(),
            storage.requested_upload_keys.lock().unwrap()[0]
        );
        assert!(
            prepared
                .upload
                .url
                .contains(prepared.attachment.object_key())
        );
    }

    #[tokio::test]
    async fn rejects_invalid_metadata_before_ever_asking_storage_for_a_url() {
        let storage = RecordingStorage::default();

        let error = prepare_attachment_upload(
            &storage,
            ReportId::new(),
            AttachmentContentType::ImageJpeg,
            0,
            "deadbeef".into(),
        )
        .await
        .unwrap_err();

        assert_eq!(
            error,
            PrepareAttachmentError::Invalid(AttachmentError::ZeroSize)
        );
        assert!(storage.requested_upload_keys.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn surfaces_a_storage_failure() {
        let error = prepare_attachment_upload(
            &AlwaysFailsStorage,
            ReportId::new(),
            AttachmentContentType::ImageJpeg,
            2048,
            "deadbeef".into(),
        )
        .await
        .unwrap_err();

        assert!(matches!(error, PrepareAttachmentError::Storage(_)));
    }
}
