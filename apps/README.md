# Applications

Phase 1 introduced `ferrum-api` and `ferrum-worker`. Both load validated configuration, write JSON logs to stderr, and shut down on SIGINT or SIGTERM. The worker waits for cancellation until Phase 4 connects the queue. See [local startup](../README.md) and [Phase 1](../phases/01-rust-foundation.md).

| Target | Owns |
|---|---|
| `apps/api` | Axum routing, body limits, authenticated caller context, response mapping and health endpoints. Calls submission use cases. |
| `apps/worker` | Job registration, bounded intake and graceful drain. Wires execution to the trusted language registry and Linux sandbox adapter. |

The API never invokes a sandbox. Privileged host operations remain behind a narrowly scoped worker seam. Add administrative operations once Phase 7 establishes permissions and auditing.
