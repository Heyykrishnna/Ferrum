# Ferrum Execution Engine

Ferrum is a planned Rust service for asynchronous code execution and judging. A client submits code, polls for a result, and receives a bounded record of what happened. Compilation and execution belong on dedicated Linux workers.

The Rust workspace builds an Axum API with health endpoints and PostgreSQL-backed submission intake. Queue processing and sandbox execution arrive in later phases.

| Path | Purpose |
|---|---|
| [phases/](phases/README.md) | Phases 0–10, architecture sketch, implementation tasks and acceptance checks |
| [docs/](docs/README.md) | Architecture, technology choices, threat model and first milestone contract |
| [apps/](apps/README.md) | API and worker entry points |
| [web/](web/README.md) | Monaco playground for running and judging code |
| [crates/](crates/README.md) | Ownership and dependencies for Rust modules |
| [configs/](configs/README.md) | Settings, credentials and toolchain policy |
| [migrations/](migrations/README.md) | Order and ownership of database changes |
| [deployment/](deployment/README.md) | Local setup and production topology |
| [tests/](tests/README.md) | Test environments and evidence requirements |

Start with [Phase 0](phases/00-architecture-and-environment.md) and its [Linux worker verification runbook](docs/phase0-linux-worker.md). The first milestone is [Phase 4](phases/04-apalis-worker-pipeline.md): submit a Python program through Axum, schedule it through PostgreSQL and Apalis, run it inside Isolate on Linux and retrieve the saved result. C++ follows as the first compiled language.

With Rustup installed, run the API from the repository root to try its health endpoints. The workspace pins Rust 1.100.0.

```sh
cargo run --locked -p ferrum-api
```

```sh
curl -fsS http://127.0.0.1:8080/health/live
curl -fsS http://127.0.0.1:8080/health/ready
```

Press Ctrl+C to shut down the API. To enable and test submission intake, start PostgreSQL and apply the migrations. [Private submission API](docs/submission-api.md) · [Phase 2 verification](phases/02-database-and-submissions.md#verification)
