# Phase 0 Linux worker qualification

The candidate runner is a disposable Debian 13 (Trixie) amd64 VM, booted with Debian's stock Linux 6.12 series kernel and systemd. Use a dedicated VM with at least 2 vCPUs and 2 GiB RAM for these checks. A maintainer can use this same image and configuration for a dedicated CI VM once the profile is verified. The Mac and ordinary hosted or Docker runners can build the Rust probe but cannot qualify the sandbox.

| Component | Candidate configuration |
|---|---|
| Isolate | Upstream Debian amd64 package `isolate=2.7` from the signed `trixie-isolate` APT repository; stock configuration and system call restrictions |
| Privilege | Only the package's Isolate helper has elevated privileges. The verifier runs as an ordinary user in the `isolate` group. The sandbox has its own unprivileged UID/GID. |
| cgroups | Unified cgroup v2, with `isolate.service` providing delegated `cpuset` and `memory` and cgroup CPU accounting. Swap is disabled for this profile. |
| Workspace | Dedicated tmpfs at `/var/lib/isolate` with user quotas. Only the per-attempt `/box` is writable. Expose the stock runtime's `/usr`, `/lib`, and `/lib64` read-only and non-recursively; mount a sandbox-local read-only `/proc`. |
| Network and environment | Use Isolate's separate network namespace and keep only its sandbox loopback. Pass no inherited environment or extra descriptors. Keep database, queue, telemetry, and other service credentials on the trusted side. |
| Python | Debian's `python3.13` package, at `/usr/bin/python3`. Run `/usr/bin/python3 -I -B /box/main.py`, with one server-named source file, bounded stdin, and no compilation or third-party package installation. Record the exact package revision from the verified VM. |

See [Debian 13 release notes](https://www.debian.org/releases/trixie/release-notes/), [Isolate 2.7 installation and flags](https://github.com/ioi/isolate/blob/v2.7/isolate.1.txt), and [Linux 6.12 tmpfs quota options](https://www.kernel.org/doc/html/v6.12/filesystems/tmpfs.html).

## Initial Python policy for Phase 3

These are starting ceilings for a single invocation. Phase 2 persists the resolved policy with the submission. Phase 3 must enforce the policy and fail readiness if any required control is unavailable.

| Input or resource | Initial ceiling |
|---|---|
| Source and stdin | One Python source file of at most 64 KiB and at most 64 KiB of stdin, validated before staging |
| CPU and wall time | 2 seconds of CPU, 6 seconds elapsed, with no extra-time allowance |
| Memory and stack | 128 MiB total in the cgroup; 8 MiB per-process stack |
| Processes or threads; open descriptors | One process or thread; 64 open descriptors |
| Files and workspace | 64 KiB per file; 4 MiB aggregate quota and 128 inodes, including staged files and captures |
| stdout and stderr | 64 KiB per stream, independently. Keep both the sandbox-side bound and the trusted reader's byte cap. |

The verifier uses lower time limits to distinguish CPU from wall-clock enforcement and permits two processes for the teardown fixture. These exceptions are confined to the verification cases. Phase 3 owns trustworthy output collection, result classification and cleanup reconciliation. Phase 9 owns measured host concurrency and admission sizing.

## Run on a fresh VM

Keep the VM free of real data, production credentials and other workloads. Use the standard package settings and reserve box 0 exclusively for this run. The commands below assume a fresh disposable VM with `sudo` access.

```sh
sudo apt-get update
sudo apt-get install -y ca-certificates curl python3 git
sudo install -d -m 0755 /etc/apt/keyrings
key_file=$(mktemp)
curl -fsSLo "$key_file" https://www.ucw.cz/isolate/debian/signing-key.asc &&
  sudo install -m 0644 "$key_file" /etc/apt/keyrings/isolate.asc
rm -f "$key_file"
sudo tee /etc/apt/sources.list.d/isolate.sources >/dev/null <<'APT'
Types: deb
URIs: https://www.ucw.cz/isolate/debian/
Suites: trixie-isolate
Components: main
Architectures: amd64
Signed-By: /etc/apt/keyrings/isolate.asc
APT
sudo apt-get update
apt-cache policy isolate
sudo apt-get install -y isolate=2.7
sudo systemctl enable --now isolate.service
sudo adduser "$(id -un)" isolate
sudo swapoff -a
```

Confirm `/var/lib/isolate` is an empty package-created directory and has no existing mount before mounting the workspace:

```sh
sudo ls -A /var/lib/isolate
mountpoint /var/lib/isolate
sudo mount -t tmpfs -o size=256m,nr_inodes=4096,mode=0755,nosuid,nodev,usrquota tmpfs /var/lib/isolate
```

Sign out and back in to pick up the new group. From the Ferrum checkout containing the Phase 0 files, run:

```sh
git rev-parse HEAD
id -nG
(set -o pipefail; python3 -I -u tests/phase0/verify-isolate.py 2>&1 | tee /tmp/ferrum-phase0-isolate.log)
```

A successful run ends with `PASS all Phase 0 Isolate checks`. It prints the OS, kernel, package versions, controllers, mount options, and case metadata. The checks cover the echo fixture, host path and environment isolation, read-only runtime, separate networking, CPU/wall/memory and process limits, file and output size, workspace quotas, and teardown after success, limit stops and SIGTERM. The verifier checks that the cgroup is empty before cleanup and that the workspace and cgroup disappear and the lock record is empty afterward. Isolate deliberately keeps the empty lock file for reuse.

Attach the revision, package provenance from `apt-cache policy isolate`, and the verifier transcript to the [Phase 0 evidence record](../phases/00-architecture-and-environment.md#acceptance-checks). If a preflight or case fails, keep the transcript and record the failed control. Stop using the VM for candidate execution if cleanup cannot be verified. Destroy the VM after collecting evidence, or unmount the temporary workspace before reusing the host. A passing run qualifies this specific development profile for the Phase 3 adapter work; production isolation and recovery remain later acceptance gates.
