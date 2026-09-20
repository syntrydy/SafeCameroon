use safe_cameroon_application::case_workflow::{
    Actor, CaseCreation, CaseReportLink, CaseReview, case_created_event_payload,
    case_report_linked_event_payload, case_status_changed_event_payload,
};
use safe_cameroon_domain::{
    Case, CaseEvent, CaseEventType, CaseId, CaseStatus, IncidentType, ReportId,
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::audit_events::organization_id_for_actor;

const UNIQUE_VIOLATION: &str = "23505";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseCreationOutcome {
    Created,
    /// The report is already linked to a different case (`case_reports` allows
    /// a report to belong to at most one case).
    ReportAlreadyLinked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseLinkOutcome {
    Linked,
    ReportAlreadyLinked,
    /// The in-memory case was mutated from a stale read; the caller must reload
    /// the case and retry rather than silently overwrite a concurrent change.
    Conflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseReviewOutcome {
    Applied,
    Conflict,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CaseFilter {
    pub status: Option<CaseStatus>,
    pub incident_type: Option<IncidentType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseEventRecord {
    pub id: Uuid,
    pub case_id: Uuid,
    pub event_type: CaseEventType,
    pub aggregate_version: u64,
    pub actor_type: String,
    pub actor_id: Option<Uuid>,
    /// See `AuditEventRecord::occurred_at` (`postgres/audit_events.rs`) for
    /// why this stays a plain `String` rather than a decoded date/time type.
    pub occurred_at: String,
}

#[derive(Clone)]
pub struct PostgresCaseRepository {
    pool: PgPool,
}

impl PostgresCaseRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(&self, case_id: CaseId) -> Result<Option<Case>, sqlx::Error> {
        let Some((incident_type, status, aggregate_version)) = sqlx::query_as::<
            _,
            (String, String, i64),
        >(
            "SELECT incident_type::text, status::text, aggregate_version FROM cases WHERE id = $1",
        )
        .bind(case_id.as_uuid())
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };

        let report_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT report_id FROM case_reports WHERE case_id = $1 ORDER BY linked_at",
        )
        .bind(case_id.as_uuid())
        .fetch_all(&self.pool)
        .await?;

        Ok(Some(Case::reconstitute(
            case_id,
            IncidentType::from_database_value(&incident_type)
                .expect("cases.incident_type is constrained by the incident_type enum"),
            CaseStatus::from_database_value(&status)
                .expect("cases.status is constrained by the case_status enum"),
            report_ids.into_iter().map(ReportId::from_uuid).collect(),
            aggregate_version as u64,
        )))
    }

    /// Most recently updated first (`cases_status_updated_at_idx`), optionally
    /// narrowed by status and/or incident type. `limit`/`offset` are taken as
    /// given — the API layer clamps `limit` to a sane maximum before it ever
    /// reaches here.
    pub async fn list(
        &self,
        filter: &CaseFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Case>, sqlx::Error> {
        let rows: Vec<(Uuid, String, String, i64)> = sqlx::query_as(
            r#"
            SELECT id, incident_type::text, status::text, aggregate_version
            FROM cases
            WHERE ($1::case_status IS NULL OR status = $1::case_status)
              AND ($2::incident_type IS NULL OR incident_type = $2::incident_type)
            ORDER BY updated_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(filter.status.map(CaseStatus::as_database_value))
        .bind(filter.incident_type.map(IncidentType::as_database_value))
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let case_ids: Vec<Uuid> = rows.iter().map(|(id, ..)| *id).collect();
        let links: Vec<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT case_id, report_id FROM case_reports WHERE case_id = ANY($1) ORDER BY linked_at",
        )
        .bind(&case_ids)
        .fetch_all(&self.pool)
        .await?;

        let mut report_ids_by_case: std::collections::HashMap<Uuid, Vec<ReportId>> =
            std::collections::HashMap::new();
        for (case_id, report_id) in links {
            report_ids_by_case
                .entry(case_id)
                .or_default()
                .push(ReportId::from_uuid(report_id));
        }

        Ok(rows
            .into_iter()
            .map(|(id, incident_type, status, aggregate_version)| {
                Case::reconstitute(
                    CaseId::from_uuid(id),
                    IncidentType::from_database_value(&incident_type)
                        .expect("cases.incident_type is constrained by the incident_type enum"),
                    CaseStatus::from_database_value(&status)
                        .expect("cases.status is constrained by the case_status enum"),
                    report_ids_by_case.remove(&id).unwrap_or_default(),
                    aggregate_version as u64,
                )
            })
            .collect())
    }

    /// Persists the case, its first report link, the `CASE_CREATED` event, the
    /// audit record, and the outbox event in a single transaction.
    pub async fn create(
        &self,
        creation: &CaseCreation,
    ) -> Result<CaseCreationOutcome, sqlx::Error> {
        let report_id = *creation
            .case
            .report_ids()
            .first()
            .expect("a freshly created case always has exactly one report");

        let mut transaction = self.pool.begin().await?;
        insert_case(&mut transaction, &creation.case).await?;

        if let Err(error) =
            insert_case_report_link(&mut transaction, creation.case.id(), report_id).await
        {
            if is_unique_violation(&error) {
                transaction.rollback().await?;
                return Ok(CaseCreationOutcome::ReportAlreadyLinked);
            }
            return Err(error);
        }

        mark_report_linked_to_case(&mut transaction, report_id).await?;
        insert_case_event(&mut transaction, &creation.event, creation.actor).await?;
        insert_audit_event(
            &mut transaction,
            creation.audit_event_id.as_uuid(),
            creation.actor,
            creation.event.event_type.as_database_value(),
            creation.case.id().as_uuid(),
            creation.request_id,
        )
        .await?;
        insert_outbox_event(
            &mut transaction,
            creation.outbox_event_id.as_uuid(),
            creation.case.id().as_uuid(),
            creation.event.event_type.as_database_value(),
            case_created_event_payload(creation),
        )
        .await?;
        transaction.commit().await?;
        Ok(CaseCreationOutcome::Created)
    }

    /// Links an additional report to an already-persisted case. `case` must be
    /// the aggregate *after* `link_report_to_case` mutated it in memory, so its
    /// version already reflects the new event.
    pub async fn link_report(
        &self,
        case: &Case,
        link: &CaseReportLink,
    ) -> Result<CaseLinkOutcome, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;

        if let Err(error) =
            insert_case_report_link(&mut transaction, case.id(), link.report_id).await
        {
            if is_unique_violation(&error) {
                transaction.rollback().await?;
                return Ok(CaseLinkOutcome::ReportAlreadyLinked);
            }
            return Err(error);
        }
        mark_report_linked_to_case(&mut transaction, link.report_id).await?;

        if !advance_case_version(&mut transaction, case.id(), None, case.version()).await? {
            transaction.rollback().await?;
            return Ok(CaseLinkOutcome::Conflict);
        }

        insert_case_event(&mut transaction, &link.event, link.actor).await?;
        insert_audit_event(
            &mut transaction,
            link.audit_event_id.as_uuid(),
            link.actor,
            link.event.event_type.as_database_value(),
            case.id().as_uuid(),
            link.request_id,
        )
        .await?;
        insert_outbox_event(
            &mut transaction,
            link.outbox_event_id.as_uuid(),
            case.id().as_uuid(),
            link.event.event_type.as_database_value(),
            case_report_linked_event_payload(case.id(), link),
        )
        .await?;
        transaction.commit().await?;
        Ok(CaseLinkOutcome::Linked)
    }

    /// Applies a reviewer-authorized (or automated review-start) status change.
    /// `case` must already reflect the transition; `review.event.aggregate_version`
    /// is used as the expected new version for optimistic concurrency control.
    pub async fn apply_review(
        &self,
        case: &Case,
        review: &CaseReview,
    ) -> Result<CaseReviewOutcome, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;

        if !advance_case_version(
            &mut transaction,
            case.id(),
            Some(case.status()),
            case.version(),
        )
        .await?
        {
            transaction.rollback().await?;
            return Ok(CaseReviewOutcome::Conflict);
        }

        insert_case_event(&mut transaction, &review.event, review.actor).await?;
        insert_audit_event(
            &mut transaction,
            review.audit_event_id.as_uuid(),
            review.actor,
            review.event.event_type.as_database_value(),
            case.id().as_uuid(),
            review.request_id,
        )
        .await?;
        insert_outbox_event(
            &mut transaction,
            review.outbox_event_id.as_uuid(),
            case.id().as_uuid(),
            review.event.event_type.as_database_value(),
            case_status_changed_event_payload(case, review),
        )
        .await?;
        transaction.commit().await?;
        Ok(CaseReviewOutcome::Applied)
    }

    /// Chronological order (`aggregate_version` ascending) — the full history
    /// of a case's lifecycle, not just its current status.
    pub async fn list_events(&self, case_id: CaseId) -> Result<Vec<CaseEventRecord>, sqlx::Error> {
        #[allow(clippy::type_complexity)]
        let rows: Vec<(Uuid, String, i64, String, Option<Uuid>, String)> = sqlx::query_as(
            r#"
            SELECT id, event_type::text, aggregate_version, actor_type, actor_id, occurred_at::text
            FROM case_events
            WHERE case_id = $1
            ORDER BY aggregate_version
            "#,
        )
        .bind(case_id.as_uuid())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(id, event_type, aggregate_version, actor_type, actor_id, occurred_at)| {
                    CaseEventRecord {
                        id,
                        case_id: case_id.as_uuid(),
                        event_type: CaseEventType::from_database_value(&event_type).expect(
                            "case_events.event_type is constrained by the case_event_type enum",
                        ),
                        aggregate_version: aggregate_version as u64,
                        actor_type,
                        actor_id,
                        occurred_at,
                    }
                },
            )
            .collect())
    }
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|error| error.code())
        .is_some_and(|code| code == UNIQUE_VIOLATION)
}

