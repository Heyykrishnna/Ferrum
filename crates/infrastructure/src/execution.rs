use ferrum_contracts::{ExecuteSubmission, SubmissionCompleted, SubmissionCreated};
use ferrum_domain::{
    ComparisonMode, EffectiveLimits, ProcessingState, QuarantinedAttempt, ReconciledAttempt,
    StaleAttemptAction, SubmissionInput, TestCase, ValidationError, Verdict,
};
use ferrum_sandbox::{
    CleanupDisposition, ExecutionReport, ExecutionResult, ProgramOutcome, SandboxError,
};
use sqlx::{PgPool, Postgres, Transaction, types::Json};
use thiserror::Error;
use uuid::Uuid;

use super::{dispatcher::RETRY_EVENT, log_sql_failure};

const MAX_ATTEMPTS: i32 = 3;

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("could not update execution state in PostgreSQL")]
    Storage,
    #[error("could not use the Apalis PostgreSQL queue")]
    Queue,
    #[error("execution attempt lease was lost")]
    Lease,
    #[error("worker is draining")]
    Draining,
    #[error("unsupported execution job version")]
    UnsupportedVersion,
}

#[derive(Clone, Copy)]
pub struct ExecutionOwner {
    pub host_id: Uuid,
    pub worker_id: Uuid,
}

#[derive(sqlx::FromRow)]
struct StoredExecutionInput {
    language: String,
    runtime_profile: String,
    source: String,
    stdin: String,
    limits: Json<EffectiveLimits>,
}

#[derive(sqlx::FromRow)]
struct StoredTestCase {
    case_index: i32,
    input: String,
    expected_output: String,
    weight: i32,
    hidden: bool,
    comparison_mode: String,
}

pub struct ExecutionInput {
    language: String,
    runtime_profile: String,
    source: String,
    stdin: String,
    limits: Json<EffectiveLimits>,
    cases: Vec<TestCase>,
}

impl ExecutionInput {
    pub fn validate(self) -> Result<SubmissionInput, ValidationError> {
        SubmissionInput::from_persisted_with_cases(
            &self.language,
            &self.runtime_profile,
            self.source,
            self.stdin,
            self.limits.0,
            self.cases,
        )
    }
}

