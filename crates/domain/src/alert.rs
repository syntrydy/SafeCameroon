//! Alert policy and the verified-case-to-alert projection (docs/ALERT_SAFETY.md,
//! docs/DOMAIN_MODEL.md section 8). An [`Alert`] is a controlled, policy-scoped
//! view of a [`Case`] — never the case itself, and never the raw report.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::{AlertEventId, AlertId, Case, CaseEventType, CaseId, CaseStatus, IncidentType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlertVisibility {
    Internal,
    Partner,
    Community,
    Public,
}

impl AlertVisibility {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::Internal => "INTERNAL",
            Self::Partner => "PARTNER",
            Self::Community => "COMMUNITY",
            Self::Public => "PUBLIC",
        }
    }

    pub fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "INTERNAL" => Some(Self::Internal),
            "PARTNER" => Some(Self::Partner),
            "COMMUNITY" => Some(Self::Community),
            "PUBLIC" => Some(Self::Public),
            _ => None,
        }
    }

    /// Internal/partner audiences may legitimately need fields (exact
    /// location, internal notes, witness details) that a community or public
    /// audience must never see (docs/ALERT_SAFETY.md, section 8).
    pub fn admits_internal_only_fields(self) -> bool {
        matches!(self, Self::Internal | Self::Partner)
    }
}

/// The closed vocabulary of data an alert may ever carry. Keeping this a
/// finite enum (rather than a free-form string field name) is what makes the
/// allowlist deterministic and checkable at compile time and in tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlertField {
    IncidentCategory,
    ApproximateAge,
    LastSeenGeneralArea,
    TimeWindow,
    SafeDescription,
    OfficialContact,
    CaseReference,
    ReporterIdentity,
    InternalNotes,
    ExactLocation,
    WitnessDetails,
}

impl AlertField {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::IncidentCategory => "INCIDENT_CATEGORY",
            Self::ApproximateAge => "APPROXIMATE_AGE",
            Self::LastSeenGeneralArea => "LAST_SEEN_GENERAL_AREA",
            Self::TimeWindow => "TIME_WINDOW",
            Self::SafeDescription => "SAFE_DESCRIPTION",
            Self::OfficialContact => "OFFICIAL_CONTACT",
            Self::CaseReference => "CASE_REFERENCE",
            Self::ReporterIdentity => "REPORTER_IDENTITY",
            Self::InternalNotes => "INTERNAL_NOTES",
            Self::ExactLocation => "EXACT_LOCATION",
            Self::WitnessDetails => "WITNESS_DETAILS",
        }
    }

    pub fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "INCIDENT_CATEGORY" => Some(Self::IncidentCategory),
            "APPROXIMATE_AGE" => Some(Self::ApproximateAge),
            "LAST_SEEN_GENERAL_AREA" => Some(Self::LastSeenGeneralArea),
            "TIME_WINDOW" => Some(Self::TimeWindow),
            "SAFE_DESCRIPTION" => Some(Self::SafeDescription),
            "OFFICIAL_CONTACT" => Some(Self::OfficialContact),
            "CASE_REFERENCE" => Some(Self::CaseReference),
            "REPORTER_IDENTITY" => Some(Self::ReporterIdentity),
            "INTERNAL_NOTES" => Some(Self::InternalNotes),
            "EXACT_LOCATION" => Some(Self::ExactLocation),
            "WITNESS_DETAILS" => Some(Self::WitnessDetails),
            _ => None,
        }
    }

    /// Never permitted in any alert, at any visibility, full stop. Reporter
    /// identity is case/report domain data; an `Alert` projection never
    /// carries it (AGENTS.md: "Never expose reporter identity by default").
    pub fn is_forbidden(self) -> bool {
        matches!(self, Self::ReporterIdentity)
    }

    /// Permitted only where [`AlertVisibility::admits_internal_only_fields`]
    /// is true.
    pub fn is_restricted_to_internal_audiences(self) -> bool {
        matches!(
            self,
            Self::InternalNotes | Self::ExactLocation | Self::WitnessDetails
        )
    }
}

/// A stable, human-readable policy code, e.g. `"MISSING_CHILD_COMMUNITY"`.
/// Paired with [`AlertPolicy::version`] this is the "explicit policy
/// ID/version" prompt 05 requires.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AlertPolicyId(String);

