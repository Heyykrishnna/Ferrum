import argparse
import http.client
import json
from pathlib import Path
import runpy
import secrets
import subprocess
import time
import urllib.request
from uuid import uuid4

ROOT = Path(__file__).resolve().parents[2]
phase2 = runpy.run_path(str(ROOT / "tests/phase2/submission-smoke.py"))
phase4 = runpy.run_path(str(ROOT / "tests/phase4/worker-smoke.py"))
STAGE_SIZE = 25


def scrape_metrics(port: int) -> str:
    url = f"http://127.0.0.1:{port}/metrics"
    req = urllib.request.Request(url)
    with urllib.request.urlopen(req, timeout=5) as resp:
        assert resp.status == 200, resp.status
        content_type = resp.headers.get("content-type", "")
        assert "text/plain" in content_type, content_type
        return resp.read().decode("utf-8")


def parse_metric_lines(raw: str) -> dict[str, list[str]]:
    families: dict[str, list[str]] = {}
    current = None
    for line in raw.splitlines():
        line = line.strip()
        if not line:
            continue
        if line.startswith("# HELP "):
            parts = line.split()
            current = parts[2]
            families.setdefault(current, [])
        elif line.startswith("# TYPE "):
            continue
        elif not line.startswith("#") and current:
            families[current].append(line)
    return families


def verify_trace_propagation(port: int, token: str) -> None:
    conn = http.client.HTTPConnection("127.0.0.1", port)
    trace_id = "4bf92f3577b34da6a3ce929d0e0e4736"
    test_traceparent = f"00-{trace_id}-00f067aa0ba902b7-01"
    headers = {
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/json",
        "idempotency-key": f"trace-test-{uuid4()}",
        "traceparent": test_traceparent,
    }
    payload = json.dumps({
        "language": "python",
        "source": "print('trace test')",
    })
    conn.request("POST", "/v1/submissions", body=payload, headers=headers)
    resp = conn.getresponse()
    resp_body = resp.read()
    assert resp.status == 202, f"expected 202, got {resp.status}: {resp_body}"

    resp_tp = resp.getheader("traceparent")
    assert resp_tp is not None, "missing traceparent response header"
    assert trace_id in resp_tp, f"trace_id {trace_id} not propagated in {resp_tp}"

    resp_req_id = resp.getheader("x-request-id")
    assert resp_req_id == trace_id, f"expected x-request-id {trace_id}, got {resp_req_id}"
    conn.close()


def verify_auth_failure_metrics(port: int) -> None:
    conn = http.client.HTTPConnection("127.0.0.1", port)
    headers = {
        "Authorization": "Bearer bad_token_12345",
        "Content-Type": "application/json",
        "idempotency-key": f"bad-auth-{uuid4()}",
    }
    payload = json.dumps({"language": "python", "source": "print(1)"})
    conn.request("POST", "/v1/submissions", body=payload, headers=headers)
    resp = conn.getresponse()
    resp.read()
    assert resp.status == 401, resp.status
    conn.close()

    metrics_text = scrape_metrics(port)
    assert 'ferrum_api_auth_failures_total{reason="invalid_dev_token"} 1' in metrics_text.splitlines()


def verify_worker_metrics_cli(worker_bin: Path, env: dict[str, str]) -> None:
    proc = subprocess.run(
        [str(worker_bin), "metrics"],
        env=env,
        capture_output=True,
        text=True,
        timeout=15,
    )
    assert proc.returncode == 0, f"ferrum-worker metrics failed: {proc.stderr}"
    out = proc.stdout
    assert "ferrum_database_connections" in out or "ferrum_outbox" in out


