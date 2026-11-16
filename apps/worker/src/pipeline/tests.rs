use std::sync::atomic::{AtomicUsize, Ordering};

use ferrum_application::{IdempotencyKey, SubmissionRepository};
use ferrum_config::DatabaseSettings;
use ferrum_domain::{ProcessingState, SubmissionInput, Verdict};
use ferrum_infrastructure::{PostgresSubmissions, connect, execution::recover_after_host_probe};
use ferrum_sandbox::{
    CleanupError, ExecutionFacts, ExecutionReport, ExecutionResult, ProgramOutcome, StageResult,
};
use tokio::sync::Notify;
use uuid::Uuid;

use super::*;

fn owner() -> ExecutionOwner {
    ExecutionOwner {
        host_id: Uuid::new_v4(),
        worker_id: Uuid::new_v4(),
    }
}

struct Fixture {
    pool: PgPool,
    organization_id: Uuid,
    repository: PostgresSubmissions,
}

impl Fixture {
    async fn new() -> Self {
        let settings = DatabaseSettings::load().expect("use disposable PostgreSQL");
        let pool = connect(&settings).await.unwrap();
        let organization_id = Uuid::new_v4();
        let repository = PostgresSubmissions::initialize(pool.clone(), organization_id)
            .await
            .unwrap();
        Self {
            pool,
            organization_id,
            repository,
        }
    }

    async fn submit(&self) -> Uuid {
        self.repository
            .create(
                self.organization_id,
                IdempotencyKey::parse(&Uuid::new_v4().to_string()).unwrap(),
                SubmissionInput::new(
                    "python",
                    "print(input())".into(),
                    Some("hello\n".into()),
                    Default::default(),
                )
                .unwrap(),
            )
            .await
            .unwrap()
            .id
    }

    async fn latest_job(&self, submission_id: Uuid) -> ExecuteSubmission {
        let attempt_id = sqlx::query_scalar(
            "SELECT id FROM execution_attempts WHERE organization_id = $1 AND submission_id = $2
             ORDER BY attempt_number DESC LIMIT 1",
        )
        .bind(self.organization_id)
        .bind(submission_id)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        ExecuteSubmission {
            version: ExecuteSubmission::VERSION,
            organization_id: self.organization_id,
            submission_id,
            attempt_id,
        }
    }
}

#[derive(Clone)]
struct ControlledSandbox {
    calls: Arc<AtomicUsize>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
    outcome: ProgramOutcome,
    cleanup: CleanupDisposition,
}

impl ControlledSandbox {
    fn new(outcome: ProgramOutcome, cleanup: CleanupDisposition) -> Self {
        Self {
            calls: Arc::default(),
            entered: Arc::default(),
            release: Arc::default(),
            outcome,
            cleanup,
        }
    }
}

impl Sandbox for ControlledSandbox {
    async fn execute(&self, input: SubmissionInput, cancel: CancellationToken) -> ExecutionReport {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        let outcome = tokio::select! {
            _ = self.release.notified() => self.outcome,
            _ = cancel.cancelled() => ProgramOutcome::Cancelled,
        };
        ExecutionReport {
            result: Ok(ExecutionResult {
                outcome,
                stdout: input.stdin().as_bytes().to_vec(),
                stderr: Vec::new(),
                facts: ExecutionFacts {
                    exit_code: Some(0),
                    peak_memory_kib: Some(1024),
                    ..Default::default()
                },
                compilation: None,
            }),
            cleanup: self.cleanup,
        }
    }
}

fn pipeline(
    fixture: &Fixture,
    owner: ExecutionOwner,
    sandbox: ControlledSandbox,
) -> Pipeline<ControlledSandbox> {
    Pipeline {
        pool: fixture.pool.clone(),
        owner,
        sandbox,
        stop: CancellationToken::new(),
        failure: CancellationToken::new(),
    }
}

