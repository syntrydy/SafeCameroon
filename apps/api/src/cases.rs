//! Case endpoints (docs/API.md, section 3). All state-changing operations here
//! are "authenticated authorized users only" per that spec; the caller's
//! reviewer identity comes from a verified session token (see `reviewer.rs`).
//! No `Authorization` header at all means the request is treated as an
//! automated/system actor, which the application layer allows to start
//! review but never to verify, reject, activate, resolve, or cancel a case.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use safe_cameroon_application::authorization::{Capability, authorize};
use safe_cameroon_application::case_workflow::{
    Actor, CaseReviewError, create_case_from_report, link_report_to_case, review_case,
};
use safe_cameroon_domain::{
    Case, CaseEventType, CaseId, CaseStatus, DuplicateReportLink, IncidentType, ReportId,
};
use safe_cameroon_infrastructure::postgres::{
    CaseCreationOutcome, CaseEventRecord, CaseFilter, CaseLinkOutcome, CaseReviewOutcome,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::request_id::request_id_from_headers;
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

fn not_authorized(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "Only an identified reviewer may view case detail.",
        request_id,
    }
}

const DEFAULT_LIMIT: u32 = 50;
/// A hard ceiling regardless of what the caller asks for, so a single
/// request can never force an unbounded scan/response.
const MAX_LIMIT: u32 = 100;

#[derive(Serialize)]
pub struct CaseResponse {
    case_id: Uuid,
    incident_type: IncidentType,
    status: CaseStatus,
    report_ids: Vec<Uuid>,
    version: u64,
}

fn case_response(case: &Case) -> CaseResponse {
    CaseResponse {
        case_id: case.id().as_uuid(),
        incident_type: case.incident_type(),
        status: case.status(),
        report_ids: case.report_ids().iter().map(|id| id.as_uuid()).collect(),
        version: case.version(),
    }
}

#[derive(Deserialize)]
pub struct ListCasesQuery {
    status: Option<CaseStatus>,
    incident_type: Option<IncidentType>,
    limit: Option<u32>,
    offset: Option<u32>,
}

/// Most recently updated first (`cases_status_updated_at_idx`), optionally
/// narrowed by `status` and/or `incident_type` — the only way to find a case
/// without already knowing its id.
pub async fn list_cases(
    State(state): State<AppState>,
    Query(query): Query<ListCasesQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<CaseResponse>>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ViewCase).map_err(|_| not_authorized(request_id))?;

    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;
    let filter = CaseFilter {
        status: query.status,
        incident_type: query.incident_type,
    };

    let cases = state
        .cases
        .list(&filter, limit, offset)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok(Json(cases.iter().map(case_response).collect()))
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
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;

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
    headers: HeaderMap,
) -> Result<Json<CaseResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ViewCase).map_err(|_| not_authorized(request_id))?;

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
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
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
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    transition_case(state, case_id, actor, request.to, request_id).await
}

pub async fn verify_case(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<CaseResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    transition_case(state, case_id, actor, CaseStatus::Verified, request_id).await
}

pub async fn resolve_case(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<CaseResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    transition_case(state, case_id, actor, CaseStatus::Resolved, request_id).await
}

#[derive(Serialize)]
pub struct CaseEventHistoryResponse {
    id: Uuid,
    case_id: Uuid,
    event_type: CaseEventType,
    aggregate_version: u64,
    actor_type: String,
    actor_id: Option<Uuid>,
    occurred_at: String,
}

fn case_event_history_response(event: &CaseEventRecord) -> CaseEventHistoryResponse {
    CaseEventHistoryResponse {
        id: event.id,
        case_id: event.case_id,
        event_type: event.event_type,
        aggregate_version: event.aggregate_version,
        actor_type: event.actor_type.clone(),
        actor_id: event.actor_id,
        occurred_at: event.occurred_at.clone(),
    }
}

/// The case's full lifecycle history (docs/OBSERVABILITY.md), not just its
/// current status — same "may this reviewer see case-adjacent detail"
/// question as delivery visibility, so it reuses `Capability::ViewCase`
/// rather than introducing a new one.
pub async fn list_case_events(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<CaseEventHistoryResponse>>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ViewCase).map_err(|_| not_authorized(request_id))?;

    let events = state
        .cases
        .list_events(CaseId::from_uuid(case_id))
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok(Json(
        events.iter().map(case_event_history_response).collect(),
    ))
}
