//! Persistence for reviewer accounts (migration 0011). Password hashing
//! itself is `safe_cameroon_application::reviewer_auth`'s job — this
//! repository only ever stores/reads the already-hashed string.

use sqlx::PgPool;
use uuid::Uuid;

const UNIQUE_VIOLATION: &str = "23505";

fn is_unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|error| error.code())
        .is_some_and(|code| code == UNIQUE_VIOLATION)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerRecord {
    pub id: Uuid,
    pub password_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateReviewerOutcome {
    Created,
    /// `reviewers_email_unique_idx` is case-insensitive; this is the normal,
    /// expected outcome of a duplicate registration attempt, not a database
    /// failure — mirrors `CaseCreationOutcome::ReportAlreadyLinked`.
    EmailAlreadyRegistered,
}

#[derive(Clone)]
pub struct PostgresReviewerRepository {
    pool: PgPool,
}

impl PostgresReviewerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Used to decide whether a registration is the deployment's
    /// bootstrapping first reviewer (`authorize_reviewer_registration`).
    pub async fn count(&self) -> Result<u64, sqlx::Error> {
        let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM reviewers")
            .fetch_one(&self.pool)
            .await?;
        Ok(count as u64)
    }

    pub async fn create(
        &self,
        id: Uuid,
        email: &str,
        password_hash: &str,
    ) -> Result<CreateReviewerOutcome, sqlx::Error> {
        let result =
            sqlx::query("INSERT INTO reviewers (id, email, password_hash) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(email)
                .bind(password_hash)
                .execute(&self.pool)
                .await;

        match result {
            Ok(_) => Ok(CreateReviewerOutcome::Created),
            Err(error) if is_unique_violation(&error) => {
                Ok(CreateReviewerOutcome::EmailAlreadyRegistered)
            }
            Err(error) => Err(error),
        }
    }

    pub async fn find_by_email(&self, email: &str) -> Result<Option<ReviewerRecord>, sqlx::Error> {
        let row: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT id, password_hash FROM reviewers WHERE lower(email) = lower($1)",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(id, password_hash)| ReviewerRecord { id, password_hash }))
    }
}
