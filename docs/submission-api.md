# Private submission API

Enable Phase 2 intake with a PostgreSQL connection URL, a tenant UUID, and a development bearer token. The API binds to a loopback address in development mode when intake is enabled. Each instance uses the one tenant selected by its own configuration. Request headers and bodies carry no trusted tenant identity. [API configuration](../configs/README.md)

## Start locally

Start PostgreSQL and set a stable local password using the [database setup instructions](../deployment/README.md#local-postgresql). With the example Compose user, database, and port, set these values in your shell. Replace `YOUR_URL_ENCODED_PASSWORD` with the URL-encoded local database password.

```sh
export FERRUM_DATABASE_URL='postgres://ferrum:YOUR_URL_ENCODED_PASSWORD@127.0.0.1:54329/ferrum'
export FERRUM_DEV_TENANT_ID="$(uuidgen)"
export FERRUM_DEV_TOKEN="$(openssl rand -hex 32)"
cargo run --locked -p ferrum-api -- migrate
cargo run --locked -p ferrum-api
```

Keep the same tenant UUID across restarts to retrieve its existing submissions. Startup registers the configured tenant if needed. The migration command only needs the database settings. The API runs in health-only mode when all three settings are absent.

## Submit and poll

Use the same exported token in the client shell. A new logical submission needs a new `Idempotency-Key`.

```sh
curl -i http://127.0.0.1:8080/v1/submissions \
  -H "Authorization: Bearer $FERRUM_DEV_TOKEN" \
  -H 'Idempotency-Key: first-python-run' \
  -H 'Content-Type: application/json' \
  --data '{"language":"python","source":"print(input())\n","stdin":"hello\n"}'
```

Use the `Location` response header to poll the submission.

```sh
curl -i "http://127.0.0.1:8080/v1/submissions/$SUBMISSION_ID" \
  -H "Authorization: Bearer $FERRUM_DEV_TOKEN"
```

Set `SUBMISSION_ID` to the returned UUID. A successful create returns HTTP 202 with `Location: /v1/submissions/{id}`. A successful poll returns HTTP 200. Both bodies contain `id`, `state`, and `verdict`. Newly submitted records have state `created` and a null verdict. Work reaches later states when the worker pipeline is integrated in Phase 4.

The JSON body allows `language` (currently `python`), `source` (1–65,536 UTF-8 bytes), optional `stdin` (up to 65,536 UTF-8 bytes), and optional `limits`. Source and stdin exclude NUL characters. The encoded JSON request is capped at 800 KiB. The optional limits are positive integers in the following units.

| Field | Default and ceiling |
|---|---:|
| `cpu_time_ms` | 2,000 milliseconds |
| `wall_time_ms` | 6,000 milliseconds, at least the effective CPU time |
| `memory_kib` | 131,072 KiB |

Ferrum resolves the runtime to `python3.13-v1` and persists the full effective policy. [Python policy](phase0-linux-worker.md#initial-python-policy-for-phase-3)

Idempotency keys contain 1–128 ASCII letters, digits, or `-_.:`. A matching replay within the same tenant returns the same ID and its current state. Equality uses the exact UTF-8 source and stdin after JSON decoding, the resolved runtime profile, and the effective limits. Omitted or null stdin equals an empty string. Omitted limit fields equal their defaults. Whitespace and line endings in input strings stay significant.

Errors have an `error` object with a stable `code` and a human-readable `message`.

| HTTP status | Condition |
|---|---|
| 400 | Invalid body, input, or idempotency key |
| 401 | Missing or invalid bearer token |
| 404 | Submission absent from the configured tenant |
| 409 | A tenant already used that key for different normalized input |
| 413 | Encoded request exceeds the body limit |
| 415 | A submission POST needs a JSON content type |
| 503 | PostgreSQL unavailable or a storage timeout |
| 500 | Unexpected storage failure |

`/health/live` reports the API process. With intake enabled, `/health/ready` probes PostgreSQL and returns HTTP 503 when the probe fails or shutdown begins.
