# Phase 3 · Isolate integration

**Prerequisite** · The verified Linux host profile and Phase 2 submission inputs. [Phase 0](00-architecture-and-environment.md) · [Phase 2](02-database-and-submissions.md)

## Outcome

Run one pinned Python program in a fresh Isolate sandbox per attempt and return bounded output, execution facts and cleanup evidence. The worker's trusted orchestration owns the sandbox lifecycle.

## Work plan

1. Define the execution interface around a trusted profile, validated files, stdin, effective limits and cancellation. Keep Isolate invocation and metadata parsing in the adapter. Return typed initialization, run and cleanup outcomes.
2. Stage a server-named source file and bounded stdin in a fresh workspace. Resolve program and argument vectors from the trusted runtime profile. Use a minimal environment and filesystem view. Keep network access disabled and keep host credentials outside the sandbox.
3. Verify required controls before the worker admits execution. Enforce CPU, wall time, memory, process, file, disk and output bounds as supported by the verified profile. Stream bounded stdout and stderr and retain exit code, signal, resource measurements and limit reason when available.
4. On timeout, cancellation or shutdown, stop and reap the full process tree. Clean and verify the sandbox after each path, including partial setup. Register unresolved cleanup so the host can reconcile it without releasing affected capacity early.

## Verification

Run the workspace checks and the real-host suites before closing this phase. The real-host suites exercise the fixed Python profile, output capture, CPU/wall/memory limits, file and process limits, isolated networking, cancellation and cleanup recovery. Record the target host and Rust toolchain with both transcripts. [Host setup and verification](../docs/phase3-isolate.md#qualify-the-adapter)

Rust 1.100.0 on macOS arm64 passed workspace formatting, check, Clippy, unit tests and the locked release build. The existing API and PostgreSQL smoke tests passed. The Debian amd64 Docker image passed formatting, Clippy and nine tests. The real Isolate adapter test was also invoked explicitly in the container and stopped at `HostError::Platform`, before any sandbox was created. Both configured Docker contexts use the `7.0.14-linuxkit` kernel. The container mounts cgroup v2 read-only, so Isolate cannot configure the required per-box controls there. [Docker verification commands](../docs/phase3-isolate.md#run-the-linux-checks-in-docker)

## Acceptance checks

- [x] The adapter refuses an unqualified Docker host before starting a sandbox.
- [x] The Linux supervisor caps live process output and reaps the child after overflow or cancellation. The deadline test also verifies that it reaps a child that ignores SIGTERM.
- [x] The adapter interface exposes typed program outcomes and infrastructure errors, with cleanup status reported independently. Metadata tests distinguish program failures from adapter faults.
- [x] An interrupted recovery-record write cannot replace an existing attempt record or authorize cleanup.

These checks still require actual runs on the qualified Linux host:

- [ ] The Python fixture returns expected output and resource facts.
- [ ] The startup probe verifies the required controls on the supported host.
- [ ] Limits and output caps are observed in real sandbox runs.
- [ ] Normal completion, initialization failure and cancelled execution have a verified teardown or a recorded unresolved cleanup.
- [ ] Real runs and a forced cleanup failure confirm the outcome and cleanup classification.

**Handoff** · Integrate this execution interface with queue claims, attempt records and worker admission. [Phase 4](04-apalis-worker-pipeline.md)
