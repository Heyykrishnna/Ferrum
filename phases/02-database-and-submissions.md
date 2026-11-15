# Phase 2 · Database and submissions

**Prerequisite** · The foundation workspace and disposable PostgreSQL. [Phase 1](01-rust-foundation.md) · [Local database setup](../deployment/README.md)

## Outcome

Accept and retrieve a private, tenant-scoped submission. A successful create response means PostgreSQL has committed both the request and the event that drives execution.

## Work plan

1. Add SQLx pool settings and ordered migrations for the minimal tenant scope, submissions and outbox. Enforce tenant-scoped idempotency in PostgreSQL and index the tenant lookup and unpublished event scan. Keep Apalis backend schema setup in Phase 4. [Migration policy](../migrations/README.md)
2. Resolve tenant identity from a trusted development caller context. Validate language, source, stdin and requested limits against server policy. Store bounded payloads and the resolved profile and effective limits with the submission so later workers use the accepted policy.
3. Define typed processing states and verdicts. Keep response DTOs separate from storage models. Implement `POST /v1/submissions` and `GET /v1/submissions/{id}` using the [first milestone contract](../docs/first-milestone.md).
4. Define semantic normalization for idempotency. Concurrent requests with the same tenant, key and normalized input resolve to one submission and one creation event. Different content with the same key returns HTTP 409.
5. Commit the submission and its outbox event together. Return the submission ID after commit. The event carries routing identifiers and version information for Phase 4.

## Verification

Use disposable PostgreSQL and the HTTP routes. Apply migrations to an empty database, restart the API and fetch a committed submission. Exercise concurrent create, conflict and rollback. Compare the response for an unknown ID with the response for another tenant's ID. Inspect generated events for identifier-only payloads.

## Acceptance checks

- [ ] Valid create returns HTTP 202 with an ID backed by both committed records.
- [ ] Rollback leaves both records absent and a committed submission survives restart.
- [ ] Matching replays resolve to the existing ID, including simultaneous requests.
- [ ] A conflicting replay returns HTTP 409 and cross-tenant reads return the same HTTP 404 as unknown IDs.
- [ ] Malformed input and over-policy limits are rejected before writes.

**Handoff** · The submission record feeds trusted execution inputs and the outbox contract feeds durable publication. [Phase 3](03-isolate-integration.md) · [Phase 4](04-apalis-worker-pipeline.md)