impl AlertPolicyId {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A coarse, human-authored target area (a municipality or neighborhood name,
/// never raw coordinates). The canonical Cameroon administrative geography
/// model is an open product question (docs/OPEN_QUESTIONS.md); until it is
/// resolved with partner organizations, this stays a plain description
/// rather than a guessed hierarchy or spatial type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetGeography(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyTargetGeography;

impl fmt::Display for EmptyTargetGeography {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "target geography cannot be blank")
    }
}

impl std::error::Error for EmptyTargetGeography {}

impl TargetGeography {
    pub fn new(description: impl Into<String>) -> Result<Self, EmptyTargetGeography> {
        let description = description.into();
        let trimmed = description.trim();
        if trimmed.is_empty() {
            return Err(EmptyTargetGeography);
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertPolicyError {
    EmptyFieldAllowlist,
    FieldNeverAllowed {
        field: AlertField,
    },
    FieldNotAllowedForVisibility {
        field: AlertField,
        visibility: AlertVisibility,
    },
    /// The trigger must be a case event that only occurs at or after
    /// verification; an alert policy can never fire on an unverified case.
    TriggerBeforeVerification {
        trigger: CaseEventType,
    },
}

impl fmt::Display for AlertPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFieldAllowlist => write!(f, "an alert policy must allow at least one field"),
            Self::FieldNeverAllowed { field } => {
                write!(f, "{field:?} may never appear in any alert policy")
            }
            Self::FieldNotAllowedForVisibility { field, visibility } => {
                write!(f, "{field:?} is not allowed for {visibility:?} visibility")
            }
            Self::TriggerBeforeVerification { trigger } => write!(
                f,
                "{trigger:?} cannot trigger an alert policy; a case must be verified first"
            ),
        }
    }
}

impl std::error::Error for AlertPolicyError {}

/// Answers "given a case, what information may be exposed to which audience,
/// and what geography should be targeted?" (docs/ALERT_SAFETY.md, section 7).
/// Deliberately does not decide *who* receives the alert; that is the
/// subscription engine's job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertPolicy {
    id: AlertPolicyId,
    version: u32,
    incident_type: IncidentType,
    visibility: AlertVisibility,
    trigger: CaseEventType,
    field_allowlist: Vec<AlertField>,
}

impl AlertPolicy {
    pub fn new(
        id: AlertPolicyId,
        version: u32,
        incident_type: IncidentType,
        visibility: AlertVisibility,
        trigger: CaseEventType,
        field_allowlist: Vec<AlertField>,
    ) -> Result<Self, AlertPolicyError> {
        if field_allowlist.is_empty() {
            return Err(AlertPolicyError::EmptyFieldAllowlist);
        }
        if let Some(&field) = field_allowlist.iter().find(|field| field.is_forbidden()) {
            return Err(AlertPolicyError::FieldNeverAllowed { field });
        }
        if !visibility.admits_internal_only_fields() {
            if let Some(&field) = field_allowlist
                .iter()
                .find(|field| field.is_restricted_to_internal_audiences())
            {
                return Err(AlertPolicyError::FieldNotAllowedForVisibility { field, visibility });
            }
        }
        if !matches!(
            trigger,
            CaseEventType::CaseVerified
                | CaseEventType::CaseActivated
                | CaseEventType::CaseResolved
        ) {
            return Err(AlertPolicyError::TriggerBeforeVerification { trigger });
        }

        Ok(Self {
            id,
            version,
            incident_type,
            visibility,
            trigger,
            field_allowlist,
        })
    }

    /// The initial missing-child community-alert template (prompt 05).
    pub fn missing_child_community_v1() -> Self {
        Self::new(
            AlertPolicyId::new("MISSING_CHILD_COMMUNITY"),
            1,
            IncidentType::MissingChild,
            AlertVisibility::Community,
            CaseEventType::CaseVerified,
            vec![
                AlertField::IncidentCategory,
                AlertField::ApproximateAge,
                AlertField::LastSeenGeneralArea,
                AlertField::TimeWindow,
                AlertField::SafeDescription,
                AlertField::OfficialContact,
                AlertField::CaseReference,
            ],
        )
        .expect("the built-in missing-child community template is a valid policy")
    }

