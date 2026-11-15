# Technology and services

| Choice | Why | Adoption |
|---|---|---|
| Rust + Cargo workspace | Shared typed domain logic and repeatable builds for the two binaries | Phase 1 |
| Tokio + Axum | Async HTTP, I/O and lifecycle management in the same Rust runtime | Phase 1 |
| [PostgreSQL](https://www.postgresql.org/) + [SQLx](https://github.com/launchbadge/sqlx) | Durable submissions and results, SQL migrations and atomic submission/outbox writes | Local Compose definition now; app integration in Phase 2 |
| [Apalis with PostgreSQL](https://apalis.dev/docs/introduction/architecture) | Background job processing using the existing database, with no separate broker to operate initially | Compatibility evaluation in Phase 0; integration in Phase 4 |
| [Isolate](https://github.com/ioi/isolate) on dedicated Linux | Linux namespaces and cgroups for restricted execution; actual enforcement must be measured on the selected host | Manual evidence in Phase 0; adapter in Phase 3 |
| Trusted toolchains | Start with Python 3 to validate execution; C++ validates compilation next, followed by Node.js, C, Java and Rust | Minimal profile in Phase 3; full registry in Phase 5 |
| Docker Compose | Consistent local PostgreSQL setup using the [official image](https://hub.docker.com/_/postgres), with a local-only listener | Configured for development now |
| `tracing`, OpenTelemetry, Prometheus, Grafana, Loki or equivalent | Structured logs, lifecycle traces, metrics, dashboards and searchable log history | Basic tracing in Phase 1; operational stack in Phase 9 |
| Private object storage | Stores artifacts that outgrow bounded PostgreSQL fields | Deferred until size or retention needs justify it |

The initial deployment needs a public API, privately connected worker and database, and dedicated Linux execution capacity. No cloud vendor has been selected. Use PostgreSQL as the source of truth; defer Redis, shared response caches and Kubernetes until measured needs justify them. Use the [Phase 0 dependency evidence](../phases/00-architecture-and-environment.md#dependency-decisions-and-evidence) to seed the application workspace, and commit its own `Cargo.lock`.
