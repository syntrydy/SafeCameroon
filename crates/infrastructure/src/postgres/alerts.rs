use safe_cameroon_application::alert_workflow::{
    AlertCancellation, AlertCreation, alert_cancelled_event_payload, alert_created_event_payload,
};
use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_domain::{
    Alert, AlertEvent, AlertField, AlertFieldValue, AlertId, AlertPolicyId, AlertStatus,
    AlertVisibility, CaseEventType, IncidentType, OrganizationId, Severity,
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertCancelOutcome {
    Cancelled,
    /// The in-memory alert was mutated from a stale read; the caller must
    /// reload the alert and retry rather than silently overwrite a
    /// concurrent change.
    Conflict,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AlertFilter {
    pub status: Option<AlertStatus>,
    pub visibility: Option<AlertVisibility>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertCreationOutcome {
    Created,
    /// An alert with this `idempotency_key_hash` already exists (mirrors
    /// `SubmissionResult::Duplicate` in `postgres/reports.rs`).
    Duplicate {
        alert_id: Uuid,
    },
}

#[derive(Clone)]
pub struct PostgresAlertRepository {
    pool: PgPool,
}

impl PostgresAlertRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(&self, alert_id: AlertId) -> Result<Option<Alert>, sqlx::Error> {
        #[allow(clippy::type_complexity)]
        let Some(row): Option<(
            Uuid,
            String,
            i16,
            String,
            String,
            String,
            String,
            String,
            String,
            i64,
            Option<Uuid>,
        )> = sqlx::query_as(
            r#"
            SELECT case_id, policy_id, policy_version, incident_type::text, severity::text,
                   visibility::text, trigger::text, target_geography, status::text,
                   aggregate_version, issued_by_organization_id
            FROM alerts
            WHERE id = $1
            "#,
        )
        .bind(alert_id.as_uuid())
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        let (
            case_id,
            policy_id,
            policy_version,
            incident_type,
            severity,
            visibility,
            trigger,
            target_geography,
            status,
            aggregate_version,
            issued_by_organization_id,
        ) = row;

        let field_rows: Vec<(String, String)> =
            sqlx::query_as("SELECT field::text, value FROM alert_fields WHERE alert_id = $1")
                .bind(alert_id.as_uuid())
                .fetch_all(&self.pool)
                .await?;
        let fields = field_rows
            .into_iter()
            .map(|(field, value)| AlertFieldValue {
                field: AlertField::from_database_value(&field)
                    .expect("alert_fields.field is constrained by the alert_field enum"),
                value,
            })
            .collect();

        Ok(Some(Alert::reconstitute(
            alert_id,
            safe_cameroon_domain::CaseId::from_uuid(case_id),
            AlertPolicyId::new(policy_id),
            policy_version as u32,
            IncidentType::from_database_value(&incident_type)
                .expect("alerts.incident_type is constrained by the incident_type enum"),
            Severity::from_database_value(&severity)
                .expect("alerts.severity is constrained by the severity_level enum"),
            AlertVisibility::from_database_value(&visibility)
                .expect("alerts.visibility is constrained by the alert_visibility enum"),
            CaseEventType::from_database_value(&trigger)
                .expect("alerts.trigger is constrained by the case_event_type enum"),
            safe_cameroon_domain::TargetGeography::new(target_geography)
                .expect("a persisted target_geography was validated as non-blank on write"),
            AlertStatus::from_database_value(&status)
                .expect("alerts.status is constrained by the alert_status enum"),
            fields,
            aggregate_version as u64,
            issued_by_organization_id.map(OrganizationId::from_uuid),
        )))
    }

    /// Most recently created first (`alerts_status_created_at_idx`),
    /// optionally narrowed by status and/or visibility. `limit`/`offset` are
    /// taken as given — the API layer clamps `limit` to a sane maximum
    /// before it ever reaches here.
    pub async fn list(
        &self,
        filter: &AlertFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Alert>, sqlx::Error> {
        #[allow(clippy::type_complexity)]
        let rows: Vec<(
            Uuid,
            Uuid,
            String,
            i16,
            String,
            String,
            String,
            String,
            String,
            String,
            i64,
            Option<Uuid>,
        )> = sqlx::query_as(
            r#"
            SELECT id, case_id, policy_id, policy_version, incident_type::text, severity::text,
                   visibility::text, trigger::text, target_geography, status::text,
                   aggregate_version, issued_by_organization_id
            FROM alerts
            WHERE ($1::alert_status IS NULL OR status = $1::alert_status)
              AND ($2::alert_visibility IS NULL OR visibility = $2::alert_visibility)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(filter.status.map(AlertStatus::as_database_value))
        .bind(filter.visibility.map(AlertVisibility::as_database_value))
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let alert_ids: Vec<Uuid> = rows.iter().map(|(id, ..)| *id).collect();
        let field_rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            "SELECT alert_id, field::text, value FROM alert_fields WHERE alert_id = ANY($1)",
        )
        .bind(&alert_ids)
        .fetch_all(&self.pool)
        .await?;

        let mut fields_by_alert: std::collections::HashMap<Uuid, Vec<AlertFieldValue>> =
            std::collections::HashMap::new();
        for (alert_id, field, value) in field_rows {
            fields_by_alert
                .entry(alert_id)
                .or_default()
                .push(AlertFieldValue {
                    field: AlertField::from_database_value(&field)
                        .expect("alert_fields.field is constrained by the alert_field enum"),
                    value,
                });
        }

        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    case_id,
                    policy_id,
                    policy_version,
                    incident_type,
                    severity,
                    visibility,
                    trigger,
                    target_geography,
                    status,
                    aggregate_version,
                    issued_by_organization_id,
                )| {
                    Alert::reconstitute(
                        AlertId::from_uuid(id),
                        safe_cameroon_domain::CaseId::from_uuid(case_id),
                        AlertPolicyId::new(policy_id),
                        policy_version as u32,
                        IncidentType::from_database_value(&incident_type).expect(
                            "alerts.incident_type is constrained by the incident_type enum",
                        ),
                        Severity::from_database_value(&severity)
                            .expect("alerts.severity is constrained by the severity_level enum"),
                        AlertVisibility::from_database_value(&visibility).expect(
                            "alerts.visibility is constrained by the alert_visibility enum",
                        ),
                        CaseEventType::from_database_value(&trigger)
                            .expect("alerts.trigger is constrained by the case_event_type enum"),
                        safe_cameroon_domain::TargetGeography::new(target_geography).expect(
                            "a persisted target_geography was validated as non-blank on write",
                        ),
                        AlertStatus::from_database_value(&status)
                            .expect("alerts.status is constrained by the alert_status enum"),
                        fields_by_alert.remove(&id).unwrap_or_default(),
                        aggregate_version as u64,
                        issued_by_organization_id.map(OrganizationId::from_uuid),
                    )
                },
            )
            .collect())
    }

    /// Persists the alert, its field values, the `ALERT_CREATED` event, the
    /// audit record, and the outbox event in a single transaction.
    pub async fn create(
        &self,
        creation: &AlertCreation,
    ) -> Result<AlertCreationOutcome, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        if let Some(alert_id) = insert_alert(
            &mut transaction,
            &creation.alert,
            creation.idempotency_key_hash.as_deref(),
        )
        .await?
        {
            transaction.rollback().await?;
            return Ok(AlertCreationOutcome::Duplicate { alert_id });
        }
        insert_alert_fields(
            &mut transaction,
            creation.alert.id(),
            creation.alert.fields(),
        )
        .await?;
        insert_alert_event(&mut transaction, &creation.event, creation.actor).await?;
        insert_audit_event(
            &mut transaction,
            creation.audit_event_id.as_uuid(),
            creation.actor,
            creation.event.event_type.as_database_value(),
            creation.alert.id().as_uuid(),
            creation.request_id,
        )
        .await?;
        insert_outbox_event(
            &mut transaction,
            creation.outbox_event_id.as_uuid(),
            creation.alert.id().as_uuid(),
            creation.event.event_type.as_database_value(),
            alert_created_event_payload(creation),
        )
        .await?;
        transaction.commit().await?;
        Ok(AlertCreationOutcome::Created)
    }

    /// `alert` must be the aggregate *after* `cancel_alert` mutated it in
    /// memory, so its version already reflects the `ALERT_CANCELLED` event.
    pub async fn cancel(
        &self,
        alert: &Alert,
        cancellation: &AlertCancellation,
    ) -> Result<AlertCancelOutcome, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let expected_previous_version = alert.version() as i64 - 1;
        let result = sqlx::query(
            r#"
            UPDATE alerts
            SET status = $1::alert_status, aggregate_version = $2, updated_at = now()
            WHERE id = $3 AND aggregate_version = $4
            "#,
        )
        .bind(alert.status().as_database_value())
        .bind(alert.version() as i64)
        .bind(alert.id().as_uuid())
        .bind(expected_previous_version)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() != 1 {
            transaction.rollback().await?;
            return Ok(AlertCancelOutcome::Conflict);
        }

        insert_alert_event(&mut transaction, &cancellation.event, cancellation.actor).await?;
        insert_audit_event(
            &mut transaction,
            cancellation.audit_event_id.as_uuid(),
            cancellation.actor,
            cancellation.event.event_type.as_database_value(),
            alert.id().as_uuid(),
            cancellation.request_id,
        )
        .await?;
        insert_outbox_event(
            &mut transaction,
            cancellation.outbox_event_id.as_uuid(),
            alert.id().as_uuid(),
            cancellation.event.event_type.as_database_value(),
            alert_cancelled_event_payload(alert, cancellation),
        )
        .await?;
        transaction.commit().await?;
        Ok(AlertCancelOutcome::Cancelled)
    }
}

