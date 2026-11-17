#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "FAIL: Isolate host verification requires Linux" >&2
  exit 1
fi

if [[ "$(id -u)" -eq 0 ]]; then
  echo "FAIL: Run Isolate host verification as the dedicated non-root worker account" >&2
  exit 1
fi

if [[ -f "/.dockerenv" ]]; then
  echo "FAIL: Isolate qualification cannot run inside a Docker container" >&2
  exit 1
fi

if ! command -v isolate >/dev/null 2>&1; then
  echo "FAIL: isolate binary not found in PATH" >&2
  exit 1
fi

isolate_version="$(isolate --version 2>&1 | head -n 1)"
echo "Isolate: ${isolate_version}"
echo "Kernel: $(uname -srmo)"

if [[ ! -f /sys/fs/cgroup/cgroup.controllers ]]; then
  echo "FAIL: cgroup v2 unified hierarchy not detected at /sys/fs/cgroup/cgroup.controllers" >&2
  exit 1
fi

hardlinks="$(cat /proc/sys/fs/protected_hardlinks 2>/dev/null || echo 0)"
if [[ "${hardlinks}" != "1" ]]; then
  echo "FAIL: fs.protected_hardlinks must be 1 (found ${hardlinks})" >&2
  exit 1
fi

core_pattern="$(cat /proc/sys/kernel/core_pattern 2>/dev/null || echo "|")"
if [[ "${core_pattern}" == \|* ]]; then
  echo "FAIL: kernel.core_pattern must not start with a pipe (found ${core_pattern})" >&2
  exit 1
fi

swap_lines="$(tail -n +2 /proc/swaps 2>/dev/null | wc -l | tr -d ' ')"
if [[ "${swap_lines}" != "0" ]]; then
  echo "FAIL: swap must be disabled on qualified Isolate hosts" >&2
  exit 1
fi

if [[ ! -d /var/lib/ferrum-sandbox ]]; then
  echo "FAIL: /var/lib/ferrum-sandbox does not exist" >&2
  exit 1
fi

recovery_mode="$(stat -c '%a' /var/lib/ferrum-sandbox)"
if [[ "${recovery_mode}" != "700" ]]; then
  echo "FAIL: /var/lib/ferrum-sandbox must have mode 700 (found ${recovery_mode})" >&2
  exit 1
fi

echo "PASS dedicated Linux Isolate host preflight verified"

