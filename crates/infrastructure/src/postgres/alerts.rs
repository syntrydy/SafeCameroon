use safe_cameroon_application::alert_workflow::{
    AlertCancellation, AlertCreation, alert_cancelled_event_payload, alert_created_event_payload,
};
use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_domain::{
    Alert, AlertEvent, AlertField, AlertFieldValue, AlertId, AlertPolicyId, AlertStatus,
    AlertVisibility, CaseEventType, IncidentType, Severity,
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
        )> = sqlx::query_as(
            r#"
            SELECT case_id, policy_id, policy_version, incident_type::text, severity::text,
                   visibility::text, trigger::text, target_geography, status::text,
                   aggregate_version
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
        )))
    }

    /// Persists the alert, its field values, the `ALERT_CREATED` event, the
    /// audit record, and the outbox event in a single transaction.
    pub async fn create(&self, creation: &AlertCreation) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        insert_alert(&mut transaction, &creation.alert).await?;
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
        Ok(())
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

async fn insert_alert(
    transaction: &mut Transaction<'_, Postgres>,
    alert: &Alert,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO alerts (
            id, case_id, policy_id, policy_version, incident_type, severity, visibility,
            trigger, target_geography, status, aggregate_version
        )
        VALUES (
            $1, $2, $3, $4, $5::incident_type, $6::severity_level, $7::alert_visibility,
            $8::case_event_type, $9, $10::alert_status, $11
        )
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
    .execute(&mut **transaction)
    .await?;
    Ok(())
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
