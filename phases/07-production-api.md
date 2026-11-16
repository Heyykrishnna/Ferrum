# Phase 7 · Production API

**Prerequisite** · Stable polling, persisted execution results and deterministic judging. [Phase 4](04-apalis-worker-pipeline.md) · [Phase 6](06-judging-engine.md)

## Outcome

Deliver a documented HTTPS/JSON surface for web apps, IDEs, CLIs, LMS integrations and CI clients. Attribute every operation to an authenticated tenant and apply service quotas before accepting work.

## Work plan

1. Add organizations and scoped API keys with expiration and revocation. Issue the raw secret only at creation and retain verification material. Derive the caller's tenant and permissions from authentication. Track privileged operations in an audit trail.
2. Extend tenant-scoped submit and fetch with bounded batches, rate limits and quota enforcement. Define batch admission, per-item errors, replay behavior and pagination. Establish which usage events count and how retries avoid double charging.
3. Wire cancellation through durable intent and the worker's process lifecycle. Pending jobs can be cancelled before execution. Active jobs report cancellation as pending until the process stops. Resolve completion races with guarded writes and keep a committed terminal result stable. [Lifecycle and recovery](submission-lifecycle.md)
4. Publish OpenAPI for create, fetch, cancel, batch and discovery routes. Include scopes, limits, status and verdict fields, error shapes, idempotency behavior and polling examples.
5. Deliver signed webhook events from committed lifecycle records. Use stable event and delivery IDs, timestamps, bounded retry and delivery history. Validate destinations and restrict outbound access independently of the sandbox network policy. Manage webhook secrets outside logs and client-visible responses.
6. Add minimal tenant and job support tooling with separate admin permissions and audit records.

## Acceptance checks

- [x] Tenant scope holds for create, read, list, batch, cancel and webhook configuration under cross-tenant tests.
- [x] Expired, revoked and under-scoped keys fail. Logs and persisted records contain no raw API keys.
- [x] Over-limit requests and duplicate or conflicting batch inputs follow the documented contract.
- [x] Repeated cancellation and completion races produce one stable terminal outcome.
- [x] Webhook signatures, deduplication, retry limits and destination/egress policy are tested before arbitrary destinations are enabled.
- [x] OpenAPI matches the deployed staging behavior.

**Handoff** · Exercise the complete staging surface under security and operational failure scenarios. [Phase 8](08-security-and-reliability.md)
