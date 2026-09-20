//! Persistence for [`GenerationRecord`] (docs/AI.md section 4: provenance).
//! Mirrors `extractions.rs`'s shape -- every row here is written by this
//! repository from an already-validated [`GeneratedDescription`], so
//! read-back only ever `.expect()`s the shape it itself wrote.

use safe_cameroon_application::alert_description_generation::GenerationRecord;
use safe_cameroon_domain::{AlertDescriptionGenerationId, CaseId, GeneratedDescription};
use sqlx::PgPool;
use uuid::Uuid;

type GenerationRow = (
    Uuid,
    Uuid,
    Uuid,
    String,
    String,
    String,
    String,
    String,
    String,
);

fn generation_from_row(row: GenerationRow) -> GenerationRecord {
    let (
        id,
        case_id,
        requested_by,
        provider,
        model,
        prompt_version,
        source_text,
        description_en,
        description_fr,
    ) = row;
    GenerationRecord {
        id: AlertDescriptionGenerationId::from_uuid(id),
        case_id: CaseId::from_uuid(case_id),
        requested_by,
        provider,
        model,
        prompt_version,
        source_text,
        description: GeneratedDescription {
            description_en,
            description_fr,
        },
    }
}

#[derive(Clone)]
pub struct PostgresAlertDescriptionGenerationRepository {
    pool: PgPool,
}

impl PostgresAlertDescriptionGenerationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, record: &GenerationRecord) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO alert_description_generations
                (id, case_id, requested_by, provider, model, prompt_version, source_text,
                 description_en, description_fr)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(record.id.as_uuid())
        .bind(record.case_id.as_uuid())
        .bind(record.requested_by)
        .bind(&record.provider)
        .bind(&record.model)
        .bind(&record.prompt_version)
        .bind(&record.source_text)
        .bind(&record.description.description_en)
        .bind(&record.description.description_fr)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Most recent first -- a reviewer may request more than one attempt
    /// (e.g. after editing the draft), and every past suggestion stays
    /// visible rather than being overwritten.
    pub async fn find_by_case(
        &self,
        case_id: CaseId,
    ) -> Result<Vec<GenerationRecord>, sqlx::Error> {
        let rows: Vec<GenerationRow> = sqlx::query_as(
            r#"
            SELECT id, case_id, requested_by, provider, model, prompt_version, source_text,
                   description_en, description_fr
            FROM alert_description_generations
            WHERE case_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(case_id.as_uuid())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(generation_from_row).collect())
    }
}
