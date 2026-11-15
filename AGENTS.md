# AGENTS.md

# Ferrum Execution Engine: Instructions for Coding Agents

> This file is the primary operating guide for AI coding agents and human contributors working in this repository. Read it before making changes. It defines the project goals, architecture, engineering rules, security requirements, implementation sequence, and acceptance criteria.
>
> **Project principle:** Ferrum executes untrusted code. Correctness, isolation, recoverability, and observability are more important than feature count or development speed.

---

## 1. Mission and product scope

Ferrum is a Rust-based code execution and judging platform inspired by Judge0. It accepts source code and execution requests over an API, schedules work asynchronously, runs compilation and execution inside restricted Linux sandboxes, collects resource usage and output, computes a verdict, and returns the result to the caller.

The initial platform is intended to support:

- Python 3
- C
- C++
- JavaScript through Node.js
- Java
- Rust

The platform should eventually support multiple files, standard input and output, resource limits, batches, test cases, expected-output comparison, custom checkers, partial scoring, organizations, API keys, quotas, webhooks, administrative operations, and usage analytics.

### 1.1 What success looks like

The first meaningful milestone is a complete vertical slice:

1. A client submits source code to the Axum API.
2. The API validates the request and stores the submission in PostgreSQL.
3. The submission and an outbox event are committed in one database transaction.
4. An outbox dispatcher publishes a small job payload to Apalis.
5. A Rust worker claims the job and checks that the submission is still eligible to run.
6. The worker reserves execution capacity.
7. The worker creates an Isolate sandbox on Linux.
8. The worker places source files and standard input inside the sandbox.
9. Compilation, when required, happens inside the sandbox.
10. The program executes inside the sandbox with enforced operating-system limits.
11. The worker collects stdout, stderr, compiler output, exit status, signal and resource metadata.
12. The application computes a verdict and persists the result.
13. The sandbox is destroyed and cleanup is verified.
14. The client retrieves the result using the submission ID.

Do not prioritize secondary product features until this path works reliably and is covered by integration tests.

### 1.2 Out of scope for the first release

Do not introduce these features unless a maintainer explicitly prioritizes them:

- AI code review or AI-generated debugging
- Interactive terminals or browser-based shells
- GPU execution
- Network-enabled user programs
- Arbitrary user-provided Docker images or toolchains
- User-defined host executables or unrestricted compiler arguments
- Distributed compilation caches
- MicroVM migration before the Isolate baseline is tested
- Premature microservices or Kubernetes adoption

These features materially increase security and operational complexity. Keep the initial system focused on deterministic, isolated execution and judging.

---

## 2. Instructions for agents before changing code

Every agent must follow this workflow.

### 2.1 Inspect before editing

Before changing files:

1. Read this `AGENTS.md` completely.
2. Inspect the current repository structure and existing implementation.
3. Read the relevant crate's `Cargo.toml`, module structure, tests and nearby code.
4. Check `Cargo.lock` before suggesting dependency updates.
5. Search for existing abstractions before creating new ones.
6. Identify affected interfaces, database migrations, configuration and tests.
7. State the intended scope and implementation plan in the task response when working interactively.

Never assume the repository is empty or that the roadmap describes code that has already been implemented. The actual repository is the source of truth for implementation status.

### 2.2 Make focused changes

- Implement one coherent capability at a time.
- Prefer small, reviewable changes over broad rewrites.
- Do not reformat or rename unrelated files.
- Do not introduce abstractions without a concrete use case.
- Do not duplicate an existing service, repository, type or configuration mechanism.
- Preserve public API compatibility unless the task explicitly allows a breaking change.
- Update documentation and tests whenever behavior or interfaces change.
- Do not mark a roadmap item complete merely because types or placeholder functions exist.

### 2.3 Verify changes

After implementation:

1. Run formatting checks.
2. Run `cargo check` for the affected package or workspace as appropriate.
3. Run focused unit and integration tests.
4. Run Clippy where feasible.
5. Run migrations against a disposable test database if schema changes were made.
6. Run Linux sandbox tests for sandbox-related changes. A mocked sandbox test is not a substitute for a real Isolate integration test.
7. Report exactly which commands passed and which could not be run.

Do not claim a test passed if it was not executed. Do not suppress a failing test or weaken a security assertion merely to obtain a green build.

### 2.4 Do not guess security behavior

If an Isolate flag, cgroup behavior, kernel feature, Apalis API, or operating-system guarantee is unclear:

- Inspect the installed version's documentation or source.
- Add a focused experiment or integration test.
- Document version and environment assumptions.
- Fail closed when a required security control is unavailable.

Do not invent command-line flags, metadata formats, APIs or enforcement guarantees.

---

## 3. Architecture and technology decisions

### 3.1 Baseline stack

| Concern | Baseline | Responsibility |
|---|---|---|
| Language | Rust | Core application, workers and execution orchestration |
| Async runtime | Tokio | Async I/O, task scheduling and process management |
| HTTP API | Axum | Public and internal HTTP endpoints |
| Database | PostgreSQL | Durable application state and submission records |
| Database access | SQLx | Queries, transactions and migrations |
| Background jobs | Apalis with PostgreSQL backend | Durable asynchronous job processing |
| Sandbox | Isolate | Linux namespace/cgroup-based execution isolation |
| Logs and tracing | `tracing` | Structured application telemetry |
| Metrics | Prometheus-compatible instrumentation | System and workload metrics |
| Distributed traces | OpenTelemetry | End-to-end request and execution tracing |
| Dashboards | Grafana | Operational visibility |
| Log aggregation | Loki or equivalent | Centralized searchable logs |
| Local dependencies | Docker Compose | Reproducible developer environment |
| Production workers | Dedicated Linux machines | Actual untrusted-code execution |

