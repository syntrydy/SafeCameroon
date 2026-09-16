//! The AI extraction port (docs/AI.md, CLAUDE.md "AI integration") --
//! concrete providers (OpenRouter today) live in `crates/infrastructure`
//! and must never leak a vendor request/response shape back into this
//! trait, mirroring [`crate::channel::Channel`]'s "a channel is not a
//! provider" boundary applied to AI providers instead.

use core::fmt;

use async_trait::async_trait;
use safe_cameroon_domain::{ExtractedReportFields, ReportExtractionId, ReportId};
use uuid::Uuid;

/// The prompt/schema version stamped onto every [`ExtractionRecord`]
/// (docs/AI.md section 4: "prompt/template version where practical").
/// Bump this whenever the extraction prompt or requested schema changes, so
/// a historical suggestion stays attributable to the exact instructions
/// that produced it.
pub const EXTRACTION_PROMPT_VERSION: &str = "v1";

/// A provider/network/validation failure. Deliberately not split into
/// retryable/non-retryable like [`crate::channel::ChannelError`] -- a
/// failed extraction has no automatic retry path (prompt 14: avoid
/// speculative infrastructure); a reviewer simply requests it again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionError {
    pub message: String,
}

impl fmt::Display for ExtractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ExtractionError {}

/// A provider adapter turning raw report text into candidate structured
/// fields. Never called with any actor/authorization context of its own --
/// the API layer decides who may request an extraction
/// (`Capability::ViewCase`, matching every other report-read endpoint).
#[async_trait]
pub trait ReportExtractor: Send + Sync {
    /// A stable, human-readable provider name for provenance
    /// (docs/AI.md section 4), e.g. `"OPENROUTER"`.
    fn provider(&self) -> &'static str;

    /// The specific model used, e.g. `"openai/gpt-4o-mini"`.
    fn model(&self) -> &str;

    async fn extract(&self, raw_content: &str) -> Result<ExtractedReportFields, ExtractionError>;
}

/// Full provenance for one extraction request (docs/AI.md section 4):
/// provider, model, prompt version, who asked for it, and when, alongside
/// the structured suggestion itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionRecord {
    pub id: ReportExtractionId,
    pub report_id: ReportId,
    pub requested_by: Uuid,
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
    pub fields: ExtractedReportFields,
}

/// Requests an extraction and packages it with provenance, ready to
/// persist. Never touches the report or any case -- the caller
/// (`apps/api/src/extractions.rs`) only ever stores and returns this; a
/// reviewer decides what to do with it, if anything.
pub async fn request_report_extraction(
    extractor: &dyn ReportExtractor,
    report_id: ReportId,
    raw_content: &str,
    requested_by: Uuid,
) -> Result<ExtractionRecord, ExtractionError> {
    let fields = extractor.extract(raw_content).await?;
    Ok(ExtractionRecord {
        id: ReportExtractionId::new(),
        report_id,
        requested_by,
        provider: extractor.provider().to_owned(),
        model: extractor.model().to_owned(),
        prompt_version: EXTRACTION_PROMPT_VERSION.to_owned(),
        fields,
    })
}

/// A canned/scripted extractor for tests -- no network, no vendor SDK,
/// mirroring [`crate::google_identity::FakeGoogleIdentityVerifier`]'s role
/// for Google sign-in.
pub struct FakeReportExtractor {
    pub result: Result<ExtractedReportFields, ExtractionError>,
}

#[async_trait]
impl ReportExtractor for FakeReportExtractor {
    fn provider(&self) -> &'static str {
        "FAKE"
    }

    fn model(&self) -> &str {
        "fake-extractor-v1"
    }

    async fn extract(&self, _raw_content: &str) -> Result<ExtractedReportFields, ExtractionError> {
        self.result.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn packages_a_successful_extraction_with_full_provenance() {
        let extractor = FakeReportExtractor {
            result: Ok(ExtractedReportFields::new(
                Some("a young girl in a blue uniform".into()),
                Some("about 8 years old".into()),
                None,
                Some("Douala - Bonamoussadi".into()),
                None,
                None,
                None,
            )),
        };
        let report_id = ReportId::new();
        let requested_by = Uuid::new_v4();

        let record =
            request_report_extraction(&extractor, report_id, "some report text", requested_by)
                .await
                .unwrap();

        assert_eq!(record.report_id, report_id);
        assert_eq!(record.requested_by, requested_by);
        assert_eq!(record.provider, "FAKE");
        assert_eq!(record.model, "fake-extractor-v1");
        assert_eq!(record.prompt_version, EXTRACTION_PROMPT_VERSION);
        assert_eq!(record.fields.place, Some("Douala - Bonamoussadi".into()));
    }

    #[tokio::test]
    async fn propagates_a_provider_failure() {
        let extractor = FakeReportExtractor {
            result: Err(ExtractionError {
                message: "provider unavailable".into(),
            }),
        };

        let error =
            request_report_extraction(&extractor, ReportId::new(), "content", Uuid::new_v4())
                .await
                .unwrap_err();

        assert_eq!(error.message, "provider unavailable");
    }
}