pub async fn claim_execution(
    pool: &PgPool,
    job: &ExecuteSubmission,
    owner: ExecutionOwner,
) -> Result<Option<ExecutionInput>, ExecutionError> {
    if job.version != ExecuteSubmission::VERSION {
        return Ok(None);
    }
    let mut tx = pool.begin().await.map_err(execution_storage)?;
    let claimed = sqlx::query_as::<_, StoredExecutionInput>(
        "UPDATE execution_attempts AS a
         SET state = 'running', host_id = $4, worker_id = $5,
             lease_expires_at = clock_timestamp() + interval '30 seconds', started_at = now()
         FROM submissions AS s
         WHERE a.organization_id = $1 AND a.submission_id = $2 AND a.id = $3 AND a.state = 'queued'
           AND s.organization_id = a.organization_id AND s.id = a.submission_id AND s.state = 'queued'
         RETURNING s.language, s.runtime_profile, s.source, s.stdin, s.limits",
    )
    .bind(job.organization_id)
    .bind(job.submission_id)
    .bind(job.attempt_id)
    .bind(owner.host_id)
    .bind(owner.worker_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(execution_storage)?;

    let input = if let Some(stored) = claimed {
        set_submission_state(&mut tx, job, ProcessingState::Processing, None).await?;
        let stored_cases = sqlx::query_as::<_, StoredTestCase>(
            "SELECT case_index, input, expected_output, weight, hidden, comparison_mode
             FROM submission_test_cases
             WHERE organization_id = $1 AND submission_id = $2
             ORDER BY case_index ASC",
        )
        .bind(job.organization_id)
        .bind(job.submission_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(execution_storage)?;

        let cases = stored_cases
            .into_iter()
            .map(|c| TestCase {
                index: c.case_index as u32,
                input: c.input,
                expected_output: c.expected_output,
                weight: c.weight as u32,
                hidden: c.hidden,
                comparison_mode: c.comparison_mode.parse().unwrap_or(ComparisonMode::Exact),
            })
            .collect();

        Some(ExecutionInput {
            language: stored.language,
            runtime_profile: stored.runtime_profile,
            source: stored.source,
            stdin: stored.stdin,
            limits: stored.limits,
            cases,
        })
    } else {
        None
    };

    tx.commit().await.map_err(execution_storage)?;
    Ok(input)
}

pub async fn renew_attempt_lease(
    pool: &PgPool,
    job: &ExecuteSubmission,
    owner: ExecutionOwner,
) -> Result<bool, ExecutionError> {
    let updated = sqlx::query(
        "UPDATE execution_attempts SET lease_expires_at = clock_timestamp() + interval '30 seconds'
         WHERE organization_id = $1 AND submission_id = $2 AND id = $3 AND worker_id = $4
           AND state = 'running' AND lease_expires_at > clock_timestamp()",
    )
    .bind(job.organization_id)
    .bind(job.submission_id)
    .bind(job.attempt_id)
    .bind(owner.worker_id)
    .execute(pool)
    .await
    .map_err(execution_storage)?;
    if updated.rows_affected() != 1 {
        return Err(ExecutionError::Lease);
    }

    let cancel_requested = sqlx::query_scalar::<_, bool>(
        "SELECT cancel_requested FROM submissions WHERE organization_id = $1 AND id = $2",
    )
    .bind(job.organization_id)
    .bind(job.submission_id)
    .fetch_optional(pool)
    .await
    .map_err(execution_storage)?
    .unwrap_or(false);

    Ok(!cancel_requested)
}

pub enum AttemptReport<'a> {
    Sandbox(&'a ExecutionReport),
    InvalidInput,
}

pub async fn finalize_execution(
    pool: &PgPool,
    job: &ExecuteSubmission,
    owner: ExecutionOwner,
    report: AttemptReport<'_>,
) -> Result<(), ExecutionError> {
    let (result, failure, cleanup, retry) = match report {
        AttemptReport::Sandbox(report) => {
            let failure = report.result.as_ref().err();
            (
                report.result.as_ref().ok(),
                failure.map(sandbox_failure),
                report.cleanup,
                matches!(failure, Some(SandboxError::Initialize))
                    && matches!(report.cleanup, CleanupDisposition::Verified),
            )
        }
        AttemptReport::InvalidInput => (
            None,
            Some("invalid_persisted_input"),
            CleanupDisposition::NotNeeded,
            false,
        ),
    };

    let (state, verdict) = result.map_or(
        (ProcessingState::Failed, Some(Verdict::InternalError)),
        |result| {
            if let Some(cases) = &result.cases {
                let verdicts: Vec<Verdict> = cases.iter().map(|c| c.verdict).collect();
                let verdict = ferrum_judge::aggregate_verdict(&verdicts);
                (ProcessingState::Completed, Some(verdict))
            } else {
                match result.outcome {
                    ProgramOutcome::Success => (ProcessingState::Completed, None),
                    ProgramOutcome::CompilationError => {
                        (ProcessingState::Completed, Some(Verdict::CompilationError))
                    }
                    ProgramOutcome::RuntimeError => {
                        (ProcessingState::Completed, Some(Verdict::RuntimeError))
                    }
                    ProgramOutcome::CpuTimeLimit | ProgramOutcome::WallTimeLimit => {
                        (ProcessingState::Completed, Some(Verdict::TimeLimitExceeded))
                    }
                    ProgramOutcome::MemoryLimit => (
                        ProcessingState::Completed,
                        Some(Verdict::MemoryLimitExceeded),
                    ),
                    ProgramOutcome::OutputLimit(_) => (
                        ProcessingState::Completed,
                        Some(Verdict::OutputLimitExceeded),
                    ),
                    ProgramOutcome::Cancelled => (ProcessingState::Cancelled, None),
                }
            }
        },
    );
    let (cleanup, cleanup_error) = match cleanup {
        CleanupDisposition::NotNeeded => ("not_needed", None),
        CleanupDisposition::Verified => ("verified", None),
        CleanupDisposition::Unresolved(error) => ("unresolved", Some(error.to_string())),
    };

    let mut tx = pool.begin().await.map_err(execution_storage)?;
    let attempt_number = sqlx::query_scalar::<_, i32>(
        "UPDATE execution_attempts
         SET state = $5, cleanup_disposition = $6, cleanup_error = $7,
             failure_reason = $8, completed_at = now()
         WHERE organization_id = $1 AND submission_id = $2 AND id = $3 AND worker_id = $4
           AND state = 'running' AND lease_expires_at > clock_timestamp()
         RETURNING attempt_number",
    )
    .bind(job.organization_id)
    .bind(job.submission_id)
    .bind(job.attempt_id)
    .bind(owner.worker_id)
    .bind(state.as_storage())
    .bind(cleanup)
    .bind(cleanup_error)
    .bind(failure)
    .fetch_optional(&mut *tx)
    .await
    .map_err(execution_storage)?
    .ok_or(ExecutionError::Lease)?;

    if let Some(result) = result {
        persist_result(&mut tx, job, result).await?;
    }
    finish_or_retry(
        &mut tx,
        job,
        state,
        verdict,
        retry && attempt_number < MAX_ATTEMPTS,
    )
    .await?;
    tx.commit().await.map_err(execution_storage)?;
    tracing::info!(event = "execution_finalized", submission_id = %job.submission_id, attempt_id = %job.attempt_id, attempt_number, state = state.as_storage(), cleanup, failure);
    Ok(())
}

pub async fn recover_after_host_probe(
    pool: &PgPool,
    owner: ExecutionOwner,
) -> Result<(), ExecutionError> {
    loop {
        let mut tx = pool.begin().await.map_err(execution_storage)?;
        let previous = sqlx::query_as::<_, (Uuid, Uuid, Uuid, i32)>(
            "UPDATE execution_attempts SET state = 'failed', cleanup_disposition = 'verified',
                 failure_reason = 'worker_lost', completed_at = now()
             WHERE id = (
                 SELECT id FROM execution_attempts
                 WHERE host_id = $1 AND worker_id <> $2 AND state = 'running'
                 ORDER BY id LIMIT 1 FOR UPDATE
             )
             RETURNING organization_id, submission_id, id, attempt_number",
        )
        .bind(owner.host_id)
        .bind(owner.worker_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(execution_storage)?;
        let Some((organization_id, submission_id, attempt_id, number)) = previous else {
            break;
        };
        let job = ExecuteSubmission {
            version: ExecuteSubmission::VERSION,
            organization_id,
            submission_id,
            attempt_id,
        };
        finish_or_retry(
            &mut tx,
            &job,
            ProcessingState::Failed,
            Some(Verdict::InternalError),
            number < MAX_ATTEMPTS,
        )
        .await?;
        tx.commit().await.map_err(execution_storage)?;
        tracing::info!(event = "execution_recovered", %submission_id, %attempt_id, attempt_number = number);
    }

    let recovered = sqlx::query(
        "UPDATE execution_attempts SET cleanup_recovered_at = now()
         WHERE host_id = $1 AND cleanup_disposition = 'unresolved' AND cleanup_recovered_at IS NULL",
    )
    .bind(owner.host_id)
    .execute(pool)
    .await
    .map_err(execution_storage)?;
    if recovered.rows_affected() != 0 {
        tracing::info!(
            event = "cleanup_recovered",
            attempts = recovered.rows_affected()
        );
    }
    Ok(())
}

pub async fn reconcile_stale_attempts(
    pool: &PgPool,
) -> Result<Vec<ReconciledAttempt>, ExecutionError> {
    let mut reconciled = Vec::new();
    loop {
        let mut tx = pool.begin().await.map_err(execution_storage)?;
        let row = sqlx::query_as::<_, (Uuid, Uuid, Uuid, i32)>(
            "SELECT a.organization_id, a.submission_id, a.id, a.attempt_number
             FROM execution_attempts a
             WHERE a.state = 'running' AND a.lease_expires_at <= clock_timestamp()
             ORDER BY a.lease_expires_at ASC
             LIMIT 1 FOR UPDATE SKIP LOCKED",
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(execution_storage)?;

        let Some((organization_id, submission_id, attempt_id, attempt_number)) = row else {
            break;
        };

        sqlx::query(
            "UPDATE execution_attempts
             SET state = 'failed', failure_reason = 'lease_expired', completed_at = now()
             WHERE id = $1",
        )
        .bind(attempt_id)
        .execute(&mut *tx)
        .await
        .map_err(execution_storage)?;

        let cancel_requested = sqlx::query_scalar::<_, bool>(
            "SELECT cancel_requested FROM submissions WHERE organization_id = $1 AND id = $2",
        )
        .bind(organization_id)
        .bind(submission_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(execution_storage)?
        .unwrap_or(false);

        let job = ExecuteSubmission {
            version: ExecuteSubmission::VERSION,
            organization_id,
            submission_id,
            attempt_id,
        };

        let action = if cancel_requested {
            finish_or_retry(&mut tx, &job, ProcessingState::Cancelled, None, false).await?;
            StaleAttemptAction::MarkedFailed
        } else if attempt_number < MAX_ATTEMPTS {
            finish_or_retry(
                &mut tx,
                &job,
                ProcessingState::Failed,
                Some(Verdict::InternalError),
                true,
            )
            .await?;
            StaleAttemptAction::Requeued
        } else {
            finish_or_retry(
                &mut tx,
                &job,
                ProcessingState::Failed,
                Some(Verdict::InternalError),
                false,
            )
            .await?;
            StaleAttemptAction::MarkedFailed
        };

        tx.commit().await.map_err(execution_storage)?;

        tracing::warn!(
            event = "stale_attempt_reconciled",
            %organization_id,
            %submission_id,
            %attempt_id,
            attempt_number,
            action = action.as_str()
        );

        reconciled.push(ReconciledAttempt {
            submission_id,
            attempt_id,
            action,
            attempt_number: attempt_number.max(0) as u32,
        });

        if reconciled.len() >= 100 {
            break;
        }
    }

    Ok(reconciled)
}

#[derive(sqlx::FromRow)]
struct StoredQuarantinedAttempt {
    organization_id: Uuid,
    submission_id: Uuid,
    id: Uuid,
    worker_id: Option<Uuid>,
    failure_reason: Option<String>,
    cleanup_error: Option<String>,
    created_at: Option<i64>,
}

pub async fn list_quarantined_attempts(
    pool: &PgPool,
) -> Result<Vec<QuarantinedAttempt>, ExecutionError> {
    let rows = sqlx::query_as::<_, StoredQuarantinedAttempt>(
        "SELECT organization_id, submission_id, id, worker_id, failure_reason, cleanup_error,
                COALESCE(EXTRACT(EPOCH FROM started_at)::bigint, 0) AS created_at
         FROM execution_attempts
         WHERE cleanup_disposition = 'unresolved' AND cleanup_recovered_at IS NULL
         ORDER BY started_at ASC, id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(execution_storage)?;

    Ok(rows
        .into_iter()
        .map(|row| QuarantinedAttempt {
            organization_id: row.organization_id,
            submission_id: row.submission_id,
            attempt_id: row.id,
            worker_id: row.worker_id.unwrap_or_default(),
            failure_reason: row.failure_reason,
            cleanup_error: row.cleanup_error,
            created_at: row.created_at.unwrap_or_default(),
        })
        .collect())
}

pub async fn resolve_quarantine(pool: &PgPool, attempt_id: Uuid) -> Result<bool, ExecutionError> {
    let updated = sqlx::query(
        "UPDATE execution_attempts
         SET cleanup_recovered_at = now()
         WHERE id = $1 AND cleanup_disposition = 'unresolved' AND cleanup_recovered_at IS NULL",
    )
    .bind(attempt_id)
    .execute(pool)
    .await
    .map_err(execution_storage)?;

    let resolved = updated.rows_affected() == 1;
    if resolved {
        tracing::info!(event = "quarantine_resolved", %attempt_id);
    }
    Ok(resolved)
}

pub async fn resolve_all_quarantines_for_host(
    pool: &PgPool,
    host_id: Uuid,
) -> Result<u64, ExecutionError> {
    let updated = sqlx::query(
        "UPDATE execution_attempts
         SET cleanup_recovered_at = now()
         WHERE host_id = $1 AND cleanup_disposition = 'unresolved' AND cleanup_recovered_at IS NULL",
    )
    .bind(host_id)
    .execute(pool)
    .await
    .map_err(execution_storage)?;

    let count = updated.rows_affected();
    if count > 0 {
        tracing::info!(event = "host_quarantines_resolved", %host_id, count);
    }
    Ok(count)
}

async fn set_submission_state(
    tx: &mut Transaction<'_, Postgres>,
    job: &ExecuteSubmission,
    state: ProcessingState,
    verdict: Option<Verdict>,
) -> Result<(), ExecutionError> {
    sqlx::query(
        "UPDATE submissions SET state = $3, verdict = $4, updated_at = now()
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(job.organization_id)
    .bind(job.submission_id)
    .bind(state.as_storage())
    .bind(verdict.map(Verdict::as_storage))
    .execute(&mut **tx)
    .await
    .map_err(execution_storage)?;
    Ok(())
}

async fn finish_or_retry(
    tx: &mut Transaction<'_, Postgres>,
    job: &ExecuteSubmission,
    state: ProcessingState,
    verdict: Option<Verdict>,
    retry: bool,
) -> Result<(), ExecutionError> {
    let (state, verdict, event, version) = if retry {
        (
            ProcessingState::Queued,
            None,
            RETRY_EVENT,
            SubmissionCreated::VERSION,
        )
    } else {
        (
            state,
            verdict,
            SubmissionCompleted::EVENT_TYPE,
            SubmissionCompleted::VERSION,
        )
    };
    set_submission_state(tx, job, state, verdict).await?;
    sqlx::query(
        "INSERT INTO outbox_events (id, organization_id, submission_id, event_type, event_version)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(job.organization_id)
    .bind(job.submission_id)
    .bind(event)
    .bind(version)
    .execute(&mut **tx)
    .await
    .map_err(execution_storage)?;
    Ok(())
}

async fn persist_result(
    tx: &mut Transaction<'_, Postgres>,
    job: &ExecuteSubmission,
    result: &ExecutionResult,
) -> Result<(), ExecutionError> {
    let measurement = |value: Option<u64>| {
        value
            .map(i64::try_from)
            .transpose()
            .map_err(|_| ExecutionError::Storage)
    };

    sqlx::query(
        "INSERT INTO execution_results (organization_id, submission_id, attempt_id, outcome,
            stdout, stderr, exit_code, signal, cpu_time_ms, wall_time_ms, peak_memory_kib, score, max_score)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(job.organization_id)
    .bind(job.submission_id)
    .bind(job.attempt_id)
    .bind(result.outcome.as_str())
    .bind(&result.stdout)
    .bind(&result.stderr)
    .bind(result.facts.exit_code.map(i16::from))
    .bind(result.facts.signal.map(i16::from))
    .bind(measurement(result.facts.cpu_time_ms)?)
    .bind(measurement(result.facts.wall_time_ms)?)
    .bind(measurement(result.facts.peak_memory_kib)?)
    .bind(result.score.map(|s| s as i32))
    .bind(result.max_score.map(|m| m as i32))
    .execute(&mut **tx)
    .await
    .map_err(execution_storage)?;

    if let Some(stage) = &result.compilation {
        sqlx::query(
            "INSERT INTO compilation_results (attempt_id, outcome, stdout, stderr,
                exit_code, signal, cpu_time_ms, wall_time_ms, peak_memory_kib)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(job.attempt_id)
        .bind(stage.outcome.as_str())
        .bind(&stage.stdout)
        .bind(&stage.stderr)
        .bind(stage.facts.exit_code.map(i16::from))
        .bind(stage.facts.signal.map(i16::from))
        .bind(measurement(stage.facts.cpu_time_ms)?)
        .bind(measurement(stage.facts.wall_time_ms)?)
        .bind(measurement(stage.facts.peak_memory_kib)?)
        .execute(&mut **tx)
        .await
        .map_err(execution_storage)?;
    }

    if let Some(cases) = &result.cases {
        for case in cases {
            sqlx::query(
                "INSERT INTO submission_case_results (attempt_id, case_index, verdict, score, max_score,
                    stdout, stderr, exit_code, signal, cpu_time_ms, wall_time_ms, peak_memory_kib)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
            )
            .bind(job.attempt_id)
            .bind(case.index as i32)
            .bind(case.verdict.as_storage())
            .bind(case.score as i32)
            .bind(case.max_score as i32)
            .bind(case.stdout.as_deref().unwrap_or_default().as_bytes())
            .bind(case.stderr.as_deref().unwrap_or_default().as_bytes())
            .bind(case.exit_code.map(i16::from))
            .bind(case.signal.map(i16::from))
            .bind(measurement(case.cpu_time_ms)?)
            .bind(measurement(case.wall_time_ms)?)
            .bind(measurement(case.peak_memory_kib)?)
            .execute(&mut **tx)
            .await
            .map_err(execution_storage)?;
        }
    }

    Ok(())
}

fn sandbox_failure(error: &SandboxError) -> &'static str {
    match error {
        SandboxError::Busy => "busy",
        SandboxError::RecoveryRequired => "recovery_required",
        SandboxError::Initialize => "initialize",
        SandboxError::Stage => "stage",
        SandboxError::Supervisor => "supervisor",
        SandboxError::Metadata => "metadata",
    }
}

pub(super) fn execution_storage(error: sqlx::Error) -> ExecutionError {
    log_sql_failure("execution", &error);
    ExecutionError::Storage
}