Use compatible dependency versions. Do not assume that all Apalis ecosystem crates share the same version number. Resolve compatible versions, run `cargo check`, and commit the resulting `Cargo.lock` for applications.

### 3.2 Deployment shape

Start with a **modular monolith and separately deployed process types**:

- API process: Axum routes, authentication, validation and submission creation.
- Worker process: Apalis workers and execution orchestration.
- Optional admin process or admin module: internal operational tooling.
- PostgreSQL: durable domain data and initial Apalis backend.
- Dedicated Linux worker hosts: Isolate and language toolchains.
- Observability services: metrics, traces, dashboards and logs.

Do not split every crate into a network service. Crate boundaries are code boundaries, not necessarily deployment boundaries.

### 3.3 Suggested workspace structure

Use this as a target structure. Adapt to the existing repository instead of creating duplicate folders.

```text
ferrum/
├── AGENTS.md
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── apps/
│   ├── api/
│   ├── worker/
│   └── admin/                 # optional; do not build until needed
├── crates/
│   ├── domain/
│   ├── application/
│   ├── infrastructure/
│   ├── sandbox/
│   ├── compiler/
│   ├── judge/
│   ├── contracts/
│   └── config/
├── configs/
│   ├── application.toml
│   ├── workers.toml
│   ├── sandbox.toml
│   └── languages/
├── migrations/
├── deployment/
├── docs/
└── tests/
    ├── integration/
    ├── sandbox/
    ├── security/
    ├── concurrency/
    └── languages/
```

### 3.4 Crate responsibilities

| Crate | Owns | Must not own |
|---|---|---|
| `domain` | Entities, value objects, state transitions, verdicts, policies | Axum, SQLx, Apalis, Isolate CLI details |
| `application` | Use cases, orchestration ports, application workflows | HTTP-specific response formatting or raw SQL implementation |
| `contracts` | Serialized API/job/event contracts shared across boundaries | Business logic or infrastructure clients |
| `config` | Configuration loading, validation and typed settings | Runtime secrets hardcoded in source |
| `sandbox` | Sandbox trait, Isolate adapter, process lifecycle and metadata parsing | Submission business rules or API routes |
| `compiler` | Language definitions, toolchain resolution and compile/run plans | User-controlled executable paths |
| `judge` | Comparators, test evaluation, scoring and verdict resolution | Direct database access or direct Isolate CLI execution |
| `infrastructure` | SQLx repositories, Apalis integration, outbox, storage, telemetry adapters | Core domain policy |
| `apps/api` | HTTP routes, middleware, request/response mapping and server startup | Directly executing user source code |
| `apps/worker` | Worker startup, task registration, dependency wiring and shutdown | Duplicated domain rules |
| `apps/admin` | Internal administration surface when prioritized | Sandbox host-level privilege |

### 3.5 Dependency direction

Keep dependencies pointed inward:

```text
apps/api -----------+
                    +--> application --> domain
apps/worker --------+         |             ^
                              |             |
                       infrastructure -----+
                              |
                       sandbox/compiler/judge adapters
```

The domain layer must remain independently testable. Application workflows should depend on traits/ports where external effects are needed. Infrastructure implements those ports.

---

## 4. Core domain model

### 4.1 Submission

A submission represents a user's request to execute or judge source code. It should contain, at minimum:

- Submission ID
- Organization/tenant ID
- Language ID and resolved language version/configuration reference
- Source content or artifact reference
- Optional source files
- Standard input or input artifact reference
- Requested limits, after policy validation
- Optional expected output or test-suite reference
- Processing state
- Final verdict when available
- Idempotency key or normalized request identity
- Creation, update, start and completion timestamps

Never expose host filesystem paths, internal sandbox identifiers, worker hostnames, raw secrets or infrastructure details in public responses.

### 4.2 Execution attempt

A submission may have one or more attempts due to infrastructure recovery or explicitly designed retry behavior. Each attempt should record:

- Attempt ID
- Submission ID
- Worker identity
- Attempt number
- Execution state
- Lease/heartbeat information when applicable
- Sandbox identifier as an internal value
- Start and completion timestamps
- Infrastructure error details
- Cleanup state and cleanup error, if any

A retry must not accidentally overwrite the diagnostic evidence from a previous attempt.

### 4.3 Execution result

Keep raw execution facts separate from the final judging verdict. Capture where applicable:

- Standard output
- Standard error
- Compiler output
- Exit code
- Terminating signal
- CPU time
- Wall-clock time
- Peak memory
- File/output/process-limit information
- Termination reason
- Per-test result references

Enforce output caps while collecting output. Do not buffer unbounded stdout or stderr in worker memory.

### 4.4 Processing state and verdict are different

Processing state describes the platform workflow. Verdict describes the submitted program's outcome.

Suggested processing states:

```text
created -> queued -> processing -> completed
                               |-> failed
                               |-> cancelled
```

Suggested verdicts:

```text
accepted
wrong_answer
compilation_error
runtime_error
time_limit_exceeded
memory_limit_exceeded
output_limit_exceeded
internal_error
```

For example, a submission can be `completed` with verdict `time_limit_exceeded`. The platform successfully processed the request; the program exceeded its allowed limit.

Use explicit enums and documented transitions. Avoid free-form status strings in application logic.

---

## 5. Submission and execution workflows

### 5.1 Submission creation

The API flow should be:

1. Authenticate the API key or caller.
2. Resolve organization and permissions.
3. Validate request size, language availability and requested limits.
4. Check idempotency key within the organization scope.
5. If the key already exists with the same semantic payload, return the existing submission.
6. If the key exists with a different payload, return a conflict response.
7. In one PostgreSQL transaction, insert the submission and an outbox event.
8. Commit the transaction.
9. Return the submission ID and current processing state.

