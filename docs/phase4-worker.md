# Phase 4 worker pipeline

The API stores a private Python submission and its creation event in one transaction. The worker locks one unpublished event at a time with `FOR UPDATE SKIP LOCKED`. The supported Apalis PostgreSQL insertion API uses the same transaction to publish the job, create the queued attempt, and mark the outbox event published. A database rollback undoes the entire publication. The payload contains a schema version and three UUIDs for the tenant, submission and attempt. PostgreSQL holds source, stdin and results.

The worker has one CPU slot and admits one validated Python reservation of up to 128 MiB per qualified host. It claims the attempt under a unique process identity and a 30-second lease, and renews the lease every five seconds while execution is active. Every renewal and finalization checks the owner and current expiry. When a renewal fails, the worker requests sandbox teardown, drains, and exits for recovery. A duplicate delivery cannot transition a running or finished attempt back to running.

Worker startup holds the local Isolate owner lock, reconciles its host journal, and runs the qualification probe before reconciling PostgreSQL. Use a unique, stable `/etc/machine-id` on each dedicated worker host. A verified restart on the same host can release unresolved cleanup and reconcile attempts left running by the previous process. A different worker host leaves those attempts pending until that original host can prove teardown. Restore the original host identity when replacing its operating system and verify the old instance is permanently fenced before admitting its replacement.

Verified transient Isolate initialization failures and verified recovery of a lost worker can create a fresh attempt, with jittered backoff and a maximum of three attempts total. Results, attempt state, cleanup disposition and the terminal outbox event commit together before the handler returns success to Apalis. Cleanup failure preserves any available result, records the failure, and takes the affected worker out of service until the host probe succeeds. Completion events are available for future consumers. On SIGTERM or SIGINT the worker stops admitting new jobs and lets an active bounded execution finalize. The Phase 3 host journal remains the recovery source if the process exits before teardown.

## Run locally

Run these commands from the Ferrum checkout. Docker Compose supplies a fresh PostgreSQL 17 database for each invocation and removes its volumes on exit.

```sh
python3 -I -u tests/phase4/worker-smoke.py
```

This suite uses the pinned PostgreSQL-backed Apalis queue and a Rust test double for controlled sandbox responses. It checks atomic dispatch, concurrency, tenant isolation, stale ownership, retry exhaustion, resource verdicts, cleanup recovery, durable results, acknowledgment and drain.

For real Python execution, use a dedicated Debian 13 amd64 host that passes the [Phase 3 host qualification](phase3-isolate.md#qualify-the-adapter). Run the following as the ordinary Isolate worker account, with box 0 reserved for Ferrum and Docker Compose available for the disposable database.

```sh
git rev-parse HEAD
python3 -I -u tests/phase4/worker-smoke.py --isolate
```

The command starts the API and the real worker. It polls a successful Python submission, verifies a wall-time limit, restarts the API and checks the saved result and both cleanup records. Keep the reported submission IDs together with the revision and Phase 3 host transcripts. A Debian Docker image on macOS can compile and test the Linux Rust code. The real worker startup additionally requires the supported Linux kernel, cgroup delegation and quota-enabled workspace from Phase 3.

## Start the services

For a persistent development database, use the [private submission API setup](submission-api.md), then apply the application and Apalis schemas with the migration account. Set `FERRUM_DATABASE_URL` for all three commands. The connection role that runs the worker needs access to the application and Apalis schemas.

```sh
cargo run --locked -p ferrum-api -- migrate
cargo run --locked -p ferrum-worker -- migrate
cargo run --locked -p ferrum-worker
```

Start the API separately with its private tenant token. The worker allows 30 seconds for graceful shutdown by default. `FERRUM_WORKER_SHUTDOWN_TIMEOUT_MS` overrides that budget. Database migration commands can run from macOS. The worker serving command runs on the qualified host.
