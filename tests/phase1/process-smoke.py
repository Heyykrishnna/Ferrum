import argparse
from contextlib import closing
import http.client
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

SHUTDOWN_MS = 2000


def environment(overrides: dict[str, str]) -> dict[str, str]:
    return {
        **{
            key: value
            for key, value in os.environ.items()
            if not key.startswith("FERRUM_")
        },
        **overrides,
    }


def wait_for_start(process: subprocess.Popen, log: Path, event: str) -> dict:
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        for line in log.read_text().splitlines(keepends=True):
            if line.startswith("{") and line.endswith("\n"):
                fields = json.loads(line)["fields"]
                if fields.get("event") == event:
                    return fields
        if process.poll() is not None:
            break
        time.sleep(0.02)
    raise AssertionError(f"process did not report {event}: {log.read_text()}")


def check_health(listen_addr: str) -> None:
    host, port = listen_addr.split(":")
    assert host == "127.0.0.1" and int(port) not in (0, 9999), listen_addr
    with closing(http.client.HTTPConnection(host, int(port), timeout=2)) as connection:
        for path, expected in [("/health/live", "live"), ("/health/ready", "ready")]:
            connection.request("GET", path)
            response = connection.getresponse()
            assert response.status == 200, path
            assert response.getheader("Cache-Control") == "no-store", path
            assert json.loads(response.read()) == {"status": expected}, path
        connection.request("GET", "/unknown")
        response = connection.getresponse()
        assert response.status == 404
        response.read()


def check_lifecycle(
    binary: Path,
    log: Path,
    overrides: dict[str, str],
    start_event: str,
    stop_event: str,
    mode: str,
    termination_signal: signal.Signals,
) -> None:
    with log.open("wb") as output, subprocess.Popen(
        [binary],
        env=environment(overrides),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=output,
    ) as process:
        try:
            startup = wait_for_start(process, log, start_event)
            assert startup["mode"] == mode, startup
            if start_event == "api_listening":
                check_health(startup["listen_addr"])
            process.send_signal(termination_signal)
            assert process.wait(timeout=SHUTDOWN_MS / 1000) == 0, log.read_text()
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=2)

    events = [json.loads(line)["fields"] for line in log.read_text().splitlines()]
    shutdown = next(
        fields for fields in events if fields.get("event") == "shutdown_started"
    )
    assert shutdown["signal"] == termination_signal.name, events
    assert events[-1]["event"] == stop_event, events


def check_rejected(
    binary: Path, overrides: dict[str, str], diagnostic: str, value: str
) -> None:
    result = subprocess.run(
        [binary], env=environment(overrides), capture_output=True, text=True, timeout=3
    )
    assert result.returncode == 1, result
    assert diagnostic in result.stderr, result
    assert (
        value not in result.stderr + result.stdout
    ), "startup error leaked a setting value"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin-dir", type=Path, default=Path("target/release"))
    parser.add_argument("--api-only", action="store_true")
    args = parser.parse_args()
    binaries = args.bin_dir.resolve()
    api, worker = (binaries / name for name in ("ferrum-api", "ferrum-worker"))

    with tempfile.TemporaryDirectory(prefix="ferrum-phase1-") as temporary:
        root = Path(temporary)
        log = root / "process.log"
        api_settings = root / "api.toml"
        api_settings.write_text(
            'listen_addr = "127.0.0.1:9999"\n[process]\nmode = "production"\nlog_level = "error"\n'
        )
        worker_settings = root / "worker.toml"
        worker_settings.write_text(
            f'[process]\nmode = "production"\nshutdown_timeout_ms = {SHUTDOWN_MS}\n'
        )
        for termination_signal in (signal.SIGTERM, signal.SIGINT):
            check_lifecycle(
                api,
                log,
                {
                    "FERRUM_API_CONFIG": str(api_settings),
                    "FERRUM_API_LISTEN_ADDR": "127.0.0.1:0",
                    "FERRUM_API_SHUTDOWN_TIMEOUT_MS": str(SHUTDOWN_MS),
                    "FERRUM_MODE": "development",
                    "FERRUM_LOG_LEVEL": "info",
                },
                "api_listening",
                "api_stopped",
                "development",
                termination_signal,
            )
            if not args.api_only:
                check_lifecycle(
                    worker,
                    log,
                    {"FERRUM_WORKER_CONFIG": str(worker_settings)},
                    "worker_started",
                    "worker_stopped",
                    "production",
                    termination_signal,
                )
        if not args.api_only:
            check_lifecycle(
                worker,
                log,
                {},
                "worker_started",
                "worker_stopped",
                "development",
                signal.SIGTERM,
            )

        sensitive = "redaction-fixture-value"
        check_rejected(
            api, {"FERRUM_API_LISTEN_ADDR": sensitive}, "listen_addr", sensitive
        )
        check_rejected(
            api,
            {"FERRUM_MODE": "production", "FERRUM_API_LISTEN_ADDR": "127.0.0.1:0"},
            "nonzero port in production",
            "127.0.0.1:0",
        )
        if not args.api_only:
            check_rejected(
                worker, {"FERRUM_MODE": sensitive}, "process.mode", sensitive
            )
            check_rejected(
                worker,
                {"FERRUM_WORKER_SHUTDOWN_TIMEOUT_MS": sensitive},
                "FERRUM_WORKER_SHUTDOWN_TIMEOUT_MS",
                sensitive,
            )
            worker_settings.write_text(f'[process]\nunknown = "{sensitive}"\n')
            check_rejected(
                worker,
                {"FERRUM_WORKER_CONFIG": str(worker_settings)},
                "unknown setting",
                sensitive,
            )
            check_rejected(
                worker,
                {"FERRUM_WORKER_CONFIG": str(root / sensitive)},
                "could not read the settings file",
                sensitive,
            )
    print("Phase 1 process checks passed")


if __name__ == "__main__":
    main()
