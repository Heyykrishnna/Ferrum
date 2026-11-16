import os
from pathlib import Path
import platform
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time

BOX = Path("/var/lib/isolate/0")
LOCK = Path("/run/isolate/locks/0")
ISOLATE = ("/usr/bin/isolate", "--cg", "--box-id=0")
FILE_BYTES = 64 * 1024
BASE_LIMITS = ("--cg-mem=131072", "--fsize=64", "--stack=8192", "--open-files=64")
MOUNTS = (
    "--no-default-dirs",
    "--dir=/box=./box:rw",
    "--dir=/usr:norec",
    "--dir=/lib:norec",
    *(("--dir=/lib64:norec",) if platform.machine() == "x86_64" else ()),
    "--dir=/proc=proc:fs",
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def command(*args: str) -> str:
    return subprocess.check_output(args, text=True, timeout=15).strip()


def box_is_reserved(group: Path) -> bool:
    return (
        BOX.exists() or group.exists() or (LOCK.exists() and LOCK.stat().st_size != 0)
    )


def preflight() -> Path:
    require(
        platform.system() == "Linux" and platform.machine() in {"x86_64", "aarch64"},
        "use a disposable Debian 13 amd64 or arm64 Linux VM",
    )
    require(
        platform.freedesktop_os_release().get("ID") == "debian"
        and platform.freedesktop_os_release().get("VERSION_ID") == "13",
        "this procedure qualifies Debian 13 only",
    )
    require(
        platform.release().startswith("6.12."), "boot Debian's stock Linux 6.12 kernel"
    )
    require(
        os.getuid() != 0 and "isolate" in command("id", "-nG").split(),
        "run as the ordinary user and sign in again after joining the isolate group",
    )
    require(
        subprocess.run(
            ("systemd-detect-virt", "--container", "--quiet"), timeout=5
        ).returncode
        == 1,
        "containers are outside the host profile",
    )
    require(
        command("systemctl", "is-active", "isolate.service") == "active",
        "start isolate.service before running the verifier",
    )
    version = command(ISOLATE[0], "--version")
    require(
        re.search(r"^The process isolator 2\.7\b", version) is not None,
        "install the upstream Debian package for Isolate 2.7",
    )
    config = command(ISOLATE[0], "-vv", "--check-config")
    require(
        "box_root=/var/lib/isolate lock_root=/run/isolate/locks " in config,
        "this verifier requires the package's default box and lock roots",
    )
    source, filesystem, raw_options = command(
        "findmnt", "-n", "-M", str(BOX.parent), "-o", "SOURCE,FSTYPE,OPTIONS"
    ).split()
    options = set(raw_options.split(","))
    require({"nosuid", "nodev"} <= options, "restrict the workspace mount")
    if filesystem == "ext4" and platform.machine() == "aarch64":
        backing = Path("/run/ferrum-workspace/isolate.ext4")
        require(
            re.fullmatch(r"/dev/loop[0-9]+", source) is not None
            and Path(f"/sys/block/{Path(source).name}/loop/backing_file").read_text().strip() == str(backing),
            "use the dedicated memory-backed loop device",
        )
        backing_type, backing_options = command(
            "findmnt", "-n", "-M", str(backing.parent), "-o", "FSTYPE,OPTIONS"
        ).split()
        require(
            backing_type == "tmpfs"
            and {"size=262144k", "nodev", "nosuid", "noexec"} <= set(backing_options.split(","))
            and not backing.is_symlink()
            and backing.is_file()
            and backing.stat().st_uid == 0
            and backing.stat().st_mode & 0o777 == 0o600
            and backing.stat().st_nlink == 1
            and backing.stat().st_size == 256 * 1024 * 1024
            and "usrquota" in options,
            "enable user quotas on the dedicated memory-backed ext4 workspace",
        )
    else:
        require(
            filesystem == "tmpfs" and bool({"usrquota", "quota"} & options),
            "mount the dedicated quota-enabled tmpfs",
        )
    require(
        len(Path("/proc/swaps").read_text().splitlines()) == 1,
        "disable swap on this VM",
    )
    group_root = Path(command(*ISOLATE, "--print-cg-root"))
    controllers = (group_root / "cgroup.subtree_control").read_text().split()
    require(
        {"cpuset", "memory"} <= set(controllers),
        "Isolate needs delegated cpuset and memory controllers",
    )
    package = command(
        "dpkg-query",
        "-W",
        "-f=${Package} ${Version} ${Architecture}\n",
        "isolate",
        "python3.13",
    )
    print(f"Debian 13 {platform.machine()}, kernel {platform.release()}")
    print(package)
    print(
        f"{version}\nPython {platform.python_version()}; cgroup v2 ({', '.join(controllers)})"
    )
    print(
        command("findmnt", "-n", "-T", str(BOX.parent), "-o", "TARGET,FSTYPE,OPTIONS")
    )
    return group_root / "box-0"


def run_case(name: str, evidence: Path, group: Path, *fixture_args: str) -> None:
    require(
        not box_is_reserved(group),
        "Isolate box 0 is already in use",
    )
    try:
        require(
            command(*ISOLATE, "--quota=4096,128", "--init") == str(BOX),
            "unexpected Isolate workspace",
        )
        workspace = BOX / "box"
        shutil.copyfile(
            Path(__file__).with_name("isolate-fixture.py"), workspace / "fixture.py"
        )
        (workspace / "input").write_text("hello\n")
        metadata = evidence / f"{name}.meta"
        cpu = "0.25" if name == "cpu" else "2"
        wall = "0.8" if name == "wall" else "6"
        processes = "2" if name == "terminate" else "1"
        args = (
            *ISOLATE,
            *BASE_LIMITS,
            *MOUNTS,
            f"--time={cpu}",
            f"--wall-time={wall}",
            f"--processes={processes}",
            f"--meta={metadata}",
            "--stdin=/box/input",
            "--stdout=/box/stdout",
            "--stderr=/box/stderr",
            "--run",
            "--",
            "/usr/bin/python3",
            "-I",
            "-B",
            "/box/fixture.py",
            name,
            *fixture_args,
        )
        with subprocess.Popen(
            args,
            stdin=subprocess.DEVNULL,
            env={**os.environ, "FERRUM_PHASE0_CANARY": "synthetic"},
        ) as process:
            try:
                if name == "terminate":
                    deadline = time.monotonic() + 4
                    while len((group / "cgroup.procs").read_text().split()) < 2:
                        require(
                            process.poll() is None and time.monotonic() < deadline,
                            "fixture did not start two sandbox processes",
                        )
                        time.sleep(0.02)
                    process.terminate()
                code = process.wait(timeout=10)
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=3)

        facts = dict(
            line.split(":", 1)
            for line in metadata.read_text().splitlines()
            if ":" in line
        )
        events = dict(
            line.split() for line in (group / "cgroup.events").read_text().splitlines()
        )
        require(
            events.get("populated") == "0",
            f"{name}: live processes remain after execution",
        )
        require(
            (group / "cpu.stat").is_file(),
            f"{name}: cgroup CPU accounting is unavailable",
        )
        if name in ("cpu", "wall"):
            require(
                code == 1 and facts.get("status") == "TO",
                f"{name}: expected a timeout: {facts}",
            )
            cpu_used, wall_used = float(facts["time"]), float(facts["time-wall"])
            expected = (
                (cpu_used >= 0.25 and wall_used < 6)
                if name == "cpu"
                else (cpu_used < 2 and 0.8 <= wall_used < 3)
            )
            require(
                expected,
                f"{name}: timing does not identify the expected limit: {facts}",
            )
        elif name == "memory":
            require(
                code == 1 and facts.get("cg-oom-killed") == "1",
                f"{name}: expected a cgroup OOM kill: {facts}",
            )
        elif name == "terminate":
            require(
                code == 1
                and facts.get("status") == "SG"
                and facts.get("killed") == "1"
                and facts.get("exitsig") == str(signal.SIGTERM.value),
                f"{name}: expected controlled SIGTERM: {facts}",
            )
        else:
            require(
                code == 0 and facts.get("exitcode") == "0",
                f"{name}: fixture failed ({facts}): {(workspace / 'stderr').read_text()}",
            )
            if name == "visibility":
                require(
                    (workspace / "stdout").read_bytes() == b"ferrum:hello\n"
                    and (workspace / "scratch").read_bytes() == b"ok",
                    "unexpected fixture output",
                )
            output_name = {
                "file": "large-file",
                "stdout": "stdout",
                "stderr": "stderr",
            }.get(name)
            if output_name:
                require(
                    (workspace / output_name).stat().st_size == FILE_BYTES,
                    f"{name}: limit was not exercised",
                )
            if name in ("blocks", "inodes"):
                print((workspace / "stdout").read_text().strip())
    finally:
        command(*ISOLATE, "--cleanup")
        require(
            not box_is_reserved(group),
            f"{name}: workspace, cgroup or active lock remains after cleanup",
        )
    print(
        f"PASS {name}: {facts}; cgroup empty; workspace and cgroup removed; lock released",
        flush=True,
    )


def main() -> None:
    group = preflight()
    with (
        tempfile.TemporaryDirectory(prefix="ferrum-phase0-") as temporary,
        tempfile.TemporaryDirectory(prefix="ferrum-phase0-canary-") as outside,
        socket.socket() as listener,
    ):
        evidence = Path(temporary)
        Path(outside).chmod(0o755)
        canary = Path(outside) / "host-canary"
        canary.write_text("synthetic\n")
        canary.chmod(0o644)
        listener.bind(("127.0.0.1", 0))
        listener.listen()
        run_case(
            "visibility",
            evidence,
            group,
            str(canary),
            os.readlink("/proc/self/ns/net"),
            str(listener.getsockname()[1]),
        )
        for case in (
            "cpu",
            "wall",
            "memory",
            "processes",
            "file",
            "stdout",
            "stderr",
            "blocks",
            "inodes",
            "terminate",
        ):
            run_case(case, evidence, group)
    print("PASS all Phase 0 Isolate checks")


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
        sys.exit(f"Phase 0 Isolate verification failed: {error}")
