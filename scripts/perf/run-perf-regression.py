#!/usr/bin/env python3
import argparse
import json
import os
import signal
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BASELINE_PATH = ROOT / "scripts" / "perf" / "baseline.json"
K6_SCRIPT_PATH = ROOT / "scripts" / "perf" / "intake-regression.js"


def find_free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def resolve_api_binary() -> Path:
    env_override = os.environ.get("FERRUM_API_BIN")
    if env_override:
        return Path(env_override)
    for profile in ("release", "debug"):
        candidate = ROOT / "target" / profile / "ferrum-api"
        if candidate.is_file():
            return candidate
    raise RuntimeError("could not locate ferrum-api binary; build with cargo build --release --bin ferrum-api --locked")


def wait_for_ready(base_url: str, proc: subprocess.Popen, timeout_secs: float = 20.0) -> None:
    deadline = time.monotonic() + timeout_secs
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(f"ferrum-api exited prematurely with code {proc.returncode}")
        try:
            with urllib.request.urlopen(f"{base_url}/health/ready", timeout=2) as resp:
                if resp.status == 200:
                    return
        except urllib.error.URLError:
            pass
        time.sleep(0.15)
    raise RuntimeError("timed out waiting for ferrum-api /health/ready")


def evaluate_regression(summary: dict, baseline: dict) -> tuple[list[str], dict]:
    metrics = summary.get("metrics", {})
    intake = metrics.get("intake_duration", {}).get("values", {})
    cancel = metrics.get("cancel_duration", {}).get("values", {})
    checks = metrics.get("checks", {}).get("values", {})
    dropped = metrics.get("dropped_iterations", {}).get("values", {})
    rejected = metrics.get("intake_rejected", {}).get("values", {})
    accepted = metrics.get("intake_accepted", {}).get("values", {})

    measured = {
        "intake_count": int(accepted.get("count", 0)),
        "intake_p50_ms": float(intake.get("p(50)", 0.0)),
        "intake_p95_ms": float(intake.get("p(95)", 0.0)),
        "intake_p99_ms": float(intake.get("p(99)", 0.0)),
        "intake_avg_ms": float(intake.get("avg", 0.0)),
        "cancel_p95_ms": float(cancel.get("p(95)", 0.0)),
        "check_rate": float(checks.get("rate", 0.0)),
        "dropped_iterations": int(dropped.get("count", 0)),
        "intake_rejected": int(rejected.get("count", 0)),
    }

    budgets = baseline["budgets"]
    failures: list[str] = []
    if measured["intake_count"] == 0:
        failures.append("zero accepted intake samples recorded")
    for metric_key in (
        "intake_p50_ms",
        "intake_p95_ms",
        "intake_p99_ms",
        "intake_avg_ms",
        "cancel_p95_ms",
    ):
        if measured[metric_key] > budgets[metric_key]:
            failures.append(
                f"{metric_key} {measured[metric_key]:.2f}ms exceeded budget {budgets[metric_key]:.2f}ms"
            )
    if measured["check_rate"] < budgets["min_check_rate"]:
        failures.append(
            f"check_rate {measured['check_rate']:.4f} below minimum {budgets['min_check_rate']:.4f}"
        )
    if measured["dropped_iterations"] > budgets["max_dropped_iterations"]:
        failures.append(
            f"dropped_iterations {measured['dropped_iterations']} exceeded {budgets['max_dropped_iterations']}"
        )
    if measured["intake_rejected"] > budgets["max_intake_rejected"]:
        failures.append(
            f"intake_rejected {measured['intake_rejected']} exceeded {budgets['max_intake_rejected']}"
        )

    return failures, measured


