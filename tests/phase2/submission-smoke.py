import argparse
from concurrent.futures import ThreadPoolExecutor
from contextlib import closing, contextmanager
import http.client
import json
import os
from pathlib import Path
import secrets
import signal
import subprocess
import tempfile
from threading import Barrier
import time
from urllib.parse import urlsplit
from uuid import uuid4

ROOT = Path(__file__).resolve().parents[2]
REQUEST = {"language": "python", "source": "print(input())\n"}
DEFAULT_LIMITS = {"cpu_time_ms": 2000, "wall_time_ms": 6000, "memory_kib": 131072}


def environment() -> dict[str, str]:
    return {
        key: value for key, value in os.environ.items() if not key.startswith("FERRUM_")
    }


class Database:
    def __init__(self) -> None:
        self.password = secrets.token_hex(24)
        self.project = f"ferrum-phase2-{secrets.token_hex(6)}"
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


def assert_startup_error(binary: Path, env: dict[str, str], message: str) -> None:
    result = subprocess.run(
        [binary], env=env, capture_output=True, text=True, timeout=10
    )
    assert result.returncode == 1 and message in result.stderr, result.returncode
    url = env["FERRUM_DATABASE_URL"]
    password = urlsplit(url).password
    assert url not in result.stderr and password not in result.stderr
    token = env.get("FERRUM_DEV_TOKEN")
    assert token is None or token not in result.stderr


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
    extra: dict[str, str] | None = None,
) -> tuple[int, dict | list, dict]:
    headers = {"Content-Type": "application/json", **(extra or {})}
    if token is not None:
        headers["Authorization"] = f"Bearer {token}"
    if key is not None:
        headers["Idempotency-Key"] = key
    data = json.dumps(body) if isinstance(body, dict) else body
    with closing(
        http.client.HTTPConnection("127.0.0.1", port, timeout=10)
    ) as connection:
        connection.request(method, path, body=data, headers=headers)
        response = connection.getresponse()
        return (
            response.status,
            json.loads(response.read()),
            {key.lower(): value for key, value in response.getheaders()},
        )


def create(
    port: int, token: str, key: str, body: dict = REQUEST
) -> tuple[int, dict, dict]:
    return request(port, "POST", "/v1/submissions", token=token, body=body, key=key)


def counts(db: Database) -> tuple[int, int]:
    values = db.query(
        "SELECT (SELECT count(*) FROM submissions), (SELECT count(*) FROM outbox_events)"
    )
    first, second = values.split("|")
    return int(first), int(second)


def parallel_create(
    port: int, token: str, key: str, requests: list[dict]
) -> list[tuple[int, dict, dict]]:
    barrier = Barrier(len(requests))

    def submit(body: dict) -> tuple[int, dict, dict]:
        barrier.wait(timeout=5)
        return create(port, token, key, body)

    with ThreadPoolExecutor(max_workers=len(requests)) as pool:
        return list(pool.map(submit, requests))


def verify_languages(port: int, token: str) -> None:
    status, languages, _ = request(port, "GET", "/v1/languages", token=token)
    assert status == 200
    assert [item["language"] for item in languages] == [
        "python", "cpp", "javascript", "c", "java", "rust"
    ]
    assert len({item["id"] for item in languages}) == 6
    assert len({item["profile"] for item in languages}) == 6
    assert [item["language"] for item in languages if item["compiled"]] == [
        "cpp", "c", "java", "rust"
    ]
    assert request(port, "GET", "/v1/languages")[0] == 401


def verify_invalid_requests(port: int, token: str, db: Database) -> None:
    baseline = counts(db)
    status, _, headers = request(
        port, "POST", "/v1/submissions", body=REQUEST, key="missing-auth"
    )
    assert status == 401 and headers["www-authenticate"] == "Bearer"
    assert create(port, secrets.token_hex(32), "wrong-auth")[0] == 401
    assert request(port, "POST", "/v1/submissions", token=token, body=REQUEST)[0] == 400
    assert create(port, token, "bad key")[0] == 400

    invalid_requests = [
        {**REQUEST, "language": "unsupported"},
        {**REQUEST, "source": ""},
        {**REQUEST, "source": "é" * 32769},
        {**REQUEST, "source": "a\0b"},
        {**REQUEST, "stdin": "a" * 65537},
        {**REQUEST, "stdin": "a\0b"},
        {**REQUEST, "limits": {"cpu_time_ms": 0}},
        {**REQUEST, "limits": {"memory_kib": 131073}},
        {**REQUEST, "limits": {"cpu_time_ms": 2000, "wall_time_ms": 1999}},
        {**REQUEST, "limits": {"processes": 2}},
        {**REQUEST, "organization_id": "client-controlled"},
    ]
    for body in invalid_requests:
        status, result, _ = create(port, token, "invalid", body)
        assert status == 400 and "error" in result
        assert "client-controlled" not in str(result)

    for data, expected in [(b"{", 400), (b"z" * (800 * 1024 + 1), 413)]:
        status, _, _ = request(
            port, "POST", "/v1/submissions", token=token, body=data, key="bad-body"
        )
        assert status == expected, status
    assert counts(db) == baseline


