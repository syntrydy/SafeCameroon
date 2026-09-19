//! Alert endpoints (docs/API.md, section 4). Alert creation validates policy,
//! visibility, and safe fields before persistence; see
//! `safe_cameroon_application::alert_workflow` for the authorization rules.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use safe_cameroon_application::alert_workflow::{
    AlertCancellationError, AlertCreationUseCaseError, cancel_alert, create_alert_from_case,
    resolve_policy,
};
use safe_cameroon_application::authorization::{
    Capability, authorize, authorize_alert_cancellation, authorize_alert_issuance,
};
use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_domain::{
    AlertCreationError, AlertField, AlertFieldValue, AlertId, AlertStatus, AlertTransitionError,
    AlertVisibility, CaseEventType, CaseId, IncidentType, Membership, OrganizationId, Role,
    Severity, TargetGeography,
};
use safe_cameroon_infrastructure::postgres::{
    AlertCancelOutcome, AlertCreationOutcome, AlertFilter,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::idempotency::idempotency_key_from_headers;
use crate::request_id::request_id_from_headers;
use crate::reviewer::actor_from_headers;
use crate::state::AppState;

fn alert_not_found(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        code: "ALERT_NOT_FOUND",
        message: "No alert exists with the given id.",
        request_id,
    }
}

fn persistence_failed(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "ALERT_PERSISTENCE_FAILED",
        message: "The alert could not be saved. Please try again.",
        request_id,
    }
}

#[derive(Deserialize)]
pub struct AlertFieldInput {
    field: AlertField,
    value: String,
}

#[derive(Deserialize)]
pub struct CreateAlertRequest {
    policy_id: String,
    severity: Severity,
    target_geography: String,
    fields: Vec<AlertFieldInput>,
}

#[derive(Serialize)]
pub struct AlertFieldOutput {
    field: AlertField,
    value: String,
}

#[derive(Serialize)]
pub struct AlertResponse {
    alert_id: Uuid,
    case_id: Uuid,
    policy_id: String,
    policy_version: u32,
    incident_type: IncidentType,
    severity: Severity,
    visibility: AlertVisibility,
    trigger: CaseEventType,
    target_geography: String,
    status: AlertStatus,
    fields: Vec<AlertFieldOutput>,
    version: u64,
}

fn alert_response(alert: &safe_cameroon_domain::Alert) -> AlertResponse {
    AlertResponse {
        alert_id: alert.id().as_uuid(),
        case_id: alert.case_id().as_uuid(),
        policy_id: alert.policy_id().as_str().to_owned(),
        policy_version: alert.policy_version(),
        incident_type: alert.incident_type(),
        severity: alert.severity(),
        visibility: alert.visibility(),
        trigger: alert.trigger(),
        target_geography: alert.target_geography().as_str().to_owned(),
        status: alert.status(),
        fields: alert
            .fields()
            .iter()
            .map(|value| AlertFieldOutput {
                field: value.field,
                value: value.value.clone(),
            })
            .collect(),
        version: alert.version(),
    }
}

