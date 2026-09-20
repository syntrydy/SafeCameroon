//! [`ReportExtractor`](safe_cameroon_application::ai_extraction::ReportExtractor)
//! and [`AudioTranscriber`](safe_cameroon_application::audio_transcription::AudioTranscriber)
//! adapters, one module per provider (mirrors `channels`, `postgres`).

pub mod disabled;
pub mod openrouter;
pub mod openrouter_description;
pub mod transcription;

pub use disabled::{DisabledExtractor, DisabledGenerator, DisabledTranscriber};
pub use openrouter::OpenRouterExtractor;
pub use openrouter_description::OpenRouterDescriptionGenerator;
pub use transcription::OpenRouterTranscriber;