    pub fn id(&self) -> &AlertPolicyId {
        &self.id
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn incident_type(&self) -> IncidentType {
        self.incident_type
    }

    pub fn visibility(&self) -> AlertVisibility {
        self.visibility
    }

    pub fn trigger(&self) -> CaseEventType {
        self.trigger
    }

    pub fn field_allowlist(&self) -> &[AlertField] {
        &self.field_allowlist
    }

    pub fn allows_field(&self, field: AlertField) -> bool {
        self.field_allowlist.contains(&field)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertFieldValue {
    pub field: AlertField,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlertStatus {
    Active,
    Cancelled,
}

impl AlertStatus {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Cancelled => "CANCELLED",
        }
    }

    pub fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "ACTIVE" => Some(Self::Active),
            "CANCELLED" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlertEventType {
    AlertCreated,
    AlertCancelled,
}

impl AlertEventType {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::AlertCreated => "ALERT_CREATED",
            Self::AlertCancelled => "ALERT_CANCELLED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertEvent {
    pub id: AlertEventId,
    pub alert_id: AlertId,
    pub event_type: AlertEventType,
    pub aggregate_version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertCreationError {
    /// Only a verified (or later) case may produce an alert; this is the
    /// entire point of the `REPORT != CASE != ALERT` boundary
    /// (docs/ALERT_SAFETY.md, section 1).
    CaseNotVerified {
        status: CaseStatus,
    },
    IncidentTypeMismatch {
        case_incident_type: IncidentType,
        policy_incident_type: IncidentType,
    },
    FieldNotAllowedByPolicy {
        field: AlertField,
    },
    DuplicateField {
        field: AlertField,
    },
}

impl fmt::Display for AlertCreationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CaseNotVerified { status } => {
                write!(f, "case must be verified or later, was {status:?}")
            }
            Self::IncidentTypeMismatch {
                case_incident_type,
                policy_incident_type,
            } => write!(
                f,
                "case incident type {case_incident_type:?} does not match policy incident type {policy_incident_type:?}"
            ),
            Self::FieldNotAllowedByPolicy { field } => {
                write!(f, "{field:?} is not on the policy's field allowlist")
            }
            Self::DuplicateField { field } => write!(f, "{field:?} was supplied more than once"),
        }
    }
}

impl std::error::Error for AlertCreationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlertTransitionError {
    pub from: AlertStatus,
    pub to: AlertStatus,
}

impl fmt::Display for AlertTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "alert cannot transition from {:?} to {:?}",
            self.from, self.to
        )
    }
}

impl std::error::Error for AlertTransitionError {}

fn is_verified_or_later(status: CaseStatus) -> bool {
    matches!(
        status,
        CaseStatus::Verified | CaseStatus::Active | CaseStatus::Resolved
    )
}

/// A controlled, policy-scoped projection of a case (docs/DOMAIN_MODEL.md,
/// section 8). Only ever carries the fields its originating policy allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    id: AlertId,
    case_id: CaseId,
    policy_id: AlertPolicyId,
    policy_version: u32,
    incident_type: IncidentType,
    visibility: AlertVisibility,
    target_geography: TargetGeography,
    status: AlertStatus,
    fields: Vec<AlertFieldValue>,
    version: u64,
}

