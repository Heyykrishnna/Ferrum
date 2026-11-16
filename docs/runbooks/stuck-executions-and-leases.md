# Runbook: Stuck Executions and Expired Leases

This runbook guides operators through detecting, investigating, and recovering execution attempts that are stuck in the `running` state past their active lease expiration deadline.

---

## 1. Overview and Failure Modes

Each execution attempt claimed by a worker acquires an execution lease in PostgreSQL:

- Lease duration: 30 seconds (`lease_expires_at = clock_timestamp() + interval '30 seconds'`).
- Worker heartbeat: Renews active attempt lease every 5 seconds while execution proceeds.
- Maximum attempts: 3 attempts before marking a submission as permanently `failed` with verdict `internal_error`.

### Why an Execution Becomes Stuck

1. **Worker host crash or hard shutdown**: Node abruptly lost power, kernel panic, or OOM kill without running graceful shutdown handlers.
2. **Worker process termination**: Process killed via `SIGKILL` or hardware stockout.
3. **Database network partition**: Worker disconnected from PostgreSQL, preventing heartbeat renewal.
4. **Hanging uncooperative child process**: Zombie process blocking runtime reaping.

---

## 2. Detection Signals

### Alerts and Prometheus Metrics

- `ferrum_stale_attempts_detected_total > 0`
- `ferrum_execution_duration_seconds > 120`
- SQL check query:

```sql
SELECT a.id, a.submission_id, a.organization_id, a.attempt_number,
       a.worker_id, a.host_id, a.started_at, a.lease_expires_at
FROM execution_attempts a
WHERE a.state = 'running'
  AND a.lease_expires_at <= clock_timestamp()
ORDER BY a.lease_expires_at ASC;
```

---

## 3. Automated Reconciliation

Ferrum workers run an automated background reconciliation loop every 10 seconds:

- It queries stale attempts using `FOR UPDATE SKIP LOCKED`.
- If `attempt_number < 3`:
  - Sets the attempt state to `failed` (`failure_reason = 'lease_expired'`).
  - Sets the submission state back to `queued`.
  - Emits a `submission.execution_retry` event to `outbox_events` so the dispatcher can schedule the next attempt with exponential backoff and jitter.
- If `attempt_number >= 3`:
  - Sets the attempt state to `failed` (`failure_reason = 'lease_expired'`).
  - Sets the submission state to `failed` with verdict `internal_error`.
  - Emits `submission.completed` to `outbox_events`.

---

## 4. Manual Operator Procedures

When automated reconciliation is delayed or when manual operator intervention is required:

### Step 1: Run Manual CLI Reconciliation

Execute the operator reconciliation command on any worker node or bastion:

```bash
ferrum-worker reconcile
```

Example output:

```json
{
  "stale_attempts_reconciled": 2,
  "stuck_outbox_events_reconciled": 0
}
```

### Step 2: Investigate Offending Worker Host

Identify the host and worker that owned the expired attempt:

```sql
SELECT a.id, a.host_id, a.worker_id, a.started_at, a.failure_reason
FROM execution_attempts a
WHERE a.failure_reason = 'lease_expired'
ORDER BY a.completed_at DESC
LIMIT 10;
```

Check the node status and system logs:

```bash
journalctl -u ferrum-worker -n 100 --no-pager
```

### Step 3: Verify Submission State

Ensure the affected submissions have cleanly transitioned to `queued` (for retry) or `failed`:

```sql
SELECT id, state, verdict, updated_at
FROM submissions
WHERE id IN (
  SELECT submission_id FROM execution_attempts WHERE failure_reason = 'lease_expired'
);
```