The API should not wait for compilation or execution in the normal asynchronous path.

### 5.2 Transactional outbox

Do not perform a database insert and a separate queue publish as unrelated operations. That creates a failure window in which the submission exists but no job is queued.

Use an outbox:

1. Insert submission and outbox event in the same transaction.
2. Dispatcher claims unpublished events safely.
3. Dispatcher publishes a small Apalis job containing identifiers, not full source code.
4. Dispatcher marks the event as published.
5. Duplicate publication remains possible and must be harmless.

The worker must be idempotent because the outbox and queue may redeliver work.

### 5.3 Worker execution

A worker should:

1. Receive a job containing submission, organization and attempt identifiers.
2. Load the current submission and verify its state.
3. Verify that the job belongs to the correct organization and attempt.
4. Acquire a bounded CPU/memory execution reservation.
5. Record or transition the execution attempt to a running state.
6. Resolve a trusted language configuration.
7. Create a fresh sandbox for this attempt.
8. Prepare only validated files and stdin.
9. Compile inside the sandbox if required.
10. If compilation succeeds, execute inside the sandbox.
11. Enforce CPU, wall-clock, memory, process, file and output limits.
12. Collect output and metadata, including abnormal termination information.
13. Compute the execution outcome and, if judging, test result.
14. Persist result and completion/outbox event transactionally.
15. Destroy the sandbox in a guaranteed cleanup path.
16. Release capacity reservation.
17. Acknowledge the task only after the required durable state has been written.

The exact ordering of persistence and cleanup must handle cleanup failures explicitly. Do not lose an execution result because cleanup failed, and do not silently mark cleanup successful when it failed.

### 5.4 Cancellation

Cancellation is a state transition, not just an HTTP response.

- Pending jobs should be marked cancelled and skipped by workers when claimed.
- Running jobs need a cancellation signal delivered to the worker.
- The worker must terminate the entire process tree, wait for it, collect final diagnostics where possible, and clean the sandbox.
- Repeated cancellation requests must be idempotent.
- Do not acknowledge cancellation until the application knows whether the process has stopped or records that cancellation is still being handled.
- A cancelled worker task must not leave its child process running unnoticed.

Dropping a Tokio future does not guarantee that the operating-system process tree has been terminated.

---

## 6. Apalis job processing rules

### 6.1 Initial job types

Start with a small set of explicit job contracts:

- `ExecuteSubmission`
- `FinalizeSubmission` only if finalization needs a separate durable stage
- `DeliverWebhook`
- `CleanupSandbox`
- `RecoverExecution`
- `PurgeArtifacts`

Do not create a separate queue, worker binary or service for every type before there is an operational reason.

### 6.2 Job payloads

