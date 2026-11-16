# Phase 9 · Observability and performance

**Prerequisite** · Staging recovery evidence and representative worker hardware. [Phase 8](08-security-and-reliability.md)

## Outcome

Give operators a trace from submission to cleanup, alerts for work that stops progressing, and measured concurrency settings for each worker profile.

## Work plan

1. Propagate trace context across API, outbox, queue handling, compile/run, persistence and cleanup. Use structured `tracing` events and OpenTelemetry. Keep correlation IDs in approved logs and traces under the redaction policy.
2. Export Prometheus metrics for counts, rates, stage timing and backlog. Define low-cardinality labels such as stage, language and outcome class. Keep submission, attempt and tenant IDs out of metric labels.
3. Build Grafana dashboards for intake, queue, worker capacity and cleanup. Use Loki or the selected log backend for structured logs. Tie each alert to an owner and a recovery procedure.
4. Run staged load tests with synthetic submissions and production-like language mix. Measure warm and cold workers, bursts and sustained traffic. Observe PostgreSQL connection use alongside worker CPU, memory and quarantined capacity.
5. Record p50/p95/p99 latency by stage, throughput, queue age, failure rate and recovery time. Set safe admission budgets from observed saturation. Record candidate service targets and proposed canary stop conditions for Phase 10.

| Signal | Operator use |
|---|---|
| Oldest unpublished outbox event and queue age | Distinguish dispatch backlog from worker saturation |
| Admission wait and CPU/memory reservations | Decide whether to adjust concurrency or fleet capacity |
| Compile/run and persistence latency | Identify the stage responsible for slow completion |
| Cleanup backlog and quarantined capacity | Detect a worker that should stop admission |
| Platform failure rate and lease recovery | Separate infrastructure incidents from ordinary program verdicts |

## Acceptance checks

- [x] An operator can follow a submission from intake through persisted outcome and cleanup.
- [x] Dashboards separate queue, sandbox initialization, compilation, execution and persistence.
- [x] Alerts fire in controlled staging drills and connect to the relevant runbooks.
- [x] A repeatable capacity report records hardware, versions, workload and recommended limits.

**Handoff** · Use measured capacity and service targets to plan rollout and rollback. [Phase 10](10-production-release.md)
