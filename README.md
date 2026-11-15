# Ferrum Execution Engine

Ferrum is a planned Rust service for asynchronous code execution and judging. A client submits code, polls for a result, and receives a bounded record of what happened. Compilation and execution belong on dedicated Linux workers.

This folder contains the project structure, phase plans, architecture and local PostgreSQL definition. The Rust API, worker, migrations and Linux sandbox are future implementation work. [AGENTS.md](AGENTS.md) preserves the supplied specification.

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

Start with [Phase 0](phases/00-architecture-and-environment.md). The first milestone is [Phase 4](phases/04-apalis-worker-pipeline.md): submit a Python program through Axum, schedule it through PostgreSQL and Apalis, run it inside Isolate on Linux and retrieve the saved result. C++ follows as the first compiled language.

To prepare PostgreSQL, follow the [local database instructions](deployment/README.md). See the [technology decisions](docs/technology.md) for what each service does and when it is needed.