Job payloads should contain identifiers and minimal routing metadata. Example:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecuteSubmission {
    pub submission_id: uuid::Uuid,
    pub organization_id: uuid::Uuid,
    pub attempt_id: uuid::Uuid,
}
```

Do not put source code, API keys, user secrets, large stdin, test archives or full result payloads in queue messages.

### 6.3 Retry policy

Retry transient infrastructure failures only. Use bounded retries with exponential backoff and jitter.

| Outcome | Retry policy |
|---|---|
| Wrong answer | Never retry |
| Compilation error | Never retry |
| Runtime error caused by the program | Never retry |
| Time limit exceeded | Never retry |
| Memory limit exceeded | Never retry |
| Output limit exceeded | Never retry |
| Temporary database/network/storage failure | Retry within bounded policy |
| Worker process crash | Recover/reconcile through queue and execution state |
| Sandbox initialization infrastructure failure | Retry selectively after classification |
| Cleanup failure | Schedule cleanup recovery; do not lose the original result |

Do not treat all errors as retryable. A bad retry policy can multiply resource consumption and hide real faults.

### 6.4 Heartbeats, leases and task timeouts

- Configure worker heartbeat and task lease durations intentionally.
- Ensure long-running jobs can remain alive without being incorrectly marked orphaned.
- Heartbeats do not replace application-level execution state.
- Apalis task timeout does not itself guarantee that the OS process tree is killed.
- When a worker loses a lease or is shutting down, its process supervisor must terminate or safely hand off execution according to a documented policy.
- Add a reconciliation process for attempts left in a running state after worker loss.

### 6.5 Concurrency and admission control

Apalis worker concurrency is not sufficient resource protection.

Before launching a sandbox, reserve capacity based on the validated resource policy. Track at least CPU slots and memory reservations. Use bounded worker concurrency and ensure one host cannot overcommit itself with too many concurrent sandboxes.

Do not assume a particular concurrency count is universally safe. Establish it through load tests on the actual worker hardware and language toolchains.

---

## 7. Sandbox and operating-system security

This is the highest-risk component. Treat every submitted program, source archive, filename, stdin and generated output as hostile.

### 7.1 Mandatory baseline

- Use Isolate on supported Linux hosts for untrusted execution.
- Create one isolated sandbox per execution attempt.
- Compile untrusted source inside the sandbox.
- Execute only trusted, server-configured executable paths.
- Pass arguments as an argument vector; do not construct shell command strings.
- Disable network access by default.
- Restrict filesystem visibility and writable paths.
- Apply operating-system resource limits.
- Bound all captured output.
- Restrict child process counts.
- Terminate the full process tree on cancellation, timeout or worker shutdown.
- Destroy the sandbox and reconcile leftovers.
- Never expose worker credentials, database credentials or queue credentials to sandboxed programs.

### 7.2 Resource controls

Implement and verify the following controls:

| Control | Purpose |
|---|---|
| CPU time | Bound CPU consumption |
| Wall-clock time | Bound sleep, blocking and total elapsed execution |
| Memory | Prevent worker-host memory exhaustion |
| Process/PID count | Prevent fork bombs and process storms |
| Stack | Bound stack consumption where supported |
| File size | Bound generated files |
| Output size | Bound stdout/stderr and result storage |
| Source size | Bound input size and compilation cost |
| File count | Bound multi-file submissions and archive extraction |
| Disk/workspace | Bound temporary sandbox storage |
| Network | Prevent outbound and lateral access by default |

An application configuration value is not proof that the kernel enforces a limit. Add tests that demonstrate the actual behavior under the supported kernel, cgroup configuration and Isolate version.

### 7.3 Process management

- Use `tokio::process::Command` or a carefully isolated process supervisor.
- Use fixed executable paths from trusted configuration.
- Pass arguments using structured arrays.
- Never use `sh -c`, `bash -c`, or equivalent with user-provided or source-derived strings.
- Track child process identity and process group/session as required by the host implementation.
- Kill the entire process tree on cancellation and timeout.
- Wait for process termination and reap children.
- Do not leave background descendants alive after the main process exits.
- Capture exit status, terminating signal and sandbox metadata.

### 7.4 Filesystem and archive handling

For multi-file requests or archives:

- Reject absolute paths.
- Reject `..` traversal components.
- Normalize and validate all paths before writing.
- Reject symlinks unless a narrowly defined safe policy is implemented and tested.
- Enforce file count, per-file size and total uncompressed size limits.
- Prevent overwriting reserved platform files.
- Extract only inside the sandbox workspace.
- Do not follow links out of the workspace.
- Avoid trusting client-supplied MIME types or filenames.

### 7.5 Network isolation

Network must be disabled for submitted programs by default. In particular, sandboxed programs must not be able to reach:

- PostgreSQL
- Apalis or queue internals
- Cloud metadata endpoints
- Internal admin panels
- Other worker hosts
- Other tenants' workloads
- Internal observability endpoints

Any future network-enabled execution mode requires a separate security design, explicit product policy, egress enforcement and threat review. Do not implement it as a simple boolean flag.

### 7.6 Privilege boundaries

- The API must not run as root.
- Do not run the entire worker process with broad root privileges.
- If Isolate requires privileged operations, isolate those operations behind the narrowest practical helper or deployment boundary.
- Do not grant the API unrestricted access to sandbox management.
- Do not assume Docker itself is the security boundary for hostile code.
- Do not use privileged containers as a shortcut without a documented and reviewed security design.
- Keep host mounts minimal and read-only where possible.
- Keep kernel, Isolate and compiler toolchains patched.

### 7.7 Security testing

At minimum, test:

- Infinite loop
- Long sleep
- Memory exhaustion
- Fork bomb/process storm
- Huge stdout/stderr
- Huge file generation
- Background child process
- Host filesystem access attempt
- Network access attempt
- Invalid or malicious archive paths
- Symlink traversal attempts
- Compiler crash or timeout
- Cancellation during compile and execution
- Worker termination during execution
- Sandbox cleanup failure

A passing unit test with a mocked sandbox is not evidence of isolation. Real Linux integration and adversarial testing are mandatory before production.

---

## 8. Language registry and toolchain design

### 8.1 Configuration-driven languages

Do not hardcode compiler and runtime logic throughout the worker. Use a validated, server-controlled language registry.

Each language definition should describe:

- Stable language ID and slug
- Display name
- Language/runtime version
- Toolchain image or host toolchain identity
- Source filename conventions
- Supported file patterns
- Compile executable and structured argument list, when applicable
- Runtime executable and structured argument list
- Output binary/class naming convention
- Language-specific resource defaults or policy references
- Feature capabilities such as multi-file support

Example conceptual definition:

```json
{
  "id": 71,
  "slug": "python",
  "name": "Python 3",
  "version": "3.x",
  "source_file": "main.py",
  "compile": null,
  "run": {
    "program": "/usr/bin/python3",
    "args": ["main.py"]
  }
}
```

This is illustrative only. Validate the final schema and paths against the actual toolchain packaging.

### 8.2 Initial language order

1. Python 3: validate the runtime-only path.
2. C++: validate compile-then-run.
3. Node.js: validate another runtime-only path.
4. C: reuse the native compilation path.
5. Java: handle compilation and JVM execution.
6. Rust: handle `rustc` compilation and native execution.

Build and test one language at a time. Each language must have a smoke test, compile-failure test if applicable, runtime-failure test, stdin/stdout test and resource-limit test.

### 8.3 Toolchain isolation

Compiler and runtime binaries must be available within the sandboxed execution environment. Use pinned, known toolchain versions. Do not let users choose arbitrary binaries or host paths.

If using separate toolchain images or mounted compiler roots, document and test exactly what is visible inside each sandbox. Do not expose unrelated host files or secrets through mounts.

### 8.4 Compilation cache

Defer compilation caching until execution correctness and isolation are stable. When implemented, cache keys must include source content, compiler version, compiler flags, language configuration and relevant dependencies. Enforce tenant/artifact isolation and bounded storage. Cache hits must not reuse a prior execution sandbox.

---

## 9. Judging engine

The judging engine evaluates program output. It should be independent from the sandbox adapter and should consume execution results through application-level interfaces.

### 9.1 Comparison modes

Support modes only when their semantics are documented and tested:

- Exact byte comparison
- Explicit whitespace-tolerant comparison
- Custom checker execution
- Partial scoring over multiple test cases

Do not silently normalize output in ways that cause incorrect answers to pass. Preserve raw output and make the comparison policy explicit per problem or request.

### 9.2 Test-case model

A test case may contain:

- Test-case ID
- Problem or test-suite ID
- Input or input artifact reference
- Expected output or expected-output artifact reference
- Per-case limits or policy reference
- Weight/score
- Visibility metadata (public/hidden)
- Ordering and enabled state

Store per-test execution and comparison results. Aggregate them into a final verdict using deterministic, documented precedence rules.

### 9.3 Custom checkers

A custom checker is also untrusted executable code. Run it through the same sandbox security boundary with tighter, explicit policies. Never execute a checker directly on the worker host.

### 9.4 Test scheduling

Initially, one parent judging task should orchestrate cases sequentially or with strictly bounded concurrency. Do not fan out thousands of test cases into independent jobs without measured justification. The parent must be able to cancel remaining work and aggregate partial results safely.

---

## 10. API design and developer experience

### 10.1 Initial routes

Use a versioned API prefix such as `/v1`.

| Method | Route | Purpose |
|---|---|---|
| `POST` | `/v1/submissions` | Create asynchronous submission |
| `GET` | `/v1/submissions/{id}` | Retrieve state and result |
| `DELETE` | `/v1/submissions/{id}` | Request cancellation |
| `POST` | `/v1/submissions/batch` | Create a bounded batch |
| `GET` | `/v1/submissions/batch` | Retrieve batch outcomes, if needed |
| `GET` | `/v1/languages` | List supported languages |
| `GET` | `/v1/statuses` | List processing states and verdicts |
| `GET` | `/v1/me` | Identify authenticated API-key owner |
| `GET` | `/health/live` | Process liveness |
| `GET` | `/health/ready` | Dependency/readiness status |
| `GET` | `/metrics` | Private metrics endpoint |

Keep API response DTOs separate from database models and internal domain representations.

### 10.2 API requirements

- Validate request bodies and query parameters.
- Enforce maximum source size, file count, input size and batch size.
- Validate requested limits against server policy.
- Use consistent error response structures.
- Document status codes and retry semantics.
- Add pagination where collections can grow.
- Use idempotency keys for submission creation.
- Scope all tenant-owned reads and writes by authenticated organization.
- Do not reveal whether another tenant owns a specific submission ID.
- Do not expose internal host or sandbox details.
- Keep the synchronous execution path out of the default API.
- Generate and maintain an OpenAPI specification when the route surface is stable.

### 10.3 API key security

- Generate high-entropy API secrets.
- Store only a secure hash or keyed digest suitable for verification, never the raw secret.
- Return the raw secret only once at creation time.
- Support revocation, expiration and scopes.
- Never log API keys or authorization headers.
- Use constant-time comparison where appropriate for secret verification.
- Separate administrative identity and permissions from ordinary API keys.

### 10.4 Webhooks

Add webhooks only after polling and result persistence are reliable.

Suggested events:

- `submission.queued`
- `submission.started`
- `submission.completed`
- `submission.failed`
- `submission.cancelled`

Use signed payloads, a unique event/delivery ID, timestamps, bounded retries and delivery history. Protect webhook delivery against SSRF using destination validation and egress restrictions. Encrypt webhook secrets at rest or use a secrets manager. Treat duplicate delivery as expected; consumers should be able to deduplicate by event ID.

---

## 11. Database and persistence rules

### 11.1 Initial tables

Expected logical entities include:

- `organizations`
- `api_keys`
- `languages`
- `submissions`
- `execution_attempts`
- `execution_results`
- `outbox_events`
- `webhook_endpoints`
- `test_cases`
- `test_results`
- `usage_records`

Do not create every table before its feature is implemented. Add migrations incrementally with clear ownership and indexes.

### 11.2 Database rules

- Use migrations for all schema changes.
- Avoid editing a migration that has already been applied in shared environments; add a new migration instead.
- Use transactions for multi-record state transitions.
- Add unique constraints for idempotency and other invariants.
- Add indexes based on actual access patterns, such as organization plus creation time, status plus creation time, lease/heartbeat state and unpublished outbox records.
- Keep query result sizes bounded.
- Use connection pools with explicit limits and timeouts.
- Never hold a database transaction open while user code executes.
- Persist enough diagnostic information to recover and investigate failures.
- Define data retention and deletion policies.
- Test backup restoration, not only backup creation.

### 11.3 Large artifacts

PostgreSQL may store source and small input payloads initially. For large artifacts, use private object storage behind an application abstraction. Do not expose object storage credentials to workers' submitted programs. Use short-lived internal access or worker-side service credentials that remain outside the sandbox.

---

## 12. Configuration and secrets

Configuration should be typed and validated at startup. A possible split is:

```text
configs/
├── application.toml
├── workers.toml
├── sandbox.toml
└── languages/
    ├── python.json
    ├── cpp.json
    ├── javascript.json
    ├── c.json
    ├── java.json
    └── rust.json
