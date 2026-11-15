# Test strategy

[Phase 0](../phases/00-architecture-and-environment.md) contains the standalone Rust dependency probe and [Linux worker qualification instructions](../docs/phase0-linux-worker.md).

| Planned path | Environment and evidence |
|---|---|
| In-crate tests | State transitions, idempotency policy, limits and verdict rules |
| `integration/` | Disposable PostgreSQL and real Apalis: transaction rollback, duplicate delivery, restart and saved outcomes |
| `sandbox/` | Supported Linux with the pinned Isolate build: limits, cancellation and verified teardown |
| `security/` | Disposable hosts and test tenants: host and network isolation, protected routes, file validation and webhook egress policy |
| `concurrency/` | Duplicate consumers and interrupted leases: admission, recovery and cleanup under contention |
| `languages/` | Smoke, stdin/stdout, runtime failure and limit cases per runtime; compile failure where applicable |

Phase 1 checks the readiness transition and shutdown deadline in Rust tests. The process smoke test checks HTTP responses, configuration failures, and SIGINT and SIGTERM handling. The current workspace runs it with `--api-only`. [Workspace verification](../phases/01-rust-foundation.md#verification)

Wire later cross-service suites into explicit Cargo test targets. Record Linux host, kernel, toolchain and Isolate versions alongside sandbox results.

Phase 2 starts a new PostgreSQL 17 Compose project with a random loopback port and temporary volume. The suite applies migrations, starts and restarts the API, and verifies concurrent replay, tenant scope, rollback, input bounds, and database readiness. Run the locked release build first. [Phase 2 verification](../phases/02-database-and-submissions.md#verification)
