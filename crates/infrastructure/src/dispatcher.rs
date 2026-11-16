use std::time::Duration;

use apalis::prelude::TaskBuilder;
use apalis_postgres::{Config, PostgresStorage, queries::push_tasks};
use ferrum_contracts::{ExecuteSubmission, SubmissionCreated};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::execution::{ExecutionError, execution_storage};

const QUEUE: &str = "ferrum.execute.v1";
pub(crate) const RETRY_EVENT: &str = "submission.execution_retry";

#[derive(sqlx::FromRow)]
struct OutboxEvent {
    id: Uuid,
    organization_id: Uuid,
    submission_id: Uuid,
    event_type: String,
    traceparent: Option<String>,
}

pub async fn setup_apalis(pool: &PgPool) -> Result<(), ExecutionError> {
    PostgresStorage::<()>::setup(pool).await.map_err(|_| {
        tracing::warn!(event = "apalis_schema_setup_failed");
        ExecutionError::Queue
    })
}

pub fn execution_queue(pool: &PgPool) -> PostgresStorage<ExecuteSubmission> {
    PostgresStorage::new(pool).with_config(
        Config::default()
            .queue(QUEUE)
            .batch_size(1)
            .persist_results(true),
    )
}

pub async fn dispatch_execution(pool: &PgPool) -> Result<bool, ExecutionError> {
    let start = std::time::Instant::now();
    let mut tx = pool.begin().await.map_err(execution_storage)?;
    let event = sqlx::query_as::<_, OutboxEvent>(
        "SELECT id, organization_id, submission_id, event_type, traceparent FROM outbox_events
         WHERE published_at IS NULL AND event_version = $1 AND event_type IN ($2, $3)
         ORDER BY created_at, id LIMIT 1 FOR UPDATE SKIP LOCKED",
    )
    .bind(SubmissionCreated::VERSION)
    .bind(SubmissionCreated::EVENT_TYPE)
    .bind(RETRY_EVENT)
    .fetch_optional(&mut *tx)
    .await
    .map_err(execution_storage)?;
    let Some(event) = event else {
        return Ok(false);
    };

    publish_event_tx(&mut tx, &event).await?;
    tx.commit().await.map_err(execution_storage)?;

    let duration = start.elapsed().as_secs_f64();
    let metrics = ferrum_runtime::MetricsRegistry::global();
    metrics
        .outbox_dispatch_duration_seconds
        .observe(&[], duration);
    metrics.outbox_events_total.inc(&[
        ("event_type", event.event_type.as_str()),
        ("status", "published"),
    ]);

    Ok(true)
}

pub async fn reconcile_stuck_outbox(
    pool: &PgPool,
    stale_after: Duration,
) -> Result<usize, ExecutionError> {
    let mut reconciled_count = 0;
    let stale_ms = stale_after.as_millis().max(1) as i64;
    loop {
        let mut tx = pool.begin().await.map_err(execution_storage)?;
        let event = sqlx::query_as::<_, OutboxEvent>(
            "SELECT id, organization_id, submission_id, event_type, traceparent FROM outbox_events
             WHERE published_at IS NULL AND created_at <= clock_timestamp() - ($1 * interval '1 millisecond')
             ORDER BY created_at ASC, id ASC
             LIMIT 1 FOR UPDATE SKIP LOCKED",
        )
        .bind(stale_ms)
        .fetch_optional(&mut *tx)
        .await
        .map_err(execution_storage)?;

        let Some(event) = event else {
            break;
        };

        if event.event_type == SubmissionCreated::EVENT_TYPE || event.event_type == RETRY_EVENT {
            publish_event_tx(&mut tx, &event).await?;
        } else {
            sqlx::query("UPDATE outbox_events SET published_at = now() WHERE id = $1")
                .bind(event.id)
                .execute(&mut *tx)
                .await
                .map_err(execution_storage)?;
        }

        tx.commit().await.map_err(execution_storage)?;

        tracing::warn!(
            event = "stuck_outbox_reconciled",
            event_id = %event.id,
            %event.organization_id,
            %event.submission_id,
            event_type = %event.event_type
        );

        reconciled_count += 1;
        if reconciled_count >= 100 {
            break;
        }
    }

    let metrics = ferrum_runtime::MetricsRegistry::global();
    metrics
        .reconciliation_runs_total
        .inc(&[("target", "stuck_outbox")]);
    if reconciled_count > 0 {
        metrics
            .reconciled_items_total
            .inc_by(&[("target", "stuck_outbox")], reconciled_count as u64);
    }

    Ok(reconciled_count)
}

async fn publish_event_tx(
    tx: &mut Transaction<'_, Postgres>,
    event: &OutboxEvent,
) -> Result<(), ExecutionError> {
    let expected = if event.event_type == RETRY_EVENT {
        "queued"
    } else {
        "created"
    };
    let eligible = sqlx::query(
        "UPDATE submissions SET state = 'queued', updated_at = now()
         WHERE organization_id = $1 AND id = $2 AND state = $3",
    )
    .bind(event.organization_id)
    .bind(event.submission_id)
    .bind(expected)
    .execute(&mut **tx)
    .await
    .map_err(execution_storage)?;

    if eligible.rows_affected() == 1 {
        let number: i32 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(attempt_number), 0) + 1 FROM execution_attempts
             WHERE organization_id = $1 AND submission_id = $2",
        )
        .bind(event.organization_id)
        .bind(event.submission_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(execution_storage)?;
        let mut job = ExecuteSubmission::new(event.submission_id, event.organization_id, event.id);
        if let Some(traceparent) = &event.traceparent {
            job = job.with_traceparent(traceparent);
        }
        sqlx::query(
            "INSERT INTO execution_attempts (id, organization_id, submission_id, attempt_number, state)
             VALUES ($1, $2, $3, $4, 'queued')
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(job.attempt_id)
        .bind(job.organization_id)
        .bind(job.submission_id)
        .bind(number)
        .execute(&mut **tx)
        .await
        .map_err(execution_storage)?;

        let delay = if number == 1 {
            Duration::ZERO
        } else {
            let jitter_ms = u64::from(job.attempt_id.as_bytes()[0]) * 4;
            Duration::from_millis((1_000_u64 << (number - 2)) + jitter_ms)
        };
        let payload = serde_json::to_vec(&job).map_err(|_| ExecutionError::Queue)?;
        let task = TaskBuilder::new(payload)
            .max_attempts(10)
            .run_after(delay)
            .build();
        push_tasks(&mut **tx, QUEUE, vec![task])
            .await
            .map_err(|_| ExecutionError::Queue)?;
    }

    sqlx::query("UPDATE outbox_events SET published_at = now() WHERE id = $1")
        .bind(event.id)
        .execute(&mut **tx)
        .await
        .map_err(execution_storage)?;
    Ok(())
}
