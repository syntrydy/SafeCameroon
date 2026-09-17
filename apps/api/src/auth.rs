//! Reviewer account registration and Google sign-in (prompt 09). Reviewers
//! authenticate via a verified Google ID token, never a locally stored
//! password (crates/application/src/google_identity.rs); registration
//! allowlists an email and grants it a role/organization membership
//! (crates/domain/src/organization.rs) — login exchanges a Google-verified
//! email matching that allowlist for a signed session token (`reviewer.rs`
//! verifies it back on every subsequent request).

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use safe_cameroon_application::authorization::{
    authorize_membership_grant, validate_membership_shape,
};
use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_application::rate_limit::RateLimitScope;
use safe_cameroon_application::reviewer_auth::SessionRevocationStore;
use safe_cameroon_domain::{OrganizationId, Role};
use safe_cameroon_infrastructure::postgres::CreateReviewerOutcome;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::rate_limit::enforce_rate_limit;
use crate::request_id::request_id_from_headers;
use crate::reviewer::actor_from_headers;
use crate::source_key::source_key_from_headers;
use crate::state::AppState;

fn persistence_failed(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "REVIEWER_PERSISTENCE_FAILED",
        message: "The request could not be saved. Please try again.",
        request_id,
    }
}

fn not_authorized(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "You are not authorized to register a reviewer with this role/organization.",
        request_id,
    }
}

/// `role`/`organization_id` are ignored for the deployment's bootstrapping
/// first reviewer, who always becomes `PlatformAdmin` with no organization
/// — otherwise both are required, and must satisfy
/// `validate_membership_shape` (`PlatformAdmin` has no organization;
/// `OrgAdmin`/`Member` each require one).
#[derive(Deserialize)]
pub struct RegisterRequest {
    email: String,
    role: Option<Role>,
    organization_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    reviewer_id: Uuid,
    email: String,
    role: Role,
    organization_id: Option<Uuid>,
}

pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;

    let existing_reviewer_count = state
        .reviewers
        .count()
        .await
        .map_err(|_| persistence_failed(request_id))?;

    let (role, organization_id) = if existing_reviewer_count == 0 {
        (Role::PlatformAdmin, None)
    } else {
        let role = request.role.ok_or(ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "MISSING_ROLE",
            message: "role is required.",
            request_id,
        })?;
        let organization_id = request.organization_id.map(OrganizationId::from_uuid);
        validate_membership_shape(role, organization_id).map_err(|_| ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "INVALID_MEMBERSHIP_SHAPE",
            message: "PlatformAdmin must have no organization_id; OrgAdmin and Member must each have one.",
            request_id,
        })?;
        (role, organization_id)
    };

    let granter = match actor {
        Actor::Reviewer(reviewer_id) => state
            .organizations
            .find_membership(reviewer_id)
            .await
            .map_err(|_| persistence_failed(request_id))?,
        Actor::Automated => None,
    };
    authorize_membership_grant(granter, existing_reviewer_count, role, organization_id)
        .map_err(|_| not_authorized(request_id))?;

    if let Some(organization_id) = organization_id {
        state
            .organizations
            .find_by_id(organization_id)
            .await
            .map_err(|_| persistence_failed(request_id))?
            .ok_or(ApiError {
                status: StatusCode::BAD_REQUEST,
                code: "ORGANIZATION_NOT_FOUND",
                message: "No organization exists with the given id.",
                request_id,
            })?;
    }

    let email = request.email.trim();
    if email.is_empty() || !email.contains('@') {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "INVALID_EMAIL",
            message: "email must be a non-blank address.",
            request_id,
        });
    }

    let reviewer_id = Uuid::new_v4();
    match state
        .reviewers
        .create(reviewer_id, email)
        .await
        .map_err(|_| persistence_failed(request_id))?
    {
        CreateReviewerOutcome::Created => {
            state
                .organizations
                .add_membership(reviewer_id, role, organization_id)
                .await
                .map_err(|_| persistence_failed(request_id))?;
            state
                .reviewers
                .record_registration(reviewer_id, actor, request_id)
                .await
                .map_err(|_| persistence_failed(request_id))?;
            Ok((
                StatusCode::CREATED,
                Json(RegisterResponse {
                    reviewer_id,
                    email: email.to_owned(),
                    role,
                    organization_id: organization_id.map(OrganizationId::as_uuid),
                }),
            ))
        }
        CreateReviewerOutcome::EmailAlreadyRegistered => Err(ApiError {
            status: StatusCode::CONFLICT,
            code: "EMAIL_ALREADY_REGISTERED",
            message: "An account with this email already exists.",
            request_id,
        }),
    }
}

#[derive(Deserialize)]
pub struct GoogleLoginRequest {
    id_token: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    token: String,
    expires_in_seconds: u64,
    reviewer_id: Uuid,
    email: String,
}

pub async fn google_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<GoogleLoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    // Keyed by source IP, not the (not yet known) target email: this must
    // run before spending a call against Google's tokeninfo endpoint, to
    // keep an attacker spamming garbage tokens from burning that quota.
    enforce_rate_limit(
        state.rate_limiter.as_ref(),
        RateLimitScope::ReviewerLoginAttempt,
        source_key_from_headers(&headers),
        request_id,
    )
    .await?;

    let identity = state
        .google_identity_verifier
        .verify_id_token(&request.id_token)
        .await
        .map_err(|_| ApiError {
            status: StatusCode::UNAUTHORIZED,
            code: "INVALID_GOOGLE_TOKEN",
            message: "Google could not verify this sign-in.",
            request_id,
        })?;
    let normalized_email = identity.email.trim().to_lowercase();

    let reviewer_id = state
        .reviewers
        .find_by_email(&normalized_email)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    let Some(reviewer_id) = reviewer_id else {
        state
            .reviewers
            .record_login_failure(None, &normalized_email, request_id)
            .await
            .map_err(|_| persistence_failed(request_id))?;
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            code: "REVIEWER_NOT_REGISTERED",
            message: "This Google account is not registered as a reviewer.",
            request_id,
        });
    };

    state
        .reviewers
        .record_login_success(reviewer_id, request_id)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    let issued = state.reviewer_session_tokens.issue(reviewer_id);
    Ok(Json(LoginResponse {
        token: issued.token,
        expires_in_seconds: issued.expires_in.as_secs(),
        reviewer_id,
        email: normalized_email,
    }))
}

/// Invalidates every session currently issued to the calling reviewer
/// (docs/SECURITY_PRIVACY.md section 10: "unauthorized access" incident
/// response) — "logout everywhere", not just the token used for this
/// request. Requires an authenticated reviewer; there is nothing to log out
/// of `Actor::Automated`.
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    let Actor::Reviewer(reviewer_id) = actor else {
        return Err(ApiError {
            status: StatusCode::UNAUTHORIZED,
            code: "NOT_AUTHENTICATED",
            message: "Logout requires an authenticated session.",
            request_id,
        });
    };

    state
        .reviewers
        .revoke_all_sessions(reviewer_id)
        .await
        .map_err(|_| persistence_failed(request_id))?;
    state
        .reviewers
        .record_logout(reviewer_id, request_id)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok(StatusCode::NO_CONTENT)
}
