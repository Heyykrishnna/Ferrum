# Submission lifecycle and recovery

This planning reference follows the durable handoff and execution flow in the [architecture sketch](reference/ferrum-handdrawn-architecture.png). It defines the intended responsibilities across [submission intake](02-database-and-submissions.md), [sandbox execution](03-isolate-integration.md), [queue integration](04-apalis-worker-pipeline.md), [judging](06-judging-engine.md) and the [production API](07-production-api.md).

## Ownership and state

| Record | What it represents |
|---|---|
| Submission | Durable tenant-scoped request, including its resolved runtime profile, effective limits and processing state |
| Outbox event | A committed request for publication. A dispatcher may publish the same event again after a crash. |
| Apalis job | A durable delivery containing version and routing identifiers. The submission and attempt records govern eligibility to execute. |
| Execution attempt | A specific run with owner, lease generation, timestamps and cleanup disposition |
| Execution result | Bounded output, exit facts, measured resources and any program verdict, linked to the producing attempt |

The normal progression is `created → queued → processing → completed`. A worker may receive the job before the dispatcher records publication. The guarded claim should accept that delivery once it verifies the committed submission and event, while a late publication update must leave an already processing or terminal submission unchanged. An unrecoverable platform fault can terminate processing as `failed`. A verified cancellation terminates it as `cancelled`. Platform retries create new attempts only after the previous attempt is safely stopped or its host is fenced. Terminal states remain stable under duplicate deliveries.

Program verdict and cleanup status have separate meanings. `completed` with `time_limit_exceeded` is a correctly reported execution outcome. If teardown fails after a result is written, preserve the result, record the cleanup failure, and keep the affected worker capacity unavailable pending verification.

## Durable handoffs

| Window | Recovery behavior to prove |
|---|---|
| Request transaction rolls back | Neither a submission nor its outbox event is visible. |
| Commit succeeds but the response is lost | A matching tenant-scoped idempotency key returns the same submission ID. |
| Publish succeeds before the event is marked published | A repeated publication reaches the same guarded claim. |
| Worker loses its lease during execution | The stale owner cannot finalize. Stop and verify its process tree or fence the host before starting a replacement attempt. |
| Finalization commits before queue acknowledgment | Redelivery observes the durable outcome. If cleanup is unresolved, it enters cleanup recovery without executing the submission again. |
| Cancellation races with finalization | Conditional writes determine the winner. Preserve an already committed terminal result. Publish a terminal cancellation only once execution is known to have stopped. |
| Cleanup fails | Retain the outcome and durable cleanup work. Release the affected capacity only after verified cleanup or safe host replacement. |

Finalize attempt facts, the submission outcome and the completion event together when they are ready to commit. Derive signed webhook deliveries from committed events in Phase 7. Retry only classified transient infrastructure failures, with a bounded budget and backoff. A deterministic program failure resolves normally and uses no infrastructure retry budget.

## Contract decisions to close during implementation

- Phase 2 records idempotency normalization, the public response shape and the initial `created` state. [Private submission API](../docs/submission-api.md)
- Phase 4 validates acknowledgment, dispatcher claim recovery and retry behavior against the selected Apalis release. It records the lease and fencing mechanism and the authority that releases quarantined capacity.
- Phases 6 and 7 document verdict precedence, cancellation responses, and the stable event identifiers used for webhook deduplication.