#[tokio::test]
#[ignore = "requires a freshly migrated disposable PostgreSQL database"]
async fn postgres_apalis_dispatch_drain_and_redelivery() {
    tokio::time::timeout(Duration::from_secs(35), async {
        let fixture = Fixture::new().await;
        let submission = fixture.submit().await;
        sqlx::query(
            "ALTER TABLE outbox_events ADD CONSTRAINT block_publish CHECK (published_at IS NULL)",
        )
        .execute(&fixture.pool)
        .await
        .unwrap();
        assert!(dispatch_execution(&fixture.pool).await.is_err());
        let snapshot: (i64, i64) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM apalis.jobs), (SELECT count(*) FROM execution_attempts)",
        )
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        assert_eq!(snapshot, (0, 0));
        sqlx::query("ALTER TABLE outbox_events DROP CONSTRAINT block_publish")
            .execute(&fixture.pool)
            .await
            .unwrap();
        let (first, second) = tokio::join!(
            dispatch_execution(&fixture.pool),
            dispatch_execution(&fixture.pool)
        );
        assert_ne!(first.unwrap(), second.unwrap());

        let job = fixture.latest_job(submission).await;
        let delivery: Vec<u8> =
            sqlx::query_scalar("SELECT job FROM apalis.jobs WHERE job_type = $1")
                .bind(execution_queue(&fixture.pool).config().queue.as_ref())
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        assert_eq!(
            serde_json::from_slice::<ExecuteSubmission>(&delivery).unwrap(),
            job
        );
        assert!(!delivery.windows(5).any(|window| window == b"hello"));

        let sandbox = ControlledSandbox::new(ProgramOutcome::Success, CleanupDisposition::Verified);
        let actual_owner = owner();
        let stop = CancellationToken::new();
        let consumer = tokio::spawn(run(
            fixture.pool.clone(),
            actual_owner,
            sandbox.clone(),
            stop.clone(),
        ));
        sandbox.entered.notified().await;

        let competing = pipeline(&fixture, owner(), sandbox.clone());
        competing.handle(job.clone()).await.unwrap();
        let wrong_tenant = ExecuteSubmission {
            organization_id: Uuid::new_v4(),
            ..job.clone()
        };
        competing.handle(wrong_tenant).await.unwrap();
        assert_eq!(sandbox.calls.load(Ordering::SeqCst), 1);

        stop.cancel();
        let waiting = fixture.submit().await;
        sandbox.release.notify_one();
        consumer.await.unwrap().unwrap();
        competing.handle(job).await.unwrap();
        assert_eq!(sandbox.calls.load(Ordering::SeqCst), 1);

        let settings = DatabaseSettings::load().unwrap();
        let reopened = PostgresSubmissions::initialize(
            connect(&settings).await.unwrap(),
            fixture.organization_id,
        )
        .await
        .unwrap();
        let result = reopened
            .get(fixture.organization_id, submission)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.state, ProcessingState::Completed);
        assert_eq!(result.verdict, None);
        let result = result.result.unwrap();
        assert_eq!(result.execution.outcome, "success");
        assert_eq!(result.execution.stdout, "hello\n");
        assert_eq!(result.execution.peak_memory_kib, Some(1024));
        assert!(
            serde_json::to_value(result)
                .unwrap()
                .get("compilation")
                .is_none()
        );
        assert!(
            reopened
                .get(Uuid::new_v4(), submission)
                .await
                .unwrap()
                .is_none()
        );

        let result = reopened
            .get(fixture.organization_id, waiting)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            result.state,
            ProcessingState::Created | ProcessingState::Queued
        ));
        let done: i64 =
            sqlx::query_scalar("SELECT count(*) FROM apalis.jobs WHERE status = 'Done'")
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        assert_eq!(
            done, 1,
            "Apalis must acknowledge the durable result before drain finishes"
        );
        if result.state == ProcessingState::Created {
            assert!(dispatch_execution(&fixture.pool).await.unwrap());
        }
    })
    .await
    .expect("pipeline test exceeded 35 seconds");
}

#[tokio::test]
#[ignore = "requires a freshly migrated disposable PostgreSQL database"]
async fn postgres_compiler_outcomes_and_diagnostics_are_durable() {
    let fixture = Fixture::new().await;
    let owner = owner();
    for (outcome, verdict) in [
        (ProgramOutcome::CompilationError, Verdict::CompilationError),
        (ProgramOutcome::CpuTimeLimit, Verdict::TimeLimitExceeded),
    ] {
        let input =
            SubmissionInput::new("cpp", "int main() {}".into(), None, Default::default()).unwrap();
        let submission = fixture
            .repository
            .create(
                fixture.organization_id,
                IdempotencyKey::parse(&Uuid::new_v4().to_string()).unwrap(),
                input,
            )
            .await
            .unwrap();
        assert!(dispatch_execution(&fixture.pool).await.unwrap());
        let job = fixture.latest_job(submission.id).await;
        let claimed = claim_execution(&fixture.pool, &job, owner)
            .await
            .unwrap()
            .unwrap()
            .validate()
            .unwrap();
        assert_eq!(claimed.profile().revision, "gcc14-cpp20-v1");

        let report = ExecutionReport {
            result: Ok(ExecutionResult {
                outcome,
                stdout: Vec::new(),
                stderr: Vec::new(),
                facts: ExecutionFacts::default(),
                compilation: Some(StageResult {
                    outcome,
                    stdout: b"compiler output with NUL \0".to_vec(),
                    stderr: b"bounded compiler diagnostics".to_vec(),
                    facts: ExecutionFacts {
                        cpu_time_ms: Some(12),
                        ..Default::default()
                    },
                }),
            }),
            cleanup: CleanupDisposition::Verified,
        };
        finalize_execution(&fixture.pool, &job, owner, AttemptReport::Sandbox(&report))
            .await
            .unwrap();
        let fetched = fixture
            .repository
            .get(fixture.organization_id, submission.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.state, ProcessingState::Completed);
        assert_eq!(fetched.verdict, Some(verdict));
        let result = serde_json::to_value(fetched.result.unwrap()).unwrap();
        assert_eq!(result["outcome"], outcome.as_str());
        assert_eq!(result["stdout"], "");
        assert_eq!(result["exit_code"], serde_json::Value::Null);
        assert_eq!(result["compilation"]["outcome"], outcome.as_str());
        assert_eq!(
            result["compilation"]["stdout"],
            "compiler output with NUL \0"
        );
        assert_eq!(
            result["compilation"]["stderr"],
            "bounded compiler diagnostics"
        );
        assert_eq!(result["compilation"]["cpu_time_ms"], 12);
        assert!(!dispatch_execution(&fixture.pool).await.unwrap());
    }
}

