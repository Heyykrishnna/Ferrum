# Ferrum

[![CI](https://github.com/Heyykrishnna/Ferrum/actions/workflows/ci.yml/badge.svg)](https://github.com/Heyykrishnna/Ferrum/actions/workflows/ci.yml)
[![PostgreSQL Integration & API Smoke](https://github.com/Heyykrishnna/Ferrum/actions/workflows/postgres-integration.yml/badge.svg)](https://github.com/Heyykrishnna/Ferrum/actions/workflows/postgres-integration.yml)
[![Linux Sandbox Security](https://github.com/Heyykrishnna/Ferrum/actions/workflows/sandbox-security.yml/badge.svg)](https://github.com/Heyykrishnna/Ferrum/actions/workflows/sandbox-security.yml)
[![Security Supply Chain](https://github.com/Heyykrishnna/Ferrum/actions/workflows/security.yml/badge.svg)](https://github.com/Heyykrishnna/Ferrum/actions/workflows/security.yml)
[![CodeQL](https://github.com/Heyykrishnna/Ferrum/actions/workflows/codeql.yml/badge.svg)](https://github.com/Heyykrishnna/Ferrum/actions/workflows/codeql.yml)

Ferrum is an asynchronous code execution and judging engine written in Rust. It accepts source code over a versioned HTTP API, persists submissions and outbox events transactionally in PostgreSQL, schedules jobs through Apalis, compiles and executes untrusted programs inside restricted Linux Isolate sandboxes, evaluates outputs across multi-case test suites, and returns bounded verdicts and resource telemetry.

Because Ferrum executes untrusted code, the system is designed around **fail-closed isolation, durable state transitions, recoverability under crash or cleanup failure, and end-to-end observability**.

---

## Architecture

Ferrum is structured as a modular Rust workspace deployed as two distinct process roles backed by PostgreSQL, plus an optional local Monaco web playground:

```text
Client / Web Playground
        │  HTTP (Bearer token, Idempotency-Key, W3C traceparent)
        ▼
  ferrum-api (Axum)
        │  Single ACID transaction: submission + test cases + outbox event
        ▼
  PostgreSQL 17 ◄─── Transactional Outbox Dispatcher ──► Apalis Queue (ferrum.execute.v1)
        ▲                                                        │
        │  Leased attempts, results, diagnostics, quarantine     │
        └────────────────── ferrum-worker (Linux) ◄──────────────┘
                                    │
                                    ▼
                        Isolate 2.7 + cgroup v2
                    (compile stage ──► execute stage)
                                    │
                                    ▼
                    Judging Engine (Exact / Whitespace / Tokens)
```

### Core Properties

- **Separation of control and execution planes:** `ferrum-api` validates requests, enforces tenant quotas and idempotency, and persists state; it never invokes compilers or sandboxes. `ferrum-worker` runs exclusively on qualified Debian 13 (`amd64` / `arm64`) Linux hosts and refuses to start as `root` or inside Docker.
- **Transactional outbox and idempotent delivery:** Submissions and outbox events commit in one PostgreSQL transaction. Queue messages carry identifiers and trace context rather than source code or credentials.
- **Explicit workflow state vs. verdict:** Processing states (`created`, `queued`, `processing`, `completed`, `failed`, `cancelled`) are tracked independently of program verdicts (`accepted`, `wrong_answer`, `compilation_error`, `runtime_error`, `time_limit_exceeded`, `memory_limit_exceeded`, `output_limit_exceeded`, `internal_error`).
- **Crash recovery and quarantine:** Running attempts hold a 30-second PostgreSQL lease refreshed every 5 seconds. Background reconciliation recovers expired leases and stuck outbox events; unresolved sandbox cleanups quarantine host capacity until verified and resolved by an operator.

---

## Supported Languages

All compilers and runtimes are resolved from an immutable, server-controlled registry ([crates/domain/src/languages.rs](crates/domain/src/languages.rs)) using fixed absolute executable paths and structured argument vectors—never shell interpolation.

| ID | Language | Profile Revision | Source File | Compile Stage | Default Run Limits (CPU / Wall / Memory) |
|---:|---|---|---|---|---|
| `1` | `python` | `python3.13-v1` | `main.py` | Interpreted (`python3 -I -B`) | 2 s / 6 s / 128 MiB |
| `2` | `cpp` | `gcc14-cpp20-v1` | `main.cpp` | `g++-14 -std=c++20 -O0` | 2 s / 6 s / 128 MiB |
| `3` | `javascript` | `node20.19-v1` | `main.cjs` | Interpreted (`node --max-old-space-size=96`) | 3 s / 6 s / 256 MiB |
| `4` | `c` | `gcc14-c17-v1` | `main.c` | `gcc-14 -std=c17 -O0` | 2 s / 6 s / 128 MiB |
| `5` | `java` | `openjdk21-v1` | `Main.java` | `javac` (OpenJDK 21, `-proc:none`) | 3 s / 6 s / 256 MiB |
| `6` | `rust` | `rust1.85-v1` | `main.rs` | `rustc --edition=2024` | 2 s / 6 s / 128 MiB |

Compilation runs inside the same sandbox boundary with bounded compilation limits (8 s CPU, 12 s wall, 512 MiB memory; 768 MiB for `rustc`).

---

## Security & Isolation Model

| Control | Enforcement |
|---|---|
| **OS Sandbox** | Isolate 2.7 with cgroup v2 unified hierarchy, read-only system mounts, disk block and inode quotas, swap disabled, `fs.protected_hardlinks=1`, direct `core_pattern` |
| **Process & Output Bounds** | Per-profile process limits, strict CPU/wall/memory/file-size caps, independent kernel pipes capped at 64 KiB for stdout and stderr, full cgroup process-tree teardown on timeout or cancellation |
| **Sterile Environment** | `env_clear()` on every child process with an explicit minimal whitelist; database URLs, tokens, and host secrets never enter sandboxes |
| **Filesystem Guardrails** | Path component validation rejecting absolute paths, `..` traversal, and workspace escapes; `0700` worker-owned recovery state |
| **Network Isolation** | Isolated network namespace per box with no external connectivity or host loopback access |
| **Multi-Tenant API Auth** | SHA-256 hashed API keys verified in constant time (`subtle`), scoped permissions (`submissions:read`, `submissions:write`, `submissions:cancel`, `webhooks:manage`, `admin`), expiration, revocation, and per-tenant concurrency quotas |
| **Webhook SSRF Defense** | Explicit scheme validation (`http`/`https`) and blocking of loopback, RFC 1918 private networks, link-local/cloud metadata (`169.254.169.254`), `.internal`, and `.local` targets, paired with HMAC-SHA256 payload signatures |

---

## HTTP API

The full contract is specified in [docs/openapi.yaml](docs/openapi.yaml) and [docs/openapi.json](docs/openapi.json).

| Method | Path | Scope / Auth | Description |
|---|---|---|---|
| `GET` | `/health/live` | Public | Process liveness probe |
| `GET` | `/health/ready` | Public | Readiness probe (verifies PostgreSQL connectivity and drain state) |
| `GET` | `/metrics` | Internal | Prometheus 0.0.4 metrics scrape endpoint |
| `GET` | `/v1/languages` | Authenticated | List supported language profiles and effective limits |
| `GET` | `/v1/execution/status` | `submissions:read` | Report Apalis worker availability (`available`, `offline`, `queue_uninitialized`) |
| `POST` | `/v1/submissions` | `submissions:write` | Enqueue an asynchronous submission (`Idempotency-Key` required, HTTP `202`) |
| `POST` | `/v1/submissions/batch` | `submissions:write` | Submit a bounded batch of up to `MAX_BATCH_SIZE` items |
| `GET` | `/v1/submissions/{id}` | `submissions:read` | Retrieve processing state, verdict, per-case findings, and resource facts |
| `POST` | `/v1/submissions/{id}/cancel` | `submissions:cancel` | Idempotently cancel a pending or running submission |
| `POST` | `/v1/api-keys` | `admin` | Issue a scoped tenant API key (raw secret returned once) |
| `GET` | `/v1/api-keys` | `admin` | List tenant API keys (metadata and prefix only) |
| `DELETE` | `/v1/api-keys/{id}` | `admin` | Revoke an API key |
| `POST` | `/v1/webhooks` | `webhooks:manage` | Register an SSRF-validated webhook endpoint |
| `GET` | `/v1/webhooks` | `webhooks:manage` | List tenant webhook endpoints |
| `DELETE` | `/v1/webhooks/{id}` | `webhooks:manage` | Delete a webhook endpoint |

Judging supports `exact`, `whitespace` (line-ending and trailing-space tolerant), and `tokens` comparison modes, weighted multi-case scoring, and hidden test-case redaction.

---

## Quick Start

### Prerequisites

- **Rust toolchain:** Pinned via [rust-toolchain.toml](rust-toolchain.toml) (`1.100.0`, Edition 2024, `clippy`, `rustfmt`).
- **PostgreSQL 17:** Provided locally via Docker Compose ([compose.yaml](compose.yaml)).
- **Node.js `>= 24.11`:** Required only for the optional local web playground under [web/](web/README.md).
- **Linux execution worker (for running submitted code):** Dedicated Debian 13 (`amd64` or `arm64`) host with Isolate 2.7 and language toolchains. See [docs/phase0-linux-worker.md](docs/phase0-linux-worker.md), [docs/phase3-isolate.md](docs/phase3-isolate.md), and [docs/macos-worker.md](docs/macos-worker.md) for Apple Silicon VMs.

### 1. Start Local Infrastructure

```sh
cp -n .env.example .env
```

Set `FERRUM_POSTGRES_PASSWORD` in `.env`, then start PostgreSQL (and optionally Prometheus and Grafana):

```sh
docker compose up -d --wait postgres
```

To include the local observability stack (Prometheus on `:9092`, Grafana on `:3000`):

```sh
docker compose up -d --wait
```

### 2. Apply Migrations and Start the API

```sh
export FERRUM_DATABASE_URL="postgres://ferrum:<password>@127.0.0.1:54329/ferrum"
export FERRUM_DEV_TENANT_ID="$(uuidgen | tr '[:upper:]' '[:lower:]')"
export FERRUM_DEV_TOKEN="$(openssl rand -hex 32)"

cargo run --locked -p ferrum-api -- migrate
cargo run --locked -p ferrum-worker -- migrate
cargo run --locked -p ferrum-api
```

### 3. Verify Endpoints

```sh
curl -fsS http://127.0.0.1:8080/health/live
curl -fsS http://127.0.0.1:8080/health/ready

curl -fsS \
  -H "Authorization: Bearer ${FERRUM_DEV_TOKEN}" \
  http://127.0.0.1:8080/v1/languages

curl -i -fsS -X POST http://127.0.0.1:8080/v1/submissions \
  -H "Authorization: Bearer ${FERRUM_DEV_TOKEN}" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: quickstart-1" \
  -d '{"language":"python","source":"print(40 + 2)","expected_output":"42\n"}'
```

### 4. Start the Linux Worker (Qualified Host)

On a qualified Debian 13 Isolate 2.7 host pointing `FERRUM_DATABASE_URL` at the same PostgreSQL instance:

```sh
cargo run --locked -p ferrum-worker
```

Operational CLI subcommands work from any host with database connectivity:

```sh
ferrum-worker migrate
ferrum-worker reconcile
ferrum-worker metrics
ferrum-worker quarantine list
ferrum-worker quarantine resolve <attempt_id>
ferrum-worker quarantine resolve-host <host_id>
```

### 5. Launch the Web Playground (Optional)

```sh
npm --prefix web ci
npm --prefix web run dev
```

Open `http://127.0.0.1:5173`. See [web/README.md](web/README.md) for build mode and gateway configuration.

---

## Configuration

Configuration is typed, rejects unknown keys (`deny_unknown_fields`), and validates all bounds at startup ([crates/config/src/lib.rs](crates/config/src/lib.rs)).

| Variable | Component | Description |
|---|---|---|
| `FERRUM_MODE` | API, Worker | `development` (default) or `production` |
| `FERRUM_LOG_LEVEL` | API, Worker | `error`, `warn`, `info` (default), `debug`, `trace` |
| `FERRUM_API_LISTEN_ADDR` | API | Bind address (default `127.0.0.1:8080`; nonzero port required in production) |
| `FERRUM_API_CONFIG` | API | Optional path to TOML configuration file |
| `FERRUM_API_SHUTDOWN_TIMEOUT_MS` | API | Graceful shutdown deadline (`1`–`60000`, default `10000`) |
| `FERRUM_DATABASE_URL` | API, Worker | PostgreSQL connection URI (`postgres://` or `postgresql://`) |
| `FERRUM_DB_MAX_CONNECTIONS` | API, Worker | Connection pool cap (`1`–`100`, default `10`) |
| `FERRUM_DB_ACQUIRE_TIMEOUT_MS` | API, Worker | Pool acquire timeout (`1`–`30000`, default `5000`) |
| `FERRUM_DEV_TENANT_ID` | API | Non-nil tenant UUID for local development intake (loopback + development mode only) |
| `FERRUM_DEV_TOKEN` | API | 64-character hexadecimal bearer token for local development intake |
| `FERRUM_WORKER_CONFIG` | Worker | Optional path to TOML configuration file |
| `FERRUM_WORKER_SHUTDOWN_TIMEOUT_MS` | Worker | Graceful drain deadline (`1`–`60000`, default `30000`) |
| `FERRUM_WORKER_METRICS_LISTEN_ADDR` | Worker | Optional HTTP listener for worker `/metrics` and `/health/live` |
| `FERRUM_WEB_API_URL` | Web | API origin (default `http://127.0.0.1:8080`; HTTPS required for non-loopback) |
| `FERRUM_WEB_API_TOKEN` | Web | Server-side bearer token (falls back to `FERRUM_DEV_TOKEN`) |

---

## Observability & Operations

- **Distributed Tracing:** W3C TraceContext (`traceparent` and `x-request-id`) is extracted or generated at API ingress, persisted across `submissions` and `outbox_events`, propagated in Apalis job payloads, and attached to worker spans without logging source code, stdin, stdout, or credentials.
- **Metrics & Dashboards:** Low-cardinality Prometheus metrics are exposed on `/metrics` and provisioned across 6 Grafana dashboards under [deployment/grafana/dashboards/](deployment/grafana/dashboards/): platform overview, queue and workers, sandbox and execution, language and verdict analytics, security and reliability, and k6 intake telemetry.
- **Alerting & Runbooks:** Prometheus alert rules ([deployment/prometheus/alerts.yml](deployment/prometheus/alerts.yml)) and unit tests ([deployment/prometheus/alerts.test.yml](deployment/prometheus/alerts.test.yml)) map directly to operational runbooks:
  - [Stuck Executions and Expired Leases](docs/runbooks/stuck-executions-and-leases.md)
  - [Sandbox Cleanup Quarantine](docs/runbooks/sandbox-cleanup-quarantine.md)
  - [Outbox and Dispatch Recovery](docs/runbooks/outbox-and-dispatch-recovery.md)
  - [Isolation Incident Containment](docs/runbooks/isolation-incident-containment.md)

---

## Testing, CI/CD & Releases

All quality, security, integration, sandbox, performance, and release pipelines live under `.github/workflows/` and are documented in [docs/ci-cd.md](docs/ci-cd.md).

| Workflow | Scope |
|---|---|
| [ci.yml](.github/workflows/ci.yml) | Rust fmt/check/clippy/test/release build, `web/` lint/typecheck/test/build, Prometheus `promtool` checks, OpenAPI/dashboard/migration contract checks, `actionlint`, `shellcheck`, and `CI Gate` |
| [codeql.yml](.github/workflows/codeql.yml) | CodeQL static security analysis (`rust` and `javascript-typescript`) |
| [security.yml](.github/workflows/security.yml) | `cargo-deny` ([deny.toml](deny.toml)), RustSec `cargo-audit` ([.cargo/audit.toml](.cargo/audit.toml)), and `npm audit` |
| [postgres-integration.yml](.github/workflows/postgres-integration.yml) | Ephemeral PostgreSQL 17 integration tests (`ferrum-infrastructure` + `ferrum-worker`) and live API/worker CLI smoke suite |
| [sandbox-security.yml](.github/workflows/sandbox-security.yml) | Locked-down Linux sandbox & 6-language toolchain containers ([deployment/phase3.Dockerfile](deployment/phase3.Dockerfile)) plus dedicated Debian 13 Isolate 2.7 host qualification |
| [performance.yml](.github/workflows/performance.yml) | Release `ferrum-api` + PostgreSQL 17 `k6` intake regression benchmark ([scripts/perf/baseline.json](scripts/perf/baseline.json)) |
| [release.yml](.github/workflows/release.yml) | Multi-arch Linux (`x86_64`, `aarch64`) & web release archives, SHA-256 manifests, CycloneDX + SPDX SBOMs, SLSA build provenance (`actions/attest`), and GitHub Releases |

Run local verification gates before pushing:

```sh
bash scripts/ci/run-local-ci.sh
```

---

## Repository Layout

| Path | Purpose |
|---|---|
| [apps/api](apps/api) | Axum HTTP API server, auth middleware, rate/quota checks, metrics, and migration CLI |
| [apps/worker](apps/worker) | Apalis execution worker, outbox dispatcher, reconciliation loop, and operator CLI |
| [crates/domain](crates/domain) | Core entities, language registry, resource policies, states, verdicts, auth scopes, and trace context |
| [crates/application](crates/application) | Application traits, idempotency key parsing, and typed use-case errors |
| [crates/contracts](crates/contracts) | Versioned cross-boundary job and outbox event payloads |
| [crates/config](crates/config) | Typed environment and TOML settings loading with strict validation |
| [crates/runtime](crates/runtime) | Structured JSON tracing, secret redaction, Prometheus registry, and graceful shutdown |
| [crates/sandbox](crates/sandbox) | Sandbox trait, Isolate 2.7 adapter, bounded pipe supervisor, metadata parser, and host qualification tests |
| [crates/judge](crates/judge) | Output comparators, multi-case evaluator, scoring, verdict precedence, and custom checker interface |
| [crates/infrastructure](crates/infrastructure) | SQLx repositories, migrations runner, Apalis PostgreSQL integration, API keys, webhooks, and integration tests |
| [migrations/](migrations/README.md) | Monotonic PostgreSQL SQL migrations (`0001`–`0006`) |
| [web/](web/README.md) | React + Vite + Monaco local execution playground and loopback gateway |
| [deployment/](deployment/README.md) | Dockerfiles, systemd units, Prometheus configs/alerts, and Grafana provisioning/dashboards |
| [scripts/](scripts) | Tracked CI verification, smoke testing, Isolate preflight, k6 regression, and release packaging helpers |
| [docs/](docs/README.md) | Architecture, threat model, OpenAPI spec, capacity report, CI/CD guide, and operational runbooks |

---

## Documentation

- [Architecture](docs/architecture.md) · [Technology Decisions](docs/technology.md) · [Threat Model](docs/threat-model.md)
- [Submission API Guide](docs/submission-api.md) · [OpenAPI YAML](docs/openapi.yaml) · [OpenAPI JSON](docs/openapi.json)
- [Linux Worker Setup](docs/phase0-linux-worker.md) · [Isolate Adapter](docs/phase3-isolate.md) · [Apple Silicon Worker VM](docs/macos-worker.md)
- [Worker Pipeline](docs/phase4-worker.md) · [Language Runtimes](docs/phase5-languages.md) · [Judging Engine](docs/phase6-judging.md)
- [Production API](docs/phase7-production-api.md) · [Security Hardening](docs/phase8-security-hardening.md) · [Observability & Performance](docs/phase9-observability-performance.md)
- [Capacity Benchmark Report](docs/capacity-report.md) · [CI/CD & Release Guide](docs/ci-cd.md)

---

## Security

Do not open public issues for suspected vulnerabilities. Follow the responsible disclosure process in [SECURITY.md](SECURITY.md).
