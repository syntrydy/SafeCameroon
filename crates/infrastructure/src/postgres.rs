use safe_cameroon_application::{AnonymousReportSubmission, report_submitted_event_payload};
use sqlx::{PgPool, Postgres, Transaction};

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
    ) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        insert_report(&mut transaction, submission).await?;
        insert_audit_event(&mut transaction, submission).await?;
        insert_outbox_event(&mut transaction, submission).await?;
        transaction.commit().await
    }
}

async fn insert_report(
    transaction: &mut Transaction<'_, Postgres>,
    submission: &AnonymousReportSubmission,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO reports (id, source_channel, raw_content, follow_up_token_hash)
        VALUES ($1, $2::report_source_channel, $3, $4)
        "#,
    )
    .bind(submission.report.id.as_uuid())
    .bind(submission.report.source_channel.as_database_value())
    .bind(&submission.report.raw_content)
    .bind(&submission.reference_code_hash)
    .execute(&mut **transaction)
    .await?;
    Ok(())
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
