import argparse
import json
from pathlib import Path
import platform
import runpy
import secrets
import subprocess
from uuid import uuid4

ROOT = Path(__file__).resolve().parents[2]
phase2 = runpy.run_path(str(ROOT / "tests/phase2/submission-smoke.py"))
phase4 = runpy.run_path(str(ROOT / "tests/phase4/worker-smoke.py"))


def verify_cli(worker_binary: Path, env: dict[str, str]) -> None:
    reconcile = subprocess.run(
        [worker_binary, "reconcile"],
        env=env,
        capture_output=True,
        text=True,
        check=True,
        timeout=20,
    )
    data = json.loads(reconcile.stdout)
    assert "stale_attempts_reconciled" in data
    assert "stuck_outbox_events_reconciled" in data

    q_list = subprocess.run(
        [worker_binary, "quarantine", "list"],
        env=env,
        capture_output=True,
        text=True,
        check=True,
        timeout=20,
    )
    quarantined = json.loads(q_list.stdout)
    assert isinstance(quarantined, list)

    dummy_id = str(uuid4())
    q_resolve = subprocess.run(
        [worker_binary, "quarantine", "resolve", dummy_id],
        env=env,
        capture_output=True,
        text=True,
        timeout=20,
    )
    assert q_resolve.returncode != 0
    assert "no active quarantine found" in q_resolve.stderr


def verify_adversarial_isolate(
    api: Path, worker_binary: Path, db, database_env: dict[str, str]
) -> None:
    tenant, token = str(uuid4()), secrets.token_hex(32)
    env = phase2["api_env"](database_env["FERRUM_DATABASE_URL"], tenant, token)
    with phase4["worker"](worker_binary, database_env) as process:
        with phase2["running_api"](api, env) as port:
            status, env_test, headers = phase2["create"](
                port,
                token,
                "sterile-env",
                {
                    "language": "python",
                    "source": "import os\nfor k in sorted(os.environ):\n    print(k)",
                },
            )
            assert status == 202, status
            path = headers["location"]
            finished = phase4["poll"](port, path, token, process)
            assert finished["state"] == "completed", finished
            stdout = finished["result"]["stdout"]
            assert "FERRUM_DATABASE_URL" not in stdout
            assert "DATABASE_URL" not in stdout
            assert "POSTGRES" not in stdout

            status, oom_test, headers = phase2["create"](
                port,
                token,
                "memory-exhaustion",
                {
                    "language": "python",
                    "source": "x = ' ' * (100 * 1024 * 1024)",
                    "limits": {"memory_kib": 16384},
                },
            )
            assert status == 202, status
            path = headers["location"]
            finished = phase4["poll"](port, path, token, process)
            assert finished["state"] == "completed", finished
            assert finished["verdict"] in {"memory_limit_exceeded", "runtime_error"}

            status, loop_test, headers = phase2["create"](
                port,
                token,
                "cpu-exhaustion",
                {
                    "language": "python",
                    "source": "while True: pass",
                    "limits": {"cpu_time_ms": 300, "wall_time_ms": 600},
                },
            )
            assert status == 202, status
            path = headers["location"]
            finished = phase4["poll"](port, path, token, process)
            assert finished["state"] == "completed", finished
            assert finished["verdict"] == "time_limit_exceeded"

            status, output_test, headers = phase2["create"](
                port,
                token,
                "huge-output",
                {
                    "language": "python",
                    "source": "print('A' * 200000)",
                    "limits": {"stdout_bytes": 1024},
                },
            )
            assert status == 202, status
            path = headers["location"]
            finished = phase4["poll"](port, path, token, process)
            assert finished["state"] == "completed", finished
            assert (
                finished["verdict"] == "output_limit_exceeded"
                or len(finished["result"]["stdout"]) <= 65536
            )

            status, fork_test, headers = phase2["create"](
                port,
                token,
                "fork-storm",
                {
                    "language": "python",
                    "source": "import os\nfor _ in range(50):\n    try: os.fork()\n    except OSError: pass",
                    "limits": {"processes": 10},
                },
            )
            assert status == 202, status
            path = headers["location"]
            finished = phase4["poll"](port, path, token, process)
            assert finished["state"] == "completed", finished


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--isolate", action="store_true")
    args = parser.parse_args()
    if args.isolate and (platform.system(), platform.machine()) != ("Linux", "x86_64"):
        parser.error("--isolate requires the qualified Debian 13 amd64 host")

    phase4["cargo"]("build", "--locked", "-p", "ferrum-api", "-p", "ferrum-worker")
    metadata = json.loads(
        phase4["cargo"]("metadata", "--locked", "--no-deps", "--format-version=1")
    )
    target = Path(metadata["target_directory"]) / "debug"
    api, worker_binary = target / "ferrum-api", target / "ferrum-worker"

    db = phase2["Database"]()
    db.project = f"ferrum-phase8-{secrets.token_hex(6)}"
    with db.running() as url:
        env = {**phase2["environment"](), "FERRUM_DATABASE_URL": url}
        for binary in (api, worker_binary):
            for _ in range(2):
                subprocess.run([binary, "migrate"], env=env, check=True, timeout=20)

        verify_cli(worker_binary, env)

        if args.isolate:
            verify_adversarial_isolate(api, worker_binary, db, env)
            print("Phase 8 security and adversarial smoke passed with live Isolate sandboxes")
        else:
            subprocess.run(
                [
                    "cargo",
                    "test",
                    "--locked",
                    "-p",
                    "ferrum-worker",
                    "--",
                    "--ignored",
                    "--test-threads=1",
                    "--nocapture",
                ],
                cwd=ROOT,
                env={**env, "DATABASE_URL": url},
                check=True,
                timeout=240,
            )
            print("Phase 8 security and reliability suite passed with the sandbox test double")


if __name__ == "__main__":
    main()
