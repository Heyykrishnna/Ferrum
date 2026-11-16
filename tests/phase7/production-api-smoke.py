import argparse
from contextlib import contextmanager
import http.client
import json
import os
from pathlib import Path
import secrets
import signal
import subprocess
import tempfile
import time
from urllib.parse import urlsplit
from uuid import uuid4

ROOT = Path(__file__).resolve().parents[2]
REQUEST = {"language": "python", "source": "print(input())\n"}


def environment() -> dict[str, str]:
    return {
        key: value for key, value in os.environ.items() if not key.startswith("FERRUM_")
    }


class Database:
    def __init__(self) -> None:
        self.password = secrets.token_hex(24)
        self.project = f"ferrum-phase7-{secrets.token_hex(6)}"
        self.env = {
            **environment(),
            "FERRUM_POSTGRES_PASSWORD": self.password,
            "FERRUM_POSTGRES_PORT": "0",
        }

    def compose(self, *args: str) -> str:
        result = subprocess.run(
            ["docker", "compose", "--env-file", os.devnull, "-p", self.project, *args],
            cwd=ROOT,
            env=self.env,
            stdout=subprocess.PIPE,
            text=True,
            check=True,
            timeout=90,
        )
        return result.stdout.strip()

    def query(self, sql: str) -> str:
        return self.compose(
            "exec",
            "-T",
            "postgres",
            "psql",
            "-X",
            "-qAt",
            "-U",
            "ferrum",
            "-d",
            "ferrum",
            "-v",
            "ON_ERROR_STOP=1",
            "-c",
            sql,
        )

    @contextmanager
    def running(self):
        try:
            self.compose("up", "-d", "--wait", "--wait-timeout", "60", "postgres")
            _, port = self.compose("port", "postgres", "5432").rsplit(":", 1)
            yield f"postgres://ferrum:{self.password}@127.0.0.1:{int(port)}/ferrum"
        finally:
            self.compose("down", "--volumes")


def api_env(url: str, tenant: str, token: str) -> dict[str, str]:
    return {
        **environment(),
        "FERRUM_DATABASE_URL": url,
        "FERRUM_DEV_TENANT_ID": tenant,
        "FERRUM_DEV_TOKEN": token,
        "FERRUM_API_LISTEN_ADDR": "127.0.0.1:0",
        "FERRUM_DB_ACQUIRE_TIMEOUT_MS": "2000",
    }


@contextmanager
def running_api(binary: Path, env: dict[str, str]):
    with tempfile.TemporaryDirectory(prefix="ferrum-api-smoke-") as directory:
        path = Path(directory) / "stderr"
        with path.open("wb") as log, subprocess.Popen(
            [binary],
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=log,
        ) as process:
            try:
                deadline = time.monotonic() + 10
                while process.poll() is None and time.monotonic() < deadline:
                    for line in path.read_text().splitlines(keepends=True):
                        if line.startswith("{") and line.endswith("\n"):
                            fields = json.loads(line)["fields"]
                            if fields.get("event") == "api_listening":
                                _, port = fields["listen_addr"].rsplit(":", 1)
                                yield int(port)
                                return
                    time.sleep(0.02)
                raise AssertionError(f"API could not start: {path.read_text()}")
            finally:
                if process.poll() is None:
                    process.send_signal(signal.SIGTERM)
                    try:
                        assert process.wait(timeout=5) == 0, path.read_text()
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=2)
                        raise


def request(
    port: int,
    method: str,
    path: str,
    *,
    token: str | None = None,
    body: dict | bytes | None = None,
    key: str | None = None,
) -> tuple[int, dict | list, dict]:
    headers = {"Content-Type": "application/json"}
    if token is not None:
        headers["Authorization"] = f"Bearer {token}"
    if key is not None:
        headers["Idempotency-Key"] = key
    data = json.dumps(body) if isinstance(body, dict) else body
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
    connection.request(method, path, body=data, headers=headers)
    response = connection.getresponse()
    raw = response.read()
    payload = json.loads(raw.decode("utf-8")) if raw else {}
    headers_dict = dict(response.getheaders())
    connection.close()
    return response.status, payload, headers_dict