impl Alert {
    /// The only way to construct an `Alert`. Every check here holds
    /// regardless of what the caller supplies, so a policy/case pair that
    /// fails any of them simply cannot produce an alert instance.
    pub fn create_from_case(
        case: &Case,
        policy: &AlertPolicy,
        target_geography: TargetGeography,
        fields: Vec<AlertFieldValue>,
    ) -> Result<(Self, AlertEvent), AlertCreationError> {
        if !is_verified_or_later(case.status()) {
            return Err(AlertCreationError::CaseNotVerified {
                status: case.status(),
            });
        }
        if case.incident_type() != policy.incident_type() {
            return Err(AlertCreationError::IncidentTypeMismatch {
                case_incident_type: case.incident_type(),
                policy_incident_type: policy.incident_type(),
            });
        }

        let mut seen_fields: Vec<AlertField> = Vec::with_capacity(fields.len());
        for field_value in &fields {
            if !policy.allows_field(field_value.field) {
                return Err(AlertCreationError::FieldNotAllowedByPolicy {
                    field: field_value.field,
                });
            }
            if seen_fields.contains(&field_value.field) {
                return Err(AlertCreationError::DuplicateField {
                    field: field_value.field,
                });
            }
            seen_fields.push(field_value.field);
        }

        let alert = Self {
            id: AlertId::new(),
            case_id: case.id(),
            policy_id: policy.id().clone(),
            policy_version: policy.version(),
            incident_type: policy.incident_type(),
            visibility: policy.visibility(),
            target_geography,
            status: AlertStatus::Active,
            fields,
            version: 1,
        };
        let event = alert.event(AlertEventType::AlertCreated);
        Ok((alert, event))
    }

    /// Rebuilds an alert aggregate from persisted state; performs no
    /// validation, mirroring [`Case::reconstitute`].
    #[allow(clippy::too_many_arguments)]
    pub fn reconstitute(
        id: AlertId,
        case_id: CaseId,
        policy_id: AlertPolicyId,
        policy_version: u32,
        incident_type: IncidentType,
        visibility: AlertVisibility,
        target_geography: TargetGeography,
        status: AlertStatus,
        fields: Vec<AlertFieldValue>,
        version: u64,
    ) -> Self {
        Self {
            id,
            case_id,
            policy_id,
            policy_version,
            incident_type,
            visibility,
            target_geography,
            status,
            fields,
            version,
        }
    }

    pub fn cancel(&mut self) -> Result<AlertEvent, AlertTransitionError> {
        if self.status != AlertStatus::Active {
            return Err(AlertTransitionError {
                from: self.status,
                to: AlertStatus::Cancelled,
            });
        }
        self.status = AlertStatus::Cancelled;
        self.version += 1;
        Ok(self.event(AlertEventType::AlertCancelled))
    }

    pub fn id(&self) -> AlertId {
        self.id
    }
    pub fn case_id(&self) -> CaseId {
        self.case_id
    }
    pub fn policy_id(&self) -> &AlertPolicyId {
        &self.policy_id
    }
    pub fn policy_version(&self) -> u32 {
        self.policy_version
    }
    pub fn incident_type(&self) -> IncidentType {
        self.incident_type
    }
    pub fn visibility(&self) -> AlertVisibility {
        self.visibility
    }
    pub fn target_geography(&self) -> &TargetGeography {
        &self.target_geography
    }
    pub fn status(&self) -> AlertStatus {
        self.status
    }
    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn fields(&self) -> &[AlertFieldValue] {
        &self.fields
    }
    pub fn field_value(&self, field: AlertField) -> Option<&str> {
        self.fields
            .iter()
            .find(|value| value.field == field)
            .map(|value| value.value.as_str())
    }

