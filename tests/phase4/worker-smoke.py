import argparse
from contextlib import contextmanager
import json
from pathlib import Path
import platform
import runpy
import secrets
import signal
import subprocess
import tempfile
import time
from uuid import UUID, uuid4

ROOT = Path(__file__).resolve().parents[2]
phase2 = runpy.run_path(str(ROOT / "tests/phase2/submission-smoke.py"))


def cargo(*args: str) -> str:
    result = subprocess.run(
        ["cargo", *args], cwd=ROOT, stdout=subprocess.PIPE,
        text=True, check=True, timeout=600,
    )
    return result.stdout


@contextmanager
def worker(binary: Path, env: dict[str, str]):
    with tempfile.TemporaryFile(mode="w+b") as log, subprocess.Popen(
        [binary], env=env, stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL, stderr=log,
    ) as process:
        try:
            yield process
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGTERM)
                try:
                    process.wait(timeout=35)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=3)
                    raise
            if process.returncode != 0:
                log.seek(0)
                raise AssertionError(log.read().decode(errors="replace"))


def poll(port: int, path: str, token: str, process: subprocess.Popen) -> dict:
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline and process.poll() is None:
        status, body, _ = phase2["request"](port, "GET", path, token=token)
        assert status == 200, status
        if body["state"] in {"completed", "failed", "cancelled"}:
            return body
        time.sleep(0.15)
    raise AssertionError(f"submission {path} did not reach a terminal state")


def verify_isolate(api: Path, worker_binary: Path, db, database_env: dict[str, str]):
    tenant, token = str(uuid4()), secrets.token_hex(32)
    env = phase2["api_env"](database_env["FERRUM_DATABASE_URL"], tenant, token)
    with worker(worker_binary, database_env) as process:
        with phase2["running_api"](api, env) as port:
            status, created, headers = phase2["create"](
                port, token, "success",
                {"language": "python", "source": "print(input())", "stdin": "phase 4\n"},
            )
            assert status == 202, status
            path = headers["location"]
            finished = poll(port, path, token, process)
            assert finished["state"] == "completed" and finished["verdict"] is None, finished
            result = finished["result"]
            assert result["stdout"] == "phase 4\n" and result["stderr"] == "", result
            assert result["exit_code"] == 0 and result["peak_memory_kib"] > 0, result

            status, limited, headers = phase2["create"](
                port, token, "time-limit",
                {"language": "python", "source": "import time\ntime.sleep(10)",
                 "limits": {"cpu_time_ms": 500, "wall_time_ms": 500}},
            )
            assert status == 202, status
            timed_out = poll(port, headers["location"], token, process)
            assert timed_out["state"] == "completed", timed_out
            assert timed_out["verdict"] == "time_limit_exceeded", timed_out
            assert timed_out["result"]["outcome"] == "wall_time_limit", timed_out

            status, cpp, headers = phase2["create"](
                port, token, "cpp-success",
                {
                    "language": "cpp",
                    "source": "#include <iostream>\n#include <string>\n"
                    "int main() { std::string line; std::getline(std::cin, line);"
                    " std::cout << line << std::endl; }",
                    "stdin": "phase 5\n",
                },
            )
            assert status == 202, status
            cpp_path = headers["location"]
            compiled = poll(port, cpp_path, token, process)
            assert compiled["state"] == "completed" and compiled["verdict"] is None, compiled
            assert compiled["result"]["stdout"] == "phase 5\n", compiled
            assert compiled["result"]["compilation"]["outcome"] == "success", compiled

            status, invalid_cpp, headers = phase2["create"](
                port, token, "cpp-invalid",
                {"language": "cpp", "source": "this is not valid source !"},
            )
            assert status == 202, status
            failed = poll(port, headers["location"], token, process)
            assert failed["verdict"] == "compilation_error", failed
            assert failed["result"]["compilation"]["stderr"], failed
            assert failed["result"]["exit_code"] is None, failed

        with phase2["running_api"](api, env) as port:
            assert phase2["request"](port, "GET", path, token=token)[:2] == (200, finished)
            assert phase2["request"](port, "GET", cpp_path, token=token)[:2] == (200, compiled)

    for submission in (created, limited, cpp, invalid_cpp):
        submission_id = UUID(submission["id"])
        row = db.query(
            "SELECT count(*), bool_and(cleanup_disposition = 'verified') "
            f"FROM execution_attempts WHERE submission_id = '{submission_id}'"
        )
        assert row == "1|t", row
        print(f"Isolate verified submission {submission_id}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--isolate", action="store_true")
    args = parser.parse_args()
    if args.isolate and (platform.system(), platform.machine()) != ("Linux", "x86_64"):
        parser.error("--isolate requires the qualified Debian 13 amd64 host")

    cargo("build", "--locked", "-p", "ferrum-api", "-p", "ferrum-worker")
    metadata = json.loads(cargo("metadata", "--locked", "--no-deps", "--format-version=1"))
    target = Path(metadata["target_directory"]) / "debug"
    api, worker_binary = target / "ferrum-api", target / "ferrum-worker"

    db = phase2["Database"]()
    db.project = f"ferrum-phase4-{secrets.token_hex(6)}"
    with db.running() as url:
        env = {**phase2["environment"](), "FERRUM_DATABASE_URL": url}
        for binary in (api, worker_binary):
            for _ in range(2):
                subprocess.run([binary, "migrate"], env=env, check=True, timeout=20)

        if args.isolate:
            verify_isolate(api, worker_binary, db, env)
        else:
            subprocess.run(
                ["cargo", "test", "--locked", "-p", "ferrum-worker", "--",
                 "--ignored", "--test-threads=1", "--nocapture"],
                cwd=ROOT, env=env, check=True, timeout=240,
            )
            print("Phase 4 PostgreSQL/Apalis pipeline passed with the sandbox test double")


if __name__ == "__main__":
    main()
