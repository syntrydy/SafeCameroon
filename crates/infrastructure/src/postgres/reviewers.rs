//! Persistence for reviewer accounts (migration 0011). Password hashing
//! itself is `safe_cameroon_application::reviewer_auth`'s job — this
//! repository only ever stores/reads the already-hashed string. Also
//! implements `SessionRevocationStore` (migration 0015): the same table
//! backs both concerns, so one repository type serves both roles rather
//! than introducing a second one for a single column.

use async_trait::async_trait;
use safe_cameroon_application::reviewer_auth::{SessionRevocationError, SessionRevocationStore};
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

#[async_trait]
impl SessionRevocationStore for PostgresReviewerRepository {
    /// An unknown `reviewer_id` is treated as revoked (deny by default) —
    /// unlike `InMemorySessionRevocationStore` (which only ever tracks
    /// revocation timestamps and has no notion of "does this reviewer
    /// exist"), this repository owns the `reviewers` table and can tell the
    /// two cases apart. A validly-signed token can only ever have been
    /// issued for a reviewer that existed at login time, so this path is
    /// unreachable in practice today (there is no reviewer-deletion
    /// feature) — the stricter default is defense in depth, not a case this
    /// exercises normally.
    async fn is_session_revoked(
        &self,
        reviewer_id: Uuid,
        issued_at_epoch_seconds: i64,
    ) -> Result<bool, SessionRevocationError> {
        let row: Option<(bool,)> = sqlx::query_as(
            "SELECT sessions_revoked_at IS NOT NULL AND sessions_revoked_at > to_timestamp($2) \
             FROM reviewers WHERE id = $1",
        )
        .bind(reviewer_id)
        .bind(issued_at_epoch_seconds as f64)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| SessionRevocationError {
            reason: error.to_string(),
        })?;
        Ok(row.is_none_or(|(revoked,)| revoked))
    }

    async fn revoke_all_sessions(&self, reviewer_id: Uuid) -> Result<(), SessionRevocationError> {
        sqlx::query("UPDATE reviewers SET sessions_revoked_at = now() WHERE id = $1")
            .bind(reviewer_id)
            .execute(&self.pool)
            .await
            .map_err(|error| SessionRevocationError {
                reason: error.to_string(),
            })?;
        Ok(())
    }
}
