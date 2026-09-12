//! Report attachment metadata (docs/DATABASE.md section 7: "Postgres stores
//! metadata; the file itself lives in R2"). An [`Attachment`] never carries
//! the file's bytes — only enough to locate and validate it through a
//! storage port.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::{AttachmentId, ReportId};

/// The only closed vocabulary today (docs/DATABASE.md section 7 lists
/// `storage_provider` as a column, not a fixed enum, but a real second
/// provider is not expected soon and a closed set is safer than a free
/// string until one is).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorageProvider {
    R2,
}

impl StorageProvider {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::R2 => "R2",
        }
    }

    pub fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "R2" => Some(Self::R2),
            _ => None,
        }
    }
}

/// The closed set of content types accepted as report evidence
/// (docs/SECURITY_PRIVACY.md section 8: "attachment limits"). Malware
/// scanning is a separate, later infrastructure concern; restricting the
/// declared content type at all is a cheap first control. Revisit this list
/// as real evidence types are observed during the pilot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttachmentContentType {
    ImageJpeg,
    ImagePng,
    ImageWebp,
    ApplicationPdf,
    AudioMpeg,
    AudioOgg,
    VideoMp4,
}

impl AttachmentContentType {
    pub fn as_mime_type(self) -> &'static str {
        match self {
            Self::ImageJpeg => "image/jpeg",
            Self::ImagePng => "image/png",
            Self::ImageWebp => "image/webp",
            Self::ApplicationPdf => "application/pdf",
            Self::AudioMpeg => "audio/mpeg",
            Self::AudioOgg => "audio/ogg",
            Self::VideoMp4 => "video/mp4",
        }
    }

    pub fn from_mime_type(value: &str) -> Option<Self> {
        match value {
            "image/jpeg" => Some(Self::ImageJpeg),
            "image/png" => Some(Self::ImagePng),
            "image/webp" => Some(Self::ImageWebp),
            "application/pdf" => Some(Self::ApplicationPdf),
            "audio/mpeg" => Some(Self::AudioMpeg),
            "audio/ogg" => Some(Self::AudioOgg),
            "video/mp4" => Some(Self::VideoMp4),
            _ => None,
        }
    }

    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::ImageJpeg => "IMAGE_JPEG",
            Self::ImagePng => "IMAGE_PNG",
            Self::ImageWebp => "IMAGE_WEBP",
            Self::ApplicationPdf => "APPLICATION_PDF",
            Self::AudioMpeg => "AUDIO_MPEG",
            Self::AudioOgg => "AUDIO_OGG",
            Self::VideoMp4 => "VIDEO_MP4",
        }
    }

    pub fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "IMAGE_JPEG" => Some(Self::ImageJpeg),
            "IMAGE_PNG" => Some(Self::ImagePng),
            "IMAGE_WEBP" => Some(Self::ImageWebp),
            "APPLICATION_PDF" => Some(Self::ApplicationPdf),
            "AUDIO_MPEG" => Some(Self::AudioMpeg),
            "AUDIO_OGG" => Some(Self::AudioOgg),
            "VIDEO_MP4" => Some(Self::VideoMp4),
            _ => None,
        }
    }
}

/// A conservative starting cap (docs/SECURITY_PRIVACY.md section 8:
/// "attachment limits"); revisit once real evidence sizes are observed.
pub const MAX_ATTACHMENT_SIZE_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentError {
    EmptyObjectKey,
    EmptyChecksum,
    ZeroSize,
    SizeExceedsLimit { max_bytes: u64 },
}

impl fmt::Display for AttachmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyObjectKey => write!(f, "object_key cannot be blank"),
            Self::EmptyChecksum => write!(f, "checksum cannot be blank"),
            Self::ZeroSize => write!(f, "size_bytes must be greater than zero"),
            Self::SizeExceedsLimit { max_bytes } => {
                write!(f, "size_bytes exceeds the {max_bytes}-byte limit")
            }
        }
    }
}

impl std::error::Error for AttachmentError {}

/// Metadata for one piece of evidence attached to a report. Never holds the
/// file's bytes; `object_key` is what a [`StorageProvider`] adapter resolves
/// to the actual private object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    id: AttachmentId,
    report_id: ReportId,
    storage_provider: StorageProvider,
    object_key: String,
    content_type: AttachmentContentType,
    size_bytes: u64,
    checksum: String,
}

