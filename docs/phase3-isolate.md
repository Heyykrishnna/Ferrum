# Phase 3 Rust Isolate adapter

The adapter runs the `python3.13-v1` profile on the dedicated Debian 13 amd64 or arm64 worker profile. It reserves Isolate box 0 for one execution at a time. The Phase 4 queue worker uses this adapter to execute persisted Python submissions. [Linux worker setup](phase0-linux-worker.md) · [Phase 4 handoff](../phases/04-apalis-worker-pipeline.md)

## Run the Linux checks in Docker

From the Ferrum checkout, build and run the Debian amd64 verification image:

```sh
docker build --platform linux/amd64 --file deployment/phase3.Dockerfile --tag ferrum-phase3-checks .
docker run --rm --platform linux/amd64 --network none --cap-drop ALL --security-opt no-new-privileges ferrum-phase3-checks
```

The image runs Rust formatting, Clippy, and the portable domain and Linux process tests. The container runs as an ordinary user and verifies that the adapter rejects Docker. Docker Desktop shares its LinuxKit kernel with the container. The real Isolate tests require a supported kernel and a delegated, writable cgroup hierarchy. [Docker kernel model](https://docs.docker.com/build/building/multi-platform/) · [Isolate cgroup setup](https://github.com/ioi/isolate/blob/v2.7/isolate.1.txt)

## Qualify the adapter

Follow the package and mount setup on a dedicated Debian 13 amd64 or arm64 host, then use the combined verification command below. The target uses Isolate 2.7, cgroup v2, a quota-enabled workspace, swap disabled, and the stock Isolate syscall restrictions. The adapter also requires `fs.protected_hardlinks=1` and a direct kernel core pattern so `--core=0` applies. A core pattern starting with a pipe sends dumps to a host handler outside that limit. [Phase 0 host setup](phase0-linux-worker.md#run-on-a-fresh-vm) · [Apple Silicon setup](macos-worker.md)

Create the private recovery directory once on the host. Use the ordinary Isolate worker account for the Rust tests.

```sh
sudo install -d -m 0700 -o "$(id -un)" /var/lib/ferrum-sandbox
```

From the Ferrum checkout on that host, with Rustup installed, run the two host suites as one block. The Phase 0 check stops the block if the host does not qualify. This also prevents a Cargo run with zero Linux tests from appearing to pass qualification. Keep box 0 reserved for Ferrum throughout the run.

```sh
git rev-parse HEAD
rustc -Vv
(
  set -e -o pipefail
  python3 -I -u tests/phase0/verify-isolate.py 2>&1 | tee /tmp/ferrum-phase0-isolate.log
  (
    cargo test --locked -p ferrum-sandbox --lib retains_and_reconciles_a_failed_cleanup -- --ignored --nocapture
    cargo test --locked -p ferrum-sandbox --test linux_host verifies_the_real_isolate_adapter -- --ignored --nocapture
  ) 2>&1 | tee /tmp/ferrum-phase3-isolate.log
)
```

The Phase 0 transcript records the kernel, package provenance and cgroup setup. The Rust run must pass both the real adapter suite and the cleanup recovery test. Record the checkout revision, the transcripts and the outcome in the [Phase 3 checklist](../phases/03-isolate-integration.md#acceptance-checks). These tests alter the disposable sandbox workspace, cgroup and recovery directory as part of setup and teardown.

## Execution contract

Phase 4 can reconstruct a validated `SubmissionInput` with `SubmissionInput::from_persisted`. It rechecks the recorded profile, source, stdin and full effective limits before the adapter can stage them. `Isolate::open` checks the host, holds an exclusive host lock, reconciles a previous Ferrum record if present, and runs a bounded startup probe. The probe checks the fixed Python runtime, environment, mount policy, selected resource limits and sandbox network interfaces. The qualification suites measure the controls further on the dedicated host.

Every execution calls Isolate with structured arguments, stages fixed file names, and uses the dedicated cgroup, process limits and workspace quotas. Standard output and error go through separate kernel pipes capped by the trusted supervisor. The supervisor reads at most the configured cap plus one byte from each pipe to detect overflow and retains at most the cap. Regular files inside the sandbox remain subject to the Isolate file limit and the workspace quotas. The kernel pipe buffers are separate from the sandbox files.

The caller supplies a cancellation token. Explicit cancellation and output overflow signal Isolate's manager, which terminates the sandbox's process tree. The supervisor has a separate deadline for a stuck manager and checks that the execution cgroup is empty before trusting the result. Dropping a pending execution future requests cancellation while the blocking supervisor continues its teardown. The outer worker should await execution reports during graceful shutdown.

The report contains a program outcome or a typed adapter error, plus an independent cleanup disposition. Captured bytes, exit code, terminating signal, time and cgroup peak memory are returned when available. A program result can remain valid when cleanup fails. Phase 4 owns verdict mapping and durable attempt and result records.

## Recover box 0

Ferrum publishes and syncs `/var/lib/ferrum-sandbox/box-0.pending` before attempting Isolate initialization. Each record becomes visible only after its contents are complete, and publication cannot overwrite an existing record. Startup discards an interrupted temporary write. Ferrum removes the pending record only after the sandbox workspace, cgroup and active Isolate lock are cleared. An unresolved cleanup blocks reuse in the current adapter. If a cleanup helper exits without clearing the box, Ferrum records that a later adapter may safely retry. On a later boot, the adapter can also recover a box associated with the previous boot because its helper processes have stopped. An occupied box without a Ferrum record needs operator review.

If startup reports that it cannot verify the previous supervisor on the current boot, keep the host out of admission and retain the record. Reboot the dedicated host, restore its qualified mount and cgroup setup, and start the adapter again. The restart reconciles the old record before it runs the startup probe.
