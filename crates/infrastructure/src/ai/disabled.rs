//! Stands in for a real provider when no AI extraction credentials are
//! configured (`OPENROUTER_API_KEY` unset) -- mirrors how attachment
//! storage falls back to a mock rather than refusing to boot
//! (`apps/api/src/main.rs::attachment_storage_from_env`), except there is
//! no sensible mock *extraction* to perform, so this just fails clearly
//! whenever it's actually asked to extract.

use async_trait::async_trait;
use safe_cameroon_application::ai_extraction::{ExtractionError, ReportExtractor};
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
