//! Reading the audit trail back (docs/SECURITY_PRIVACY.md section 6), gated
//! by the `Capability::ViewAudit` that already existed but had no endpoint
//! wired to it.

use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use safe_cameroon_application::authorization::{Capability, authorize};
use safe_cameroon_infrastructure::postgres::{AuditEventFilter, AuditEventRecord};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::request_id::request_id_from_headers;
use crate::reviewer::actor_from_headers;
use crate::state::AppState;

const DEFAULT_LIMIT: u32 = 50;
/// A hard ceiling regardless of what the caller asks for, so a single
/// request can never force an unbounded scan/response.
const MAX_LIMIT: u32 = 100;

fn persistence_failed(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "AUDIT_EVENT_QUERY_FAILED",
        message: "The audit trail could not be read. Please try again.",
        request_id,
    }
}

#[derive(Deserialize)]
pub struct ListAuditEventsQuery {
    resource_type: Option<String>,
    resource_id: Option<Uuid>,
    action: Option<String>,
    actor_id: Option<Uuid>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Serialize)]
pub struct AuditEventResponse {
    id: Uuid,
    actor_type: String,
    actor_id: Option<Uuid>,
    action: String,
    resource_type: String,
    resource_id: Option<Uuid>,
    request_id: Option<Uuid>,
    metadata: serde_json::Value,
    occurred_at: String,
}

fn audit_event_response(record: &AuditEventRecord) -> AuditEventResponse {
    AuditEventResponse {
        id: record.id,
        actor_type: record.actor_type.clone(),
        actor_id: record.actor_id,
        action: record.action.clone(),
        resource_type: record.resource_type.clone(),
        resource_id: record.resource_id,
        request_id: record.request_id,
        metadata: record.metadata.clone(),
        occurred_at: record.occurred_at.clone(),
    }
}

pub async fn list_audit_events(
    State(state): State<AppState>,
    Query(query): Query<ListAuditEventsQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<AuditEventResponse>>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(
        &state.reviewer_session_tokens,
        &state.reviewers,
        &headers,
        request_id,
    )
    .await?;
    authorize(actor, Capability::ViewAudit).map_err(|_| ApiError {
        status: StatusCode::FORBIDDEN,
        code: "NOT_AUTHORIZED",
        message: "Only an identified reviewer may view the audit trail.",
        request_id,
    })?;

    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;
    let filter = AuditEventFilter {
        resource_type: query.resource_type,
        resource_id: query.resource_id,
        action: query.action,
        actor_id: query.actor_id,
    };

    let events = state
        .audit_events
        .list(&filter, limit, offset)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok(Json(events.iter().map(audit_event_response).collect()))
}
