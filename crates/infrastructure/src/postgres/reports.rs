use safe_cameroon_application::{AnonymousReportSubmission, report_submitted_event_payload};
use safe_cameroon_domain::{ReportId, ReportSourceChannel, ReportStatus};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionResult {
    Created,
    Duplicate { report_id: Uuid },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportSummary {
    pub id: Uuid,
    pub source_channel: ReportSourceChannel,
    pub status: ReportStatus,
    pub raw_content: String,
    /// See `AuditEventRecord::occurred_at` (`postgres/audit_events.rs`) for
    /// why this stays a plain `String` rather than a decoded date/time type.
    pub received_at: String,
}

#[derive(Clone)]
pub struct PostgresReportRepository {
    pool: PgPool,
}

impl PostgresReportRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Persists the report, immutable audit record, and integration outbox event
    /// in a single transaction. No provider or background work is started here.
    pub async fn submit_anonymous(
        &self,
        submission: &AnonymousReportSubmission,
    ) -> Result<SubmissionResult, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        if let Some(report_id) = insert_report(&mut transaction, submission).await? {
            transaction.rollback().await?;
            return Ok(SubmissionResult::Duplicate { report_id });
        }
        insert_audit_event(&mut transaction, submission).await?;
        insert_outbox_event(&mut transaction, submission).await?;
        transaction.commit().await?;
        Ok(SubmissionResult::Created)
    }

    pub async fn exists(&self, report_id: ReportId) -> Result<bool, sqlx::Error> {
        let found: Option<Uuid> = sqlx::query_scalar("SELECT id FROM reports WHERE id = $1")
            .bind(report_id.as_uuid())
            .fetch_optional(&self.pool)
            .await?;
        Ok(found.is_some())
    }

    /// A reviewer's second way to read a report's content, alongside
    /// `list` — by id, once they already know it (e.g. from a case's
    /// `report_ids`), rather than only ever finding it in the general
    /// listing.
    pub async fn find_by_id(
        &self,
        report_id: ReportId,
    ) -> Result<Option<ReportSummary>, sqlx::Error> {
        #[allow(clippy::type_complexity)]
        let row: Option<(Uuid, String, String, String, String)> = sqlx::query_as(
            r#"
            SELECT id, source_channel::text, status::text, raw_content, received_at::text
            FROM reports
            WHERE id = $1
            "#,
        )
        .bind(report_id.as_uuid())
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(id, source_channel, status, raw_content, received_at)| ReportSummary {
                id,
                source_channel: ReportSourceChannel::from_database_value(&source_channel).expect(
                    "reports.source_channel is constrained by the report_source_channel enum",
                ),
                status: ReportStatus::from_database_value(&status)
                    .expect("reports.status is constrained by the report_status enum"),
                raw_content,
                received_at,
            },
        ))
    }

    /// Most recently received first (`reports_status_received_at_idx`),
    /// optionally narrowed by status. `limit`/`offset` are taken as given —
    /// the API layer clamps `limit` to a sane maximum before it ever reaches
    /// here.
    pub async fn list(
        &self,
        status: Option<ReportStatus>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ReportSummary>, sqlx::Error> {
        #[allow(clippy::type_complexity)]
        let rows: Vec<(Uuid, String, String, String, String)> = sqlx::query_as(
            r#"
            SELECT id, source_channel::text, status::text, raw_content, received_at::text
            FROM reports
            WHERE ($1::report_status IS NULL OR status = $1::report_status)
            ORDER BY received_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(status.map(ReportStatus::as_database_value))
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(id, source_channel, status, raw_content, received_at)| ReportSummary {
                    id,
                    source_channel: ReportSourceChannel::from_database_value(&source_channel)
                        .expect(
                            "reports.source_channel is constrained by the report_source_channel enum",
                        ),
                    status: ReportStatus::from_database_value(&status)
                        .expect("reports.status is constrained by the report_status enum"),
                    raw_content,
                    received_at,
                },
            )
            .collect())
    }
}

async fn insert_report(
    transaction: &mut Transaction<'_, Postgres>,
    submission: &AnonymousReportSubmission,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO reports (id, source_channel, raw_content, follow_up_token_hash, idempotency_key_hash)
        VALUES ($1, $2::report_source_channel, $3, $4, $5)
        ON CONFLICT (idempotency_key_hash) WHERE idempotency_key_hash IS NOT NULL DO NOTHING
        "#,
    )
    .bind(submission.report.id.as_uuid())
    .bind(submission.report.source_channel.as_database_value())
    .bind(&submission.report.raw_content)
    .bind(&submission.reference_code_hash)
    .bind(&submission.idempotency_key_hash)
    .execute(&mut **transaction)
    .await?;
    if submission.idempotency_key_hash.is_some() {
        let existing =
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM reports WHERE idempotency_key_hash = $1")
                .bind(&submission.idempotency_key_hash)
                .fetch_optional(&mut **transaction)
                .await?;
        if existing != Some(submission.report.id.as_uuid()) {
            return Ok(existing);
        }
    }
    Ok(None)
}

async fn insert_audit_event(
    transaction: &mut Transaction<'_, Postgres>,
    submission: &AnonymousReportSubmission,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO audit_events (id, actor_type, action, resource_type, resource_id, request_id)
        VALUES ($1, 'ANONYMOUS_REPORTER', 'REPORT_SUBMITTED', 'REPORT', $2, $3)
        "#,
    )
    .bind(submission.audit_event_id.as_uuid())
    .bind(submission.report.id.as_uuid())
    .bind(submission.request_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_outbox_event(
    transaction: &mut Transaction<'_, Postgres>,
    submission: &AnonymousReportSubmission,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO outbox_events (id, aggregate_type, aggregate_id, event_type, payload)
        VALUES ($1, 'REPORT', $2, 'REPORT_SUBMITTED', $3)
        "#,
    )
    .bind(submission.outbox_event_id.as_uuid())
    .bind(submission.report.id.as_uuid())
    .bind(report_submitted_event_payload(submission))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
