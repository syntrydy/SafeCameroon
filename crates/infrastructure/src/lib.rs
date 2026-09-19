//! Infrastructure adapters. This crate owns database-specific persistence
//! and provider adapters.

pub mod ai;
pub mod auth;
pub mod channels;
pub mod google_identity;
mod hex;
pub mod invite_mailer;
pub mod postgres;
pub mod storage;
pub mod webhook;