def format_markdown_report(measured: dict, baseline: dict, failures: list[str], rate: int, duration: str) -> str:
    budgets = baseline["budgets"]
    status = "PASS" if not failures else "FAIL"
    lines = [
        f"## Ferrum Performance Regression Report ({status})",
        "",
        f"- Workload: `{baseline['workload']}` ({rate} RPS for `{duration}`, {measured['intake_count']} accepted submissions)",
        "",
        "| Metric | Measured | Budget | Status |",
        "|---|---:|---:|---|",
        f"| Intake p50 | {measured['intake_p50_ms']:.2f} ms | <= {budgets['intake_p50_ms']:.2f} ms | {'PASS' if measured['intake_p50_ms'] <= budgets['intake_p50_ms'] else 'FAIL'} |",
        f"| Intake p95 | {measured['intake_p95_ms']:.2f} ms | <= {budgets['intake_p95_ms']:.2f} ms | {'PASS' if measured['intake_p95_ms'] <= budgets['intake_p95_ms'] else 'FAIL'} |",
        f"| Intake p99 | {measured['intake_p99_ms']:.2f} ms | <= {budgets['intake_p99_ms']:.2f} ms | {'PASS' if measured['intake_p99_ms'] <= budgets['intake_p99_ms'] else 'FAIL'} |",
        f"| Intake avg | {measured['intake_avg_ms']:.2f} ms | <= {budgets['intake_avg_ms']:.2f} ms | {'PASS' if measured['intake_avg_ms'] <= budgets['intake_avg_ms'] else 'FAIL'} |",
        f"| Cancel p95 | {measured['cancel_p95_ms']:.2f} ms | <= {budgets['cancel_p95_ms']:.2f} ms | {'PASS' if measured['cancel_p95_ms'] <= budgets['cancel_p95_ms'] else 'FAIL'} |",
        f"| Check rate | {measured['check_rate'] * 100:.2f}% | >= {budgets['min_check_rate'] * 100:.2f}% | {'PASS' if measured['check_rate'] >= budgets['min_check_rate'] else 'FAIL'} |",
        f"| Dropped iterations | {measured['dropped_iterations']} | <= {budgets['max_dropped_iterations']} | {'PASS' if measured['dropped_iterations'] <= budgets['max_dropped_iterations'] else 'FAIL'} |",
        f"| Intake rejected | {measured['intake_rejected']} | <= {budgets['max_intake_rejected']} | {'PASS' if measured['intake_rejected'] <= budgets['max_intake_rejected'] else 'FAIL'} |",
    ]
    if failures:
        lines.extend(["", "### Regressions Detected", ""])
        lines.extend(f"- {item}" for item in failures)
    lines.append("")
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rate", type=int, default=None)
    parser.add_argument("--duration", type=str, default=None)
    parser.add_argument("--report-dir", type=Path, default=ROOT / "output" / "perf")
    args = parser.parse_args()

    database_url = os.environ.get("FERRUM_DATABASE_URL")
    if not database_url:
        raise RuntimeError("FERRUM_DATABASE_URL must be set")

    baseline = json.loads(BASELINE_PATH.read_text(encoding="utf-8"))
    rate = args.rate or int(os.environ.get("FERRUM_PERF_RATE", baseline["default_rate_rps"]))
    duration = args.duration or os.environ.get("FERRUM_PERF_DURATION", baseline["default_duration"])

    args.report_dir.mkdir(parents=True, exist_ok=True)
    summary_path = args.report_dir / "k6-summary.json"
    report_json_path = args.report_dir / "regression-report.json"
    report_md_path = args.report_dir / "regression-report.md"

    api_bin = resolve_api_binary()
    port = find_free_port()
    base_url = f"http://127.0.0.1:{port}"
    tenant_id = str(uuid.uuid4())
    dev_token = uuid.uuid4().hex + uuid.uuid4().hex

    env = os.environ.copy()
    env.update(
        {
            "FERRUM_DATABASE_URL": database_url,
            "FERRUM_DEV_TENANT_ID": tenant_id,
            "FERRUM_DEV_TOKEN": dev_token,
            "FERRUM_API_LISTEN_ADDR": f"127.0.0.1:{port}",
            "FERRUM_MODE": "development",
            "FERRUM_LOG_LEVEL": "warn",
            "FERRUM_DB_MAX_CONNECTIONS": "20",
        }
    )

    migrate = subprocess.run([str(api_bin), "migrate"], env=env, capture_output=True, text=True, check=False)
    if migrate.returncode != 0:
        raise RuntimeError(f"ferrum-api migrate failed: {migrate.stderr.strip()}")

    api_proc = subprocess.Popen(
        [str(api_bin)],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        wait_for_ready(base_url, api_proc)

        k6_env = env.copy()
        k6_env.update(
            {
                "FERRUM_BASE_URL": base_url,
                "FERRUM_K6_RATE": str(rate),
                "FERRUM_K6_DURATION": duration,
                "FERRUM_K6_SUMMARY_PATH": str(summary_path),
            }
        )
        k6_result = subprocess.run(
            ["k6", "run", "--no-color", str(K6_SCRIPT_PATH)],
            env=k6_env,
            check=False,
        )

        with urllib.request.urlopen(f"{base_url}/metrics", timeout=5) as resp:
            metrics_body = resp.read().decode("utf-8")
            if "ferrum_http_request_duration_seconds" not in metrics_body:
                raise RuntimeError("expected ferrum_http_request_duration_seconds in /metrics")

        if not summary_path.is_file():
            raise RuntimeError(f"k6 did not produce summary JSON at {summary_path}")

        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        failures, measured = evaluate_regression(summary, baseline)
        if k6_result.returncode != 0:
            failures.append(f"k6 exited with status {k6_result.returncode}")

        report_payload = {
            "status": "pass" if not failures else "fail",
            "rate_rps": rate,
            "duration": duration,
            "measured": measured,
            "budgets": baseline["budgets"],
            "failures": failures,
        }
        report_json_path.write_text(json.dumps(report_payload, indent=2) + "\n", encoding="utf-8")
        markdown = format_markdown_report(measured, baseline, failures, rate, duration)
        report_md_path.write_text(markdown, encoding="utf-8")
        print(markdown)

        step_summary = os.environ.get("GITHUB_STEP_SUMMARY")
        if step_summary:
            with open(step_summary, "a", encoding="utf-8") as handle:
                handle.write(markdown + "\n")

        if failures:
            sys.exit(1)
    finally:
        if api_proc.poll() is None:
            api_proc.send_signal(signal.SIGTERM)
            try:
                api_proc.wait(timeout=15)
            except subprocess.TimeoutExpired:
                api_proc.kill()


if __name__ == "__main__":
    main()

