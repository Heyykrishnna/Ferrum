use std::{io, sync::Arc, time::Duration};

use apalis::prelude::*;
use ferrum_contracts::ExecuteSubmission;
use ferrum_infrastructure::{
    dispatcher::{dispatch_execution, execution_queue, reconcile_stuck_outbox},
    execution::{
        AttemptReport, ExecutionError, ExecutionOwner, claim_execution, finalize_execution,
        reconcile_stale_attempts, renew_attempt_lease,
    },
};
use ferrum_sandbox::{CleanupDisposition, Sandbox, SandboxError};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

const HEARTBEAT: Duration = Duration::from_secs(5);
const OUTBOX_POLL: Duration = Duration::from_millis(500);
const RECONCILE_POLL: Duration = Duration::from_secs(10);
const STUCK_OUTBOX_MAX_AGE: Duration = Duration::from_secs(30);

pub async fn run<S: Sandbox + Send + Sync + 'static>(
    pool: PgPool,
    owner: ExecutionOwner,
    sandbox: S,
    stop: CancellationToken,
) -> Result<(), BoxDynError> {
    let failure = CancellationToken::new();
    let worker = Arc::new(Pipeline {
        pool: pool.clone(),
        owner,
        sandbox,
        stop: stop.clone(),
        failure: failure.clone(),
    });
    let name = format!("ferrum-{}", owner.worker_id);
    let handler = worker.clone();
    let queue = WorkerBuilder::new(name.as_str())
        .backend(execution_queue(&pool))
        .concurrency(1)
        .build(move |job: ExecuteSubmission| handle_job(handler.clone(), job));

    let shutdown = stop.clone();
    let failed = failure.clone();
    let consumer = queue.run_until(async move {
        tokio::select! {
            _ = shutdown.cancelled() => {}
            _ = failed.cancelled() => {}
        }
        Ok::<(), WorkerError>(())
    });

    let dispatch = async {
        loop {
            tokio::select! {
                biased;
                _ = stop.cancelled() => break,
                _ = failure.cancelled() => break,
                result = dispatch_execution(&pool) => match result {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(error) => tracing::warn!(event = "outbox_dispatch_failed", %error),
                },
            }
            tokio::select! {
                _ = stop.cancelled() => break,
                _ = failure.cancelled() => break,
                _ = tokio::time::sleep(OUTBOX_POLL) => {}
            }
        }
        Ok::<(), WorkerError>(())
    };

    let reconcile = async {
        loop {
            tokio::select! {
                biased;
                _ = stop.cancelled() => break,
                _ = failure.cancelled() => break,
                _ = tokio::time::sleep(RECONCILE_POLL) => {}
            }
            match reconcile_stale_attempts(&pool).await {
                Ok(attempts) if !attempts.is_empty() => {
                    tracing::info!(
                        event = "background_reconciled_stale_attempts",
                        count = attempts.len()
                    );
                }
                Ok(_) => {}
                Err(error) => tracing::warn!(event = "stale_attempt_reconcile_failed", %error),
            }
            match reconcile_stuck_outbox(&pool, STUCK_OUTBOX_MAX_AGE).await {
                Ok(count) if count > 0 => {
                    tracing::info!(event = "background_reconciled_stuck_outbox", count);
                }
                Ok(_) => {}
                Err(error) => tracing::warn!(event = "stuck_outbox_reconcile_failed", %error),
            }
        }
        Ok::<(), WorkerError>(())
    };

    let (consumer_res, _, _) = tokio::join!(
        async {
            let res = consumer.await;
            stop.cancel();
            res
        },
        dispatch,
        reconcile,
    );
    consumer_res.map_err(|e| Box::new(e) as BoxDynError)?;
    if failure.is_cancelled() {
        return Err(io::Error::other("worker stopped for execution recovery").into());
    }
    Ok(())
}

async fn handle_job<S: Sandbox + Send + Sync + 'static>(
    worker: Arc<Pipeline<S>>,
    job: ExecuteSubmission,
) -> Result<(), BoxDynError> {
    worker.handle(job).await
}

struct Pipeline<S> {
    pool: PgPool,
    owner: ExecutionOwner,
    sandbox: S,
    stop: CancellationToken,
    failure: CancellationToken,
}

