#!/usr/bin/env python3
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


def find_free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def resolve_binary(name: str) -> Path:
    env_override = os.environ.get(f"FERRUM_{name.upper().replace('-', '_')}_BIN")
    if env_override:
        return Path(env_override)
    for profile in ("release", "debug"):
        candidate = ROOT / "target" / profile / name
        if candidate.is_file():
            return candidate
    raise RuntimeError(f"could not locate compiled binary {name}; run cargo build --bins --locked")


def request_json(
    method: str,
    url: str,
    headers: dict[str, str] | None = None,
    payload: dict | None = None,
) -> tuple[int, dict[str, str], object]:
    raw_body = None
    req_headers = dict(headers or {})
    if payload is not None:
        raw_body = json.dumps(payload).encode("utf-8")
        req_headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=raw_body, headers=req_headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            body_bytes = resp.read()
            resp_headers = {k.lower(): v for k, v in resp.headers.items()}
            if not body_bytes:
                return resp.status, resp_headers, None
            content_type = resp_headers.get("content-type", "")
            if "application/json" in content_type:
                return resp.status, resp_headers, json.loads(body_bytes.decode("utf-8"))
            return resp.status, resp_headers, body_bytes.decode("utf-8")
    except urllib.error.HTTPError as err:
        body_bytes = err.read()
        resp_headers = {k.lower(): v for k, v in err.headers.items()}
        parsed: object = body_bytes.decode("utf-8", errors="replace")
        try:
            parsed = json.loads(parsed)
        except json.JSONDecodeError:
            pass
        return err.code, resp_headers, parsed


def wait_for_ready(base_url: str, proc: subprocess.Popen, timeout_secs: float = 20.0) -> None:
    deadline = time.monotonic() + timeout_secs
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(f"ferrum-api exited prematurely with code {proc.returncode}")
        try:
            status, _, body = request_json("GET", f"{base_url}/health/ready")
            if status == 200 and isinstance(body, dict) and body.get("status") == "ready":
                return
        except urllib.error.URLError:
            pass
        time.sleep(0.15)
    raise RuntimeError("timed out waiting for ferrum-api /health/ready")


def assert_eq(actual, expected, label: str) -> None:
    if actual != expected:
        raise AssertionError(f"{label}: expected {expected!r}, got {actual!r}")


def run_cli(binary: Path, args: list[str], env: dict[str, str]) -> str:
    result = subprocess.run(
        [str(binary), *args],
        env=env,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"{binary.name} {' '.join(args)} failed ({result.returncode}): {result.stderr.strip()}"
        )
    return result.stdout


