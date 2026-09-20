use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_domain::{
    Attachment, AttachmentContentType, AttachmentId, ReportId, StorageProvider,
};
use sqlx::PgPool;
use uuid::Uuid;

use super::audit_events::organization_id_for_actor;

#[derive(Clone)]
pub struct PostgresAttachmentRepository {
    pool: PgPool,
}

impl PostgresAttachmentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, attachment: &Attachment) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO attachments (
                id, report_id, storage_provider, object_key, content_type, size_bytes, checksum
            )
            VALUES ($1, $2, $3::storage_provider, $4, $5::attachment_content_type, $6, $7)
            "#,
        )
        .bind(attachment.id().as_uuid())
        .bind(attachment.report_id().as_uuid())
        .bind(attachment.storage_provider().as_database_value())
        .bind(attachment.object_key())
        .bind(attachment.content_type().as_database_value())
        .bind(attachment.size_bytes() as i64)
        .bind(attachment.checksum())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(
        &self,
        attachment_id: AttachmentId,
    ) -> Result<Option<Attachment>, sqlx::Error> {
        let row: Option<(Uuid, String, String, String, i64, String)> = sqlx::query_as(
            r#"
            SELECT report_id, storage_provider::text, object_key, content_type::text,
                   size_bytes, checksum
            FROM attachments
            WHERE id = $1
            "#,
        )
        .bind(attachment_id.as_uuid())
        .fetch_optional(&self.pool)
        .await?;

        let Some((report_id, storage_provider, object_key, content_type, size_bytes, checksum)) =
            row
        else {
            return Ok(None);
        };

        Ok(Some(Attachment::reconstitute(
            attachment_id,
            ReportId::from_uuid(report_id),
            StorageProvider::from_database_value(&storage_provider)
                .expect("attachments.storage_provider is constrained by the storage_provider enum"),
            object_key,
            AttachmentContentType::from_database_value(&content_type).expect(
                "attachments.content_type is constrained by the attachment_content_type enum",
            ),
            size_bytes as u64,
            checksum,
        )))
    }

    /// A report's attachments in upload order (`created_at` ascending) —
    /// docs/API.md area `/attachments`: a reviewer can see what is attached
    /// to a report without already knowing individual attachment ids.
    pub async fn find_by_report_id(
        &self,
        report_id: ReportId,
    ) -> Result<Vec<Attachment>, sqlx::Error> {
        #[allow(clippy::type_complexity)]
        let rows: Vec<(Uuid, String, String, String, i64, String)> = sqlx::query_as(
            r#"
            SELECT id, storage_provider::text, object_key, content_type::text,
                   size_bytes, checksum
            FROM attachments
            WHERE report_id = $1
            ORDER BY created_at
            "#,
        )
        .bind(report_id.as_uuid())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(id, storage_provider, object_key, content_type, size_bytes, checksum)| {
                    Attachment::reconstitute(
                        AttachmentId::from_uuid(id),
                        report_id,
                        StorageProvider::from_database_value(&storage_provider).expect(
                            "attachments.storage_provider is constrained by the storage_provider enum",
                        ),
                        object_key,
                        AttachmentContentType::from_database_value(&content_type).expect(
                            "attachments.content_type is constrained by the attachment_content_type enum",
                        ),
                        size_bytes as u64,
                        checksum,
                    )
                },
            )
            .collect())
    }

    /// Records that `actor` was issued a short-lived download URL for this
    /// attachment (docs/SECURITY_PRIVACY.md section 6: "add audit events for
    /// sensitive operations" — reading private evidence is exactly that).
    pub async fn record_download_access(
        &self,
        attachment_id: AttachmentId,
        actor: Actor,
        request_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let organization_id = organization_id_for_actor(&self.pool, actor.actor_id()).await?;
        sqlx::query(
            r#"
            INSERT INTO audit_events (id, actor_type, actor_id, organization_id, action, resource_type, resource_id, request_id)
            VALUES ($1, $2, $3, $4, 'ATTACHMENT_DOWNLOAD_URL_ISSUED', 'ATTACHMENT', $5, $6)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(actor.as_database_value())
        .bind(actor.actor_id())
        .bind(organization_id)
        .bind(attachment_id.as_uuid())
        .bind(request_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
