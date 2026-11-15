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

Wire cross-service suites into explicit Cargo test targets when introduced. Once the workspace exists, run `cargo fmt --all --check`, `cargo check --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo test --workspace --locked`. Record Linux host, kernel, toolchain and Isolate versions alongside sandbox results.
