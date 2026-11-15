# Phase 1 · Rust foundation

**Prerequisite** · The host and dependency evidence from [Phase 0](00-architecture-and-environment.md).

## Outcome

Produce independently runnable Axum API and worker binaries from a single workspace with validated configuration, structured logs and clean shutdown.

## Work plan

1. Create the Cargo workspace, pin the Rust toolchain and commit the lockfile. Add members as their responsibilities come into use. Keep Axum routing in the API binary and process setup in the worker binary. [Application ownership](../apps/README.md) · [Crate ownership](../crates/README.md)
2. Implement typed configuration with defaults, files and environment overrides in that order. Validate ports, settings and mode at startup. Keep secrets out of errors, summaries and child environments. [Configuration policy](../configs/README.md)
3. Expose `GET /health/live` for process health and `GET /health/ready` for intake readiness. Introduce checks for PostgreSQL and worker dependencies when the respective phases add them.
4. Handle termination signals. The API stops accepting requests and drains active ones within a budget. The worker lifecycle owns its task cancellation and shutdown signal. Phase 4 connects that lifecycle to queue intake and active execution.
5. Set up CI for formatting, compilation, Clippy and tests. Add real sandbox validation on the supported Linux runner in Phase 3.

## Verification

Run the workspace foundation checks from the root.

```sh
cargo fmt --all --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
```

## Acceptance checks

- [ ] Both binaries build from the lockfile and start independently with valid settings.
- [ ] Invalid settings fail clearly without exposing sensitive values.
- [ ] Liveness and readiness reflect their documented conditions.
- [ ] Termination shuts down both processes within the configured budget.
- [ ] The CI checks and release build pass.

**Handoff** · Use the validated configuration and HTTP interface for submission persistence. [Phase 2](02-database-and-submissions.md)
