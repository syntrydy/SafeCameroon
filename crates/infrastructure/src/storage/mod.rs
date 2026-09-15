//! [`AttachmentStorage`](safe_cameroon_application::attachment_workflow::AttachmentStorage)
//! adapters.

pub mod hmac_signed;
pub mod r2;

pub use hmac_signed::HmacSignedAttachmentStorage;
pub use r2::R2AttachmentStorage;
