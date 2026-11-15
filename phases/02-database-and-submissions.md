# Phase 2 · Database and submissions

**Prerequisite** · The foundation workspace and disposable PostgreSQL. [Phase 1](01-rust-foundation.md) · [Local database setup](../deployment/README.md)

## Outcome

Accept and retrieve a private, tenant-scoped submission. A successful create response means PostgreSQL has committed both the request and the event that drives execution. [Startup and HTTP contract](../docs/submission-api.md)

## Work plan

1. Add SQLx pool settings and ordered migrations for the minimal tenant scope, submissions and outbox. Enforce tenant-scoped idempotency in PostgreSQL and index the tenant lookup and unpublished event scan. Keep Apalis backend schema setup in Phase 4. [Migration policy](../migrations/README.md)
2. Resolve tenant identity from a trusted development caller context. Validate language, source, stdin and requested limits against server policy. Store bounded payloads and the resolved profile and effective limits with the submission so later workers use the accepted policy.
3. Define typed processing states and verdicts. Keep response DTOs separate from storage models. Implement `POST /v1/submissions` and `GET /v1/submissions/{id}` using the [first milestone contract](../docs/first-milestone.md).
4. Define semantic normalization for idempotency. Concurrent requests with the same tenant, key and normalized input resolve to one submission and one creation event. Different content with the same key returns HTTP 409.
5. Commit the submission and its outbox event together. Return the submission ID after commit. The event carries routing identifiers and version information for Phase 4.

## Verification

From the workspace root, run the Rust checks and the HTTP suites with Python 3.10 or newer and a running Docker daemon with Compose.

```sh
cargo fmt --all --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
python3 -I tests/phase1/process-smoke.py --api-only
python3 -I tests/phase2/submission-smoke.py
```

The Phase 2 suite runs migrations on a fresh PostgreSQL 17 database, exercises the release API twice across a restart, and removes its Compose project and volume. It compares cross-tenant and unknown-ID responses, checks that the database contains a single event for concurrent replays, and forces an outbox write failure to verify rollback. The workflow runs these commands on pushes and pull requests. [Rust workflow](../.github/workflows/rust.yml)

## Acceptance checks

- [x] Valid create returns HTTP 202 with an ID backed by both committed records.
- [x] Rollback leaves both records absent and a committed submission survives restart.
- [x] Matching replays resolve to the existing ID, including simultaneous requests.
- [x] A conflicting replay returns HTTP 409 and cross-tenant reads return the same HTTP 404 as unknown IDs.
- [x] Malformed input and over-policy limits are rejected before writes.

Rust 1.100.0 on macOS arm64 passed formatting, compilation, Clippy, workspace tests, and the locked release build. The release API passed both HTTP suites with PostgreSQL 17 in a disposable Compose project. The Phase 2 suite also verified credential redaction, startup restrictions, and independent liveness during a database outage. The workflow passed `actionlint`. Its hosted run is pending a push.

**Handoff** · The submission record feeds trusted execution inputs and the outbox contract feeds durable publication. [Phase 3](03-isolate-integration.md) · [Phase 4](04-apalis-worker-pipeline.md)
