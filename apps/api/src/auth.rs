//! Reviewer account registration and login (prompt 09). Registration is
//! open only to bootstrap the very first reviewer in a fresh deployment or
//! to an already-authenticated reviewer with `ManageOrganization`
//! (`authorize_reviewer_registration`); login exchanges verified
//! credentials for a signed session token (`reviewer.rs` verifies it back
//! on every subsequent request).

use std::sync::OnceLock;

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use safe_cameroon_application::authorization::authorize_reviewer_registration;
use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_application::rate_limit::RateLimitScope;
use safe_cameroon_application::reviewer_auth::{
    PasswordError, SessionRevocationStore, hash_password, verify_password,
};
use safe_cameroon_infrastructure::postgres::CreateReviewerOutcome;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::rate_limit::enforce_rate_limit;
use crate::request_id::request_id_from_headers;
use crate::reviewer::actor_from_headers;
use crate::state::AppState;

fn persistence_failed(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "REVIEWER_PERSISTENCE_FAILED",
        message: "The request could not be saved. Please try again.",
        request_id,
    }
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    reviewer_id: Uuid,
    email: String,
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
    authorize_reviewer_registration(actor, existing_reviewer_count).map_err(|_| ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "Only an existing reviewer with ManageOrganization may register another reviewer.",
        request_id,
    })?;

    let email = request.email.trim();
    if email.is_empty() || !email.contains('@') {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "INVALID_EMAIL",
            message: "email must be a non-blank address.",
            request_id,
        });
    }

    let password_hash = hash_password(&request.password).map_err(|error| match error {
        PasswordError::TooShort => ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "WEAK_PASSWORD",
            message: "password must be at least 8 characters.",
            request_id,
        },
    })?;

    let reviewer_id = Uuid::new_v4();
    match state
        .reviewers
        .create(reviewer_id, email, &password_hash)
        .await
        .map_err(|_| persistence_failed(request_id))?
    {
        CreateReviewerOutcome::Created => {
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
pub struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    token: String,
    expires_in_seconds: u64,
    reviewer_id: Uuid,
}

/// A hash of a fixed, never-issued password, computed once per process.
/// Verifying against it when `email` matches no account keeps a failed
/// login's timing close to a wrong-password failure, rather than letting an
/// unknown email return noticeably faster and revealing which emails have
/// accounts.
fn dummy_password_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        hash_password("not-a-real-account-password")
            .expect("this hardcoded password satisfies the minimum length")
    })
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let normalized_email = request.email.trim().to_lowercase();
    // Keyed by the targeted account (not the caller's IP) so a credential-
    // stuffing attempt against one reviewer is throttled even if the
    // attacker rotates source addresses.
    enforce_rate_limit(
        state.rate_limiter.as_ref(),
        RateLimitScope::ReviewerLoginAttempt,
        &normalized_email,
        request_id,
    )
    .await?;

    let invalid_credentials = || ApiError {
        status: StatusCode::UNAUTHORIZED,
        code: "INVALID_CREDENTIALS",
        message: "Invalid email or password.",
        request_id,
    };

    let record = state
        .reviewers
        .find_by_email(&normalized_email)
        .await
        .map_err(|_| persistence_failed(request_id))?;
    let matched_reviewer_id = record.as_ref().map(|record| record.id);

    let password_matches = match &record {
        Some(record) => verify_password(&request.password, &record.password_hash),
        None => {
            verify_password(&request.password, dummy_password_hash());
            false
        }
    };

    if !password_matches {
        state
            .reviewers
            .record_login_failure(matched_reviewer_id, &normalized_email, request_id)
            .await
            .map_err(|_| persistence_failed(request_id))?;
        return Err(invalid_credentials());
    }
    let record = record.expect("password_matches is only true when a record was found");

    state
        .reviewers
        .record_login_success(record.id, request_id)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    let issued = state.reviewer_session_tokens.issue(record.id);
    Ok(Json(LoginResponse {
        token: issued.token,
        expires_in_seconds: issued.expires_in.as_secs(),
        reviewer_id: record.id,
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