def main() -> None:
    database_url = os.environ.get("FERRUM_DATABASE_URL")
    if not database_url:
        raise RuntimeError("FERRUM_DATABASE_URL must be set")

    api_bin = resolve_binary("ferrum-api")
    worker_bin = resolve_binary("ferrum-worker")

    tenant_id = os.environ.get("FERRUM_DEV_TENANT_ID") or str(uuid.uuid4())
    dev_token = os.environ.get("FERRUM_DEV_TOKEN") or (uuid.uuid4().hex + uuid.uuid4().hex)
    port = find_free_port()
    base_url = f"http://127.0.0.1:{port}"

    env = os.environ.copy()
    env.update(
        {
            "FERRUM_DATABASE_URL": database_url,
            "FERRUM_DEV_TENANT_ID": tenant_id,
            "FERRUM_DEV_TOKEN": dev_token,
            "FERRUM_API_LISTEN_ADDR": f"127.0.0.1:{port}",
            "FERRUM_MODE": "development",
            "FERRUM_LOG_LEVEL": "info",
        }
    )

    run_cli(api_bin, ["migrate"], env)
    run_cli(worker_bin, ["migrate"], env)
    print("PASS PostgreSQL and Apalis migrations applied")

    api_proc = subprocess.Popen(
        [str(api_bin)],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        wait_for_ready(base_url, api_proc)

        status, headers, body = request_json("GET", f"{base_url}/health/live")
        assert_eq(status, 200, "GET /health/live")
        assert_eq(body, {"status": "live"}, "GET /health/live body")
        assert "traceparent" in headers, "missing traceparent response header"
        assert "x-request-id" in headers, "missing x-request-id response header"

        status, _, _ = request_json("GET", f"{base_url}/v1/languages")
        assert_eq(status, 401, "unauthenticated /v1/languages")

        auth_headers = {"Authorization": f"Bearer {dev_token}"}
        status, _, langs = request_json("GET", f"{base_url}/v1/languages", headers=auth_headers)
        assert_eq(status, 200, "GET /v1/languages")
        lang_names = {item["language"] for item in langs}
        expected_langs = {"python", "c", "cpp", "javascript", "java", "rust"}
        assert expected_langs.issubset(lang_names), f"missing languages: {expected_langs - lang_names}"

        status, _, exec_status = request_json(
            "GET", f"{base_url}/v1/execution/status", headers=auth_headers
        )
        assert_eq(status, 200, "GET /v1/execution/status")
        assert exec_status["worker"] in {"available", "offline", "queue_uninitialized"}

        idem_key = f"smoke-{uuid.uuid4().hex}"
        sub_headers = {**auth_headers, "Idempotency-Key": idem_key}
        payload = {"language": "python", "source": "print(42)"}
        status, resp_headers, created = request_json(
            "POST", f"{base_url}/v1/submissions", headers=sub_headers, payload=payload
        )
        assert_eq(status, 202, "POST /v1/submissions")
        submission_id = created["id"]
        assert_eq(resp_headers.get("location"), f"/v1/submissions/{submission_id}", "Location header")

        status, _, replayed = request_json(
            "POST", f"{base_url}/v1/submissions", headers=sub_headers, payload=payload
        )
        assert_eq(status, 202, "idempotent replay status")
        assert_eq(replayed["id"], submission_id, "idempotent replay id")

        status, _, _ = request_json(
            "POST",
            f"{base_url}/v1/submissions",
            headers=sub_headers,
            payload={"language": "python", "source": "print(99)"},
        )
        assert_eq(status, 409, "idempotency conflict status")

        status, _, fetched = request_json(
            "GET", f"{base_url}/v1/submissions/{submission_id}", headers=auth_headers
        )
        assert_eq(status, 200, "GET /v1/submissions/{id}")
        assert_eq(fetched["id"], submission_id, "fetched submission id")

        status, _, cancelled = request_json(
            "POST", f"{base_url}/v1/submissions/{submission_id}/cancel", headers=auth_headers
        )
        assert_eq(status, 200, "POST /v1/submissions/{id}/cancel")
        assert_eq(cancelled["outcome"], "cancelled", "cancel outcome")

        status, _, repeat_cancel = request_json(
            "POST", f"{base_url}/v1/submissions/{submission_id}/cancel", headers=auth_headers
        )
        assert_eq(status, 200, "repeat cancel status")
        assert_eq(repeat_cancel["outcome"], "already_terminal", "repeat cancel outcome")

        batch_payload = {
            "submissions": [
                {
                    "idempotency_key": f"batch-ok-{uuid.uuid4().hex}",
                    "language": "python",
                    "source": "print('ok')",
                },
                {
                    "idempotency_key": f"batch-bad-{uuid.uuid4().hex}",
                    "language": "unknown-lang",
                    "source": "print('bad')",
                },
            ]
        }
        status, _, batch = request_json(
            "POST", f"{base_url}/v1/submissions/batch", headers=auth_headers, payload=batch_payload
        )
        assert_eq(status, 200, "POST /v1/submissions/batch")
        assert_eq(len(batch["items"]), 2, "batch response item count")
        assert batch["items"][0]["id"] is not None
        assert batch["items"][1]["error"] is not None

        status, _, _ = request_json(
            "POST",
            f"{base_url}/v1/submissions/{batch['items'][0]['id']}/cancel",
            headers=auth_headers,
        )
        assert_eq(status, 200, "cancel batch item")

        status, _, created_key = request_json(
            "POST",
            f"{base_url}/v1/api-keys",
            headers=auth_headers,
            payload={"name": "smoke-readonly", "scopes": ["submissions:read"]},
        )
        assert_eq(status, 201, "POST /v1/api-keys")
        ro_headers = {"Authorization": f"Bearer {created_key['raw_key']}"}
        status, _, _ = request_json("GET", f"{base_url}/v1/languages", headers=ro_headers)
        assert_eq(status, 200, "read-only key GET /v1/languages")
        status, _, _ = request_json(
            "POST",
            f"{base_url}/v1/submissions",
            headers={**ro_headers, "Idempotency-Key": f"ro-{uuid.uuid4().hex}"},
            payload=payload,
        )
        assert_eq(status, 403, "read-only key forbidden from creating submissions")

        status, _, _ = request_json(
            "DELETE", f"{base_url}/v1/api-keys/{created_key['id']}", headers=auth_headers
        )
        assert_eq(status, 204, "DELETE /v1/api-keys/{id}")
        status, _, _ = request_json("GET", f"{base_url}/v1/languages", headers=ro_headers)
        assert_eq(status, 401, "revoked API key rejected")

        for blocked_url in (
            "http://127.0.0.1:8080/hook",
            "http://169.254.169.254/latest/meta-data",
            "file:///etc/passwd",
        ):
            status, _, _ = request_json(
                "POST",
                f"{base_url}/v1/webhooks",
                headers=auth_headers,
                payload={"url": blocked_url, "events": ["submission.completed"]},
            )
            assert_eq(status, 400, f"SSRF webhook blocked for {blocked_url}")

        status, _, hook = request_json(
            "POST",
            f"{base_url}/v1/webhooks",
            headers=auth_headers,
            payload={"url": "https://hooks.example.com/ferrum-smoke", "events": ["submission.completed"]},
        )
        assert_eq(status, 201, "POST /v1/webhooks")
        status, _, _ = request_json(
            "DELETE", f"{base_url}/v1/webhooks/{hook['id']}", headers=auth_headers
        )
        assert_eq(status, 204, "DELETE /v1/webhooks/{id}")

        status, _, metrics_text = request_json("GET", f"{base_url}/metrics")
        assert_eq(status, 200, "GET /metrics")
        assert "ferrum_http_requests_total" in metrics_text
        assert "ferrum_submissions_created_total" in metrics_text

        reconcile_out = json.loads(run_cli(worker_bin, ["reconcile"], env))
        assert "stale_attempts_reconciled" in reconcile_out
        assert "stuck_outbox_events_reconciled" in reconcile_out

        quarantine_out = json.loads(run_cli(worker_bin, ["quarantine", "list"], env))
        assert isinstance(quarantine_out, list)

        worker_metrics = run_cli(worker_bin, ["metrics"], env)
        assert "ferrum_database_connections_active" in worker_metrics

        print("PASS API, auth, idempotency, batch, SSRF, metrics, and worker CLI smoke checks")
    finally:
        if api_proc.poll() is None:
            api_proc.send_signal(signal.SIGTERM)
            try:
                exit_code = api_proc.wait(timeout=15)
            except subprocess.TimeoutExpired:
                api_proc.kill()
                raise RuntimeError("ferrum-api did not shut down gracefully on SIGTERM")
            if exit_code != 0:
                stderr = api_proc.stderr.read() if api_proc.stderr else ""
                raise RuntimeError(f"ferrum-api exited with {exit_code} after SIGTERM: {stderr}")
        print("PASS ferrum-api graceful SIGTERM shutdown")


if __name__ == "__main__":
    main()

