//! Attachment endpoints (docs/API.md area `/attachments`,
//! docs/SECURITY_PRIVACY.md section 5). Uploading is part of anonymous
//! report submission and requires no actor; requesting a download URL is a
//! protected, audited operation gated by
//! [`Capability::ViewCase`](safe_cameroon_application::authorization::Capability::ViewCase).

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use safe_cameroon_application::attachment_workflow::{
    PrepareAttachmentError, prepare_attachment_upload,
};
use safe_cameroon_application::authorization::{Capability, authorize};
use safe_cameroon_domain::{AttachmentContentType, AttachmentError, AttachmentId, ReportId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::request_id::request_id_from_headers;
use crate::reviewer::actor_from_headers;
use crate::state::AppState;

fn persistence_failed(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "ATTACHMENT_PERSISTENCE_FAILED",
        message: "The attachment could not be saved. Please try again.",
        request_id,
    }
}

fn storage_unavailable(request_id: Uuid) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "ATTACHMENT_STORAGE_UNAVAILABLE",
        message: "Attachment storage is temporarily unavailable. Please try again.",
        request_id,
    }
}

#[derive(Deserialize)]
pub struct CreateAttachmentRequest {
    content_type: String,
    size_bytes: u64,
    checksum: String,
}

#[derive(Serialize)]
pub struct CreateAttachmentResponse {
    attachment_id: Uuid,
    object_key: String,
    upload_url: String,
    upload_expires_in_seconds: u64,
}

pub async fn create_attachment(
    State(state): State<AppState>,
    Path(report_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<CreateAttachmentRequest>,
) -> Result<(StatusCode, Json<CreateAttachmentResponse>), ApiError> {
    let request_id = request_id_from_headers(&headers);
    let report_id = ReportId::from_uuid(report_id);

    if !state
        .reports
        .exists(report_id)
        .await
        .map_err(|_| persistence_failed(request_id))?
    {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            code: "REPORT_NOT_FOUND",
            message: "No report exists with the given id.",
            request_id,
        });
    }

    let content_type =
        AttachmentContentType::from_mime_type(&request.content_type).ok_or(ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "UNSUPPORTED_ATTACHMENT_CONTENT_TYPE",
            message: "This attachment content type is not supported.",
            request_id,
        })?;

    let prepared = prepare_attachment_upload(
        state.attachment_storage.as_ref(),
        report_id,
        content_type,
        request.size_bytes,
        request.checksum,
    )
    .await
    .map_err(|error| match error {
        PrepareAttachmentError::Invalid(AttachmentError::ZeroSize) => ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "INVALID_ATTACHMENT_SIZE",
            message: "size_bytes must be greater than zero.",
            request_id,
        },
        PrepareAttachmentError::Invalid(AttachmentError::SizeExceedsLimit { .. }) => ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "ATTACHMENT_TOO_LARGE",
            message: "This attachment exceeds the size limit.",
            request_id,
        },
        PrepareAttachmentError::Invalid(_) => ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "INVALID_ATTACHMENT",
            message: "The attachment metadata is invalid.",
            request_id,
        },
        PrepareAttachmentError::Storage(_) => storage_unavailable(request_id),
    })?;

    state
        .attachments
        .create(&prepared.attachment)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateAttachmentResponse {
            attachment_id: prepared.attachment.id().as_uuid(),
            object_key: prepared.attachment.object_key().to_owned(),
            upload_url: prepared.upload.url,
            upload_expires_in_seconds: prepared.upload.expires_in.as_secs(),
        }),
    ))
}

#[derive(Serialize)]
pub struct DownloadUrlResponse {
    download_url: String,
    expires_in_seconds: u64,
}

pub async fn create_download_url(
    State(state): State<AppState>,
    Path(attachment_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DownloadUrlResponse>, ApiError> {
    let request_id = request_id_from_headers(&headers);
    let actor = actor_from_headers(&headers, request_id)?;
    authorize(actor, Capability::ViewCase).map_err(|_| {
        tracing::warn!(%request_id, attachment_id = %attachment_id, "unauthorized attachment access attempt");
        ApiError {
            status: StatusCode::FORBIDDEN,
            code: "NOT_AUTHORIZED",
            message: "Only an identified reviewer may access an attachment.",
            request_id,
        }
    })?;

    let attachment = state
        .attachments
        .find_by_id(AttachmentId::from_uuid(attachment_id))
        .await
        .map_err(|_| persistence_failed(request_id))?
        .ok_or(ApiError {
            status: StatusCode::NOT_FOUND,
            code: "ATTACHMENT_NOT_FOUND",
            message: "No attachment exists with the given id.",
            request_id,
        })?;

    let download = state
        .attachment_storage
        .create_download_url(attachment.object_key())
        .await
        .map_err(|_| storage_unavailable(request_id))?;

    state
        .attachments
        .record_download_access(attachment.id(), actor, request_id)
        .await
        .map_err(|_| persistence_failed(request_id))?;

    tracing::info!(%request_id, attachment_id = %attachment.id().as_uuid(), "attachment download url issued");
    Ok(Json(DownloadUrlResponse {
        download_url: download.url,
        expires_in_seconds: download.expires_in.as_secs(),
    }))
}
