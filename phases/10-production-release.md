# Phase 10 · Production release

**Prerequisite** · Security and recovery evidence, measured capacity, and named service and incident owners. [Phase 8](08-security-and-reliability.md) · [Phase 9](09-observability-and-performance.md)

## Outcome

Deploy API and worker processes independently, verify recovery on the production topology, and increase traffic only while the agreed canary conditions hold.

## Work plan

1. Package and version the API and worker separately. Provision dedicated Linux workers with the qualified Isolate and pinned toolchains. Select hosting, managed or self-operated PostgreSQL, and secret management based on the verified topology and operational needs.
2. Configure TLS at the public API, private networking for data and telemetry, least-privilege database identities and secret rotation. Restrict worker intake to qualified hosts, and keep artifacts and source under the configured access and retention policy.
3. Prepare a migration and rollback plan for overlapping API and worker revisions. Define the compatible job and schema versions. Rehearse rollback with queued and active work on staging. Document which database changes require forward repair.
4. Configure backups and restore to a disposable environment. Validate submission history, results and retained artifacts. Reconcile queue and outbox state before restarting dispatch against restored data. Record observed recovery time and data loss against the agreed objectives.
5. Rehearse draining workers, including active compile/run and quarantined capacity. Verify replacement workers qualify before resuming intake.
6. Start a scheduled canary with limited traffic. Watch queue age, platform error rate, latency, saturation and cleanup backlog against Phase 9 measures. Define who can stop intake and roll back. Publish API docs and operator guidance after release checks pass.

## Release checks

- [ ] API and worker deploy independently with compatible schema and job contracts.
- [ ] A restore exercise verifies persisted submissions, results and recoverable queue state.
- [ ] Worker drain and rollback are demonstrated with queued and active work.
- [ ] Canary outcomes meet the agreed criteria and the release owner signs off.
- [ ] Operational ownership, on-call actions, retention and client documentation are recorded.
