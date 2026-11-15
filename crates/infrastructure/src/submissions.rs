use ferrum_application::{IdempotencyKey, SubmissionError, SubmissionRepository};
use ferrum_contracts::SubmissionCreated;
use ferrum_domain::{ProcessingState, Submission, SubmissionInput, Verdict};
use sqlx::{PgPool, types::Json};
use uuid::Uuid;

use crate::{DatabaseError, log_sql_failure};

#[derive(Clone)]
pub struct PostgresSubmissions {
    pool: PgPool,
}

#[derive(sqlx::FromRow)]
struct SubmissionRow {
    id: Uuid,
    state: String,
    verdict: Option<String>,
}

impl SubmissionRow {
    fn into_domain(self) -> Result<Submission, SubmissionError> {
        let state = ProcessingState::from_storage(&self.state).ok_or(SubmissionError::Storage)?;
        let verdict = self
            .verdict
            .as_deref()
            .map(|verdict| Verdict::from_storage(verdict).ok_or(SubmissionError::Storage))
            .transpose()?;
        Ok(Submission {
            id: self.id,
            state,
            verdict,
        })
    }
}

impl PostgresSubmissions {
    pub async fn is_ready(&self) -> bool {
        crate::database_ready(&self.pool).await
    }

    pub async fn initialize(pool: PgPool, organization_id: Uuid) -> Result<Self, DatabaseError> {
        sqlx::query("INSERT INTO organizations (id) VALUES ($1) ON CONFLICT (id) DO NOTHING")
            .bind(organization_id)
            .execute(&pool)
            .await
            .map_err(|error| {
                log_sql_failure("initialize_tenant", &error);
                DatabaseError::Initialize
            })?;
        Ok(Self { pool })
    }
}

impl SubmissionRepository for PostgresSubmissions {
    async fn create(
        &self,
        organization_id: Uuid,
        key: IdempotencyKey,
        input: SubmissionInput,
    ) -> Result<Submission, SubmissionError> {
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;

        let row = sqlx::query_as::<_, SubmissionRow>(
            "INSERT INTO submissions
                (organization_id, id, idempotency_key, language, runtime_profile, source, stdin, limits)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (organization_id, idempotency_key) DO NOTHING
             RETURNING id, state, verdict",
        )
        .bind(organization_id)
        .bind(Uuid::new_v4())
        .bind(key.as_str())
        .bind(SubmissionInput::LANGUAGE)
        .bind(SubmissionInput::PROFILE)
        .bind(input.source())
        .bind(input.stdin())
        .bind(Json(input.limits()))
        .fetch_optional(&mut *transaction)
        .await
        .map_err(storage_error)?;

        let submission = if let Some(row) = row {
            let event = SubmissionCreated {
                event_id: Uuid::new_v4(),
                organization_id,
                submission_id: row.id,
            };
            sqlx::query(
                "INSERT INTO outbox_events (id, organization_id, submission_id, event_type, event_version)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(event.event_id)
            .bind(event.organization_id)
            .bind(event.submission_id)
            .bind(SubmissionCreated::EVENT_TYPE)
            .bind(SubmissionCreated::VERSION)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
            row
        } else {
            sqlx::query_as::<_, SubmissionRow>(
                "SELECT id, state, verdict FROM submissions
                 WHERE organization_id = $1 AND idempotency_key = $2 AND language = $3
                   AND runtime_profile = $4 AND source COLLATE \"C\" = $5 AND stdin COLLATE \"C\" = $6
                   AND limits = $7",
            )
            .bind(organization_id)
            .bind(key.as_str())
            .bind(SubmissionInput::LANGUAGE)
            .bind(SubmissionInput::PROFILE)
            .bind(input.source())
            .bind(input.stdin())
            .bind(Json(input.limits()))
            .fetch_optional(&mut *transaction)
            .await
            .map_err(storage_error)?
            .ok_or(SubmissionError::IdempotencyConflict)?
        }
        .into_domain()?;

        transaction.commit().await.map_err(storage_error)?;
        Ok(submission)
    }

    async fn get(
        &self,
        organization_id: Uuid,
        submission_id: Uuid,
    ) -> Result<Option<Submission>, SubmissionError> {
        sqlx::query_as::<_, SubmissionRow>(
            "SELECT id, state, verdict FROM submissions WHERE organization_id = $1 AND id = $2",
        )
        .bind(organization_id)
        .bind(submission_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?
        .map(SubmissionRow::into_domain)
        .transpose()
    }
}

fn storage_error(error: sqlx::Error) -> SubmissionError {
    log_sql_failure("submission", &error);
    if matches!(
        &error,
        sqlx::Error::PoolClosed
            | sqlx::Error::PoolTimedOut
            | sqlx::Error::Io(_)
            | sqlx::Error::Tls(_)
    ) || error
        .as_database_error()
        .and_then(|error| error.code())
        .is_some_and(|code| code.starts_with("08") || matches!(code.as_ref(), "57014" | "57P01"))
    {
        SubmissionError::Unavailable
    } else {
        SubmissionError::Storage
    }
}
