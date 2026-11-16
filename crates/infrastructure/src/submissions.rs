use ferrum_application::{IdempotencyKey, SubmissionError, SubmissionRepository};
use ferrum_contracts::SubmissionCreated;
use ferrum_domain::{
    BatchItemResult, BatchSubmissionResult, CaseContent, CaseFinding, CaseTelemetry,
    ProcessingState, Submission, SubmissionCancelOutcome, SubmissionInput, SubmissionResult,
    SubmissionStageResult, Verdict, MAX_BATCH_SIZE,
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
    attempt_id: Option<Uuid>,
    score: Option<i32>,
    max_score: Option<i32>,
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
    fn into_domain(self, cases: Option<Vec<CaseFinding>>) -> Result<Submission, SubmissionError> {
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
            score: self.score.and_then(|s| u32::try_from(s).ok()),
            max_score: self.max_score.and_then(|m| u32::try_from(m).ok()),
            cases,
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
struct CaseResultRow {
    case_index: i32,
    verdict: String,
    score: i32,
    max_score: i32,
    hidden: Option<bool>,
    input: Option<String>,
    expected_output: Option<String>,
    stdout: Option<Vec<u8>>,
    stderr: Option<Vec<u8>>,
    exit_code: Option<i16>,
    signal: Option<i16>,
    cpu_time_ms: Option<i64>,
    wall_time_ms: Option<i64>,
    peak_memory_kib: Option<i64>,
}

impl CaseResultRow {
    fn into_domain(self) -> Option<CaseFinding> {
        let verdict = Verdict::from_storage(&self.verdict)?;
        let score = u32::try_from(self.score).ok()?;
        let max_score = u32::try_from(self.max_score).ok()?;
        let index = u32::try_from(self.case_index).ok()?;
        let cpu_time_ms = self.cpu_time_ms.and_then(|t| u64::try_from(t).ok());
        let wall_time_ms = self.wall_time_ms.and_then(|t| u64::try_from(t).ok());
        let peak_memory_kib = self.peak_memory_kib.and_then(|m| u64::try_from(m).ok());
        let hidden = self.hidden.unwrap_or(false);
        let telemetry = CaseTelemetry {
            exit_code: self.exit_code.and_then(|c| u8::try_from(c).ok()),
            signal: self.signal.and_then(|s| u8::try_from(s).ok()),
            cpu_time_ms,
            wall_time_ms,
            peak_memory_kib,
        };

        if hidden {
            Some(CaseFinding::hidden(
                index, verdict, score, max_score, telemetry,
            ))
        } else {
            let stdout = String::from_utf8_lossy(&self.stdout.unwrap_or_default()).into_owned();
            let stderr = String::from_utf8_lossy(&self.stderr.unwrap_or_default()).into_owned();
            Some(CaseFinding::public(
                index,
                verdict,
                score,
                max_score,
                CaseContent {
                    input: self.input.unwrap_or_default(),
                    expected_output: self.expected_output.unwrap_or_default(),
                    stdout,
                    stderr,
                },
                telemetry,
            ))
        }
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
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

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

    pub async fn check_tenant_concurrency(
        &self,
        organization_id: Uuid,
        max_active: i64,
    ) -> Result<bool, SubmissionError> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM submissions
             WHERE organization_id = $1 AND state IN ('created', 'queued', 'processing')",
        )
        .bind(organization_id)
        .fetch_one(&self.pool)
        .await
        .map_err(storage_error)?;

        Ok(count.0 < max_active)
    }

    pub async fn cancel(
        &self,
        organization_id: Uuid,
        submission_id: Uuid,
    ) -> Result<Option<SubmissionCancelOutcome>, SubmissionError> {
        let mut tx = self.pool.begin().await.map_err(storage_error)?;
        let row = sqlx::query_as::<_, (String, bool)>(
            "SELECT state, cancel_requested FROM submissions
             WHERE organization_id = $1 AND id = $2 FOR UPDATE",
        )
        .bind(organization_id)
        .bind(submission_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage_error)?;

        let Some((state, cancel_requested)) = row else {
            return Ok(None);
        };

        let outcome = match state.as_str() {
            "created" | "queued" => {
                sqlx::query(
                    "UPDATE submissions
                     SET state = 'cancelled', cancel_requested = true, updated_at = clock_timestamp()
                     WHERE organization_id = $1 AND id = $2",
                )
                .bind(organization_id)
                .bind(submission_id)
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;

                let event_id = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO outbox_events (id, organization_id, submission_id, event_type, event_version)
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(event_id)
                .bind(organization_id)
                .bind(submission_id)
                .bind("submission.cancelled")
                .bind(1i16)
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;

                SubmissionCancelOutcome::Cancelled
            }
            "processing" => {
                if !cancel_requested {
                    sqlx::query(
                        "UPDATE submissions
                         SET cancel_requested = true, updated_at = clock_timestamp()
                         WHERE organization_id = $1 AND id = $2",
                    )
                    .bind(organization_id)
                    .bind(submission_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage_error)?;
                }
                SubmissionCancelOutcome::CancellationRequested
            }
            _ => SubmissionCancelOutcome::AlreadyTerminal,
        };

        tx.commit().await.map_err(storage_error)?;
        Ok(Some(outcome))
    }

    pub async fn create_batch(
        &self,
        organization_id: Uuid,
        items: Vec<(IdempotencyKey, SubmissionInput)>,
    ) -> Result<BatchSubmissionResult, SubmissionError> {
        if items.len() > MAX_BATCH_SIZE {
            return Err(SubmissionError::BatchLimitExceeded);
        }

        let mut results = Vec::with_capacity(items.len());
        for (key, input) in items {
            let key_str = key.as_str().to_string();
            match self.create(organization_id, key, input).await {
                Ok(sub) => {
                    results.push(BatchItemResult {
                        idempotency_key: key_str,
                        id: Some(sub.id),
                        state: Some(sub.state),
                        error: None,
                    });
                }
                Err(SubmissionError::IdempotencyConflict) => {
                    results.push(BatchItemResult {
                        idempotency_key: key_str,
                        id: None,
                        state: None,
                        error: Some("idempotency_conflict".to_string()),
                    });
                }
                Err(SubmissionError::Unavailable) => {
                    return Err(SubmissionError::Unavailable);
                }
                Err(err) => {
                    results.push(BatchItemResult {
                        idempotency_key: key_str,
                        id: None,
                        state: None,
                        error: Some(err.to_string()),
                    });
                }
            }
        }

        Ok(BatchSubmissionResult { items: results })
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
            for case in input.cases() {
                sqlx::query(
                    "INSERT INTO submission_test_cases
                        (organization_id, submission_id, case_index, input, expected_output, weight, hidden, comparison_mode)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                )
                .bind(organization_id)
                .bind(row.id)
                .bind(case.index as i32)
                .bind(&case.input)
                .bind(&case.expected_output)
                .bind(case.weight as i32)
                .bind(case.hidden)
                .bind(case.comparison_mode.as_str())
                .execute(&mut *transaction)
                .await
                .map_err(storage_error)?;
            }
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
            let existing = sqlx::query_as::<_, SubmissionRow>(
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
            .ok_or(SubmissionError::IdempotencyConflict)?;

            let existing_cases = sqlx::query_as::<_, (i32, String, String, i32, bool, String)>(
                "SELECT case_index, input, expected_output, weight, hidden, comparison_mode
                 FROM submission_test_cases
                 WHERE organization_id = $1 AND submission_id = $2
                 ORDER BY case_index ASC",
            )
            .bind(organization_id)
            .bind(existing.id)
            .fetch_all(&mut *transaction)
            .await
            .map_err(storage_error)?;

            let cases_match = existing_cases.len() == input.cases().len()
                && existing_cases.iter().zip(input.cases().iter()).all(
                    |(stored, input_case)| {
                        stored.0 == input_case.index as i32
                            && stored.1 == input_case.input
                            && stored.2 == input_case.expected_output
                            && stored.3 == input_case.weight as i32
                            && stored.4 == input_case.hidden
                            && stored.5 == input_case.comparison_mode.as_str()
                    },
                );

            if !cases_match {
                return Err(SubmissionError::IdempotencyConflict);
            }
            existing
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
        let detail = sqlx::query_as::<_, SubmissionDetailRow>(
            "SELECT
                s.id,
                s.state,
                s.verdict,
                r.attempt_id,
                r.score,
                r.max_score,
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
        .map_err(storage_error)?;

        let Some(detail) = detail else {
            return Ok(None);
        };

        let cases = if let Some(attempt_id) = detail.attempt_id {
            let case_rows = sqlx::query_as::<_, CaseResultRow>(
                "SELECT
                    c.case_index,
                    c.verdict,
                    c.score,
                    c.max_score,
                    t.hidden,
                    t.input,
                    t.expected_output,
                    c.stdout,
                    c.stderr,
                    c.exit_code,
                    c.signal,
                    c.cpu_time_ms,
                    c.wall_time_ms,
                    c.peak_memory_kib
                 FROM submission_case_results c
                 JOIN execution_results r ON r.attempt_id = c.attempt_id
                 LEFT JOIN submission_test_cases t
                    ON (t.organization_id, t.submission_id, t.case_index) = (r.organization_id, r.submission_id, c.case_index)
                 WHERE c.attempt_id = $1
                 ORDER BY c.case_index ASC",
            )
            .bind(attempt_id)
            .fetch_all(&self.pool)
            .await
            .map_err(storage_error)?;

            if case_rows.is_empty() {
                None
            } else {
                let parsed: Vec<CaseFinding> = case_rows
                    .into_iter()
                    .filter_map(CaseResultRow::into_domain)
                    .collect();
                Some(parsed)
            }
        } else {
            None
        };

        detail.into_domain(cases).map(Some)
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