```

Requirements:

- Validate required paths, limits, URLs and timeouts.
- Fail startup or readiness when required infrastructure or sandbox configuration is invalid.
- Keep secrets out of committed files.
- Provide `.env.example` with placeholders only.
- Do not log secret values.
- Document precedence between defaults, config files and environment variables.
- Do not silently accept unknown security-sensitive settings.
- Make language configuration immutable to ordinary tenants.

---

## 13. Observability and operations

### 13.1 Trace a submission end to end

Use one trace/correlation context across the lifecycle where possible. Useful spans include:

```text
api.request
  -> submission.validate
  -> submission.persist
  -> outbox.publish
  -> apalis.claim
  -> execution.admission
  -> sandbox.initialize
  -> compiler.execute
  -> runtime.execute
  -> metadata.collect
  -> verdict.calculate
  -> result.persist
  -> sandbox.cleanup
  -> webhook.dispatch
```

Do not put source code, stdin, credentials or unbounded program output into trace attributes.

### 13.2 Metrics

Track at least:

**API**
- Request count, duration and error rate
- Authentication failures and rate-limit rejections

**Queue**
- Pending/running/failed jobs
- Queue wait duration
- Retry count and task age

**Workers**
- Active workers and execution slots
- CPU/memory usage and capacity reservations
- Worker crashes, restarts and drain duration

**Sandbox**
- Initialization and cleanup duration
- Cleanup failures and orphan count
- Execution CPU, wall time and peak memory
- Timeout and resource-limit violations

**Languages**
- Submission count by language
- Compilation time and failure count
- Verdict distribution

**Database and storage**
- Pool saturation and query latency
- Outbox age and unpublished count
- Artifact size, upload failures and retention cleanup

Avoid high-cardinality metric labels such as submission ID, attempt ID, user ID or raw organization ID. Use traces/logs for per-submission diagnostics.

### 13.3 Operational dashboards

Create dashboards for:

1. Platform overview
2. Queue and workers
3. Sandbox and execution performance
4. Language and verdict analytics
5. Security and reliability

### 13.4 Latency decomposition

Measure end-to-end duration as separate stages:

```text
T_total = T_queue + T_admission + T_sandbox
        + T_compile + T_execute + T_persist
