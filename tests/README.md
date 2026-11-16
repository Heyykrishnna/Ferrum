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

Phase 1 checks the readiness transition and shutdown deadline in Rust tests. The process smoke test checks HTTP responses, configuration failures, and SIGINT and SIGTERM handling. The current workspace runs it with `--api-only`. [Workspace verification](../phases/01-rust-foundation.md#verification)

Wire later cross-service suites into explicit Cargo test targets. Record Linux host, kernel, toolchain and Isolate versions alongside sandbox results.

Phase 2 starts a new PostgreSQL 17 Compose project with a random loopback port and temporary volume. The suite applies migrations, starts and restarts the API, and verifies concurrent replay, tenant scope, rollback, input bounds, and database readiness. Run the locked release build first. [Phase 2 verification](../phases/02-database-and-submissions.md#verification)

Phase 3 adds portable metadata tests, Linux process supervision tests and an opt-in suite for the real Isolate host. Run the first two in Docker on a Mac. The dedicated Linux host suite checks the adapter, resource limits, cancellation and recovery from a failed cleanup. [Docker checks](../docs/phase3-isolate.md#run-the-linux-checks-in-docker) · [Isolate host verification](../docs/phase3-isolate.md#qualify-the-adapter)

Phase 4 adds worker pipeline execution and leasing tests with PostgreSQL and Apalis. Run `python3 tests/phase4/worker-smoke.py` for sandbox test-double verification or with `--isolate` on the qualified Debian 13 host. [Phase 4 worker architecture](../phases/04-worker-execution.md)

Phase 5 adds multi-language execution and toolchain configuration verification for Python, C, C++, JavaScript, Java, and Rust. [Phase 5 language runtimes](../docs/phase5-languages.md)

Phase 6 adds the judging engine test suites: comparators (exact, whitespace, tokens), custom checkers, multi-case suites, partial scoring, and hidden case privacy. Run `python3 tests/phase6/judging-smoke.py` with PostgreSQL Compose or with `--isolate` on the Linux host. [Phase 6 judging engine](../docs/phase6-judging.md)

Phase 7 adds production API and developer experience verification: scoped API keys, SHA-256 constant-time authentication, permission enforcement, bounded batch submissions (1..=20 items), submission cancellation and terminal idempotency, webhook endpoint management, SSRF protection against private/loopback/cloud metadata destinations, and HMAC-SHA256 payload signing. Run `python3 tests/phase7/production-api-smoke.py` with PostgreSQL Compose. [Phase 7 production API](../docs/phase7-production-api.md)
