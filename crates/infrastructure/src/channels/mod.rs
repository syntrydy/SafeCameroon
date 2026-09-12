//! [`Channel`](safe_cameroon_application::channel::Channel) adapters, one
//! module per provider (mirrors `postgres`, one module per aggregate).

pub mod logging;

pub use logging::LoggingChannel;
