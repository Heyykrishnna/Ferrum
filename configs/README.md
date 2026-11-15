# Configuration

Both binaries load built-in defaults, then an optional TOML file, then environment overrides. Set `FERRUM_API_CONFIG` or `FERRUM_WORKER_CONFIG` to select the file for that process. Relative paths resolve from the process's working directory. An explicitly selected file must exist. Unknown TOML keys, malformed files, and invalid settings fail startup. The startup error names the setting or file selector without including its value.

| TOML key | Environment override | Default and accepted values |
|---|---|---|
| `listen_addr` (API only) | `FERRUM_API_LISTEN_ADDR` | `127.0.0.1:8080`. Use a numeric IPv4 or bracketed IPv6 socket address. Port `0` assigns an available port in development mode. |
| `process.mode` | `FERRUM_MODE` | `development` or `production`. Defaults to `development`. Production requires a nonzero API port. |
| `process.log_level` | `FERRUM_LOG_LEVEL` | `error`, `warn`, `info`, `debug`, or `trace`. Defaults to `info`. |
| `process.shutdown_timeout_ms` | `FERRUM_API_SHUTDOWN_TIMEOUT_MS` or `FERRUM_WORKER_SHUTDOWN_TIMEOUT_MS` | 1 to 60,000 milliseconds. Defaults to 10,000. |

For example, save these API settings to `configs/application.toml` and launch with `FERRUM_API_CONFIG=configs/application.toml cargo run --locked -p ferrum-api`.

```toml
listen_addr = "127.0.0.1:8081"

[process]
mode = "development"
log_level = "info"
shutdown_timeout_ms = 5000
```

A worker file such as `configs/workers.toml` uses the same `[process]` table. Start it with `FERRUM_WORKER_CONFIG=configs/workers.toml cargo run --locked -p ferrum-worker`.

The [.env.example](../.env.example) file lists the local Docker Compose database settings. Phase 2 adds database settings. Phase 3 adds sandbox policy and a trusted Python profile. Phase 5 expands the language registry. Inject credentials into the processes that need them and keep them out of checked-in files, logs, and sandbox environments.