/// Checks organization trust (only meaningful for a visibility that already
/// requires an identified reviewer -- community/public; internal/partner
/// alerts are unaffected, docs/OPEN_QUESTIONS.md: "which organizations may
/// issue community/public alerts" — resolved per-organization by
/// crates/domain/src/organization.rs rather than a policy baked in here) and
/// returns the creating reviewer's own organization regardless of
/// visibility, so it can be stamped onto the alert as
/// `issued_by_organization_id` — later used to scope who may cancel it
/// (`authorize_alert_cancellation`).
async fn issuing_organization(
    state: &AppState,
    actor: Actor,
    visibility: AlertVisibility,
    request_id: Uuid,
) -> Result<Option<OrganizationId>, ApiError> {
    let Actor::Reviewer(reviewer_id) = actor else {
        return Ok(None);
    };
    let membership = state
        .organizations
        .find_membership(reviewer_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
        .unwrap_or(Membership {
            role: Role::Member,
            organization_id: None,
        });

    if !visibility.admits_internal_only_fields() {
        let organization = match membership.organization_id {
            Some(organization_id) => Some(
                state
                    .organizations
                    .find_by_id(organization_id)
                    .await
                    .map_err(|_| persistence_failed(request_id))?
                    .expect("a reviewer's membership never references a deleted organization"),
            ),
            None => None,
        };
        authorize_alert_issuance(membership, organization.as_ref(), visibility).map_err(|_| {
            ApiError {
                status: StatusCode::FORBIDDEN,
                code: "ORGANIZATION_NOT_TRUSTED_FOR_ALERT_VISIBILITY",
                message: "Your organization is not trusted to issue alerts at this visibility.",
                request_id,
            }
        })?;
    }

    Ok(membership.organization_id)
}

/// Whether `actor` may cancel an alert issued by `issued_by_organization_id`
/// (`authorize_alert_cancellation`, crates/application/src/authorization.rs).
/// `cancel_alert`'s own `authorize_alert_cancellation` (a differently-scoped,
/// same-named check in `alert_workflow`) already rejects `Actor::Automated`
/// before this org-scoping matters.
async fn authorize_cancellation_by_organization(
    state: &AppState,
    actor: Actor,
    issued_by_organization_id: Option<OrganizationId>,
    request_id: Uuid,
) -> Result<(), ApiError> {
    let Actor::Reviewer(reviewer_id) = actor else {
        return Ok(());
    };
    let membership = state
        .organizations
        .find_membership(reviewer_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
        .unwrap_or(Membership {
            role: Role::Member,
            organization_id: None,
        });
    authorize_alert_cancellation(membership, issued_by_organization_id).map_err(|_| ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "You may not cancel an alert issued by another organization.",
        request_id,
    })
}

pub async fn create_alert(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<CreateAlertRequest>,
) -> Result<(StatusCode, Json<AlertResponse>), ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    let idempotency_key = idempotency_key_from_headers(&headers, request_id)?;

    let policy = resolve_policy(&request.policy_id).ok_or(ApiError {
        status: StatusCode::BAD_REQUEST,
        code: "UNKNOWN_ALERT_POLICY",
        message: "No alert policy is registered with this id.",
        request_id,
    })?;
    let target_geography =
        TargetGeography::new(request.target_geography).map_err(|_| ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "INVALID_TARGET_GEOGRAPHY",
            message: "target_geography cannot be blank.",
            request_id,
        })?;
    let fields = request
        .fields
        .into_iter()
        .map(|input| AlertFieldValue {
            field: input.field,
            value: input.value,
        })
        .collect();

    let case = state
        .cases
        .find_by_id(CaseId::from_uuid(case_id))
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or(ApiError {
            status: StatusCode::NOT_FOUND,
            code: "CASE_NOT_FOUND",
            message: "No case exists with the given id.",
            request_id,
        })?;

    let issued_by_organization_id =
        issuing_organization(&state, actor, policy.visibility(), request_id).await?;

    let creation = create_alert_from_case(
        &case,
        &policy,
        request.severity,
        target_geography,
        fields,
        actor,
        request_id,
        idempotency_key,
        issued_by_organization_id,
    )
    .map_err(|error| match error {
        AlertCreationUseCaseError::NotAuthorized(_) => ApiError {
            status: StatusCode::FORBIDDEN,
            code: "NOT_AUTHORIZED",
            message: "Only an identified reviewer may create an alert with this visibility.",
            request_id,
        },
        AlertCreationUseCaseError::InvalidAlert(AlertCreationError::CaseNotVerified { .. }) => {
            ApiError {
                status: StatusCode::CONFLICT,
                code: "CASE_NOT_VERIFIED",
                message: "The requested alert action is not allowed for this case state.",
                request_id,
            }
        }
        AlertCreationUseCaseError::InvalidAlert(AlertCreationError::IncidentTypeMismatch {
            ..
        }) => ApiError {
            status: StatusCode::CONFLICT,
            code: "INCIDENT_TYPE_MISMATCH",
            message: "The case's incident type does not match the policy's incident type.",
            request_id,
        },
        AlertCreationUseCaseError::InvalidAlert(AlertCreationError::FieldNotAllowedByPolicy {
            ..
        }) => ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "FIELD_NOT_ALLOWED_BY_POLICY",
            message: "A supplied field is not on this policy's allowlist.",
            request_id,
        },
        AlertCreationUseCaseError::InvalidAlert(AlertCreationError::DuplicateField { .. }) => {
            ApiError {
                status: StatusCode::BAD_REQUEST,
                code: "DUPLICATE_ALERT_FIELD",
                message: "A field was supplied more than once.",
                request_id,
            }
        }
    })?;

    match state
        .alerts
        .create(&creation)
        .await
        .map_err(|_| persistence_failed(request_id))?
    {
        AlertCreationOutcome::Created => {
            Ok((StatusCode::CREATED, Json(alert_response(&creation.alert))))
        }
        AlertCreationOutcome::Duplicate { .. } => Err(ApiError {
            status: StatusCode::CONFLICT,
            code: "IDEMPOTENCY_KEY_REUSED",
            message: "This Idempotency-Key was already used for an alert.",
            request_id,
        }),
    }
}

