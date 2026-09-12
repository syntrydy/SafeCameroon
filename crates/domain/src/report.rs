use serde::{Deserialize, Serialize};

use crate::ReportId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReportSourceChannel {
    Web,
    Sms,
    Whatsapp,
    Phone,
    PartnerApi,
}

impl ReportSourceChannel {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::Web => "WEB",
            Self::Sms => "SMS",
            Self::Whatsapp => "WHATSAPP",
            Self::Phone => "PHONE",
            Self::PartnerApi => "PARTNER_API",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousReport {
    pub id: ReportId,
    pub source_channel: ReportSourceChannel,
    pub raw_content: String,
}
