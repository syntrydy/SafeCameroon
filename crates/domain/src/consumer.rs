//! A consumer is a receiver identity subscriptions and deliveries belong to
//! (docs/DOMAIN_MODEL.md section 9): an organization (police, NGO,
//! association, school, municipality) or a citizen.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::ConsumerId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConsumerType {
    Organization,
    Citizen,
}

impl ConsumerType {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::Organization => "ORGANIZATION",
            Self::Citizen => "CITIZEN",
        }
    }

    pub fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "ORGANIZATION" => Some(Self::Organization),
            "CITIZEN" => Some(Self::Citizen),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyConsumerName;

impl fmt::Display for EmptyConsumerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "consumer name cannot be blank")
    }
}

impl std::error::Error for EmptyConsumerName {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consumer {
    id: ConsumerId,
    name: String,
    consumer_type: ConsumerType,
    /// The recipient's preferred language, e.g. `"en"`/`"fr"` (a citizen
    /// subscriber's browser-detected locale, captured at subscribe time --
    /// `apps/citizen/src/i18n/locale.ts`'s `detectLocale`). Advisory only:
    /// nothing reads this yet to translate an alert's content, it is just
    /// captured so that can be added later without touching the subscribe
    /// flow again. `None` for a consumer that never supplied one (every
    /// organization, and any citizen subscription predating this field).
    locale: Option<String>,
}

impl Consumer {
    pub fn new(
        name: impl Into<String>,
        consumer_type: ConsumerType,
    ) -> Result<Self, EmptyConsumerName> {
        let name = name.into();
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(EmptyConsumerName);
        }
        Ok(Self {
            id: ConsumerId::new(),
            name: trimmed.to_owned(),
            consumer_type,
            locale: None,
        })
    }

    /// Sets the preferred-language hint at creation time
    /// (`crates/application/src/citizen_subscription.rs`). A blank/whitespace
    /// value normalizes to `None` rather than storing a meaningless empty
    /// string; not otherwise validated against a closed set of language
    /// codes, so a new language can be added on the frontend without a
    /// backend change.
    pub fn with_locale(mut self, locale: Option<String>) -> Self {
        self.locale = locale
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        self
    }

    /// Rebuilds a consumer from persisted state; infrastructure adapters use
    /// this rather than `new`, which always mints a fresh id.
    pub fn reconstitute(
        id: ConsumerId,
        name: String,
        consumer_type: ConsumerType,
        locale: Option<String>,
    ) -> Self {
        Self {
            id,
            name,
            consumer_type,
            locale,
        }
    }

    pub fn id(&self) -> ConsumerId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn consumer_type(&self) -> ConsumerType {
        self.consumer_type
    }

    pub fn locale(&self) -> Option<&str> {
        self.locale.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_name_is_rejected() {
        assert_eq!(
            Consumer::new("   ", ConsumerType::Organization).unwrap_err(),
            EmptyConsumerName
        );
    }

    #[test]
    fn a_valid_name_is_trimmed_and_a_fresh_id_is_minted() {
        let consumer = Consumer::new("  Douala Police  ", ConsumerType::Organization).unwrap();
        assert_eq!(consumer.name(), "Douala Police");
        assert_eq!(consumer.consumer_type(), ConsumerType::Organization);
    }

    #[test]
    fn a_freshly_created_consumer_has_no_locale() {
        let consumer = Consumer::new("Douala Police", ConsumerType::Organization).unwrap();
        assert_eq!(consumer.locale(), None);
    }

    #[test]
    fn with_locale_trims_and_normalizes_a_blank_value_to_none() {
        let consumer = Consumer::new("Citizen (self-subscribed)", ConsumerType::Citizen)
            .unwrap()
            .with_locale(Some("  fr  ".into()));
        assert_eq!(consumer.locale(), Some("fr"));

        let consumer = Consumer::new("Citizen (self-subscribed)", ConsumerType::Citizen)
            .unwrap()
            .with_locale(Some("   ".into()));
        assert_eq!(consumer.locale(), None);
    }

    #[test]
    fn consumer_type_database_value_round_trips() {
        for consumer_type in [ConsumerType::Organization, ConsumerType::Citizen] {
            let value = consumer_type.as_database_value();
            assert_eq!(
                ConsumerType::from_database_value(value),
                Some(consumer_type)
            );
        }
        assert_eq!(ConsumerType::from_database_value("NOT_A_TYPE"), None);
    }
}