impl Attachment {
    pub fn new(
        report_id: ReportId,
        storage_provider: StorageProvider,
        object_key: impl Into<String>,
        content_type: AttachmentContentType,
        size_bytes: u64,
        checksum: impl Into<String>,
    ) -> Result<Self, AttachmentError> {
        let object_key = object_key.into();
        if object_key.trim().is_empty() {
            return Err(AttachmentError::EmptyObjectKey);
        }
        let checksum = checksum.into();
        if checksum.trim().is_empty() {
            return Err(AttachmentError::EmptyChecksum);
        }
        if size_bytes == 0 {
            return Err(AttachmentError::ZeroSize);
        }
        if size_bytes > MAX_ATTACHMENT_SIZE_BYTES {
            return Err(AttachmentError::SizeExceedsLimit {
                max_bytes: MAX_ATTACHMENT_SIZE_BYTES,
            });
        }

        Ok(Self {
            id: AttachmentId::new(),
            report_id,
            storage_provider,
            object_key,
            content_type,
            size_bytes,
            checksum,
        })
    }

    /// Rebuilds an attachment from persisted state; performs no validation,
    /// mirroring [`crate::Alert::reconstitute`].
    #[allow(clippy::too_many_arguments)]
    pub fn reconstitute(
        id: AttachmentId,
        report_id: ReportId,
        storage_provider: StorageProvider,
        object_key: String,
        content_type: AttachmentContentType,
        size_bytes: u64,
        checksum: String,
    ) -> Self {
        Self {
            id,
            report_id,
            storage_provider,
            object_key,
            content_type,
            size_bytes,
            checksum,
        }
    }

    pub fn id(&self) -> AttachmentId {
        self.id
    }
    pub fn report_id(&self) -> ReportId {
        self.report_id
    }
    pub fn storage_provider(&self) -> StorageProvider {
        self.storage_provider
    }
    pub fn object_key(&self) -> &str {
        &self.object_key
    }
    pub fn content_type(&self) -> AttachmentContentType {
        self.content_type
    }
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
    pub fn checksum(&self) -> &str {
        &self.checksum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> (
        ReportId,
        StorageProvider,
        &'static str,
        AttachmentContentType,
        u64,
        &'static str,
    ) {
        (
            ReportId::new(),
            StorageProvider::R2,
            "attachments/report-1/attachment-1",
            AttachmentContentType::ImageJpeg,
            1024,
            "deadbeef",
        )
    }

    #[test]
    fn constructs_a_valid_attachment() {
        let (report_id, provider, key, content_type, size, checksum) = valid();
        let attachment =
            Attachment::new(report_id, provider, key, content_type, size, checksum).unwrap();
        assert_eq!(attachment.report_id(), report_id);
        assert_eq!(attachment.object_key(), key);
        assert_eq!(attachment.content_type(), content_type);
        assert_eq!(attachment.size_bytes(), size);
        assert_eq!(attachment.checksum(), checksum);
    }

    #[test]
    fn rejects_a_blank_object_key() {
        let (report_id, provider, _, content_type, size, checksum) = valid();
        let error =
            Attachment::new(report_id, provider, "   ", content_type, size, checksum).unwrap_err();
        assert_eq!(error, AttachmentError::EmptyObjectKey);
    }

    #[test]
    fn rejects_a_blank_checksum() {
        let (report_id, provider, key, content_type, size, _) = valid();
        let error =
            Attachment::new(report_id, provider, key, content_type, size, "  ").unwrap_err();
        assert_eq!(error, AttachmentError::EmptyChecksum);
    }

    #[test]
    fn rejects_zero_size() {
        let (report_id, provider, key, content_type, _, checksum) = valid();
        let error =
            Attachment::new(report_id, provider, key, content_type, 0, checksum).unwrap_err();
        assert_eq!(error, AttachmentError::ZeroSize);
    }

    #[test]
    fn rejects_a_size_over_the_limit() {
        let (report_id, provider, key, content_type, _, checksum) = valid();
        let error = Attachment::new(
            report_id,
            provider,
            key,
            content_type,
            MAX_ATTACHMENT_SIZE_BYTES + 1,
            checksum,
        )
        .unwrap_err();
        assert_eq!(
            error,
            AttachmentError::SizeExceedsLimit {
                max_bytes: MAX_ATTACHMENT_SIZE_BYTES
            }
        );
    }

    #[test]
    fn mime_type_round_trips_through_the_content_type_enum() {
        for content_type in [
            AttachmentContentType::ImageJpeg,
            AttachmentContentType::ImagePng,
            AttachmentContentType::ImageWebp,
            AttachmentContentType::ApplicationPdf,
            AttachmentContentType::AudioMpeg,
            AttachmentContentType::AudioOgg,
            AttachmentContentType::VideoMp4,
        ] {
            assert_eq!(
                AttachmentContentType::from_mime_type(content_type.as_mime_type()),
                Some(content_type)
            );
            assert_eq!(
                AttachmentContentType::from_database_value(content_type.as_database_value()),
                Some(content_type)
            );
        }
        assert_eq!(
            AttachmentContentType::from_mime_type("application/x-executable"),
            None
        );
    }
}
