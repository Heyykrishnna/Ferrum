import argparse
import json
from pathlib import Path
import platform
import runpy
import secrets
import subprocess

ROOT = Path(__file__).resolve().parents[2]
phase2 = runpy.run_path(str(ROOT / "tests/phase2/submission-smoke.py"))
phase4 = runpy.run_path(str(ROOT / "tests/phase4/worker-smoke.py"))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--isolate", action="store_true")
    args = parser.parse_args()
    if args.isolate and (platform.system() != "Linux" or platform.machine() not in {"x86_64", "aarch64"}):
        parser.error("--isolate requires a qualified Debian 13 amd64 or arm64 host")

    phase4["cargo"]("build", "--locked", "-p", "ferrum-api", "-p", "ferrum-worker")
    metadata = json.loads(
        phase4["cargo"]("metadata", "--locked", "--no-deps", "--format-version=1")
    )
    target = Path(metadata["target_directory"]) / "debug"
    api, worker_binary = target / "ferrum-api", target / "ferrum-worker"

    db = phase2["Database"]()
    db.project = f"ferrum-phase6-{secrets.token_hex(6)}"
    with db.running() as url:
        env = {**phase2["environment"](), "FERRUM_DATABASE_URL": url}
        for binary in (api, worker_binary):
            for _ in range(2):
                subprocess.run([binary, "migrate"], env=env, check=True, timeout=20)

        db.verify_migrations()

        if args.isolate:
            phase4["verify_isolate"](api, worker_binary, db, env)
            print("Phase 6 judging smoke passed with live Isolate sandboxes")
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
            print("Phase 6 judging pipeline passed with the sandbox test double")


if __name__ == "__main__":
    main()
