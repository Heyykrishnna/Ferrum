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

Use the real Isolate adapter on the supported Linux profile. Include harmless echo input, nonzero exit, timeout, memory and output limits, network and filesystem policy, cancellation and cleanup failure. Record the host, kernel, Isolate and toolchain versions. Compare the bounded output, exit facts, limit classification and teardown evidence with expected results.

## Acceptance checks

- [ ] The Python fixture returns expected output and resource facts.
- [ ] Required host controls are verified and missing controls prevent execution.
- [ ] Limits and output caps are observed in real sandbox runs.
- [ ] Normal completion, initialization failure and cancelled execution have a verified teardown or a recorded unresolved cleanup.
- [ ] The worker can distinguish a program failure from adapter and cleanup faults.

**Handoff** · Integrate this execution interface with queue claims, attempt records and worker admission. [Phase 4](04-apalis-worker-pipeline.md)