```

Do not optimize a component until measurements show it is a bottleneck.

---

## 14. Testing strategy

Testing is part of the feature, not a later cleanup task.

### 14.1 Unit tests

Cover:

- Request and language validation
- Resource limit policy
- Submission state transitions
- Idempotency semantics
- Error classification
- Verdict resolution and precedence
- Output comparators
- Score calculation
- Configuration validation
- Retry classification
- Filename and archive path validation

### 14.2 Integration tests

Use real PostgreSQL and the configured Apalis backend for:

- Submission creation
- Transactional outbox publication
- Duplicate event/job delivery
- Worker processing
- Result persistence
- Retry behavior
- Worker recovery
- Tenant-scoped reads and writes

Mocks may be used for isolated unit tests, but do not claim end-to-end queue reliability from mocked queue tests.

### 14.3 Linux sandbox tests

Run on supported Linux infrastructure with the real Isolate version and kernel configuration:

- Python hello world
- C/C++ compile and execute
- Invalid source / compiler error
- Missing runtime executable
- Standard input and output
- Infinite loop
- Long sleep
- Memory limit violation
- Process limit violation
- Huge stdout/stderr
- Large file generation
- Background child process
- Network access attempt
- Filesystem access attempt
- Multi-file submission
- Cancellation during compile and runtime
- Sandbox initialization failure
- Sandbox cleanup failure

### 14.4 Load tests

Increase concurrency progressively (for example 10, 50 and 100 concurrent requests) and measure:

- p50/p95/p99 end-to-end latency
- Queue wait time
- Worker saturation
- Host CPU and memory
- Database pool usage and query latency
- Sandbox startup cost
- Cleanup backlog
- Failure rate under saturation

The values are test stages, not default production concurrency settings.

### 14.5 Chaos and recovery tests

Test:

- Kill a worker during compilation.
- Kill a worker during execution.
- Restart PostgreSQL during queue activity.
- Make object storage temporarily unavailable.
- Interrupt outbox dispatch.
- Cancel a running submission repeatedly.
- Simulate sandbox cleanup failure.
- Deploy while workers are processing jobs.

Acceptance requires that unfinished executions can be reconciled, results are not silently lost, duplicate processing is controlled, and no abandoned sandbox is left untracked.

---

## 15. Development phases and acceptance gates

The roadmap is ordered by dependencies. It is a target sequence, not a promise of calendar duration. Security work starts as soon as sandbox work starts and continues through release.

### Phase 0: Architecture and environment

**Goal:** Validate the execution boundary and settle initial decisions.

Tasks:
- Inspect the repository and document the actual current state.
- Record architecture, data flow and threat model.
- Select a supported Linux development/CI environment.
- Install and manually validate Isolate.
- Verify actual namespace and cgroup behavior.
- Resolve compatible Rust, Apalis and Apalis PostgreSQL versions.
- Document initial resource policies and language scope.

Acceptance:
- A simple program executes manually inside Isolate on Linux.
- Resource restrictions and cleanup are demonstrated, not assumed.
- Version and environment assumptions are documented.

### Phase 1: Rust workspace and foundation

**Goal:** Establish maintainable application structure.

Tasks:
- Create or align the workspace and crates.
- Add typed configuration and validation.
- Add shared error handling and structured tracing.
- Start Axum API and worker entry points.
- Add health and readiness routes.
- Add CI for fmt, check, Clippy and tests.
- Add graceful shutdown behavior.

Acceptance:
- Workspace builds.
- API and worker start.
- Invalid configuration fails clearly.
- CI checks pass.

### Phase 2: Database and submission lifecycle

**Goal:** Persist submissions reliably before execution exists.

Tasks:
- Add PostgreSQL and SQLx configuration.
- Add initial migrations.
- Implement domain models and repositories.
- Implement submission creation and retrieval.
- Add processing states and verdict types.
- Implement organization-scoped idempotency.
- Add transactional outbox schema and dispatcher contract.

Acceptance:
- A submission survives API restart.
- Duplicate idempotency request returns the existing submission.
- Same idempotency key with a different payload is rejected.
- Submission and outbox event commit atomically.

### Phase 3: Isolate integration

**Goal:** Execute one program safely on Linux.

Tasks:
- Define the sandbox abstraction.
- Implement Isolate adapter.
- Prepare files and stdin safely.
- Implement trusted process invocation with structured args.
- Collect stdout, stderr, exit status and metadata.
- Enforce and test initial resource limits.
- Implement cancellation, process-tree termination and cleanup.
- Add orphan sandbox reconciliation.

Acceptance:
- Hello-world program executes inside Isolate.
- Infinite loop, memory pressure and output flood are restricted as expected.
- Host/network access tests demonstrate policy enforcement.
- Sandbox cleanup is verified after success, failure and cancellation.

### Phase 4: Apalis worker pipeline

**Goal:** Connect API submissions to sandbox execution.

Tasks:
- Add compatible Apalis PostgreSQL integration.
- Implement small job contracts.
- Implement outbox publishing.
- Implement worker handler and state checks.
- Add resource admission control.
- Add bounded retries and error classification.
- Add worker heartbeat, graceful drain and recovery reconciliation.

Acceptance:
- Submission flows from API through Apalis and Isolate to persisted result.
- Duplicate jobs do not corrupt state or cause uncontrolled concurrent execution.
- Worker termination does not permanently strand execution records.

### Phase 5: Language runtimes

**Goal:** Support the six initial languages through configuration.

Tasks:
- Implement validated language registry.
- Add Python runtime.
- Add C++ compile/run.
- Add Node.js runtime.
- Add C compile/run.
- Add Java compile/run.
- Add Rust compile/run.
- Add language smoke and failure tests.
- Add multi-file handling only after single-file paths are stable.

Acceptance:
- Every language has working smoke tests and meaningful failure tests.
- All compilation and execution remains inside the sandbox.
- No user-controlled executable path or shell interpolation exists.

### Phase 6: Judging engine

**Goal:** Evaluate correctness across test cases.

Tasks:
- Implement exact byte comparison.
- Implement explicitly defined whitespace-tolerant mode.
- Add test-case persistence and retrieval.
- Execute multiple test cases with bounded concurrency.
- Store per-test outcomes.
- Implement aggregate verdict and scoring.
- Add custom checker execution inside a sandbox.

Acceptance:
- Correct, incorrect, compile-error and resource-limit submissions receive correct outcomes.
- Per-test and aggregate results are deterministic and tested.

### Phase 7: Production API and developer experience

**Goal:** Make Ferrum safe and usable for external integrations.

Tasks:
- Add organizations and API key management.
- Add key hashing, scopes, expiry and revocation.
- Add tenant isolation to every data access path.
- Add request quotas and rate limiting.
- Add bounded batch APIs.
- Add cancellation API behavior.
- Add OpenAPI documentation.
- Add webhook signing, delivery history and safe retry.
- Add minimal admin operations tooling.

Acceptance:
- Tenant isolation tests pass.
- Secrets are not persisted or logged in raw form.
- API limits and idempotency behavior are documented.
- Webhook SSRF controls are tested before enabling delivery to arbitrary URLs.

### Phase 8: Security and reliability hardening

**Goal:** Validate hostile-input handling and recovery under failure.

Tasks:
- Run adversarial sandbox suite.
- Verify host, filesystem and network boundaries.
- Test cancellation and child process cleanup.
- Test worker and database failure recovery.
- Add stuck-execution and orphan-sandbox reconciliation.
- Add operational runbooks and incident procedures.
- Review privilege boundaries and secret exposure.
- Arrange independent security review before broad public exposure.

Acceptance:
- Security tests pass on supported Linux hosts.
- Failure injection does not leave permanently stuck execution state.
- Every sandbox cleanup failure is observable and recoverable.

### Phase 9: Observability and performance

**Goal:** Measure and diagnose the full execution lifecycle.

Tasks:
- Add trace propagation and spans.
- Add Prometheus metrics.
- Add dashboards and alerts.
- Add structured worker/sandbox logs with redaction.
- Run staged load tests.
- Establish measured capacity and safe concurrency limits.
- Optimize only measured bottlenecks.

Acceptance:
- A submission can be traced end to end.
- Queue, sandbox, compile, execution and persistence times are distinguishable.
- Saturation and cleanup failures are visible.
- A load-test report documents the tested hardware and limits.

### Phase 10: Production release

**Goal:** Deploy with controlled operational risk.

Tasks:
- Separate API and worker images/processes.
- Provision dedicated Linux worker machines.
- Configure Isolate and pinned toolchains.
- Configure private networking and TLS.
- Configure secret management.
- Configure database backups and test restore.
- Configure artifact and data retention.
- Test worker draining and deployment rollback.
- Canary release and monitor before wider rollout.
- Publish API docs and operational status guidance.

Acceptance:
- API and worker deploy independently.
- Backups are restorable.
- Rollback and graceful drain are exercised.
- Production readiness checklist is signed off.

---

## 16. Suggested first 10 working days

Use this as an initial task sequence, not a fixed deadline.

| Day | Focus | Concrete result |
|---:|---|---|
| 1 | Repository and architecture | Workspace plan, architecture document, threat model |
| 2 | Foundation | Axum app, worker binary, tracing, health endpoints |
| 3 | Database | SQLx connection, PostgreSQL migrations and repository skeleton |
| 4 | Submission API | Create/retrieve submission, validation and idempotency |
| 5 | Linux and Isolate | Manual sandbox execution on Linux |
| 6 | Rust sandbox adapter | Sandbox trait, Isolate initialization, output and metadata |
| 7 | Limits and cleanup | CPU/wall/memory/process/output controls, process termination |
| 8 | First language pipeline | Python runtime and C++ compile/run inside sandbox |
| 9 | Apalis | Job contract, PostgreSQL backend, outbox dispatcher and worker |
| 10 | Vertical slice | API submission executes and returns persisted result |

Do not proceed by creating empty files for every phase. Finish and test a vertical slice before adding more layers.

---

## 17. Definition of done for a feature

A feature is not complete until all applicable points are satisfied:

- [ ] Requirements and edge cases are understood.
- [ ] Existing architecture and interfaces were inspected.
- [ ] Types and boundaries are explicit.
- [ ] Input is validated at the trust boundary.
- [ ] Errors are classified and observable.
- [ ] Security and tenant implications are considered.
- [ ] Resource usage is bounded where relevant.
- [ ] Unit tests cover domain behavior.
- [ ] Integration tests cover external boundaries.
- [ ] Linux tests cover real sandbox behavior when applicable.
- [ ] Migrations and configuration are included when needed.
- [ ] Documentation is updated.
- [ ] Formatting, checks, tests and Clippy have been run where feasible.
- [ ] The implementation does not leave TODO placeholders in a supposedly complete path.
- [ ] Known limitations and unrun tests are stated honestly.

---

## 18. Coding standards

### 18.1 Rust

- Prefer explicit types and clear error enums at domain boundaries.
- Use `thiserror` for library/domain error types where suitable and `anyhow` mainly at application entry points when contextual propagation is useful.
- Avoid `unwrap()` and `expect()` in production paths. They may be acceptable in tests or for proven compile-time invariants with an explanatory message.
- Do not use panics for expected user or infrastructure errors.
- Avoid holding synchronous locks across `.await`.
- Avoid unbounded task spawning and unbounded channels.
- Bound process output, request bodies, database result sets and queues.
- Use cancellation-aware async code for long-running worker operations.
- Do not hide errors with `.ok()` or empty fallback values unless the behavior is explicitly safe and documented.
- Keep public types and APIs documented where they form crate boundaries.
- Prefer small modules with one clear responsibility.

### 18.2 Async and process execution

- Use Tokio for async I/O and process orchestration.
- Keep CPU-heavy work from blocking the async runtime.
- Use bounded concurrency.
- Track child processes and cancellation explicitly.
- Set process environment deliberately; do not pass the worker's entire sensitive environment to untrusted programs.
- Never concatenate source, stdin, filenames or user values into shell commands.

### 18.3 Errors and logging

- Classify user-program outcomes separately from platform/infrastructure failures.
- Include trace/request/submission/attempt correlation identifiers in structured logs where safe.
- Never log API secrets, database passwords, webhook signing secrets or full source code by default.
- Bound and redact user-controlled output before logging.
- Preserve enough information for diagnosis without exposing sensitive data.

### 18.4 Naming

Use descriptive names that communicate domain meaning. Avoid ambiguous names such as `data`, `thing`, `utils`, `manager` or `handler` when a more specific name is available. Use consistent terminology for submission, attempt, execution result, processing state and verdict.

---

## 19. Dependency policy

Before adding a dependency:

1. Check whether the standard library or an existing crate already solves the problem.
2. Check maintenance activity, license, security history and platform requirements.
3. Check compatibility with the repository's Rust toolchain and current dependencies.
4. Prefer mature, narrowly scoped dependencies.
5. Avoid adding a large framework for a small helper.
6. Pin through `Cargo.lock` for deployed applications and libraries where appropriate.
7. Explain why the dependency is needed in the change summary.

Do not upgrade the entire dependency graph as an unrelated part of a feature task.

---

## 20. Deployment principles

### 20.1 Local development

Docker Compose may run supporting services such as PostgreSQL, Prometheus and Grafana. Real Isolate behavior must be tested on a supported Linux host. A Mac developer environment can be used for API and domain work, but it is not proof of Linux sandbox correctness.

### 20.2 Production

- Run API and execution workers as separate process types.
- Run workers on dedicated, hardened Linux hosts.
- Keep databases, queue storage and dashboards private.
- Avoid broad privileged containers.
- Use private networking between control-plane services.
- Keep the sandbox network isolated from internal services.
- Pin compiler/runtime toolchains.
- Configure backup, retention, patching and incident response.
- Scale workers according to measured resource capacity, not request rate alone.
- Drain workers before deployment termination.

### 20.3 Kubernetes

Kubernetes is optional. Do not introduce it until there is a clear requirement for orchestration, scheduling, fleet management or scaling that is not reasonably handled by a simpler deployment. Kubernetes does not automatically make untrusted code execution secure.

---

## 21. Agent task execution template

When an agent is assigned a task, use this structure in its work plan or final report:

```text
Task:
What behavior is being implemented?

