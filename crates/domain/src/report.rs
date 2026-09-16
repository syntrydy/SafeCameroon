use serde::{Deserialize, Serialize};

use crate::{IncidentType, ReportId};

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

    pub fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "WEB" => Some(Self::Web),
            "SMS" => Some(Self::Sms),
            "WHATSAPP" => Some(Self::Whatsapp),
            "PHONE" => Some(Self::Phone),
            "PARTNER_API" => Some(Self::PartnerApi),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousReport {
    pub id: ReportId,
    pub source_channel: ReportSourceChannel,
    pub raw_content: String,
    /// The reporter's own guess at what kind of incident this is, if the
    /// intake UI asked. Never authoritative: a reviewer still explicitly
    /// chooses the `IncidentType` when creating a case from this report
    /// (`case_workflow::create_case_from_report`), same as an AI extraction
    /// suggestion (CLAUDE.md "AI integration" -- untrusted until an
    /// application-layer decision confirms it).
    pub reported_incident_type: Option<IncidentType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReportStatus {
    Received,
    UnderReview,
    LinkedToCase,
    Closed,
}

impl ReportStatus {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::Received => "RECEIVED",
            Self::UnderReview => "UNDER_REVIEW",
            Self::LinkedToCase => "LINKED_TO_CASE",
            Self::Closed => "CLOSED",
        }
    }

    pub fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "RECEIVED" => Some(Self::Received),
            "UNDER_REVIEW" => Some(Self::UnderReview),
            "LINKED_TO_CASE" => Some(Self::LinkedToCase),
            "CLOSED" => Some(Self::Closed),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_source_channel_database_value_round_trips() {
        for channel in [
            ReportSourceChannel::Web,
            ReportSourceChannel::Sms,
            ReportSourceChannel::Whatsapp,
            ReportSourceChannel::Phone,
            ReportSourceChannel::PartnerApi,
        ] {
            let value = channel.as_database_value();
            assert_eq!(
                ReportSourceChannel::from_database_value(value),
                Some(channel)
            );
        }
        assert_eq!(
            ReportSourceChannel::from_database_value("NOT_A_CHANNEL"),
            None
        );
    }

    #[test]
    fn report_status_database_value_round_trips() {
        for status in [
            ReportStatus::Received,
            ReportStatus::UnderReview,
            ReportStatus::LinkedToCase,
            ReportStatus::Closed,
        ] {
            let value = status.as_database_value();
            assert_eq!(ReportStatus::from_database_value(value), Some(status));
        }
        assert_eq!(ReportStatus::from_database_value("NOT_A_STATUS"), None);
    }
}