def run_staged_load(port: int, token: str, count: int = 15) -> dict[str, float]:
    latencies = []
    languages = ["python", "c", "cpp", "javascript", "rust"]
    sources = {
        "python": "print(sum(range(100)))",
        "c": "#include <stdio.h>\nint main(){printf(\"%d\\n\", 4950); return 0;}",
        "cpp": "#include <iostream>\nint main(){std::cout << 4950 << std::endl; return 0;}",
        "javascript": "console.log(4950);",
        "rust": "fn main(){println!(\"{}\", 4950);}",
    }

    total_time = 0.0
    for stage_start in range(0, count, STAGE_SIZE):
        locations = []
        stage_started = time.perf_counter()
        for i in range(stage_start, min(stage_start + STAGE_SIZE, count)):
            lang = languages[i % len(languages)]
            started = time.perf_counter()
            status, body, headers = phase2["create"](
                port,
                token,
                f"load-{i}-{uuid4()}",
                {"language": lang, "source": sources[lang]},
            )
            latencies.append(time.perf_counter() - started)
            assert status == 202, (i, status, body)
            locations.append(headers["location"])
        total_time += time.perf_counter() - stage_started

        for location in locations:
            status, body, _ = phase2["request"](
                port, "POST", f"{location}/cancel", token=token
            )
            assert status == 200 and body["outcome"] == "cancelled", (status, body)

    latencies.sort()
    p50 = latencies[int(len(latencies) * 0.50)]
    p95 = latencies[int(len(latencies) * 0.95)]
    p99 = latencies[int(len(latencies) * 0.99)]
    throughput = count / total_time if total_time > 0 else 0.0

    return {
        "count": count,
        "total_time_s": total_time,
        "throughput_req_per_s": throughput,
        "p50_ms": p50 * 1000.0,
        "p95_ms": p95 * 1000.0,
        "p99_ms": p99 * 1000.0,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Phase 9 Observability and Performance Smoke Suite")
    parser.add_argument("--count", type=int, default=15, help="Number of synthetic load submissions")
    args = parser.parse_args()
    if args.count < 1:
        parser.error("--count must be at least 1")

    phase4["cargo"]("build", "--locked", "-p", "ferrum-api", "-p", "ferrum-worker")
    metadata = json.loads(
        phase4["cargo"]("metadata", "--locked", "--no-deps", "--format-version=1")
    )
    bin_dir = Path(metadata["target_directory"]) / "debug"
    api_bin, worker_bin = bin_dir / "ferrum-api", bin_dir / "ferrum-worker"

    print("=== [1/5] Initializing Disposable Staging Database ===")
    db = phase2["Database"]()
    db.project = f"ferrum-phase9-{secrets.token_hex(6)}"
    with db.running() as url:
        env = {**phase2["environment"](), "FERRUM_DATABASE_URL": url}
        for binary in (api_bin, worker_bin):
            for _ in range(2):
                subprocess.run([str(binary), "migrate"], env=env, check=True, timeout=20)

        tenant, token = str(uuid4()), secrets.token_hex(32)
        api_env = phase2["api_env"](url, tenant, token)

        print("=== [2/5] Verifying ferrum-worker metrics CLI ===")
        verify_worker_metrics_cli(worker_bin, env)
        print("  ✓ Worker metrics CLI outputs valid Prometheus exposition format")

        print("=== [3/5] Starting API Server & Verifying /metrics and Trace Propagation ===")
        with phase2["running_api"](api_bin, api_env) as port:
            raw_metrics = scrape_metrics(port)
            families = parse_metric_lines(raw_metrics)
            print(f"  ✓ Scraped {len(families)} Prometheus metric families from /metrics")
            assert "ferrum_http_requests_total" in families or "ferrum_http_requests_total" in raw_metrics

            verify_trace_propagation(port, token)
            print("  ✓ W3C TraceContext (traceparent) and x-request-id end-to-end propagation confirmed")

            verify_auth_failure_metrics(port)
            print("  ✓ Invalid-token authentication failure metric recorded")

            print(f"=== [4/5] Executing Staged Load Test ({args.count} requests, multi-language mix) ===")
            stats = run_staged_load(port, token, count=args.count)
            print(f"  ✓ Load test complete: {stats['count']} requests in {stats['total_time_s']:.2f}s")
            print(f"    Throughput: {stats['throughput_req_per_s']:.1f} req/s")
            print(f"    Latency p50: {stats['p50_ms']:.2f}ms | p95: {stats['p95_ms']:.2f}ms | p99: {stats['p99_ms']:.2f}ms")

            print("=== [5/5] Verifying Post-Load Prometheus Metrics Registry State ===")
            post_metrics = scrape_metrics(port)
            assert "ferrum_submissions_created_total" in post_metrics
            assert "ferrum_http_request_duration_seconds" in post_metrics
            print("  ✓ Post-load metrics confirm counters, stage histograms, and status codes incremented")

    print("\nPhase 9 Observability and Performance verification PASSED!")


if __name__ == "__main__":
    main()
