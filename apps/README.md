# Applications

Create the two Rust binaries in [Phase 1](../phases/01-rust-foundation.md). Each owns startup, typed configuration, dependency wiring, telemetry and shutdown.

| Target | Owns |
|---|---|
| `apps/api` | Axum routing, body limits, authenticated caller context, response mapping and health endpoints. Calls submission use cases. |
| `apps/worker` | Job registration, bounded intake and graceful drain. Wires execution to the trusted language registry and Linux sandbox adapter. |

The API never invokes a sandbox. Privileged host operations remain behind a narrowly scoped worker seam. Add administrative operations once Phase 7 establishes permissions and auditing.
