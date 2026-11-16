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

Set `SUBMISSION_ID` to the returned UUID. A successful create returns HTTP 202 with `Location: /v1/submissions/{id}`. A successful poll returns HTTP 200. Both bodies contain `id`, `state`, and `verdict`. Newly submitted records have state `created` and a null verdict. When it exists, the poll response includes `result` with `outcome`, `stdout`, `stderr`, `exit_code`, `signal`, `cpu_time_ms`, `wall_time_ms`, and `peak_memory_kib`. Unavailable measurements are null. The API displays output as UTF-8 with replacement for invalid sequences and PostgreSQL preserves the captured bytes. A successful unjudged execution has a null verdict. [Worker setup](phase4-worker.md)

The authenticated `GET /v1/languages` route lists the configured profile IDs, runtime limits and whether each profile compiles. The JSON body allows `language`, `source` (1–65,536 UTF-8 bytes), optional `stdin` (up to 65,536 UTF-8 bytes), and optional `limits`. Source and stdin exclude NUL characters. The encoded JSON request is capped at 800 KiB. The optional limits are positive integers and fall within the selected profile’s ceilings. For Python, the defaults and ceilings are below.

| Field | Default and ceiling |
|---|---:|
| `cpu_time_ms` | 2,000 milliseconds |
| `wall_time_ms` | 6,000 milliseconds, at least the effective CPU time |
| `memory_kib` | 131,072 KiB |

Ferrum persists the resolved profile revision and effective runtime limits. Compiled submissions also return `result.compilation` with the same output and resource fields for the compiler. If compilation fails, the runtime output is empty and its measurements are null. [Language profiles](phase5-languages.md)

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

`GET /v1/execution/status` requires a bearer token with `submissions:read`. It reports `worker: available` when the Ferrum Apalis queue has a heartbeat newer than two heartbeat intervals (60 seconds), `offline` when no heartbeat is fresh, or `queue_uninitialized` before the worker migrations have been applied. A recent heartbeat indicates that a worker is present. Jobs can still wait for a free slot. If API and worker database roles differ, grant the API role read access to `apalis.workers` for this check.
