//! AI-suggested structured fields extracted from a report's raw text
//! (docs/AI.md section 2 "Extraction", section 3 "Structured outputs").
//! Deliberately generic across incident types, not missing-child-specific
//! (docs/ROADMAP.md Stage 4: "each new incident class should be added by
//! configuration/domain extension, not by copying the entire product") --
//! `incident_category` is the AI's own free-text guess, never a
//! [`crate::IncidentType`] value, since a case's real incident type is
//! always a reviewer's explicit choice, never inferred automatically.
//!
//! This is a suggestion only: nothing here ever changes a
//! [`crate::AnonymousReport`] or [`crate::Case`]'s state on its own (CLAUDE.md "AI
//! integration": "an explicit application-layer decision before changing
//! case state"). A reviewer reads it, then acts (or doesn't) with their own
//! judgment through the ordinary case-creation flow -- there is no
//! accept/reject state to track here.

/// Every field is optional and free-text: the AI may find nothing for a
/// given field, and none of these values are constrained to a closed
/// vocabulary the way a reviewer's own case/alert input is, since the
/// source is unstructured, potentially multilingual, human-written text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExtractedReportFields {
    pub person_description: Option<String>,
    pub age: Option<String>,
    pub time: Option<String>,
    pub place: Option<String>,
    pub incident_category: Option<String>,
    pub vehicle_details: Option<String>,
    pub contact_request: Option<String>,
}

impl ExtractedReportFields {
    /// Normalizes raw (e.g. freshly deserialized from a model response)
    /// optional strings: blank/whitespace-only values become `None` rather
    /// than being stored as meaningless empty strings.
    pub fn new(
        person_description: Option<String>,
        age: Option<String>,
        time: Option<String>,
        place: Option<String>,
        incident_category: Option<String>,
        vehicle_details: Option<String>,
        contact_request: Option<String>,
    ) -> Self {
        fn normalize(value: Option<String>) -> Option<String> {
            value.and_then(|value| {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_owned())
                }
            })
        }
        Self {
            person_description: normalize(person_description),
            age: normalize(age),
            time: normalize(time),
            place: normalize(place),
            incident_category: normalize(incident_category),
            vehicle_details: normalize(vehicle_details),
            contact_request: normalize(contact_request),
        }
    }

    /// Whether the model found nothing at all worth surfacing -- distinct
    /// from an extraction that failed outright (a suggestion with every
    /// field empty is still a valid, successful outcome).
    pub fn is_empty(&self) -> bool {
        self.person_description.is_none()
            && self.age.is_none()
            && self.time.is_none()
            && self.place.is_none()
            && self.incident_category.is_none()
            && self.vehicle_details.is_none()
            && self.contact_request.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_and_whitespace_only_fields_normalize_to_none() {
        let fields = ExtractedReportFields::new(
            Some("  ".into()),
            Some("".into()),
            None,
            Some("Douala".into()),
            None,
            None,
            None,
        );
        assert_eq!(fields.person_description, None);
        assert_eq!(fields.age, None);
        assert_eq!(fields.place, Some("Douala".into()));
    }

    #[test]
    fn trims_surrounding_whitespace() {
        let fields = ExtractedReportFields::new(
            Some("  a young girl, about 8  ".into()),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(
            fields.person_description,
            Some("a young girl, about 8".into())
        );
    }

    #[test]
    fn is_empty_is_true_only_when_every_field_is_none() {
        assert!(ExtractedReportFields::default().is_empty());
        assert!(
            !ExtractedReportFields::new(None, None, None, Some("Douala".into()), None, None, None)
                .is_empty()
        );
    }
}
