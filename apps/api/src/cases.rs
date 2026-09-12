//! Case endpoints (docs/API.md, section 3). All state-changing operations here
//! are "authenticated authorized users only" per that spec; until the auth
//! foundation (prompt 09) lands, the caller's reviewer identity is taken from
//! an `X-Reviewer-Id` header as an explicit, temporary authorization hook.
//! Its absence means the request is treated as an automated/system actor,
//! which the application layer allows to start review but never to verify,
//! reject, activate, resolve, or cancel a case.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use safe_cameroon_application::case_workflow::{
    Actor, CaseReviewError, create_case_from_report, link_report_to_case, review_case,
};
use safe_cameroon_domain::{CaseId, CaseStatus, DuplicateReportLink, IncidentType, ReportId};
use safe_cameroon_infrastructure::postgres::{
    CaseCreationOutcome, CaseLinkOutcome, CaseReviewOutcome,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::reviewer::actor_from_headers;
use crate::state::AppState;

fn case_not_found(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        code: "CASE_NOT_FOUND",
        message: "No case exists with the given id.",
        request_id,
    }
}

fn persistence_failed(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "CASE_PERSISTENCE_FAILED",
        message: "The case could not be saved. Please try again.",
        request_id,
    }
}

#[derive(Serialize)]
pub struct CaseResponse {
    case_id: Uuid,
    incident_type: IncidentType,
    status: CaseStatus,
    report_ids: Vec<Uuid>,
    version: u64,
}

#[derive(Deserialize)]
pub struct CreateCaseRequest {
    report_id: Uuid,
    incident_type: IncidentType,
}

pub async fn create_case(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateCaseRequest>,
) -> Result<(StatusCode, Json<CaseResponse>), ApiError> {
    let request_id = Uuid::new_v4();
    let actor = actor_from_headers(&headers, request_id)?;

    let creation = create_case_from_report(
        request.incident_type,
        ReportId::from_uuid(request.report_id),
        actor,
        request_id,
    );

    match state.cases.create(&creation).await {
        Ok(CaseCreationOutcome::Created) => Ok((
            StatusCode::CREATED,
            Json(CaseResponse {
                case_id: creation.case.id().as_uuid(),
                incident_type: creation.case.incident_type(),
                status: creation.case.status(),
                report_ids: creation
                    .case
                    .report_ids()
                    .iter()
                    .map(|id| id.as_uuid())
                    .collect(),
                version: creation.case.version(),
            }),
        )),
        Ok(CaseCreationOutcome::ReportAlreadyLinked) => Err(ApiError {
            status: StatusCode::CONFLICT,
            code: "REPORT_ALREADY_LINKED",
            message: "This report is already linked to a case.",
            request_id,
        }),
        Err(_) => Err(persistence_failed(request_id)),
    }
}

pub async fn get_case(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
) -> Result<Json<CaseResponse>, ApiError> {
    let request_id = Uuid::new_v4();
    let case = state
        .cases
        .find_by_id(CaseId::from_uuid(case_id))
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or_else(|| case_not_found(request_id))?;

    Ok(Json(CaseResponse {
        case_id: case.id().as_uuid(),
        incident_type: case.incident_type(),
        status: case.status(),
        report_ids: case.report_ids().iter().map(|id| id.as_uuid()).collect(),
        version: case.version(),
    }))
}

#[derive(Deserialize)]
pub struct LinkReportRequest {
    report_id: Uuid,
}

