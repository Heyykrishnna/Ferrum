# Phase 1 · Rust foundation

The workspace uses the verified Phase 0 dependency baseline. Linux Isolate qualification is a prerequisite for the Phase 3 sandbox adapter. [Phase 0 evidence](00-architecture-and-environment.md)

## Outcome

Produce independently runnable Axum API and worker binaries from a single workspace with validated configuration, structured logs and clean shutdown.

## Work plan

1. Create the Cargo workspace, pin the Rust toolchain and commit the lockfile. Add members as their responsibilities come into use. Keep Axum routing in the API binary and process setup in the worker binary. [Application ownership](../apps/README.md) · [Crate ownership](../crates/README.md)
2. Implement typed configuration with defaults, files and environment overrides in that order. Validate ports, settings and mode at startup. Keep secrets out of errors, summaries and child environments. [Configuration policy](../configs/README.md)
3. Expose `GET /health/live` for process health and `GET /health/ready` for intake readiness. Introduce checks for PostgreSQL and worker dependencies when the respective phases add them.
4. Handle termination signals. The API stops accepting requests and drains active ones within a budget. The worker lifecycle owns its task cancellation and shutdown signal. Phase 4 connects that lifecycle to queue intake and active execution.
5. Set up CI for formatting, compilation, Clippy and tests. Add real sandbox validation on the supported Linux runner in Phase 3.

## Verification

Run the workspace foundation checks from the root with Rustup and Python 3.9 or newer. The workspace selects its pinned Rust toolchain.

```sh
cargo fmt --all --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
python3 -I tests/phase1/process-smoke.py
```

## Runtime behavior

The API binds to `127.0.0.1:8080` by default. Both binaries start from built-in defaults and support optional configuration files and environment overrides. [Configuration reference](../configs/README.md)

| Endpoint | Response |
|---|---|
| `GET /health/live` | HTTP 200 with `{"status":"live"}` while the API serves requests. |
| `GET /health/ready` | HTTP 200 with `{"status":"ready"}` once the listener is serving. On requests still served after shutdown begins, returns HTTP 503 with `{"status":"draining"}`. |

Both responses use `Cache-Control: no-store`. Readiness reflects HTTP availability in Phase 1. PostgreSQL readiness enters in Phase 2.

SIGINT and SIGTERM cancel the process lifecycle. The API stops accepting connections and allows requests to finish within the shutdown budget. The worker exits its cancellable service loop. A process that exceeds its budget exits with failure. Phase 4 connects the worker lifecycle to queue intake and execution drain.

## Acceptance checks

- [x] Both binaries build from the lockfile and start independently with valid settings.
- [x] Invalid settings fail clearly without exposing sensitive values.
- [x] Liveness and readiness reflect their documented conditions.
- [x] Termination shuts down both processes within the configured budget.
- [x] The workspace checks, release build and process smoke test pass locally.
- [ ] The GitHub-hosted workflow reports a passing run after these changes are pushed. [Rust workflow](../.github/workflows/rust.yml)

Rust 1.100.0 on macOS arm64 passed the commands in [Verification](#verification). On Debian 13 amd64, the locked compilation check, Rust tests, release build, and process smoke test also passed in a container. The smoke test used the release binaries and an ephemeral loopback port. It exercised configuration precedence and rejection, both HTTP health endpoints, and graceful SIGINT and SIGTERM exits within the test's two-second budget. The Rust tests also verified the readiness transition during shutdown and the timeout path for a service that fails to drain. The workflow passed `actionlint`.

**Handoff** · Use the validated configuration and HTTP interface for submission persistence. [Phase 2](02-database-and-submissions.md)