/// Returns the id of an already-persisted alert when `idempotency_key_hash`
/// collides with one (mirrors `postgres/reports.rs::insert_report`); `None`
/// means this alert was newly inserted.
async fn insert_alert(
    transaction: &mut Transaction<'_, Postgres>,
    alert: &Alert,
    idempotency_key_hash: Option<&[u8]>,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO alerts (
            id, case_id, policy_id, policy_version, incident_type, severity, visibility,
            trigger, target_geography, status, aggregate_version, idempotency_key_hash,
            issued_by_organization_id
        )
        VALUES (
            $1, $2, $3, $4, $5::incident_type, $6::severity_level, $7::alert_visibility,
            $8::case_event_type, $9, $10::alert_status, $11, $12, $13
        )
        ON CONFLICT (idempotency_key_hash) WHERE idempotency_key_hash IS NOT NULL DO NOTHING
        "#,
    )
    .bind(alert.id().as_uuid())
    .bind(alert.case_id().as_uuid())
    .bind(alert.policy_id().as_str())
    .bind(alert.policy_version() as i16)
    .bind(alert.incident_type().as_database_value())
    .bind(alert.severity().as_database_value())
    .bind(alert.visibility().as_database_value())
    .bind(alert.trigger().as_database_value())
    .bind(alert.target_geography().as_str())
    .bind(alert.status().as_database_value())
    .bind(alert.version() as i64)
    .bind(idempotency_key_hash)
    .bind(
        alert
            .issued_by_organization_id()
            .map(OrganizationId::as_uuid),
    )
    .execute(&mut **transaction)
    .await?;

    if let Some(hash) = idempotency_key_hash {
        let existing =
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM alerts WHERE idempotency_key_hash = $1")
                .bind(hash)
                .fetch_optional(&mut **transaction)
                .await?;
        if existing != Some(alert.id().as_uuid()) {
            return Ok(existing);
        }
    }
    Ok(None)
}