    fn event(&self, event_type: AlertEventType) -> AlertEvent {
        AlertEvent {
            id: AlertEventId::new(),
            alert_id: self.id,
            event_type,
            aggregate_version: self.version,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReportId;

    fn verified_case(incident_type: IncidentType) -> Case {
        let (mut case, _) = Case::create(incident_type, ReportId::new());
        case.transition_to(CaseStatus::UnderReview).unwrap();
        case.transition_to(CaseStatus::Verified).unwrap();
        case
    }

    fn field(field: AlertField, value: &str) -> AlertFieldValue {
        AlertFieldValue {
            field,
            value: value.to_owned(),
        }
    }

    fn safe_fields() -> Vec<AlertFieldValue> {
        vec![
            field(AlertField::IncidentCategory, "MISSING_CHILD"),
            field(AlertField::ApproximateAge, "8 years old"),
            field(AlertField::LastSeenGeneralArea, "Douala - Bonamoussadi"),
        ]
    }

    #[test]
    fn missing_child_community_template_is_valid_and_carries_no_restricted_field() {
        let policy = AlertPolicy::missing_child_community_v1();
        assert_eq!(policy.visibility(), AlertVisibility::Community);
        for allowed in policy.field_allowlist() {
            assert!(!allowed.is_forbidden());
            assert!(!allowed.is_restricted_to_internal_audiences());
        }
    }

    #[test]
    fn reporter_identity_is_rejected_for_every_visibility() {
        for visibility in [
            AlertVisibility::Internal,
            AlertVisibility::Partner,
            AlertVisibility::Community,
            AlertVisibility::Public,
        ] {
            let error = AlertPolicy::new(
                AlertPolicyId::new("TEST"),
                1,
                IncidentType::MissingChild,
                visibility,
                CaseEventType::CaseVerified,
                vec![AlertField::IncidentCategory, AlertField::ReporterIdentity],
            )
            .unwrap_err();
            assert_eq!(
                error,
                AlertPolicyError::FieldNeverAllowed {
                    field: AlertField::ReporterIdentity
                }
            );
        }
    }

    #[test]
    fn internal_only_fields_are_rejected_for_community_and_public_but_allowed_otherwise() {
        for internal_only_field in [
            AlertField::InternalNotes,
            AlertField::ExactLocation,
            AlertField::WitnessDetails,
        ] {
            for visibility in [AlertVisibility::Community, AlertVisibility::Public] {
                let error = AlertPolicy::new(
                    AlertPolicyId::new("TEST"),
                    1,
                    IncidentType::MissingChild,
                    visibility,
                    CaseEventType::CaseVerified,
                    vec![AlertField::IncidentCategory, internal_only_field],
                )
                .unwrap_err();
                assert_eq!(
                    error,
                    AlertPolicyError::FieldNotAllowedForVisibility {
                        field: internal_only_field,
                        visibility,
                    }
                );
            }

            for visibility in [AlertVisibility::Internal, AlertVisibility::Partner] {
                let policy = AlertPolicy::new(
                    AlertPolicyId::new("TEST"),
                    1,
                    IncidentType::MissingChild,
                    visibility,
                    CaseEventType::CaseVerified,
                    vec![AlertField::IncidentCategory, internal_only_field],
                )
                .unwrap();
                assert!(policy.allows_field(internal_only_field));
            }
        }
    }

    #[test]
    fn policy_rejects_an_empty_allowlist_and_a_pre_verification_trigger() {
        assert_eq!(
            AlertPolicy::new(
                AlertPolicyId::new("TEST"),
                1,
                IncidentType::MissingChild,
                AlertVisibility::Internal,
                CaseEventType::CaseVerified,
                vec![],
            )
            .unwrap_err(),
            AlertPolicyError::EmptyFieldAllowlist
        );

        assert_eq!(
            AlertPolicy::new(
                AlertPolicyId::new("TEST"),
                1,
                IncidentType::MissingChild,
                AlertVisibility::Internal,
                CaseEventType::CaseUnderReview,
                vec![AlertField::IncidentCategory],
            )
            .unwrap_err(),
            AlertPolicyError::TriggerBeforeVerification {
                trigger: CaseEventType::CaseUnderReview
            }
        );
    }

    #[test]
    fn decision_table_only_verified_or_later_cases_may_produce_an_alert() {
        let policy = AlertPolicy::missing_child_community_v1();
        let geography = TargetGeography::new("Douala - Bonamoussadi").unwrap();

        for status in [
            CaseStatus::Reported,
            CaseStatus::UnderReview,
            CaseStatus::Verified,
            CaseStatus::Active,
            CaseStatus::Resolved,
            CaseStatus::Cancelled,
            CaseStatus::Rejected,
        ] {
            let case = Case::reconstitute(
                CaseId::new(),
                IncidentType::MissingChild,
                status,
                vec![ReportId::new()],
                1,
            );
            let outcome = Alert::create_from_case(&case, &policy, geography.clone(), safe_fields());
            let expected_allowed = is_verified_or_later(status);
            assert_eq!(
                outcome.is_ok(),
                expected_allowed,
                "status {status:?} expected allowed={expected_allowed}"
            );
            if !expected_allowed {
                assert_eq!(
                    outcome.unwrap_err(),
                    AlertCreationError::CaseNotVerified { status }
                );
            }
        }
    }

    #[test]
    fn a_field_outside_the_policy_allowlist_is_rejected_even_if_it_would_otherwise_be_safe() {
        let case = verified_case(IncidentType::MissingChild);
        let policy = AlertPolicy::missing_child_community_v1();
        let geography = TargetGeography::new("Douala - Bonamoussadi").unwrap();

        // CaseReference is a safe field, but it was not requested here, so
        // supplying an unrelated value under a field the policy never listed
        // must fail closed rather than silently drop it.
        let error = Alert::create_from_case(
            &case,
            &policy,
            geography,
            vec![field(AlertField::ExactLocation, "12 Rue de la Paix")],
        )
        .unwrap_err();
        assert_eq!(
            error,
            AlertCreationError::FieldNotAllowedByPolicy {
                field: AlertField::ExactLocation
            }
        );
    }

    #[test]
    fn a_community_alert_can_never_carry_a_restricted_field_end_to_end() {
        // Even bypassing policy construction is not enough: the allowlist on
        // the built-in community template structurally excludes every
        // forbidden/internal-only field, so no combination of inputs to
        // create_from_case can smuggle one through.
        let case = verified_case(IncidentType::MissingChild);
        let policy = AlertPolicy::missing_child_community_v1();
        let geography = TargetGeography::new("Douala - Bonamoussadi").unwrap();

        for restricted in [
            AlertField::ReporterIdentity,
            AlertField::InternalNotes,
            AlertField::ExactLocation,
            AlertField::WitnessDetails,
        ] {
            let error = Alert::create_from_case(
                &case,
                &policy,
                geography.clone(),
                vec![field(restricted, "should never appear")],
            )
            .unwrap_err();
            assert_eq!(
                error,
                AlertCreationError::FieldNotAllowedByPolicy { field: restricted }
            );
        }

        let (alert, _) = Alert::create_from_case(&case, &policy, geography, safe_fields()).unwrap();
        for restricted in [
            AlertField::ReporterIdentity,
            AlertField::InternalNotes,
            AlertField::ExactLocation,
            AlertField::WitnessDetails,
        ] {
            assert_eq!(alert.field_value(restricted), None);
        }
    }

    #[test]
    fn incident_type_mismatch_is_rejected() {
        let case = verified_case(IncidentType::OtherProtectionIncident);
        let policy = AlertPolicy::missing_child_community_v1();
        let geography = TargetGeography::new("Douala").unwrap();

        let error = Alert::create_from_case(&case, &policy, geography, safe_fields()).unwrap_err();
        assert_eq!(
            error,
            AlertCreationError::IncidentTypeMismatch {
                case_incident_type: IncidentType::OtherProtectionIncident,
                policy_incident_type: IncidentType::MissingChild,
            }
        );
    }

    #[test]
    fn duplicate_field_values_are_rejected() {
        let case = verified_case(IncidentType::MissingChild);
        let policy = AlertPolicy::missing_child_community_v1();
        let geography = TargetGeography::new("Douala").unwrap();

        let error = Alert::create_from_case(
            &case,
            &policy,
            geography,
            vec![
                field(AlertField::ApproximateAge, "8 years old"),
                field(AlertField::ApproximateAge, "9 years old"),
            ],
        )
        .unwrap_err();
        assert_eq!(
            error,
            AlertCreationError::DuplicateField {
                field: AlertField::ApproximateAge
            }
        );
    }

    #[test]
    fn an_active_alert_can_be_cancelled_but_not_twice() {
        let case = verified_case(IncidentType::MissingChild);
        let policy = AlertPolicy::missing_child_community_v1();
        let geography = TargetGeography::new("Douala").unwrap();
        let (mut alert, _) =
            Alert::create_from_case(&case, &policy, geography, safe_fields()).unwrap();

        let event = alert.cancel().unwrap();
        assert_eq!(event.event_type, AlertEventType::AlertCancelled);
        assert_eq!(alert.status(), AlertStatus::Cancelled);

        let error = alert.cancel().unwrap_err();
        assert_eq!(error.from, AlertStatus::Cancelled);
    }

    #[test]
    fn target_geography_rejects_blank_input() {
        assert!(TargetGeography::new("   ").is_err());
        assert_eq!(TargetGeography::new(" Douala ").unwrap().as_str(), "Douala");
    }
}
