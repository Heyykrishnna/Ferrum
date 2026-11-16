use std::{io, sync::Arc, time::Duration};

use apalis::prelude::*;
use ferrum_contracts::ExecuteSubmission;
use ferrum_infrastructure::{
    dispatcher::{dispatch_execution, execution_queue},
    execution::{
        AttemptReport, ExecutionError, ExecutionOwner, claim_execution, finalize_execution,
        renew_attempt_lease,
    },
};
use ferrum_sandbox::{CleanupDisposition, Sandbox, SandboxError};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

const HEARTBEAT: Duration = Duration::from_secs(5);
const OUTBOX_POLL: Duration = Duration::from_millis(500);

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

    let (consumer_res, _) = tokio::join!(
        async {
            let res = consumer.await;
            stop.cancel();
            res
        },
        dispatch,
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
        let Some(input) = claim_execution(&self.pool, job, self.owner).await? else {
            return Ok(());
        };
        tracing::info!(event = "execution_claimed", submission_id = %job.submission_id, attempt_id = %job.attempt_id, worker_id = %self.owner.worker_id);

        let Ok(input) = input.validate() else {
            let result =
                finalize_execution(&self.pool, job, self.owner, AttemptReport::InvalidInput).await;
            if result.is_err() {
                self.failure.cancel();
            }
            return result;
        };
        let cancel = CancellationToken::new();
        let execution = self.sandbox.execute(input, cancel.clone());
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
                    if let Err(error) = renewal {
                        cancel.cancel();
                        execution.await;
                        self.failure.cancel();
                        return Err(error);
                    }
                }
            }
        };

        let blocked = matches!(report.cleanup, CleanupDisposition::Unresolved(_))
            || matches!(
                report.result,
                Err(SandboxError::RecoveryRequired | SandboxError::Busy)
            );
        let result =
            finalize_execution(&self.pool, job, self.owner, AttemptReport::Sandbox(&report)).await;
        if blocked || result.is_err() {
            self.failure.cancel();
        }
        result
    }
}

#[cfg(test)]
mod tests;
