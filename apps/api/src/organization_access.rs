//! Organization sessions consume subscribed alerts; raw incident operations stay platform-only.
use crate::{
    error::ApiError, request_id::request_id_from_headers, reviewer::actor_from_headers,
    state::AppState,
};
use axum::{
    extract::{Request, State},
    http::Method,
    middleware::Next,
    response::Response,
};
use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_domain::{Membership, Role};
use uuid::Uuid;

pub async fn membership(
    state: &AppState,
    actor: Actor,
    request_id: Uuid,
) -> Result<Membership, ApiError> {
    let Actor::Reviewer(id) = actor else {
        return Err(denied(request_id));
    };
    state
        .organizations
        .find_membership(id)
        .await
        .map_err(|_| ApiError {
            status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            code: "AUTHORIZATION_LOOKUP_FAILED",
            message: "Could not verify access. Please try again.",
            request_id,
        })?
        .ok_or_else(|| denied(request_id))
}

fn denied(request_id: Uuid) -> ApiError {
    ApiError {
        status: axum::http::StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "This operation requires platform administrator access.",
        request_id,
    }
}

fn platform_only(path: &str, method: &Method) -> bool {
    if method == Method::POST && (path == "/v1/reports" || path == "/v1/reports/transcribe-audio") {
        return false;
    }
    // An anonymous reporter attaches evidence to their own report the same
    // way they submit it -- no reviewer session exists yet to gate on.
    if method == Method::POST && path.starts_with("/v1/reports/") && path.ends_with("/attachments")
    {
        return false;
    }
    // Case creation, its status-transition endpoint, and verification each
    // already carry their own authorization
    // (`create_case_from_report`'s flat reviewer check, `authorize_case_verification`'s
    // per-organization trust grant in `apps/api/src/cases.rs`) -- gating them
    // to platform-admin-only here would make that trust-grant system
    // (crates/domain/src/organization.rs) unreachable for the `OrgAdmin`/
    // `Member` reviewers it exists for. Listing/reading cases in bulk stays
    // platform-only below; only these three mutating actions are exempted.
    if method == Method::POST
        && (path == "/v1/cases"
            || (path.starts_with("/v1/cases/") && path.ends_with("/events"))
            || (path.starts_with("/v1/cases/") && path.ends_with("/verify")))
    {
        return false;
    }
    // Creating an alert has its own explicit organization trust check.
    if method == Method::POST && path.starts_with("/v1/cases/") && path.ends_with("/alerts") {
        return false;
    }
    path == "/v1/reports"
        || path.starts_with("/v1/reports/")
        || path.starts_with("/v1/attachments/")
        || path == "/v1/cases"
        || path.starts_with("/v1/cases/")
        || path == "/v1/audit-events"
        || path.starts_with("/v1/deliveries/")
        || (path.starts_with("/v1/alerts/") && path.ends_with("/deliveries"))
        || path == "/v1/consumers"
}

pub async fn restrict_incident_access(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if platform_only(request.uri().path(), request.method()) {
        let request_id = request_id_from_headers(request.headers());
        let actor = actor_from_headers(
            &state.reviewer_session_tokens,
            &state.reviewers,
            request.headers(),
            request_id,
        )
        .await?;
        if membership(&state, actor, request_id).await?.role != Role::PlatformAdmin {
            return Err(denied(request_id));
        }
    }
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_incidents_and_delivery_details_require_platform_access() {
        for path in [
            "/v1/reports",
            "/v1/reports/id",
            "/v1/reports/id/attachments",
            "/v1/reports/id/extractions",
            "/v1/attachments/id/download-url",
            "/v1/cases",
            "/v1/cases/id",
            "/v1/cases/id/reports",
            "/v1/audit-events",
            "/v1/alerts/id/deliveries",
            "/v1/deliveries/id",
        ] {
            assert!(platform_only(path, &Method::GET), "{path}");
        }
        assert!(platform_only("/v1/reports/id/review", &Method::POST));
        assert!(!platform_only("/v1/reports", &Method::POST));
        assert!(!platform_only("/v1/alerts", &Method::GET));
        assert!(!platform_only("/v1/cases/id/alerts", &Method::POST));
    }

    #[test]
    fn an_organizations_own_case_and_report_workflow_actions_bypass_the_platform_only_gate() {
        assert!(!platform_only("/v1/reports/id/attachments", &Method::POST));
        assert!(!platform_only("/v1/cases", &Method::POST));
        assert!(!platform_only("/v1/cases/id/events", &Method::POST));
        assert!(!platform_only("/v1/cases/id/verify", &Method::POST));
    }

    #[test]
    fn listing_and_reading_cases_and_reports_in_bulk_stays_platform_only() {
        assert!(platform_only("/v1/reports/id/attachments", &Method::GET));
        assert!(platform_only("/v1/cases", &Method::GET));
        assert!(platform_only("/v1/cases/id", &Method::GET));
        assert!(platform_only("/v1/cases/id/events", &Method::GET));
    }
}
