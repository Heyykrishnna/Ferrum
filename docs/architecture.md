# Architecture

Ferrum starts as one Rust workspace with an API process and a worker process. They share the domain and application modules and deploy separately. The outbox dispatcher can start within the worker deployment; a third process is unnecessary for the first milestone.

```text
Client -> Axum API -> PostgreSQL: submission + outbox event
                          |
                    Outbox dispatcher
                          |
                  Apalis PostgreSQL queue
                          |
                    Linux worker -> Isolate -> pinned toolchain
                          |
            PostgreSQL: result and cleanup status
                          |
                    Axum API -> Client
```

1. Resolve the authenticated tenant and validate the request. Atomically store the submission and an outbox event, then return its ID.
2. Claim the event, publish an identifier-only job and record publication. A crash between publish and mark can cause redelivery.
3. Verify submission and tenant scope, reserve CPU and memory, and claim the attempt with a durable lease or equivalent ownership check. Duplicate consumers cannot both own the same attempt.
4. Create a fresh sandbox, use the trusted runtime profile, and collect bounded execution facts. Compile in the sandbox when required. Database transactions remain short and do not span execution.
5. Store execution facts and outcome under the current attempt's ownership. Persist cleanup outcome independently, so a failed teardown cannot erase an execution result. Hold or quarantine the affected capacity until teardown is verified. Acknowledge only after result and cleanup disposition are durable.
6. The API reads by tenant and ID. Reconciliation handles stale attempts and incomplete cleanup. If an old worker cannot prove it still owns the attempt, it stops its process tree and cannot write a terminal result.

Keep processing state, program outcome and sandbox cleanup separate. A program that exceeds its time limit can have processing state `completed` and verdict `time_limit_exceeded`. Platform faults carry their own diagnostics. Retry only classified transient infrastructure failures, within a bounded policy.

Keep domain logic independent of Axum, SQLx, Apalis and subprocess details. Application workflows own orchestration. SQLx and Isolate adapters own their implementation-specific lifecycle; configuration owns toolchain validation. [Crate ownership](../crates/README.md) maps these responsibilities to the target workspace.
