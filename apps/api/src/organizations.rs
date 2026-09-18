//! Organization management (crates/domain/src/organization.rs). Creating an
//! organization and setting its trust grants (which incident types it may
//! verify, which alert visibilities it may issue) is `PlatformAdmin`-only —
//! this is the platform's own institutional trust decision, not something
//! an organization grants itself.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use safe_cameroon_domain::{
    AlertVisibility, Consumer, ConsumerType, EmptyOrganizationName, IncidentType, Membership,
    Organization, OrganizationId, Role,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::request_id::request_id_from_headers;
use crate::reviewer::actor_from_headers;
use crate::state::AppState;
use safe_cameroon_application::case_workflow::Actor;

fn persistence_failed(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "ORGANIZATION_PERSISTENCE_FAILED",
        message: "The request could not be saved. Please try again.",
        request_id,
    }
}

fn not_authorized(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "Only a platform admin may manage organizations.",
        request_id,
    }
}

fn not_authorized_for_organization(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "You are not authorized to view this organization.",
        request_id,
    }
}

fn organization_not_found(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        code: "ORGANIZATION_NOT_FOUND",
        message: "No organization exists with the given id.",
        request_id,
    }
}

/// The only role allowed to create/configure organizations at all — this is
/// deliberately not covered by the flat `authorization::authorize`, since it
/// is the specific institutional-trust decision this module exists for.
async fn require_platform_admin(
    state: &AppState,
    actor: Actor,
    request_id: Uuid,
) -> Result<(), ApiError> {
    let Actor::Reviewer(reviewer_id) = actor else {
        return Err(not_authorized(request_id));
    };
    let membership = state
        .organizations
        .find_membership(reviewer_id)
        .await
        .map_err(|_| persistence_failed(request_id))?;
    match membership {
        Some(Membership {
            role: Role::PlatformAdmin,
            ..
        }) => Ok(()),
        _ => Err(not_authorized(request_id)),
    }
}

/// A platform admin may view any organization; an org admin may view only
/// their own — this is the "manage my organization" route (member list,
/// read-only org detail), distinct from the platform-wide organization
/// management above, which stays platform-admin-only.
async fn require_platform_admin_or_org_admin_of(
    state: &AppState,
    actor: Actor,
    organization_id: OrganizationId,
    request_id: Uuid,
) -> Result<(), ApiError> {
    let Actor::Reviewer(reviewer_id) = actor else {
        return Err(not_authorized_for_organization(request_id));
    };
    let membership = state
        .organizations
        .find_membership(reviewer_id)
        .await
        .map_err(|_| persistence_failed(request_id))?;
    match membership {
        Some(Membership {
            role: Role::PlatformAdmin,
            ..
        }) => Ok(()),
        Some(Membership {
            role: Role::OrgAdmin,
            organization_id: Some(own_organization_id),
        }) if own_organization_id == organization_id => Ok(()),
        _ => Err(not_authorized_for_organization(request_id)),
    }
}

#[derive(Serialize)]
pub struct OrganizationResponse {
    organization_id: Uuid,
    name: String,
    verified_incident_types: Vec<IncidentType>,
    verified_alert_visibilities: Vec<AlertVisibility>,
    /// The consumer this organization's alert subscription and delivery
    /// preference (WhatsApp/email endpoints) live under — see
    /// `Organization::consumer_id`'s doc comment. The console calls the
    /// existing `/v1/consumers/{id}`, `/v1/subscriptions`, and
    /// `/v1/consumers/{id}/delivery-preference` endpoints with this id
    /// directly.
    consumer_id: Option<Uuid>,
}

fn organization_response(organization: &Organization) -> OrganizationResponse {
    OrganizationResponse {
        organization_id: organization.id().as_uuid(),
        name: organization.name().to_owned(),
        verified_incident_types: organization.verified_incident_types().to_vec(),
        verified_alert_visibilities: organization.verified_alert_visibilities().to_vec(),
        consumer_id: organization.consumer_id().map(|id| id.as_uuid()),
    }
}

#[derive(Deserialize)]
pub struct CreateOrganizationRequest {
    name: String,
}

