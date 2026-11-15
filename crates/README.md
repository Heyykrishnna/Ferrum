# Rust modules

These are target Cargo members. Add a member when a phase introduces behavior that needs it.

| Module | Owns | Introduced |
|---|---|---|
| `config` | Typed settings and startup validation | Phase 1 |
| `runtime` | Structured process logging, signal handling and bounded shutdown | Phase 1 |
| `domain` | Submission and attempt rules, states and verdicts | Phase 2 |
| `application` | Workflows and interfaces for external effects | Phase 2 |
| `contracts` | Public DTOs and versioned job payloads | Phase 2 |
| `infrastructure` | SQLx persistence and outbox; adds Apalis as needed | Phase 2 |
| `sandbox` | Isolate lifecycle, bounded I/O, metadata and cleanup | Phase 3 |
| `compiler` | Trusted language definitions and structured compile/run plans | Phase 3 minimal Python; Phase 5 full registry |
| `judge` | Comparison, verdict aggregation and scoring | Phase 6 |

Keep framework, SQL and sandbox types outside domain interfaces. Keep persistence and sandbox details behind interfaces that express the complete operation callers need. See [architecture](../docs/architecture.md).
