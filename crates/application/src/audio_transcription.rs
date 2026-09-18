//! The voice-report transcription port (issue #160). Mirrors
//! [`crate::ai_extraction::ReportExtractor`]'s shape and the same trust
//! boundary: a transcript is never persisted or treated as the report on
//! its own -- the citizen reviews and confirms it client-side, and only the
//! confirmed text is ever submitted through the ordinary anonymous-report
//! endpoint. This module has no persistence/provenance record analogous to
//! [`crate::ai_extraction::ExtractionRecord`], because by the time
//! confirmed text reaches the API, it is indistinguishable from typed
//! text -- there is nothing report-specific to attribute provenance to yet.

use core::fmt;

use async_trait::async_trait;

/// A provider/network/validation failure transcribing one audio clip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptionError {
    pub message: String,
}

impl fmt::Display for TranscriptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TranscriptionError {}

/// A provider adapter turning raw audio bytes into a plain-text transcript.
/// `format` is a short audio format hint (e.g. `"webm"`, `"ogg"`, `"wav"`)
/// derived from the browser's `Content-Type`, not a MIME type string --
/// the concrete provider maps it to whatever its own API expects.
#[async_trait]
pub trait AudioTranscriber: Send + Sync {
    /// A stable, human-readable provider name, e.g. `"OPENROUTER"`.
    fn provider(&self) -> &'static str;

    /// The specific model used, e.g. `"google/gemini-2.5-flash"`.
    fn model(&self) -> &str;

    async fn transcribe(
        &self,
        audio_bytes: &[u8],
        format: &str,
    ) -> Result<String, TranscriptionError>;
}

/// A canned/scripted transcriber for tests -- no network, no vendor SDK,
/// mirroring [`crate::ai_extraction::FakeReportExtractor`].
pub struct FakeAudioTranscriber {
    pub result: Result<String, TranscriptionError>,
}

#[async_trait]
impl AudioTranscriber for FakeAudioTranscriber {
    fn provider(&self) -> &'static str {
        "FAKE"
    }

    fn model(&self) -> &str {
        "fake-transcriber-v1"
    }

    async fn transcribe(
        &self,
        _audio_bytes: &[u8],
        _format: &str,
    ) -> Result<String, TranscriptionError> {
        self.result.clone()
    }
}