def main() -> None:
    parser = argparse.ArgumentParser(description="Phase 7 production API smoke test")
    parser.add_argument("--bin", type=Path, default=ROOT / "target" / "debug" / "ferrum-api")
    args = parser.parse_args()

    subprocess.run(["cargo", "build", "--bin", "ferrum-api"], cwd=ROOT, check=True)
    api = args.bin.resolve()

    db = Database()
    with db.running() as url:
        org_id = str(uuid4())
        dev_token = secrets.token_hex(20)
        env = api_env(url, org_id, dev_token)

        subprocess.run([api, "migrate"], env=env, check=True, timeout=20)
        assert (
            db.query(
                "SELECT array_agg(version ORDER BY version) FROM _sqlx_migrations WHERE success"
            )
            == "{1,2,3,4,5}"
        )

        with running_api(api, env) as port:
            status, payload, _ = request(
                port,
                "POST",
                "/v1/api-keys",
                token=dev_token,
                body={"name": "writer_key", "scopes": ["submissions:write", "submissions:read"]},
            )
            assert status == 201, (status, payload)
            writer_key_id = payload["id"]
            writer_token = payload["raw_key"]
            assert writer_token.startswith("ferrum_live_")
            assert payload["key_prefix"].startswith("ferrum_live_")

            key1 = f"key-{secrets.token_hex(8)}"
            status, sub_payload, _ = request(
                port,
                "POST",
                "/v1/submissions",
                token=writer_token,
                key=key1,
                body=REQUEST,
            )
            assert status == 202, (status, sub_payload)
            submission_id = sub_payload["id"]

            status, fetched, _ = request(
                port,
                "GET",
                f"/v1/submissions/{submission_id}",
                token=writer_token,
            )
            assert status == 200, (status, fetched)
            assert fetched["id"] == submission_id

            status, err_payload, _ = request(
                port,
                "POST",
                f"/v1/submissions/{submission_id}/cancel",
                token=writer_token,
            )
            assert status == 403, (status, err_payload)
            assert err_payload["error"]["code"] == "insufficient_scope"

            status, err_payload, _ = request(
                port,
                "POST",
                "/v1/api-keys",
                token=writer_token,
                body={"name": "illegal", "scopes": ["admin"]},
            )
            assert status == 403, (status, err_payload)
            assert err_payload["error"]["code"] == "insufficient_scope"

            status, cancel_key_payload, _ = request(
                port,
                "POST",
                "/v1/api-keys",
                token=dev_token,
                body={"name": "canceller_key", "scopes": ["submissions:cancel"]},
            )
            assert status == 201, (status, cancel_key_payload)
            canceller_token = cancel_key_payload["raw_key"]

            status, cancel_res, _ = request(
                port,
                "POST",
                f"/v1/submissions/{submission_id}/cancel",
                token=canceller_token,
            )
            assert status == 200, (status, cancel_res)
            assert cancel_res["outcome"] == "cancelled"

            status, cancel_res2, _ = request(
                port,
                "POST",
                f"/v1/submissions/{submission_id}/cancel",
                token=canceller_token,
            )
            assert status == 200, (status, cancel_res2)
            assert cancel_res2["outcome"] == "already_terminal"

            batch_body = {
                "submissions": [
                    {
                        "idempotency_key": f"batch-{secrets.token_hex(6)}",
                        "language": "python",
                        "source": "print(1)\n",
                    },
                    {
                        "idempotency_key": f"batch-{secrets.token_hex(6)}",
                        "language": "python",
                        "source": "print(2)\n",
                    },
                ]
            }
            status, batch_res, _ = request(
                port,
                "POST",
                "/v1/submissions/batch",
                token=writer_token,
                body=batch_body,
            )
            assert status == 200, (status, batch_res)
            assert len(batch_res["items"]) == 2
            assert all(item.get("id") is not None for item in batch_res["items"])

            status, err_payload, _ = request(
                port,
                "POST",
                "/v1/submissions/batch",
                token=writer_token,
                body={"submissions": []},
            )
            assert status == 400, (status, err_payload)
            assert err_payload["error"]["code"] == "empty_batch"

            large_batch = {
                "submissions": [
                    {
                        "idempotency_key": f"batch-{i}-{secrets.token_hex(4)}",
                        "language": "python",
                        "source": "print(1)\n",
                    }
                    for i in range(21)
                ]
            }
            status, err_payload, _ = request(
                port,
                "POST",
                "/v1/submissions/batch",
                token=writer_token,
                body=large_batch,
            )
            assert status == 400, (status, err_payload)
            assert err_payload["error"]["code"] == "batch_too_large"

            status, wh_key_payload, _ = request(
                port,
                "POST",
                "/v1/api-keys",
                token=dev_token,
                body={"name": "webhook_key", "scopes": ["webhooks:manage"]},
            )
            assert status == 201, (status, wh_key_payload)
            wh_token = wh_key_payload["raw_key"]

            status, ssrf_err, _ = request(
                port,
                "POST",
                "/v1/webhooks",
                token=wh_token,
                body={"url": "http://127.0.0.1:8080/hook"},
            )
            assert status == 400, (status, ssrf_err)
            assert ssrf_err["error"]["code"] == "blocked_destination"

            status, ssrf_err, _ = request(
                port,
                "POST",
                "/v1/webhooks",
                token=wh_token,
                body={"url": "http://169.254.169.254/latest/meta-data"},
            )
            assert status == 400, (status, ssrf_err)
            assert ssrf_err["error"]["code"] == "blocked_destination"

            status, wh_payload, _ = request(
                port,
                "POST",
                "/v1/webhooks",
                token=wh_token,
                body={"url": "https://example.com/ferrum-webhook", "events": ["submission.completed"]},
            )
            assert status == 201, (status, wh_payload)
            wh_id = wh_payload["id"]
            assert wh_payload["secret"].startswith("whsec_")
            assert wh_payload["active"] is True

            status, wh_list, _ = request(
                port,
                "GET",
                "/v1/webhooks",
                token=wh_token,
            )
            assert status == 200, (status, wh_list)
            assert any(item["id"] == wh_id for item in wh_list)

            status, _, _ = request(
                port,
                "DELETE",
                f"/v1/webhooks/{wh_id}",
                token=wh_token,
            )
            assert status == 204

            status, not_found_wh, _ = request(
                port,
                "DELETE",
                f"/v1/webhooks/{wh_id}",
                token=wh_token,
            )
            assert status == 404, status

            status, _, _ = request(
                port,
                "DELETE",
                f"/v1/api-keys/{writer_key_id}",
                token=dev_token,
            )
            assert status == 204

            status, unauth_payload, _ = request(
                port,
                "GET",
                f"/v1/submissions/{submission_id}",
                token=writer_token,
            )
            assert status == 401, status

    print("Phase 7 production API smoke test passed successfully")


if __name__ == "__main__":
    main()
