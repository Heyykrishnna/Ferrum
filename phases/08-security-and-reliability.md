# Phase 8 · Security and reliability

**Prerequisite** · A staging API and the intended dedicated Linux worker profile. [Phase 7](07-production-api.md)

## Outcome

Demonstrate that a worker failure or rejected workload leaves Ferrum in a recoverable state. Verify the control-plane and execution boundaries on the host profile intended for release.

## Work plan

1. Compare actual worker identities, mounts, environment, egress and database privileges with the threat model. Verify API and worker credentials stay outside sandboxed programs. Record the supported kernel, Isolate build and toolchain versions. [Threat model](../docs/threat-model.md)
2. Use bounded synthetic fixtures to test filesystem and network policy, resource limits and cleanup across compile, run, cancellation and shutdown.
3. Interrupt workers, outbox publication and database access. Confirm operations can account for unfinished submissions, expired attempts, leftover sandboxes and quarantined capacity. [Recovery windows](submission-lifecycle.md)
4. Write and rehearse operator procedures for stuck outbox items, lost workers, cleanup failure and isolation incidents. Include detection signals, safe containment steps, reconciliation and evidence to resume intake.
5. Review secret exposure and access scopes. Arrange independent security review before broad public exposure and assign owners to unresolved findings.

| Drill | Evidence to capture |
|---|---|
| Worker stops during compile or execution | Ownership expires, old processes are stopped or host is fenced, and the submission recovers within the agreed budget |
| PostgreSQL is temporarily unavailable | Intake and finalization surface failures, and committed work remains discoverable after recovery |
| Cleanup fails after result persistence | Result remains retrievable, capacity is held and the operator can verify recovery |
| Cancellation is repeated during execution | Process tree terminates, cleanup is accounted for and status is stable |
| Sandbox policy probe runs on a qualified host | Isolation checks and resource controls behave as recorded for the deployed profile |

## Acceptance checks

- [x] The supported Linux profile passes the isolation and recovery matrix.
- [x] No recovery drill leaves an untracked attempt or sandbox.
- [x] Runbooks are exercised and record the evidence required to resume service.
- [x] Security review findings have owners and release blockers are addressed before exposure.

**Handoff** · Measure recovery and saturation with staging observability and representative workloads. [Phase 9](09-observability-and-performance.md)
