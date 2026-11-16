# Runbook: Sandbox Cleanup Quarantine

This runbook describes the procedure for investigating, managing, and resolving quarantined sandbox slots resulting from cleanup failures.

---

## 1. Overview and Architecture

When an execution attempt completes or terminates inside Isolate, the worker must destroy the sandbox workspace, unmount filesystems, remove the cgroup slice, and release the capacity reservation.

### Quarantine Guarantee

If Isolate cleanup fails (e.g. residual processes holding open mount points, unmounted tmpfs volumes, locked cgroup controllers, or metadata corruption):

- The execution result (stdout, stderr, exit code, resource telemetry) is already safely persisted in PostgreSQL.
- The attempt disposition is recorded as `unresolved`:
  - `cleanup_disposition = 'unresolved'`
  - `cleanup_error = '<error message>'`
  - `cleanup_recovered_at = NULL`
- The worker holds capacity and quarantines the sandbox slot so hostile or rogue processes cannot cross-contaminate subsequent submissions.

---

## 2. Detection Signals

### Alerts and Queries

- `ferrum_sandbox_quarantines_active > 0`
- Query active quarantines in PostgreSQL:

```sql
SELECT id AS attempt_id, organization_id, submission_id, host_id, worker_id,
       cleanup_error, started_at
FROM execution_attempts
WHERE cleanup_disposition = 'unresolved'
  AND cleanup_recovered_at IS NULL
ORDER BY started_at ASC;
```

---

## 3. Operator CLI Inspection

### List Active Quarantines

Run the quarantine listing command using `ferrum-worker`:

```bash
ferrum-worker quarantine list
```

Example JSON response:

```json
[
  {
    "organization_id": "a3b1c2d4-0000-0000-0000-000000000001",
    "submission_id": "5f6e7d8c-0000-0000-0000-000000000002",
    "attempt_id": "e8d7c6b5-0000-0000-0000-000000000003",
    "worker_id": "7a8b9c0d-0000-0000-0000-000000000004",
    "failure_reason": null,
    "cleanup_error": "sandbox workspace, cgroup or active lock remains",
    "created_at": 1731754000
  }
]
```

---

## 4. Host Investigation and Teardown

Connect to the affected worker host (`host_id`):

### Step 1: Inspect Isolate Box 0 Status

```bash
/usr/bin/isolate --box-id=0 --cg -s --cleanup
```

If processes remain running inside the cgroup:

```bash
cat /sys/fs/cgroup/isolate/0/cgroup.procs
```

### Step 2: Terminate Lingering Processes

Kill all processes attached to the sandbox cgroup:

```bash
kill -9 $(cat /sys/fs/cgroup/isolate/0/cgroup.procs 2>/dev/null)
```

Unmount any remaining mounts:

```bash
grep "/var/lib/isolate/0" /proc/mounts
umount -f /var/lib/isolate/0/box 2>/dev/null || true
```

Clean the workspace:

```bash
/usr/bin/isolate --box-id=0 --cg -s --cleanup
rm -f /run/isolate/locks/0
rm -rf /var/lib/ferrum-sandbox/*
```

---

## 5. Resolving the Quarantine

Once the host sandbox slot has been cleanly wiped and verified:

### Resolve a Specific Attempt Quarantine

```bash
ferrum-worker quarantine resolve e8d7c6b5-0000-0000-0000-000000000003
```

Output:
```text
quarantine resolved for attempt e8d7c6b5-0000-0000-0000-000000000003
```

### Resolve All Quarantines for a Restored Host

```bash
ferrum-worker quarantine resolve-host 7a8b9c0d-0000-0000-0000-000000000004
```

Output:
```text
resolved 1 quarantined attempts for host 7a8b9c0d-0000-0000-0000-000000000004
```

### Automatic Host Probe Recovery

When the worker daemon starts or restarts on that host, `recover_after_host_probe` automatically inspects box 0, verifies cgroups and quotas, and marks `cleanup_recovered_at = now()` for previously unresolved attempts on that host.
