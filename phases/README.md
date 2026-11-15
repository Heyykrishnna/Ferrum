# Ferrum implementation roadmap

Ferrum is a modular Rust application with separately deployed API and worker processes. The API owns the request and reads the result from PostgreSQL. Apalis uses PostgreSQL for durable job delivery. Dedicated Linux workers own Isolate and the language toolchains. Submitted code stays within the execution boundary.

[Architecture sketch](reference/ferrum-handdrawn-architecture.png) · [Architecture notes](../docs/architecture.md) · [First milestone contract](../docs/first-milestone.md) · [Submission lifecycle and recovery](submission-lifecycle.md)

## Delivery sequence

| Phase | Depends on | Deliverable |
|---|---|---|
| [00 · Architecture and environment](00-architecture-and-environment.md) | Disposable Linux test host | Verified host profile, sandbox evidence, compatible dependency set |
| [01 · Rust foundation](01-rust-foundation.md) | 00 | Two runnable binaries, configuration, health checks and CI |
| [02 · Database and submissions](02-database-and-submissions.md) | 01 | Tenant-scoped create and fetch, atomic submission/outbox write |
| [03 · Isolate integration](03-isolate-integration.md) | 00–02 | One Python program runs within a verified execution boundary |
| [04 · Apalis worker pipeline](04-apalis-worker-pipeline.md) | 02–03 | Private API-to-result execution with durable recovery |
| [05 · Language runtimes](05-language-runtimes.md) | 04 | All six language profiles, including sandboxed compilation |
| [06 · Judging engine](06-judging-engine.md) | 05 | Deterministic verdicts, per-case results and scoring |
| [07 · Production API](07-production-api.md) | 06 | Tenant authentication, rate limits, batch, cancellation and signed webhooks |
| [08 · Security and reliability](08-security-and-reliability.md) | 07 | Host isolation, failure recovery and security review evidence |
| [09 · Observability and performance](09-observability-and-performance.md) | 08 | Operational dashboards, alerts and measured worker capacity |
| [10 · Production release](10-production-release.md) | 08–09 | Independent deploy, restore, drain, rollback and canary evidence |

## How the architecture maps to delivery

| Stage in the sketch | Phase that proves it |
|---|---|
| Submit, validate and commit | 02 proves input validation and atomic persistence under a private, trusted caller context. 07 adds external authentication, quotas and bounded batch intake. |
| Enqueue and claim | 04 proves durable publication, idempotent claims, execution admission and lease recovery. |
| Execute and clean | 03 proves the Linux sandbox lifecycle. 04 connects it to the queue and recovery records. 05 proves the compiled and interpreted language profiles. |
| Judge, finalize and return | 04 persists execution facts and supports polling. 06 adds test comparison and scoring. 07 adds signed delivery to registered webhook endpoints. |
| Security and operations | 00 and 03 establish host controls. 08 rehearses recovery. 09 measures capacity. 10 exercises rollout and rollback. |

## Milestones and phase reviews

- **Private execution milestone · Phase 4.** Submit one Python program with stdin and retrieve the saved result by ID after a restart. Demonstrate duplicate delivery, timeout and cleanup recovery on real infrastructure.
- **Judging milestone · Phase 6.** Exercise the six language profiles and obtain repeatable test outcomes, verdicts and scores.
- **Production candidate · Phases 7–9.** Run the external API in staging, complete security and recovery checks, and record the workload the worker fleet can safely sustain.
- **Production release · Phase 10.** Release in stages using agreed canary measures, rollback conditions and named operational owners.

Phase 0 has design notes, with Linux and dependency evidence still open. Phases 1–10 describe planned work. Each phase review should record the revision, supported environment and versions, commands or test scenarios, observed outcomes, and remaining decisions with owners. Associate the evidence with the acceptance checks in the relevant plan. Changes to a shared contract should be reflected in the affected phase and its architecture or lifecycle reference.
