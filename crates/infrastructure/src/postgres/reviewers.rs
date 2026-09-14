//! Persistence for reviewer accounts (migration 0011). Password hashing
//! itself is `safe_cameroon_application::reviewer_auth`'s job — this
//! repository only ever stores/reads the already-hashed string. Also
//! implements `SessionRevocationStore` (migration 0015): the same table
//! backs both concerns, so one repository type serves both roles rather
//! than introducing a second one for a single column.

use async_trait::async_trait;
use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_application::reviewer_auth::{SessionRevocationError, SessionRevocationStore};
use serde_json::json;
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

    /// Records an `audit_events` row for a sensitive auth action, separately
    /// from the action itself — mirrors `PostgresAttachmentRepository::record_download_access`
    /// rather than folding this into `create`/`find_by_email`, which have
    /// other call sites/tests that do not need it.
    async fn record_audit_event(
        &self,
        actor: Actor,
        action: &str,
        resource_id: Option<Uuid>,
        request_id: Uuid,
        metadata: serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO audit_events (id, actor_type, actor_id, action, resource_type, resource_id, request_id, metadata)
            VALUES ($1, $2, $3, $4, 'REVIEWER', $5, $6, $7)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(actor.as_database_value())
        .bind(actor.actor_id())
        .bind(action)
        .bind(resource_id)
        .bind(request_id)
        .bind(metadata)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// `actor` is whoever performed the registration: `Actor::Automated` for
    /// a fresh deployment's bootstrapping first reviewer, or the
    /// authenticated reviewer who registered a colleague
    /// (`authorize_reviewer_registration`).
    pub async fn record_registration(
        &self,
        reviewer_id: Uuid,
        actor: Actor,
        request_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        self.record_audit_event(
            actor,
            "REVIEWER_REGISTERED",
            Some(reviewer_id),
            request_id,
            json!({}),
        )
        .await
    }

    pub async fn record_login_success(
        &self,
        reviewer_id: Uuid,
        request_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        self.record_audit_event(
            Actor::Reviewer(reviewer_id),
            "REVIEWER_LOGIN_SUCCEEDED",
            Some(reviewer_id),
            request_id,
            json!({}),
        )
        .await
    }

    /// `reviewer_id` is `None` when the attempted email matches no account —
    /// the caller is never authenticated at this point, so `actor` is always
    /// `Automated`. `email` is recorded in `metadata` (useful for spotting a
    /// credential-stuffing pattern against one account, or a scan across
    /// many); this is the same data the reviewer already gave the login
    /// endpoint, not new sensitive collection.
    pub async fn record_login_failure(
        &self,
        reviewer_id: Option<Uuid>,
        email: &str,
        request_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        self.record_audit_event(
            Actor::Automated,
            "REVIEWER_LOGIN_FAILED",
            reviewer_id,
            request_id,
            json!({ "email": email }),
        )
        .await
    }

    pub async fn record_logout(
        &self,
        reviewer_id: Uuid,
        request_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        self.record_audit_event(
            Actor::Reviewer(reviewer_id),
            "REVIEWER_LOGOUT",
            Some(reviewer_id),
            request_id,
            json!({}),
        )
        .await
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