pub async fn get_alert(
    State(state): State<AppState>,
    Path(alert_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AlertResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ViewCase).map_err(|_| ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "Only an identified reviewer may view an alert.",
        request_id,
    })?;

    let alert = state
        .alerts
        .find_by_id(AlertId::from_uuid(alert_id))
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or_else(|| alert_not_found(request_id))?;

    Ok(Json(alert_response(&alert)))
}

const DEFAULT_LIMIT: u32 = 50;
/// A hard ceiling regardless of what the caller asks for, so a single
/// request can never force an unbounded scan/response.
const MAX_LIMIT: u32 = 100;

#[derive(Deserialize)]
pub struct ListAlertsQuery {
    status: Option<AlertStatus>,
    visibility: Option<AlertVisibility>,
    limit: Option<u32>,
    offset: Option<u32>,
}

/// Unlike `get_alert` and `GET /v1/cases/{id}` (which have no authorization
/// gate at all — a pre-existing, unrelated gap), this listing endpoint
/// requires `Capability::ViewCase` from the start: bulk enumeration is a
/// materially larger exposure than single-record lookup by a random id, and
/// `alert_fields` can carry non-public detail depending on the alert's
/// policy/visibility (docs/ALERT_SAFETY.md).
pub async fn list_alerts(
    State(state): State<AppState>,
    Query(query): Query<ListAlertsQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<AlertResponse>>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ViewCase).map_err(|_| ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "Only an identified reviewer may list alerts.",
        request_id,
    })?;

    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;
    let filter = AlertFilter {
        status: query.status,
        visibility: query.visibility,
    };

    let alerts = state
        .alerts
        .list(&filter, limit, offset)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok(Json(alerts.iter().map(alert_response).collect()))
}

pub async fn cancel(
    State(state): State<AppState>,
    Path(alert_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AlertResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;

    let mut alert = state
        .alerts
        .find_by_id(AlertId::from_uuid(alert_id))
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or_else(|| alert_not_found(request_id))?;

    authorize_cancellation_by_organization(
        &state,
        actor,
        alert.issued_by_organization_id(),
        request_id,
    )
    .await?;

    let cancellation =
        cancel_alert(&mut alert, actor, request_id).map_err(|error| match error {
            AlertCancellationError::NotAuthorized(_) => ApiError {
                status: StatusCode::FORBIDDEN,
                code: "NOT_AUTHORIZED",
                message: "Only an identified reviewer may cancel an alert.",
                request_id,
            },
            AlertCancellationError::InvalidTransition(AlertTransitionError { .. }) => ApiError {
                status: StatusCode::CONFLICT,
                code: "ALERT_ALREADY_CANCELLED",
                message: "This alert is already cancelled.",
                request_id,
            },
        })?;

    match state.alerts.cancel(&alert, &cancellation).await {
        Ok(AlertCancelOutcome::Cancelled) => Ok(Json(alert_response(&alert))),
        Ok(AlertCancelOutcome::Conflict) => Err(ApiError {
            status: StatusCode::CONFLICT,
            code: "ALERT_MODIFIED_CONCURRENTLY",
            message: "The alert changed since it was last read. Reload and retry.",
            request_id,
        }),
        Err(_) => Err(persistence_failed(request_id)),
    }
}
