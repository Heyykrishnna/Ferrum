# Threat model

Protect kernel and worker integrity, control-plane credentials, tenant data, submitted source and hidden tests. Treat every submission, generated file, compiler artifact and program output as untrusted. A compiler is inside the execution boundary too.

| Risk | Required design | Evidence |
|---|---|---|
| Workload reaches host files or internal services | Dedicated Linux workers; isolated network, minimal mounts, explicit environment and least privilege | Real-host tests demonstrate the intended filesystem and network policy |
| One workload consumes the host | Admission based on CPU and memory; enforced per-sandbox CPU, wall time, memory, process, disk and output bounds | Bounded Linux tests and measured capacity on the deployment profile |
| Process or sandbox survives cancellation | Stop and reap the entire process tree, verify teardown, track cleanup failures and quarantine affected capacity | Cancellation, crash and cleanup recovery tests |
| Cross-tenant reads, writes or job execution | Derive tenant identity server-side and scope each record lookup, claim and credential | Real database tests for authenticated ownership and denied access |
| Duplicate delivery or lost worker overwrites the result | Transactional outbox, idempotent claim, attempt ownership and reconciliation | Duplicate delivery and stale-owner tests on real PostgreSQL and Apalis |
| Credentials or source appear in telemetry | Keep data out of job payloads, process environments and logs; use correlation IDs and redaction | Inspect recorded jobs, responses and telemetry during tests |
| File handling or webhook destinations cross trust boundaries | Bounded, validated file paths; separate destination validation and restricted webhook egress | File and destination policy tests before those features are enabled |

Work on disposable Linux hosts using synthetic data. Keep development and production credentials separate. Workers fail closed if required isolation or limits cannot be verified. A passing mock test and a working Mac development environment are insufficient evidence. Phase 8 requires documented operational recovery, independent review before broad public exposure and an explicit record of unresolved risks.
