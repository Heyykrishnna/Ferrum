# Runbook: Isolation Incident Containment

This runbook specifies the protocol for containing, analyzing, and mitigating isolation breaches, hostile payload attacks, or anomalous process behavior detected on Ferrum execution worker nodes.

---

## 1. Threat Scenarios and Severity

| Severity | Scenario | Example Indicators |
|---|---|---|
| **Critical** | Sandbox Escape / Root Escalation | Non-isolated processes on host, unauthorized host `/etc` or `/proc` writes, root UID execution. |
| **High** | Host Resource Exhaustion / Fork Storm | System-wide PID exhaustion, swap thrashing, unresponsive worker host supervisor. |
| **High** | Network Isolation Egress Breach | Outbound network packets detected from sandbox cgroup / veth interfaces. |
| **Medium** | Path Traversal / Host File Staging Attempt | Submissions attempting `..` paths, symlinks, or accessing `/var/lib/ferrum-sandbox` state. |

---

## 2. Immediate Containment Procedure (Fencing)

### Step 1: Drain and Fence the Affected Worker

Immediately stop new job intake on the suspected worker host:

```bash
systemctl stop ferrum-worker
```

Revoke the worker's database access or terminate active connections from this worker IP in PostgreSQL:

```sql
SELECT pg_terminate_backend(pid)
FROM pg_stat_activity
WHERE client_addr = '<worker_ip>'
  AND application_name LIKE '%ferrum-worker%';
```

### Step 2: Emergency Process Teardown

On the affected worker host, terminate any running child processes in Isolate box 0:

```bash
# Send SIGKILL to all processes in isolate cgroup slice
if [ -d /sys/fs/cgroup/isolate/0 ]; then
  kill -9 $(cat /sys/fs/cgroup/isolate/0/cgroup.procs 2>/dev/null) 2>/dev/null || true
fi

# Reset isolate box
/usr/bin/isolate --box-id=0 --cg -s --cleanup
```

### Step 3: Verify Sterile Workspace

Confirm that `/var/lib/isolate/0` has no active mounts or leftover files:

```bash
mount | grep "/var/lib/isolate"
ls -la /var/lib/isolate/0/box
```

If unmount is blocked by active open descriptors:

```bash
fuser -kvm /var/lib/isolate/0/box
umount -f /var/lib/isolate/0/box
```

---

## 3. Evidence Collection and Forensics

Before rebooting or reimaging the host:

1. Capture system journal logs:
   ```bash
   journalctl -u ferrum-worker --since "1 hour ago" > /tmp/worker-incident.log
   dmesg -T | tail -n 200 > /tmp/dmesg-incident.log
   ```
2. Capture Isolate metadata and record files:
   ```bash
   cp /var/lib/ferrum-sandbox/box-0.meta /tmp/incident-meta.txt 2>/dev/null || true
   ```
3. Identify the offending submission:
   ```sql
   SELECT a.id AS attempt_id, a.submission_id, a.organization_id,
          s.language, s.source, a.started_at, a.failure_reason
   FROM execution_attempts a
   JOIN submissions s ON s.id = a.submission_id
   WHERE a.host_id = '<host_id>'
   ORDER BY a.started_at DESC
   LIMIT 1;
   ```

---

## 4. Remediation and Service Resumption

1. **Quarantine the Tenant**: Temporarily revoke API keys for the organization responsible for the exploit payload:
   ```sql
   UPDATE api_keys
   SET status = 'revoked', updated_at = now()
   WHERE organization_id = '<offending_org_id>';
   ```
2. **Re-validate Worker Kernel Profile**:
   Ensure kernel, cgroups v2, and Isolate version match the approved specification:
   - Debian 13 amd64
   - Kernel 6.12.x
   - Isolate 2.7 (with `restricted_init=0 syscall_flags=65535`)
3. **Reimage or Restore Worker**:
   Deploy fresh worker configuration and restart daemon:
   ```bash
   systemctl start ferrum-worker
   ```
4. **Run Startup Probe**:
   Ensure `ferrum-worker` runs startup host probe and `recover_after_host_probe` successfully.