pub async fn create_organization(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateOrganizationRequest>,
) -> Result<(StatusCode, Json<OrganizationResponse>), ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    require_platform_admin(&state, actor, request_id).await?;

    let organization =
        Organization::new(request.name).map_err(|EmptyOrganizationName| ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "INVALID_ORGANIZATION_NAME",
            message: "name cannot be blank.",
            request_id,
        })?;

    state
        .organizations
        .create(&organization)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    // Every organization gets a linked consumer up front, so its alert
    // subscription and delivery preference (WhatsApp/email endpoints) are
    // always manageable through the existing consumer pipeline the moment
    // it's onboarded — see `Organization::consumer_id`'s doc comment. The
    // name is already validated non-blank by `Organization::new` above, so
    // this can't fail on `EmptyConsumerName`.
    let consumer = Consumer::new(organization.name(), ConsumerType::Organization)
        .expect("organization name is already validated non-blank above");
    state
        .consumers
        .create(&consumer)
        .await
        .map_err(|_| persistence_failed(request_id))?;
    state
        .organizations
        .set_consumer_id(organization.id(), consumer.id())
        .await
        .map_err(|_| persistence_failed(request_id))?;

    let organization = state
        .organizations
        .find_by_id(organization.id())
        .await
        .map_err(|_| persistence_failed(request_id))?
        .expect("the organization was just created above");

    Ok((
        StatusCode::CREATED,
        Json(organization_response(&organization)),
    ))
}

pub async fn get_organization(
    State(state): State<AppState>,
    Path(organization_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<OrganizationResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    let organization_id = OrganizationId::from_uuid(organization_id);
    require_platform_admin_or_org_admin_of(&state, actor, organization_id, request_id).await?;

    let organization = state
        .organizations
        .find_by_id(organization_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or_else(|| organization_not_found(request_id))?;

    Ok(Json(organization_response(&organization)))
}

const DEFAULT_LIMIT: u32 = 50;
/// A hard ceiling regardless of what the caller asks for, so a single
/// request can never force an unbounded scan/response.
const MAX_LIMIT: u32 = 100;

#[derive(Deserialize)]
pub struct ListOrganizationsQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

pub async fn list_organizations(
    State(state): State<AppState>,
    Query(query): Query<ListOrganizationsQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<OrganizationResponse>>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    require_platform_admin(&state, actor, request_id).await?;

    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let organizations = state
        .organizations
        .list(limit, offset)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok(Json(
        organizations.iter().map(organization_response).collect(),
    ))
}

#[derive(Deserialize)]
pub struct SetTrustGrantsRequest {
    verified_incident_types: Vec<IncidentType>,
    verified_alert_visibilities: Vec<AlertVisibility>,
}

/// Replaces both trust grants wholesale (docs/OPEN_QUESTIONS.md's
/// verification-authority and alert-issuance questions, answered per real
/// organization rather than by a rule this codebase bakes in).
pub async fn set_trust_grants(
    State(state): State<AppState>,
    Path(organization_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<SetTrustGrantsRequest>,
) -> Result<Json<OrganizationResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    require_platform_admin(&state, actor, request_id).await?;

    let organization_id = OrganizationId::from_uuid(organization_id);
    state
        .organizations
        .find_by_id(organization_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or_else(|| organization_not_found(request_id))?;

    state
        .organizations
        .set_trust_grants(
            organization_id,
            &request.verified_incident_types,
            &request.verified_alert_visibilities,
        )
        .await
        .map_err(|_| persistence_failed(request_id))?;

    let updated = state
        .organizations
        .find_by_id(organization_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
        .expect("the organization was just confirmed to exist above");

    Ok(Json(organization_response(&updated)))
}

#[derive(Serialize)]
pub struct MemberResponse {
    reviewer_id: Uuid,
    email: String,
    role: Role,
}

pub async fn list_members(
    State(state): State<AppState>,
    Path(organization_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<MemberResponse>>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    let organization_id = OrganizationId::from_uuid(organization_id);
    require_platform_admin_or_org_admin_of(&state, actor, organization_id, request_id).await?;

    state
        .organizations
        .find_by_id(organization_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or_else(|| organization_not_found(request_id))?;

    let members = state
        .organizations
        .list_members(organization_id)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok(Json(
        members
            .into_iter()
            .map(|member| MemberResponse {
                reviewer_id: member.reviewer_id,
                email: member.email,
                role: member.role,
            })
            .collect(),
    ))
}