async fn insert_case(
    transaction: &mut Transaction<'_, Postgres>,
    case: &Case,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO cases (id, incident_type, status, aggregate_version)
        VALUES ($1, $2::incident_type, $3::case_status, $4)
        "#,
    )
    .bind(case.id().as_uuid())
    .bind(case.incident_type().as_database_value())
    .bind(case.status().as_database_value())
    .bind(case.version() as i64)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_case_report_link(
    transaction: &mut Transaction<'_, Postgres>,
    case_id: CaseId,
    report_id: ReportId,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO case_reports (case_id, report_id) VALUES ($1, $2)")
        .bind(case_id.as_uuid())
        .bind(report_id.as_uuid())
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn mark_report_linked_to_case(
    transaction: &mut Transaction<'_, Postgres>,
    report_id: ReportId,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE reports SET status = 'LINKED_TO_CASE', updated_at = now() WHERE id = $1")
        .bind(report_id.as_uuid())
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

/// Optimistically advances `cases.aggregate_version` (and optionally `status`)
/// from `new_version - 1` to `new_version`. Returns `false` if no row matched,
/// meaning the case was modified concurrently since it was loaded.
async fn advance_case_version(
    transaction: &mut Transaction<'_, Postgres>,
    case_id: CaseId,
    new_status: Option<CaseStatus>,
    new_version: u64,
) -> Result<bool, sqlx::Error> {
    let expected_previous_version = new_version as i64 - 1;
    let result = match new_status {
        Some(status) => {
            sqlx::query(
                r#"
                UPDATE cases
                SET status = $1::case_status, aggregate_version = $2, updated_at = now()
                WHERE id = $3 AND aggregate_version = $4
                "#,
            )
            .bind(status.as_database_value())
            .bind(new_version as i64)
            .bind(case_id.as_uuid())
            .bind(expected_previous_version)
            .execute(&mut **transaction)
            .await?
        }
        None => {
            sqlx::query(
                r#"
                UPDATE cases
                SET aggregate_version = $1, updated_at = now()
                WHERE id = $2 AND aggregate_version = $3
                "#,
            )
            .bind(new_version as i64)
            .bind(case_id.as_uuid())
            .bind(expected_previous_version)
            .execute(&mut **transaction)
            .await?
        }
    };
    Ok(result.rows_affected() == 1)
}

async fn insert_case_event(
    transaction: &mut Transaction<'_, Postgres>,
    event: &CaseEvent,
    actor: Actor,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO case_events (id, case_id, event_type, aggregate_version, actor_type, actor_id)
        VALUES ($1, $2, $3::case_event_type, $4, $5, $6)
        "#,
    )
    .bind(event.id.as_uuid())
    .bind(event.case_id.as_uuid())
    .bind(event.event_type.as_database_value())
    .bind(event.aggregate_version as i64)
    .bind(actor.as_database_value())
    .bind(actor.actor_id())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_audit_event(
    transaction: &mut Transaction<'_, Postgres>,
    audit_event_id: Uuid,
    actor: Actor,
    action: &str,
    case_id: Uuid,
    request_id: Uuid,
) -> Result<(), sqlx::Error> {
    let organization_id = organization_id_for_actor(&mut **transaction, actor.actor_id()).await?;
    sqlx::query(
        r#"
        INSERT INTO audit_events (id, actor_type, actor_id, organization_id, action, resource_type, resource_id, request_id)
        VALUES ($1, $2, $3, $4, $5, 'CASE', $6, $7)
        "#,
    )
    .bind(audit_event_id)
    .bind(actor.as_database_value())
    .bind(actor.actor_id())
    .bind(organization_id)
    .bind(action)
    .bind(case_id)
    .bind(request_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_outbox_event(
    transaction: &mut Transaction<'_, Postgres>,
    outbox_event_id: Uuid,
    case_id: Uuid,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO outbox_events (id, aggregate_type, aggregate_id, event_type, payload)
        VALUES ($1, 'CASE', $2, $3, $4)
        "#,
    )
    .bind(outbox_event_id)
    .bind(case_id)
    .bind(event_type)
    .bind(payload)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