async fn insert_alert_fields(
    transaction: &mut Transaction<'_, Postgres>,
    alert_id: AlertId,
    fields: &[AlertFieldValue],
) -> Result<(), sqlx::Error> {
    for field_value in fields {
        sqlx::query(
            "INSERT INTO alert_fields (alert_id, field, value) VALUES ($1, $2::alert_field, $3)",
        )
        .bind(alert_id.as_uuid())
        .bind(field_value.field.as_database_value())
        .bind(&field_value.value)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn insert_alert_event(
    transaction: &mut Transaction<'_, Postgres>,
    event: &AlertEvent,
    actor: Actor,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO alert_events (id, alert_id, event_type, aggregate_version, actor_type, actor_id)
        VALUES ($1, $2, $3::alert_event_type, $4, $5, $6)
        "#,
    )
    .bind(event.id.as_uuid())
    .bind(event.alert_id.as_uuid())
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
    alert_id: Uuid,
    request_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO audit_events (id, actor_type, actor_id, action, resource_type, resource_id, request_id)
        VALUES ($1, $2, $3, $4, 'ALERT', $5, $6)
        "#,
    )
    .bind(audit_event_id)
    .bind(actor.as_database_value())
    .bind(actor.actor_id())
    .bind(action)
    .bind(alert_id)
    .bind(request_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_outbox_event(
    transaction: &mut Transaction<'_, Postgres>,
    outbox_event_id: Uuid,
    alert_id: Uuid,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO outbox_events (id, aggregate_type, aggregate_id, event_type, payload)
        VALUES ($1, 'ALERT', $2, $3, $4)
        "#,
    )
    .bind(outbox_event_id)
    .bind(alert_id)
    .bind(event_type)
    .bind(payload)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