def verify_conflict_and_rollback(
    port: int, token: str, tenancy: str, db: Database
) -> tuple[dict, str]:
    status, original, headers = create(port, token, "stable")
    assert (
        status == 202 and original["state"] == "created" and original["verdict"] is None
    )
    location = headers["location"]
    assert location == "/v1/submissions/" + original["id"]
    assert headers["cache-control"] == "no-store"
    assert create(
        port, token, "stable", {**REQUEST, "stdin": "", "limits": DEFAULT_LIMITS}
    )[:2] == (202, original)
    assert (
        create(port, token, "stable", {**REQUEST, "source": REQUEST["source"] + "\n"})[
            0
        ]
        == 409
    )

    observed = db.query(
        "SELECT s.runtime_profile, s.limits->>'stdout_bytes', e.event_type, e.event_version, "
        "e.organization_id, e.submission_id, e.published_at IS NULL "
        "FROM submissions s JOIN outbox_events e "
        "ON (e.organization_id, e.submission_id) = (s.organization_id, s.id) "
        "WHERE s.idempotency_key = 'stable'"
    )
    assert observed.split("|") == [
        "python3.13-v1",
        "65536",
        "submission.created",
        "1",
        tenancy,
        original["id"],
        "t",
    ]
    assert db.query(
        "SELECT column_name FROM information_schema.columns "
        "WHERE table_name = 'outbox_events' ORDER BY ordinal_position"
    ).splitlines() == [
        "id",
        "organization_id",
        "submission_id",
        "event_type",
        "event_version",
        "created_at",
        "published_at",
    ]
    assert counts(db) == (1, 1)

    replies = parallel_create(port, token, "race", [{**REQUEST}] * 12)
    assert all(status == 202 for status, _, _ in replies)
    assert len({body["id"] for _, body, _ in replies}) == 1
    assert counts(db) == (2, 2)

    replies = parallel_create(
        port, token, "conflicting-race", [REQUEST, {**REQUEST, "stdin": "different"}]
    )
    assert sorted(status for status, _, _ in replies) == [202, 409]
    assert counts(db) == (3, 3)

    db.query(
        "ALTER TABLE outbox_events ADD CONSTRAINT phase2_force_failure CHECK (false) NOT VALID"
    )
    try:
        status, body, _ = create(port, token, "rollback")
        assert status == 500 and body["error"]["code"] == "storage_error"
        assert "phase2_force_failure" not in str(body)
        assert counts(db) == (3, 3)
        assert (
            db.query(
                "SELECT count(*) FROM submissions WHERE idempotency_key = 'rollback'"
            )
            == "0"
        )
    finally:
        db.query("ALTER TABLE outbox_events DROP CONSTRAINT phase2_force_failure")

    assert create(port, token, "rollback")[0] == 202
    assert create(port, token, "boundary", {**REQUEST, "stdin": "é" * 32768})[0] == 202
    assert counts(db) == (5, 5)
    return original, location


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin-dir", type=Path, default=Path("target/release"))
    binary = (parser.parse_args().bin_dir / "ferrum-api").resolve()
    db = Database()

    with db.running() as url:
        assert db.query("SHOW server_version_num").startswith("17")
        tenant, other_tenant = str(uuid4()), str(uuid4())
        token, other_token = secrets.token_hex(32), secrets.token_hex(32)
        env = api_env(url, tenant, token)
        assert_startup_error(
            binary,
            {**env, "FERRUM_API_LISTEN_ADDR": "0.0.0.0:8080"},
            "use development mode and a loopback HTTP listener",
        )
        assert_startup_error(
            binary,
            {
                **env,
                "FERRUM_API_LISTEN_ADDR": "127.0.0.1:8080",
                "FERRUM_MODE": "production",
            },
            "use development mode and a loopback HTTP listener",
        )
        assert_startup_error(
            binary,
            {key: value for key, value in env.items() if key != "FERRUM_DEV_TOKEN"},
            "FERRUM_DEV_TOKEN",
        )
        assert_startup_error(binary, env, "verify that migrations are applied")
        migration_env = {**environment(), "FERRUM_DATABASE_URL": url}
        for _ in range(2):
            subprocess.run(
                [binary, "migrate"], env=migration_env, check=True, timeout=15
            )
        assert db.query("SELECT array_agg(version ORDER BY version) FROM _sqlx_migrations WHERE success") == "{1,2,3}"

        with running_api(binary, env) as port:
            assert request(port, "GET", "/health/ready")[:2] == (
                200,
                {"status": "ready"},
            )
            verify_languages(port, token)
            verify_invalid_requests(port, token, db)
            original, location = verify_conflict_and_rollback(port, token, tenant, db)
            assert request(port, "GET", location, token=token)[:2] == (200, original)

        with running_api(binary, env) as port, running_api(
            binary, api_env(url, other_tenant, other_token)
        ) as other:
            assert request(port, "GET", location, token=token)[:2] == (200, original)
            assert create(port, token, "stable")[:2] == (202, original)
            foreign = request(
                other, "GET", location, token=other_token, extra={"X-Tenant-ID": tenant}
            )
            missing = request(
                other, "GET", f"/v1/submissions/{uuid4()}", token=other_token
            )
            assert foreign[:2] == missing[:2] and foreign[0] == 404
            status, independent, _ = create(other, other_token, "stable")
            assert status == 202 and independent["id"] != original["id"]
            assert counts(db) == (6, 6)
            db.compose("stop", "postgres")
            assert request(port, "GET", "/health/live")[:2] == (200, {"status": "live"})
            assert request(port, "GET", "/health/ready")[:2] == (
                503,
                {"status": "database_unavailable"},
            )

    print("Phase 2 PostgreSQL and HTTP checks passed")


if __name__ == "__main__":
    main()
