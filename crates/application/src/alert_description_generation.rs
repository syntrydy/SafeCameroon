//! The AI alert-description-generation port (docs/AI.md section 2
//! "Translation", CLAUDE.md "AI integration") -- concrete providers
//! (OpenRouter today) live in `crates/infrastructure`, mirroring
//! [`crate::ai_extraction`]'s "a port is not a provider" boundary applied
//! to this second AI capability.

use core::fmt;

use async_trait::async_trait;
use safe_cameroon_domain::{AlertDescriptionGenerationId, CaseId, GeneratedDescription};
use uuid::Uuid;

/// The prompt/schema version stamped onto every [`GenerationRecord`]
/// (docs/AI.md section 4). Bump this whenever the generation prompt or
/// requested schema changes.
pub const DESCRIPTION_GENERATION_PROMPT_VERSION: &str = "v1";

/// A provider/network/validation failure -- no automatic retry path, same
/// stance as [`crate::ai_extraction::ExtractionError`]: a reviewer simply
/// requests it again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationError {
    pub message: String,
}

impl fmt::Display for GenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for GenerationError {}

/// A provider adapter formalizing/translating a reviewer's draft into a
/// formal English + French pair. Never called with any actor/authorization
/// context of its own -- the API layer decides who may request this
/// (`Capability::ViewCase`, same gate as report extraction).
#[async_trait]
pub trait AlertDescriptionGenerator: Send + Sync {
    /// A stable, human-readable provider name for provenance, e.g.
    /// `"OPENROUTER"`.
    fn provider(&self) -> &'static str;

    /// The specific model used, e.g. `"openai/gpt-4o-mini"`.
    fn model(&self) -> &str;

    async fn generate(&self, source_text: &str) -> Result<GeneratedDescription, GenerationError>;
}

/// Full provenance for one generation request (docs/AI.md section 4):
/// provider, model, prompt version, who asked for it, and when, alongside
/// the generated pair itself. Tied to `case_id`, not an alert id, since a
/// suggestion is requested before the alert exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationRecord {
    pub id: AlertDescriptionGenerationId,
    pub case_id: CaseId,
    pub requested_by: Uuid,
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
    pub source_text: String,
    pub description: GeneratedDescription,
}

/// Requests a generation and packages it with provenance, ready to persist.
/// Never touches the case or any alert -- the caller
/// (`apps/api/src/alert_description.rs`) only ever stores and returns this;
/// a reviewer decides what to do with it, if anything.
pub async fn request_description_generation(
    generator: &dyn AlertDescriptionGenerator,
    case_id: CaseId,
    source_text: &str,
    requested_by: Uuid,
) -> Result<GenerationRecord, GenerationError> {
    let description = generator.generate(source_text).await?;
    Ok(GenerationRecord {
        id: AlertDescriptionGenerationId::new(),
        case_id,
        requested_by,
        provider: generator.provider().to_owned(),
        model: generator.model().to_owned(),
        prompt_version: DESCRIPTION_GENERATION_PROMPT_VERSION.to_owned(),
        source_text: source_text.to_owned(),
        description,
    })
}

/// A canned/scripted generator for tests -- no network, no vendor SDK,
/// mirroring [`crate::ai_extraction::FakeReportExtractor`].
pub struct FakeAlertDescriptionGenerator {
    pub result: Result<GeneratedDescription, GenerationError>,
}

#[async_trait]
impl AlertDescriptionGenerator for FakeAlertDescriptionGenerator {
    fn provider(&self) -> &'static str {
        "FAKE"
    }

    fn model(&self) -> &str {
        "fake-generator-v1"
    }

    async fn generate(&self, _source_text: &str) -> Result<GeneratedDescription, GenerationError> {
        self.result.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn packages_a_successful_generation_with_full_provenance() {
        let generator = FakeAlertDescriptionGenerator {
            result: Ok(GeneratedDescription {
                description_en: "An 8-year-old girl was last seen near the central market.".into(),
                description_fr:
                    "Une fillette de 8 ans a ete vue pour la derniere fois pres du marche central."
                        .into(),
            }),
        };
        let case_id = CaseId::new();
        let requested_by = Uuid::new_v4();

        let record = request_description_generation(
            &generator,
            case_id,
            "girl, 8yo, last seen near central market",
            requested_by,
        )
        .await
        .unwrap();

        assert_eq!(record.case_id, case_id);
        assert_eq!(record.requested_by, requested_by);
        assert_eq!(record.provider, "FAKE");
        assert_eq!(record.model, "fake-generator-v1");
        assert_eq!(record.prompt_version, DESCRIPTION_GENERATION_PROMPT_VERSION);
        assert_eq!(
            record.description.description_en,
            "An 8-year-old girl was last seen near the central market."
        );
        assert_eq!(
            record.source_text,
            "girl, 8yo, last seen near central market"
        );
    }

    #[tokio::test]
    async fn propagates_a_provider_failure() {
        let generator = FakeAlertDescriptionGenerator {
            result: Err(GenerationError {
                message: "provider unavailable".into(),
            }),
        };

        let error =
            request_description_generation(&generator, CaseId::new(), "content", Uuid::new_v4())
                .await
                .unwrap_err();

        assert_eq!(error.message, "provider unavailable");
    }
}