#[tokio::test]
#[ignore = "requires a freshly migrated disposable PostgreSQL database"]
async fn postgres_expired_lease_retry_bound_and_cleanup_quarantine() {
    let fixture = Fixture::new().await;
    let submission = fixture.submit().await;
    let mut abandoned = owner();

    for number in 1..=3 {
        assert!(dispatch_execution(&fixture.pool).await.unwrap());
        let job = fixture.latest_job(submission).await;
        let expire = || async {
            sqlx::query(
                "UPDATE execution_attempts SET lease_expires_at = now() - interval '1 second' WHERE id = $1",
            ).bind(job.attempt_id).execute(&fixture.pool).await.unwrap();
        };
        if number == 1 {
            let sandbox =
                ControlledSandbox::new(ProgramOutcome::Success, CleanupDisposition::Verified);
            let runner = pipeline(&fixture, abandoned, sandbox.clone());
            let (execution, ()) = tokio::time::timeout(HEARTBEAT * 3, async {
                tokio::join!(runner.execute(&job), async {
                    sandbox.entered.notified().await;
                    expire().await;
                })
            })
            .await
            .expect("worker must cancel execution after losing its lease");
            assert!(matches!(execution, Err(ExecutionError::Lease)));
            assert!(runner.failure.is_cancelled());
        } else {
            assert!(
                claim_execution(&fixture.pool, &job, abandoned)
                    .await
                    .unwrap()
                    .is_some()
            );
            expire().await;
        }

        assert!(matches!(
            renew_attempt_lease(&fixture.pool, &job, abandoned).await,
            Err(ExecutionError::Lease)
        ));
        assert!(matches!(
            finalize_execution(&fixture.pool, &job, abandoned, AttemptReport::InvalidInput).await,
            Err(ExecutionError::Lease)
        ));
        recover_after_host_probe(&fixture.pool, owner())
            .await
            .unwrap();
        let current = fixture
            .repository
            .get(fixture.organization_id, submission)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current.state, ProcessingState::Processing);

        let restarted = ExecutionOwner {
            worker_id: Uuid::new_v4(),
            ..abandoned
        };
        recover_after_host_probe(&fixture.pool, restarted)
            .await
            .unwrap();
        let current = fixture
            .repository
            .get(fixture.organization_id, submission)
            .await
            .unwrap()
            .unwrap();
        if number < 3 {
            assert_eq!(current.state, ProcessingState::Queued);
        } else {
            assert_eq!(current.state, ProcessingState::Failed);
            assert_eq!(current.verdict, Some(Verdict::InternalError));
            assert!(current.result.is_none());
        }
        abandoned = restarted;
    }

    let limited = fixture.submit().await;
    assert!(dispatch_execution(&fixture.pool).await.unwrap());
    let job = fixture.latest_job(limited).await;
    let sandbox = ControlledSandbox::new(
        ProgramOutcome::WallTimeLimit,
        CleanupDisposition::Unresolved(CleanupError::Residual),
    );
    let runner = pipeline(&fixture, abandoned, sandbox.clone());
    sandbox.release.notify_one();
    runner.handle(job.clone()).await.unwrap();
    assert!(runner.failure.is_cancelled());
    assert!(runner.handle(job.clone()).await.is_err());

    let current = fixture
        .repository
        .get(fixture.organization_id, limited)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.state, ProcessingState::Completed);
    assert_eq!(current.verdict, Some(Verdict::TimeLimitExceeded));
    assert_eq!(current.result.unwrap().execution.outcome, "wall_time_limit");
    assert!(!dispatch_execution(&fixture.pool).await.unwrap());

    let restarted = ExecutionOwner {
        worker_id: Uuid::new_v4(),
        ..abandoned
    };
    recover_after_host_probe(&fixture.pool, restarted)
        .await
        .unwrap();
    let cleanup: (String, bool) = sqlx::query_as(
        "SELECT cleanup_disposition, cleanup_recovered_at IS NOT NULL FROM execution_attempts WHERE id = $1",
    ).bind(job.attempt_id).fetch_one(&fixture.pool).await.unwrap();
    assert_eq!(cleanup, ("unresolved".into(), true));
    let duplicate = pipeline(&fixture, restarted, sandbox.clone());
    duplicate.handle(job).await.unwrap();
    assert_eq!(sandbox.calls.load(Ordering::SeqCst), 1);
}
