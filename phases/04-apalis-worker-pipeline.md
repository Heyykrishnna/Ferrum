# Phase 4 · Apalis worker pipeline

**Prerequisite** · Atomic submission/outbox writes and real Linux sandbox evidence. [Phase 2](02-database-and-submissions.md) · [Phase 3](03-isolate-integration.md)

## Outcome

Close the first product milestone. A client submits one Python program, the worker processes it through PostgreSQL-backed Apalis and Isolate, and the client retrieves a durable result by polling.

## Work plan

1. Apply the selected Apalis PostgreSQL backend's documented schema setup. Run a bounded dispatcher that claims committed outbox events, publishes a versioned `ExecuteSubmission` job containing routing IDs, and records publication in the same PostgreSQL transaction.
2. On delivery, validate tenant, submission and attempt eligibility against PostgreSQL. Reserve CPU and memory before creating a sandbox. Guard durable attempt ownership so duplicate deliveries cannot create competing runs for the same eligible attempt.
3. Capture bounded facts through the Phase 3 adapter. Persist the result, attempt outcome and completion event under the current lease ownership. Record cleanup disposition durably and acknowledge only once the outcome and any required cleanup recovery are recorded.
4. Add heartbeat and lease recovery, bounded retries for transient infrastructure faults, and worker drain. Reconcile claimed events, expired attempts and unresolved cleanup. Preserve a committed result through redelivery and give each replacement attempt distinct history. [Lifecycle and recovery](submission-lifecycle.md)
5. Record queue and worker diagnostics with submission and attempt correlation, keeping source, stdin and credentials out of jobs and logs. Keep PostgreSQL as the initial job store. [Apalis architecture](https://apalis.dev/docs/introduction/architecture)

## Integration scenarios

| Scenario | Expected result |
|---|---|
| Submit and poll after API restart | Same submission ID and persisted output, resource facts and processing state |
| Publication fails after inserting the queue row | PostgreSQL rolls back the queue insert, attempt and publication marker together |
| Duplicate delivery during live execution | One authorized attempt runs and host admission stays within policy |
| Worker loses its lease mid-run | Stale writes are rejected and retry waits for verified stop or host fencing |
| Finalization commits and the queue redelivers | Existing outcome is reused without a second run |
| Cleanup reports a failure | Saved result survives and affected capacity stays quarantined until recovery |
| User program hits a resource limit | Terminal program outcome is returned without an infrastructure retry |

## Acceptance checks

- [ ] The complete path passes with real PostgreSQL, Apalis and a supported Linux worker.
- [x] The PostgreSQL/Apalis integration suite demonstrates atomic dispatch, duplicate delivery, lease loss, bounded recovery, cleanup quarantine and stable terminal results with a sandbox test double.
- [x] Graceful drain stops new claims, finalizes active attempts and flushes their acknowledgments in the integration suite.
- [ ] The [first milestone](../docs/first-milestone.md) has recorded evidence and can be reproduced.

Verified with two PostgreSQL 17 and Apalis integration tests and the workspace tests and Clippy in the Debian amd64 Docker image. The overflow regression also passed 20 consecutive runs after the sandbox pipe-read fix. Reproduce the database and queue checks with `python3 -I -u tests/phase4/worker-smoke.py`. Run the same command with `--isolate` on a Phase 3-qualified Debian 13 amd64 host to record the remaining HTTP-to-Isolate evidence. [Worker runbook](../docs/phase4-worker.md)

**Handoff** · Add C++ as the first compile/run profile, then expand the registry. [Phase 5](05-language-runtimes.md)
