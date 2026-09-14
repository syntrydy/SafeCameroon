//! Infrastructure adapters. This crate owns database-specific persistence
//! and provider adapters.

pub mod auth;
pub mod channels;
mod hex;
pub mod postgres;
pub mod storage;
pub mod webhook;
