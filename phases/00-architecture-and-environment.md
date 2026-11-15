# Phase 0 · Architecture and environment

**Prerequisite** · Access to a disposable, supported Linux VM or dedicated test host.

## Outcome

Record a reproducible Linux execution profile and demonstrate that a harmless program runs with the controls Ferrum needs. Make the platform and dependency decisions that let the Rust foundation start. The current workspace is a macOS planning environment with a local PostgreSQL Compose definition.

## Work plan

1. Map the public API, PostgreSQL, trusted worker orchestration and untrusted compile/run processes. Identify which identities may reach the database and queue, which files a sandbox sees, and which configuration and secrets stay on the trusted side. [Threat model](../docs/threat-model.md)
2. Select a Linux distribution and architecture. Record kernel release, cgroup mode, Isolate distribution/version, package provenance, host privileges and the Python runtime. Identify a Linux runner that matches the execution profile. [Isolate upstream](https://github.com/ioi/isolate)
3. Run a benign stdout/stdin fixture inside a manually managed sandbox. Check CPU and elapsed time, memory, process, file and output limits with bounded fixtures. Verify expected filesystem visibility, disabled network access and clean teardown on success and controlled termination.
4. Select compatible Rust, Tokio, Axum, SQLx, Apalis and Apalis PostgreSQL versions using upstream compatibility guidance and a disposable Cargo build. Record what resolved and the selected PostgreSQL major. [Apalis architecture](https://apalis.dev/docs/introduction/architecture)
5. Record the initial supported Python profile and the resource policy to implement in Phase 3. Separate policy ceilings from worker capacity, which will be measured in Phase 9.

## Deliverables

- Supported host record and repeatable sandbox verification procedure.
- Dependency compatibility record with build output and version information.
- Initial language and resource-policy record with assumptions carried into worker readiness.

## Acceptance checks

- [ ] The Python fixture returns the expected output and the host verifies teardown.
- [ ] The chosen host demonstrates the required limits and isolation behavior with synthetic inputs.
- [ ] The dependency probe resolves and builds on the selected toolchain.
- [ ] The host profile and open constraints are recorded for Phases 1 and 3.

**Handoff** · Use the resulting dependency versions for the workspace and the verified host profile for the sandbox adapter. [Phase 1](01-rust-foundation.md)
