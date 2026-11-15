# Ferrum Execution Engine

Ferrum is a planned Rust service for asynchronous code execution and judging. A client submits code, polls for a result, and receives a bounded record of what happened. Compilation and execution belong on dedicated Linux workers.

The Rust workspace contains an Axum API with health endpoints and a worker process with a graceful shutdown lifecycle. Submission routes, database persistence, queue processing and sandbox execution arrive in later phases.

| Path | Purpose |
|---|---|
| [phases/](phases/README.md) | Phases 0–10, architecture sketch, implementation tasks and acceptance checks |
| [docs/](docs/README.md) | Architecture, technology choices, threat model and first milestone contract |
| [apps/](apps/README.md) | API and worker entry points |
| [crates/](crates/README.md) | Ownership and dependencies for Rust modules |
| [configs/](configs/README.md) | Settings, credentials and toolchain policy |
| [migrations/](migrations/README.md) | Order and ownership of database changes |
| [deployment/](deployment/README.md) | Local setup and production topology |
| [tests/](tests/README.md) | Test environments and evidence requirements |

Start with [Phase 0](phases/00-architecture-and-environment.md) and its [Linux worker verification runbook](docs/phase0-linux-worker.md). The first milestone is [Phase 4](phases/04-apalis-worker-pipeline.md): submit a Python program through Axum, schedule it through PostgreSQL and Apalis, run it inside Isolate on Linux and retrieve the saved result. C++ follows as the first compiled language.

With Rustup installed, run the API and worker in separate terminals from the repository root. The workspace pins Rust 1.100.0.

```sh
cargo run --locked -p ferrum-api
```

```sh
cargo run --locked -p ferrum-worker
```

```sh
curl -fsS http://127.0.0.1:8080/health/live
curl -fsS http://127.0.0.1:8080/health/ready
```

Press Ctrl+C in each process terminal to shut it down. The Phase 1 binaries can run independently with their defaults. See [configuration](configs/README.md) for overrides and [Phase 1 checks](phases/01-rust-foundation.md#verification) for validation. The [local database instructions](deployment/README.md) prepare PostgreSQL for later phases.
