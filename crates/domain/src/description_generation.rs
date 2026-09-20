//! An LLM-assisted formal EN/FR alert description pair (docs/AI.md section 2
//! "Translation", CLAUDE.md "AI integration"). Mirrors
//! [`crate::extraction::ExtractedReportFields`]'s role for report
//! extraction: a suggestion only, never applied to an alert automatically --
//! a reviewer reads it, then decides whether to use it, edit it, or ignore
//! it entirely through the ordinary alert-creation form.

/// Both fields are always non-blank: unlike [`crate::extraction::ExtractedReportFields`]
/// (which may legitimately find nothing in unstructured report text), this
/// is a translation/formalization of text the reviewer already supplied, so
/// an empty result is a provider failure, not a valid outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedDescription {
    pub description_en: String,
    pub description_fr: String,
}
