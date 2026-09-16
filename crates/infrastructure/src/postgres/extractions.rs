//! Persistence for [`ExtractionRecord`] (docs/AI.md section 4:
//! provenance). `fields` is stored as a plain JSON object rather than one
//! column per field, matching `subscriptions.rules`'s precedent -- the
//! schema is expected to grow as more incident types are supported
//! (docs/ROADMAP.md Stage 4), and every row here is written by this
//! repository from an already-validated `ExtractedReportFields`, so
//! read-back only ever `.expect()`s the shape it itself wrote.

use safe_cameroon_application::ai_extraction::ExtractionRecord;
use safe_cameroon_domain::{ExtractedReportFields, ReportExtractionId, ReportId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
struct FieldsJson {
    person_description: Option<String>,
    age: Option<String>,
    time: Option<String>,
    place: Option<String>,
    incident_category: Option<String>,
    vehicle_details: Option<String>,
    contact_request: Option<String>,
}

impl From<&ExtractedReportFields> for FieldsJson {
    fn from(fields: &ExtractedReportFields) -> Self {
        Self {
            person_description: fields.person_description.clone(),
            age: fields.age.clone(),
            time: fields.time.clone(),
            place: fields.place.clone(),
            incident_category: fields.incident_category.clone(),
            vehicle_details: fields.vehicle_details.clone(),
            contact_request: fields.contact_request.clone(),
        }
    }
}

impl From<FieldsJson> for ExtractedReportFields {
    fn from(raw: FieldsJson) -> Self {
        ExtractedReportFields::new(
            raw.person_description,
            raw.age,
            raw.time,
            raw.place,
            raw.incident_category,
            raw.vehicle_details,
            raw.contact_request,
        )
    }
}

type ExtractionRow = (Uuid, Uuid, Uuid, String, String, String, Value);

fn extraction_from_row(row: ExtractionRow) -> ExtractionRecord {
    let (id, report_id, requested_by, provider, model, prompt_version, fields) = row;
    let fields: FieldsJson = serde_json::from_value(fields)
        .expect("report_extractions.fields is written by this repository");
    ExtractionRecord {
        id: ReportExtractionId::from_uuid(id),
        report_id: ReportId::from_uuid(report_id),
        requested_by,
        provider,
        model,
        prompt_version,
        fields: fields.into(),
    }
}

#[derive(Clone)]
pub struct PostgresReportExtractionRepository {
    pool: PgPool,
}

impl PostgresReportExtractionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, record: &ExtractionRecord) -> Result<(), sqlx::Error> {
        let fields = serde_json::to_value(FieldsJson::from(&record.fields))
            .expect("ExtractedReportFields always serializes");
        sqlx::query(
            r#"
            INSERT INTO report_extractions
                (id, report_id, requested_by, provider, model, prompt_version, fields)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(record.id.as_uuid())
        .bind(record.report_id.as_uuid())
        .bind(record.requested_by)
        .bind(&record.provider)
        .bind(&record.model)
        .bind(&record.prompt_version)
        .bind(fields)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Most recent first -- a report may be extracted more than once (e.g.
    /// after a follow-up adds detail), and every past suggestion stays
    /// visible rather than being overwritten.
    pub async fn find_by_report(
        &self,
        report_id: ReportId,
    ) -> Result<Vec<ExtractionRecord>, sqlx::Error> {
        let rows: Vec<ExtractionRow> = sqlx::query_as(
            r#"
            SELECT id, report_id, requested_by, provider, model, prompt_version, fields
            FROM report_extractions
            WHERE report_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(report_id.as_uuid())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(extraction_from_row).collect())
    }
}
