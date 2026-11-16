# Runbook: Outbox and Dispatch Recovery

This runbook covers diagnosing and resolving stuck or delayed outbox events between submission persistence and Apalis execution queue dispatch.

---

## 1. Overview and Architecture

Ferrum relies on the Transactional Outbox pattern to guarantee that no accepted submission is lost:

1. When a submission is created or retried, an event is inserted into `outbox_events` in the same PostgreSQL transaction.
2. The outbox dispatcher (`dispatch_execution`) polls `outbox_events` every 500ms using `FOR UPDATE SKIP LOCKED`.
3. The dispatcher transitions `submissions.state` to `queued`, inserts an initial or retry attempt in `execution_attempts`, schedules the Apalis job, and updates `outbox_events.published_at = now()`.

---

## 2. Failure Modes

1. **Dispatcher Starvation or Failure**: Worker instances running outbox pollers are overwhelmed or down.
2. **Apalis Table Schema / Lock Contention**: `apalis.jobs` lock contention or connection pool exhaustion.
3. **Poison Outbox Events**: An outbox event referring to an unhandled version or a deleted/cancelled submission.
4. **Transient Database Interruption**: Worker killed while pushing tasks to Apalis, leaving `published_at = NULL`.

---

## 3. Detection Signals

### Alerts and Queries

- `ferrum_outbox_unpublished_count > 50`
- `ferrum_outbox_oldest_event_age_seconds > 30`
- Query pending or stuck outbox events:

```sql
SELECT id, organization_id, submission_id, event_type, event_version, created_at,
       EXTRACT(EPOCH FROM (clock_timestamp() - created_at)) AS age_seconds
FROM outbox_events
WHERE published_at IS NULL
ORDER BY created_at ASC
LIMIT 20;
```

---

## 4. Automated Recovery

Ferrum workers run an automated outbox reconciliation loop every 10 seconds with `stale_after = 30s`:

- Queries outbox events with `published_at IS NULL AND created_at <= clock_timestamp() - interval '30 seconds'`.
- For `submission.created` and `submission.execution_retry`:
  - Retries atomic dispatch and queue push with `ON CONFLICT (id) DO NOTHING` on `execution_attempts`.
- For `submission.completed` or terminal/orphaned events:
  - Safely sets `published_at = now()`.

---

## 5. Manual Operator Procedures

### Step 1: Run Manual CLI Reconcile

Trigger an immediate pass of outbox and stale lease reconciliation:

```bash
ferrum-worker reconcile
```

Output:

```json
{
  "stale_attempts_reconciled": 0,
  "stuck_outbox_events_reconciled": 5
}
```

### Step 2: Check Apalis Job Queue Health

Inspect the state of the underlying Apalis PostgreSQL table:

```sql
SELECT status, count(*)
FROM apalis.jobs
WHERE queue = 'ferrum.execute.v1'
GROUP BY status;
```

### Step 3: Investigate Dead Letter / Poison Events

If specific outbox events continue failing:

```sql
SELECT e.id, e.submission_id, s.state AS submission_state, e.event_type, e.created_at
FROM outbox_events e
LEFT JOIN submissions s ON s.id = e.submission_id
WHERE e.published_at IS NULL
  AND e.created_at < clock_timestamp() - interval '5 minutes';
```

If the submission has already reached a terminal state (`completed`, `failed`, or `cancelled`), mark the orphaned event published:

```sql
UPDATE outbox_events
SET published_at = now()
WHERE id = '<event_id>'
  AND published_at IS NULL;
```
