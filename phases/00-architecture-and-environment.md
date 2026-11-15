# Phase 0 · Architecture and environment

The baseline repository revision `68504c3` contained design notes and a PostgreSQL Compose service, with no application workspace, migrations or worker. Phase 0 adds the dependency probe and a runbook for qualifying the Linux execution boundary.

## Decisions and deliverables

- [Architecture](../docs/architecture.md), [threat model](../docs/threat-model.md), and [submission lifecycle](submission-lifecycle.md) specify trust boundaries, persistent state and recovery. The API writes and reads tenant-scoped submissions in PostgreSQL. The trusted dispatcher and worker use the Apalis schema in the same database. Separate database roles restrict each process to its responsibilities. The sandbox receives only staged inputs and the read-only toolchain; it has no database or queue credentials, worker environment or host network access. These are required design properties.
- [Linux worker qualification](../docs/phase0-linux-worker.md) selects Debian 13 amd64, Isolate 2.7, systemd, cgroup v2 and Debian Python 3.13. It defines the initial Python limits and gives the host maintainer the setup, verifier, success criteria and evidence instructions. Qualification is pending a run on that VM.
- [Dependency probe](../tests/phase0/dependency-probe/Cargo.toml) is a standalone, unpublished crate with a [locked graph](../tests/phase0/dependency-probe/Cargo.lock) and a pinned Rust toolchain. It connects SQLx to PostgreSQL 17 and wires Axum and Apalis to confirm workspace compatibility without starting HTTP or queue processing.

## Dependency decisions and evidence

| Dependency | Version |
|---|---|
| Rust and Cargo toolchain | 1.100.0 |
| Tokio | 1.54.1 |
| Axum | 0.8.9 |
| SQLx | 0.9.0 |
| Apalis / Apalis PostgreSQL | 1.0.0 / 1.0.0 |
| PostgreSQL | 17; local Compose image `postgres:17-bookworm` |

Apalis PostgreSQL 1.0.0 uses the same SQLx 0.9 dependency as Ferrum, so the worker can pass its pool directly to the backend. The lockfile resolves one Apalis Core (1.2.0) and one SQLx version. Keep application and Apalis migrations separate; the backend's [migration configuration](https://docs.rs/apalis-postgres/latest/apalis_postgres/#upgrading-to-10) keeps queue migration history in the `apalis` schema.

From the Ferrum root:

```sh
cd tests/phase0/dependency-probe
rustc -Vv
cargo fmt --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
```

To run the probe, start local PostgreSQL using the [Compose instructions](../deployment/README.md), then run `cargo run --locked` from the probe directory with `FERRUM_PHASE0_DATABASE_URL` set to a local development database URL. Expected output includes the PostgreSQL server version number and confirms the Axum router and Apalis worker were built. The probe makes a read-only version query. Applying queue migrations and testing job delivery are Phase 4 work.

On 2026-11-16, Rust 1.100.0 (`0ef1bc386`, `aarch64-apple-darwin`) resolved and built the locked probe; formatting, checking and Clippy passed. A disposable Compose project became healthy, returned `1` from `SELECT 1`, and the Rust probe reported `SQLx connected to PostgreSQL 170012; Axum router and Apalis worker built`. The server was PostgreSQL 17.12 from `postgres:17-bookworm` (`sha256:40865bc877a289da75963d97d72b59633f639fc5af7a4e856bfa3714cae5b536`). The test project and its volume were removed. A separate locked, offline build passed with Rust 1.100.0 on `x86_64-unknown-linux-gnu` in the Debian 13 `rust:1.100.0-trixie` container. That build verifies the Linux compiler target only.

## Acceptance checks

- [ ] The Python fixture returns the expected output and the Linux host verifies teardown. **Owner:** Linux host maintainer. **Evidence:** pending VM transcript.
- [ ] The chosen host demonstrates the required limits and isolation behavior with synthetic inputs. **Owner:** Linux host maintainer. **Evidence:** pending VM transcript.
- [x] The dependency probe resolves and builds on the pinned toolchain. **Evidence:** locked build and live PostgreSQL 17.12 check above.
- [x] The candidate host profile and open constraints are recorded for Phases 1 and 3. **Evidence:** [host runbook](../docs/phase0-linux-worker.md).

The Isolate verifier was parsed and checked on macOS, where its host preflight correctly rejected execution. No kernel enforcement or teardown has been observed on the target VM. Phase 0 remains open until both Linux checks pass and the transcript is recorded.

**Handoff** · Use the pinned versions as input to [Phase 1](01-rust-foundation.md), and use the qualified host profile for [Phase 3](03-isolate-integration.md).
