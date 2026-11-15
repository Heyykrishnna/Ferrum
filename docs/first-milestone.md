# First milestone: API to persisted result

Prove one single-file Python execution before expanding the language matrix.

| Surface | Expected behavior |
|---|---|
| `POST /v1/submissions` | Validates language, source, stdin and requested limits; creates the submission and outbox record in one transaction; responds with HTTP 202 and the submission ID. |
| `GET /v1/submissions/{id}` | Returns the tenant's processing state and bounded result when ready. An unknown or other-tenant ID receives the same 404 response. |
| Idempotency | Same tenant, key and normalized request return the existing ID; reusing the key for different content returns HTTP 409. |
| Program outcome | Keep the state, exit facts, output, resource measurements and verdict distinct. A normal execution need not imply that any expected output was judged. |
| Scope | Private development API with a trusted caller context and explicit tenant scoping. External API keys, webhooks, multi-file input and test suites arrive later. |

Phase 2 defines the local intake and polling contract. [Private submission API](submission-api.md)

Exercise success and failure against real PostgreSQL, Apalis and the supported Linux worker. Record the submission ID and verify a durable result after restart. Deliver the same job concurrently and verify at most one eligible attempt runs. Interrupt a worker and verify old attempt ownership cannot overwrite the recovered result. Verify resource enforcement, cleanup and a useful diagnostic for failed teardown. Attach these results before marking Phase 4 complete.