impl<S: Sandbox + Send + Sync + 'static> Pipeline<S> {
    async fn handle(&self, job: ExecuteSubmission) -> Result<(), BoxDynError> {
        if self.stop.is_cancelled() || self.failure.is_cancelled() {
            return Err(ExecutionError::Draining.into());
        }
        if job.version != ExecuteSubmission::VERSION {
            return Err(ExecutionError::UnsupportedVersion.into());
        }

        let result = self.execute(&job).await;
        if let Err(error) = &result {
            tracing::warn!(event = "execution_delivery_failed", submission_id = %job.submission_id, attempt_id = %job.attempt_id, %error);
        }
        result.map_err(Into::into)
    }

    async fn execute(&self, job: &ExecuteSubmission) -> Result<(), ExecutionError> {
        let trace_ctx = ferrum_domain::TraceContext::from_traceparent_or_generate(
            job.traceparent.as_deref(),
        );
        let trace_id = trace_ctx.trace_id.as_str();

        let span = tracing::info_span!(
            "worker.execute",
            trace_id,
            submission_id = %job.submission_id,
            attempt_id = %job.attempt_id,
            worker_id = %self.owner.worker_id
        );
        let _span_guard = span.enter();

        let metrics = ferrum_runtime::MetricsRegistry::global();
        metrics.worker_active_executions.inc(&[]);
        metrics.worker_capacity_reserved_slots.inc(&[]);

        let admission_start = std::time::Instant::now();
        let claim_res = claim_execution(&self.pool, job, self.owner).await;
        metrics
            .stage_duration_seconds
            .observe(&[("stage", "admission")], admission_start.elapsed().as_secs_f64());

        let input = match claim_res? {
            Some(input) => input,
            None => {
                metrics.worker_active_executions.dec(&[]);
                metrics.worker_capacity_reserved_slots.dec(&[]);
                return Ok(());
            }
        };

        tracing::info!(
            event = "execution_claimed",
            trace_id,
            submission_id = %job.submission_id,
            attempt_id = %job.attempt_id,
            worker_id = %self.owner.worker_id
        );

        let validated_input = match input.validate() {
            Ok(valid) => valid,
            Err(_) => {
                let persist_start = std::time::Instant::now();
                let result = finalize_execution(
                    &self.pool,
                    job,
                    self.owner,
                    AttemptReport::InvalidInput,
                )
                .await;
                metrics.stage_duration_seconds.observe(
                    &[("stage", "persistence")],
                    persist_start.elapsed().as_secs_f64(),
                );
                metrics.worker_active_executions.dec(&[]);
                metrics.worker_capacity_reserved_slots.dec(&[]);
                if result.is_err() {
                    self.failure.cancel();
                }
                return result;
            }
        };

        let cancel = CancellationToken::new();
        let exec_start = std::time::Instant::now();
        let execution = self.sandbox.execute(validated_input, cancel.clone());
        tokio::pin!(execution);
        let mut heartbeat =
            tokio::time::interval_at(tokio::time::Instant::now() + HEARTBEAT, HEARTBEAT);

        let report = loop {
            tokio::select! {
                biased;
                report = &mut execution => break report,
                _ = heartbeat.tick() => {
                    let renewal = tokio::time::timeout(HEARTBEAT, renew_attempt_lease(&self.pool, job, self.owner))
                        .await.unwrap_or(Err(ExecutionError::Lease));
                    match renewal {
                        Ok(true) => {}
                        Ok(false) => {
                            tracing::info!(
                                event = "execution_cancellation_requested",
                                trace_id,
                                submission_id = %job.submission_id,
                                attempt_id = %job.attempt_id
                            );
                            cancel.cancel();
                        }
                        Err(error) => {
                            cancel.cancel();
                            execution.await;
                            self.failure.cancel();
                            metrics.worker_active_executions.dec(&[]);
                            metrics.worker_capacity_reserved_slots.dec(&[]);
                            return Err(error);
                        }
                    }
                }
            }
        };

        let exec_duration = exec_start.elapsed().as_secs_f64();
        metrics
            .stage_duration_seconds
            .observe(&[("stage", "execute")], exec_duration);

        match &report.cleanup {
            CleanupDisposition::Verified => {
                metrics
                    .sandbox_cleanups_total
                    .inc(&[("disposition", "verified")]);
            }
            CleanupDisposition::NotNeeded => {
                metrics
                    .sandbox_cleanups_total
                    .inc(&[("disposition", "not_needed")]);
            }
            CleanupDisposition::Unresolved(_) => {
                metrics
                    .sandbox_cleanups_total
                    .inc(&[("disposition", "unresolved")]);
                metrics.sandbox_quarantines_total.inc(&[]);
                metrics.worker_capacity_quarantined_slots.inc(&[]);
            }
        }

        if let Ok(run) = &report.result {
            if let Some(cpu) = run.facts.cpu_time_ms {
                metrics.execution_cpu_time_ms.observe(&[], cpu as f64);
            }
            if let Some(wall) = run.facts.wall_time_ms {
                metrics.execution_wall_time_ms.observe(&[], wall as f64);
            }
            if let Some(mem) = run.facts.peak_memory_kib {
                metrics.execution_peak_memory_kib.observe(&[], mem as f64);
            }
            match run.outcome {
                ferrum_sandbox::ProgramOutcome::CpuTimeLimit => {
                    metrics
                        .execution_limit_violations_total
                        .inc(&[("limit_type", "cpu_time")]);
                }
                ferrum_sandbox::ProgramOutcome::WallTimeLimit => {
                    metrics
                        .execution_limit_violations_total
                        .inc(&[("limit_type", "wall_time")]);
                }
                ferrum_sandbox::ProgramOutcome::MemoryLimit => {
                    metrics
                        .execution_limit_violations_total
                        .inc(&[("limit_type", "memory")]);
                }
                ferrum_sandbox::ProgramOutcome::OutputLimit(ferrum_sandbox::OutputStream::Stdout) => {
                    metrics
                        .execution_limit_violations_total
                        .inc(&[("limit_type", "stdout")]);
                }
                ferrum_sandbox::ProgramOutcome::OutputLimit(ferrum_sandbox::OutputStream::Stderr) => {
                    metrics
                        .execution_limit_violations_total
                        .inc(&[("limit_type", "stderr")]);
                }
                _ => {}
            }
        }

        let blocked = matches!(report.cleanup, CleanupDisposition::Unresolved(_))
            || matches!(
                report.result,
                Err(SandboxError::RecoveryRequired | SandboxError::Busy)
            );

        let persist_start = std::time::Instant::now();
        let result =
            finalize_execution(&self.pool, job, self.owner, AttemptReport::Sandbox(&report)).await;
        metrics.stage_duration_seconds.observe(
            &[("stage", "persistence")],
            persist_start.elapsed().as_secs_f64(),
        );

        metrics.worker_active_executions.dec(&[]);
        metrics.worker_capacity_reserved_slots.dec(&[]);

        if blocked || result.is_err() {
            self.failure.cancel();
        }
        result
    }
}

#[cfg(test)]
mod tests;
