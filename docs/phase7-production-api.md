# Phase 7 · Production API and Developer Experience

Ferrum provides an isolated code execution and judging platform exposed via a structured HTTP API for developer platforms, web applications, LMS environments, and CI runners. Phase 7 implements authentication, authorization, quota controls, cancellation, batching, webhooks, and the OpenAPI specification.

---

## 1. Authentication and Scoped API Keys

### 1.1 Key Architecture

API keys follow a secure token format:

- Prefix: `ferrum_live_`
- Entropy: 32 bytes of cryptographically secure random bytes encoded in hexadecimal.
- Verification: SHA-256 hash stored in PostgreSQL. Raw secret keys are displayed only once upon creation and never persisted in database records or logged in application traces.
- Lookup: Indexed by `key_hash` with `subtle::ConstantTimeEq` constant-time comparison to prevent timing side channels.

### 1.2 Scopes

Permissions are enforced on every route:

| Scope                | Allowed Operations                                                    |
| -------------------- | --------------------------------------------------------------------- |
| `submissions:read`   | Read submission status, verdict, outputs, and case findings           |
| `submissions:write`  | Create single or batch submissions                                    |
| `submissions:cancel` | Cancel pending or running submissions                                 |
| `webhooks:manage`    | Create, list, and delete webhook destinations                         |
| `admin`              | Full administrative access, including API key creation and revocation |

Development environments can also authenticate using the master bearer token (`FERRUM_DEV_TOKEN`), which grants `admin` scope.

---

## 2. Quota and Admission Controls

Ferrum enforces concurrency admission per organization before accepting submissions:

- Maximum active submissions: 50 concurrent in-flight submissions (`created`, `queued`, `processing`) per tenant.
- Over-quota response: HTTP `429 Too Many Requests` with structured error code `tenant_quota_exceeded`.
- Request body limit: Bounded to 800 KiB for submissions and 64 KiB for management endpoints.

---

## 3. Bounded Batch Submissions

Clients can submit batches of 1 to 20 programs atomically via `POST /v1/submissions/batch`:

```json
{
  "submissions": [
    {
      "idempotency_key": "batch-item-1",
      "language": "python",
      "source": "print(input())\n",
      "stdin": "hello"
    },
    {
      "idempotency_key": "batch-item-2",
      "language": "cpp",
      "source": "#include <iostream>\nint main() { std::cout << 42 << std::endl; }\n"
    }
  ]
}
```

Response returns HTTP `200 OK` with individual intake outcomes:

```json
{
  "items": [
    {
      "idempotency_key": "batch-item-1",
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "state": "created"
    },
    {
      "idempotency_key": "batch-item-2",
      "id": "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
      "state": "created"
    }
  ]
}
```

Batches with 0 items return `400 Bad Request` (`empty_batch`). Batches exceeding 20 items return `400 Bad Request` (`batch_too_large`).

---

## 4. Submission Cancellation

Submissions can be cancelled via `POST /v1/submissions/{id}/cancel` (requires `submissions:cancel` scope):

1. **Pending Submissions (`created`, `queued`)**:
   Immediately transitioned to `state = 'cancelled'`, outbox event `submission.cancelled` emitted, and returns `outcome = "cancelled"`. Workers claiming the task will drop it.
2. **Active Submissions (`processing`)**:
   Marks `cancel_requested = true` and returns `outcome = "cancellation_requested"`. The worker supervisor detects cancellation on heartbeat/lease renewal, triggers process tree termination, verifies sandbox cleanup, and finalizes the submission as `state = 'cancelled'`.
3. **Terminal Submissions (`completed`, `failed`, `cancelled`)**:
   Idempotently returns `outcome = "already_terminal"` without altering committed results.

---

## 5. Webhooks and SSRF Protection

### 5.1 SSRF Defense Matrix

Webhook URLs are strictly validated prior to registration. The following destinations are unconditionally blocked:

- Loopback addresses (`127.0.0.0/8`, `::1`)
- Private networks (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `fc00::/7`)
- Link-local addresses (`169.254.0.0/16`, `fe80::/10`)
- Cloud metadata services (`metadata.google.internal`, `169.254.169.254`)
- Non-HTTP schemes (e.g. `ftp://`, `file://`, `gopher://`)

### 5.2 HMAC-SHA256 Payload Signatures

Each webhook delivery includes a signature header:

```http
X-Ferrum-Signature: t=1700000000,v1=9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca7
```

Where:

- `t`: UNIX timestamp in seconds
- `v1`: Hex-encoded HMAC-SHA256 of `${timestamp}.${payload}` using the endpoint's secret key (`whsec_...`).

---

## 6. OpenAPI Specification

The complete API surface is specified in:

- [docs/openapi.yaml](openapi.yaml)
- [docs/openapi.json](openapi.json)

---

## 7. Verification and Testing

### 7.1 Automated Smoke Suite

Run the production API smoke test suite against disposable PostgreSQL:

```bash
python3 tests/phase7/production-api-smoke.py
```

This verifies:

- Database schema migration up to version 5.
- API key generation and SHA-256 verification.
- Scoped permissions (`submissions:read`, `submissions:write`, `submissions:cancel`, `webhooks:manage`, `admin`).
- Bounded batch intake and limit rejections.
- Immediate and in-flight cancellation workflows.
- SSRF prevention against private, link-local, and cloud metadata targets.
- Key revocation and cross-tenant isolation.