Repository findings:
Which existing crates, files, abstractions and tests are relevant?

Plan:
1. ...
2. ...
3. ...

Interfaces and data changes:
Which types, APIs, migrations, events or config values change?

Security and failure considerations:
What untrusted inputs, resource usage, retries, cancellation or tenant boundaries are involved?

Implementation:
What was changed and why?

Verification:
- cargo fmt --check: PASS/FAIL/NOT RUN
- cargo check: PASS/FAIL/NOT RUN
- cargo test: PASS/FAIL/NOT RUN
- cargo clippy: PASS/FAIL/NOT RUN
- Linux sandbox integration tests: PASS/FAIL/NOT RUN/NOT APPLICABLE

Limitations:
What remains incomplete or unverified?
```

Do not report `PASS` unless the command actually ran and succeeded. If Linux sandbox infrastructure is unavailable, say so and identify which behavior remains unverified.

---

## 22. Final priorities

When trade-offs arise, use this order:

1. **Security and isolation**: untrusted code must not compromise the worker host or other tenants.
2. **Correctness**: states, verdicts, limits and results must be accurate.
3. **Reliability**: retries, worker crashes, cancellation and cleanup must be recoverable.
4. **Observability**: failures and bottlenecks must be diagnosable.
5. **Performance**: optimize measured bottlenecks while preserving the above.
6. **Feature breadth**: add additional capabilities only after the core is dependable.

The first deliverable is not a large dashboard or a long list of language IDs. It is a complete, tested, recoverable path from API submission to sandboxed execution and persisted result.
