# Ferrum Playground

A local, single-operator web workspace for Ferrum. Choose a language, edit the source in Monaco and provide standard input. **Run** displays the program's output and diagnostics. **Submit** judges the program against the expected output and displays Ferrum's verdict and score. Use the comparison menu to choose exact, whitespace-tolerant or token comparison.

## Start locally

Install Node 24.11 or later. Configure and launch the [Ferrum API](../docs/submission-api.md#start-locally) with PostgreSQL. An existing root `.env` containing `FERRUM_DEV_TOKEN` can supply the web server's credential. The repository's [Linux worker setup](../docs/phase3-isolate.md#qualify-the-adapter) is required to execute jobs. An API without a worker can accept requests and report their queued state.

On the qualified Linux host, point `FERRUM_DATABASE_URL` at the same PostgreSQL instance the API uses over a private connection. Run `cargo run --locked -p ferrum-worker -- migrate` followed by `cargo run --locked -p ferrum-worker`. The playground checks for a recent worker heartbeat and enables **Run** and **Submit** when one appears. Existing submissions stay in PostgreSQL while the worker is offline.

On Apple Silicon, keep the worker on the same Mac using the [local ARM64 VM setup](../docs/macos-worker.md).

From the repository root:

```sh
npm --prefix web ci
npm --prefix web run dev
```

Open <http://127.0.0.1:5173>. Use Ctrl+Enter or Cmd+Enter to run code. The console shows output, error output and compiler diagnostics separately.

If an existing service uses port 8080, start the submission-enabled Ferrum API on another loopback port with `FERRUM_API_LISTEN_ADDR` and set `FERRUM_WEB_API_URL` to the same origin when starting the playground.

For the built site:

```sh
npm --prefix web run build
npm --prefix web start
```

Open <http://127.0.0.1:4173>. Both modes listen on loopback only.

| Setting                | Value                                                                                                                           |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `FERRUM_WEB_API_URL`   | API origin. Defaults to `http://127.0.0.1:8080`. Remote origins require HTTPS.                                                  |
| `FERRUM_WEB_API_TOKEN` | Server-only Ferrum bearer token. Falls back to `FERRUM_DEV_TOKEN`. A scoped key needs submission read, write and cancel access. |
| `FERRUM_WEB_PORT`      | Optional local port override.                                                                                                   |

The loopback gateway uses one Ferrum tenant credential and exposes language discovery, submission intake, result lookup and cancellation to the local UI. Keep the gateway on the local machine. Use a separately authenticated, tenant-aware gateway before exposing a deployment to other users.

## Checks

```sh
npm --prefix web run format:check
npm --prefix web run lint
npm --prefix web run typecheck
npm --prefix web test
npm --prefix web run build
npm --prefix web audit
```
