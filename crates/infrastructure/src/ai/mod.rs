//! [`ReportExtractor`](safe_cameroon_application::ai_extraction::ReportExtractor)
//! adapters, one module per provider (mirrors `channels`, `postgres`).

pub mod disabled;
pub mod openrouter;

pub use disabled::DisabledExtractor;
pub use openrouter::OpenRouterExtractor;
