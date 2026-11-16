use ferrum_application::{IdempotencyKey, SubmissionError, SubmissionRepository};
use ferrum_contracts::SubmissionCreated;
use ferrum_domain::{
    ProcessingState, Submission, SubmissionInput, SubmissionResult, SubmissionStageResult, Verdict,
};
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
            result: None,
        })
    }
}

#[derive(sqlx::FromRow)]
struct SubmissionDetailRow {
    id: Uuid,
    state: String,
    verdict: Option<String>,
    #[sqlx(flatten)]
    stage: StoredStage,
    compilation_outcome: Option<String>,
    compilation_stdout: Option<Vec<u8>>,
    compilation_stderr: Option<Vec<u8>>,
    compilation_exit_code: Option<i16>,
    compilation_signal: Option<i16>,
    compilation_cpu_time_ms: Option<i64>,
    compilation_wall_time_ms: Option<i64>,
    compilation_peak_memory_kib: Option<i64>,
}

impl SubmissionDetailRow {
    fn into_domain(self) -> Result<Submission, SubmissionError> {
        let state = ProcessingState::from_storage(&self.state).ok_or(SubmissionError::Storage)?;
        let verdict = self
            .verdict
            .as_deref()
            .map(|verdict| Verdict::from_storage(verdict).ok_or(SubmissionError::Storage))
            .transpose()?;
        let result = self.stage.into_domain().map(|execution| SubmissionResult {
            execution,
            compilation: StoredStage {
                outcome: self.compilation_outcome,
                stdout: self.compilation_stdout,
                stderr: self.compilation_stderr,
                exit_code: self.compilation_exit_code,
                signal: self.compilation_signal,
                cpu_time_ms: self.compilation_cpu_time_ms,
                wall_time_ms: self.compilation_wall_time_ms,
                peak_memory_kib: self.compilation_peak_memory_kib,
            }
            .into_domain(),
        });
        Ok(Submission {
            id: self.id,
            state,
            verdict,
            result,
        })
    }
}

#[derive(sqlx::FromRow)]
struct StoredStage {
    outcome: Option<String>,
    stdout: Option<Vec<u8>>,
    stderr: Option<Vec<u8>>,
    exit_code: Option<i16>,
    signal: Option<i16>,
    cpu_time_ms: Option<i64>,
    wall_time_ms: Option<i64>,
    peak_memory_kib: Option<i64>,
}

impl StoredStage {
    fn into_domain(self) -> Option<SubmissionStageResult> {
        self.outcome.map(|outcome| SubmissionStageResult {
            outcome,
            stdout: String::from_utf8_lossy(&self.stdout.unwrap_or_default()).into_owned(),
            stderr: String::from_utf8_lossy(&self.stderr.unwrap_or_default()).into_owned(),
            exit_code: self.exit_code.and_then(|c| u8::try_from(c).ok()),
            signal: self.signal.and_then(|s| u8::try_from(s).ok()),
            cpu_time_ms: self.cpu_time_ms.and_then(|t| u64::try_from(t).ok()),
            wall_time_ms: self.wall_time_ms.and_then(|t| u64::try_from(t).ok()),
            peak_memory_kib: self.peak_memory_kib.and_then(|m| u64::try_from(m).ok()),
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
        .bind(input.profile().language)
        .bind(input.profile().revision)
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
            .bind(input.profile().language)
            .bind(input.profile().revision)
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
        sqlx::query_as::<_, SubmissionDetailRow>(
            "SELECT
                s.id,
                s.state,
                s.verdict,
                r.outcome,
                r.stdout,
                r.stderr,
                r.exit_code,
                r.signal,
                r.cpu_time_ms,
                r.wall_time_ms,
                r.peak_memory_kib,
                c.outcome AS compilation_outcome,
                c.stdout AS compilation_stdout,
                c.stderr AS compilation_stderr,
                c.exit_code AS compilation_exit_code,
                c.signal AS compilation_signal,
                c.cpu_time_ms AS compilation_cpu_time_ms,
                c.wall_time_ms AS compilation_wall_time_ms,
                c.peak_memory_kib AS compilation_peak_memory_kib
             FROM submissions s
             LEFT JOIN execution_results r
                ON (r.organization_id, r.submission_id) = (s.organization_id, s.id)
             LEFT JOIN compilation_results c ON c.attempt_id = r.attempt_id
             WHERE s.organization_id = $1 AND s.id = $2",
        )
        .bind(organization_id)
        .bind(submission_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?
        .map(SubmissionDetailRow::into_domain)
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
