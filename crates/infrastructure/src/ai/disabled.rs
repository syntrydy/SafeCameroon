//! Stands in for a real provider when no AI extraction credentials are
//! configured (`OPENROUTER_API_KEY` unset) -- mirrors how attachment
//! storage falls back to a mock rather than refusing to boot
//! (`apps/api/src/main.rs::attachment_storage_from_env`), except there is
//! no sensible mock *extraction* to perform, so this just fails clearly
//! whenever it's actually asked to extract.

use async_trait::async_trait;
use safe_cameroon_application::ai_extraction::{ExtractionError, ReportExtractor};
use safe_cameroon_application::audio_transcription::{AudioTranscriber, TranscriptionError};
use safe_cameroon_domain::ExtractedReportFields;

#[derive(Debug, Clone, Copy, Default)]
pub struct DisabledExtractor;

#[async_trait]
impl ReportExtractor for DisabledExtractor {
    fn provider(&self) -> &'static str {
        "DISABLED"
    }

    fn model(&self) -> &str {
        "none"
    }

    async fn extract(&self, _raw_content: &str) -> Result<ExtractedReportFields, ExtractionError> {
        Err(ExtractionError {
            message: "AI extraction is not configured for this deployment".into(),
        })
    }
}

/// Same "fail clearly, don't refuse to boot" stance as [`DisabledExtractor`],
/// for when voice transcription has no credentials configured
/// (`OPENROUTER_API_KEY` unset) -- distinct from the `VOICE_REPORTS_ENABLED`
/// feature flag, which is an operator kill-switch checked in
/// `apps/api/src/reports.rs` before this is ever reached.
#[derive(Debug, Clone, Copy, Default)]
pub struct DisabledTranscriber;

#[async_trait]
impl AudioTranscriber for DisabledTranscriber {
    fn provider(&self) -> &'static str {
        "DISABLED"
    }

    fn model(&self) -> &str {
        "none"
    }

    async fn transcribe(
        &self,
        _audio_bytes: &[u8],
        _format: &str,
    ) -> Result<String, TranscriptionError> {
        Err(TranscriptionError {
            message: "Voice transcription is not configured for this deployment".into(),
        })
    }
}