pub async fn link_report(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<LinkReportRequest>,
) -> Result<Json<CaseResponse>, ApiError> {
    let request_id = Uuid::new_v4();
    let actor = actor_from_headers(&headers, request_id)?;
    let case_id = CaseId::from_uuid(case_id);

    let mut case = state
        .cases
        .find_by_id(case_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or_else(|| case_not_found(request_id))?;

    let link = link_report_to_case(
        &mut case,
        ReportId::from_uuid(request.report_id),
        actor,
        request_id,
    )
    .map_err(|DuplicateReportLink { .. }| ApiError {
        status: StatusCode::CONFLICT,
        code: "REPORT_ALREADY_LINKED",
        message: "This report is already linked to this case.",
        request_id,
    })?;

    match state.cases.link_report(&case, &link).await {
        Ok(CaseLinkOutcome::Linked) => Ok(Json(CaseResponse {
            case_id: case.id().as_uuid(),
            incident_type: case.incident_type(),
            status: case.status(),
            report_ids: case.report_ids().iter().map(|id| id.as_uuid()).collect(),
            version: case.version(),
        })),
        Ok(CaseLinkOutcome::ReportAlreadyLinked) => Err(ApiError {
            status: StatusCode::CONFLICT,
            code: "REPORT_ALREADY_LINKED",
            message: "This report is already linked to a case.",
            request_id,
        }),
        Ok(CaseLinkOutcome::Conflict) => Err(ApiError {
            status: StatusCode::CONFLICT,
            code: "CASE_MODIFIED_CONCURRENTLY",
            message: "The case changed since it was last read. Reload and retry.",
            request_id,
        }),
        Err(_) => Err(persistence_failed(request_id)),
    }
}

async fn transition_case(
    state: AppState,
    case_id: Uuid,
    actor: Actor,
    to: CaseStatus,
    request_id: Uuid,
) -> Result<Json<CaseResponse>, ApiError> {
    let case_id = CaseId::from_uuid(case_id);
    let mut case = state
        .cases
        .find_by_id(case_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or_else(|| case_not_found(request_id))?;

    let review = review_case(&mut case, actor, to, request_id).map_err(|error| match error {
        CaseReviewError::NotAuthorized(_) => ApiError {
            status: StatusCode::FORBIDDEN,
            code: "NOT_AUTHORIZED",
            message: "Only an identified reviewer may make this change.",
            request_id,
        },
        CaseReviewError::InvalidTransition(_) => ApiError {
            status: StatusCode::CONFLICT,
            code: "CASE_TRANSITION_NOT_ALLOWED",
            message: "The case cannot move to the requested status from its current state.",
            request_id,
        },
    })?;

    match state.cases.apply_review(&case, &review).await {
        Ok(CaseReviewOutcome::Applied) => Ok(Json(CaseResponse {
            case_id: case.id().as_uuid(),
            incident_type: case.incident_type(),
            status: case.status(),
            report_ids: case.report_ids().iter().map(|id| id.as_uuid()).collect(),
            version: case.version(),
        })),
        Ok(CaseReviewOutcome::Conflict) => Err(ApiError {
            status: StatusCode::CONFLICT,
            code: "CASE_MODIFIED_CONCURRENTLY",
            message: "The case changed since it was last read. Reload and retry.",
            request_id,
        }),
        Err(_) => Err(persistence_failed(request_id)),
    }
}

#[derive(Deserialize)]
pub struct CaseEventRequest {
    to: CaseStatus,
}

/// Generic transition endpoint for statuses without a dedicated shorthand
/// (`UNDER_REVIEW`, `ACTIVE`, `CANCELLED`, `REJECTED`).
pub async fn create_case_event(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<CaseEventRequest>,
) -> Result<Json<CaseResponse>, ApiError> {
    let request_id = Uuid::new_v4();
    let actor = actor_from_headers(&headers, request_id)?;
    transition_case(state, case_id, actor, request.to, request_id).await
}

pub async fn verify_case(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<CaseResponse>, ApiError> {
    let request_id = Uuid::new_v4();
    let actor = actor_from_headers(&headers, request_id)?;
    transition_case(state, case_id, actor, CaseStatus::Verified, request_id).await
}

pub async fn resolve_case(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<CaseResponse>, ApiError> {
    let request_id = Uuid::new_v4();
    let actor = actor_from_headers(&headers, request_id)?;
    transition_case(state, case_id, actor, CaseStatus::Resolved, request_id).await
}
