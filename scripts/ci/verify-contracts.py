#!/usr/bin/env python3
import json
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def fail(message: str) -> None:
    print(f"FAIL: {message}", file=sys.stderr)
    sys.exit(1)


def load_yaml_module():
    try:
        import yaml

        return yaml
    except ImportError:
        return None


def verify_migrations() -> None:
    migrations_dir = ROOT / "migrations"
    sql_files = sorted(migrations_dir.glob("*.sql"))
    if not sql_files:
        fail("no SQL migrations found under migrations/")

    expected = 1
    pattern = re.compile(r"^(\d{4})_[a-z0-9_]+\.sql$")
    for path in sql_files:
        match = pattern.match(path.name)
        if not match:
            fail(f"migration filename does not follow NNNN_name.sql: {path.name}")
        number = int(match.group(1))
        if number != expected:
            fail(f"migration numbering gap or disorder: expected {expected:04d}, got {path.name}")
        if not path.read_text(encoding="utf-8").strip():
            fail(f"migration file is empty: {path.name}")
        expected += 1
    print(f"PASS migrations ({len(sql_files)} monotonic SQL files)")


def verify_openapi(yaml_mod) -> None:
    json_path = ROOT / "docs" / "openapi.json"
    yaml_path = ROOT / "docs" / "openapi.yaml"
    if not json_path.is_file() or not yaml_path.is_file():
        fail("docs/openapi.json and docs/openapi.yaml must both exist")

    openapi_json = json.loads(json_path.read_text(encoding="utf-8"))
    version = str(openapi_json.get("openapi", ""))
    if not version.startswith("3."):
        fail(f"unexpected OpenAPI version in docs/openapi.json: {version!r}")

    required_paths = {
        "/v1/api-keys",
        "/v1/api-keys/{id}",
        "/v1/execution/status",
        "/v1/languages",
        "/v1/submissions",
        "/v1/submissions/batch",
        "/v1/submissions/{id}",
        "/v1/submissions/{id}/cancel",
        "/v1/webhooks",
        "/v1/webhooks/{id}",
    }
    declared_paths = set(openapi_json.get("paths", {}).keys())
    missing = sorted(required_paths - declared_paths)
    if missing:
        fail(f"docs/openapi.json missing required paths: {', '.join(missing)}")

    if yaml_mod is not None:
        openapi_yaml = yaml_mod.safe_load(yaml_path.read_text(encoding="utf-8"))
        if openapi_yaml != openapi_json:
            fail("docs/openapi.yaml and docs/openapi.json are out of sync")
        print(f"PASS OpenAPI ({len(declared_paths)} paths, JSON/YAML parity verified)")
    else:
        raw_yaml = yaml_path.read_text(encoding="utf-8")
        if "openapi:" not in raw_yaml or "/v1/submissions:" not in raw_yaml:
            fail("docs/openapi.yaml appears malformed")
        print(f"PASS OpenAPI ({len(declared_paths)} paths, JSON validated; PyYAML unavailable)")


def verify_grafana_dashboards() -> None:
    dashboards_dir = ROOT / "deployment" / "grafana" / "dashboards"
    files = sorted(dashboards_dir.glob("*.json"))
    if not files:
        fail("no Grafana dashboard JSON files found")

    seen_uids: set[str] = set()
    for path in files:
        data = json.loads(path.read_text(encoding="utf-8"))
        title = data.get("title")
        uid = data.get("uid")
        panels = data.get("panels")
        if not isinstance(title, str) or not title.strip():
            fail(f"dashboard missing title: {path.name}")
        if not isinstance(uid, str) or not uid.strip():
            fail(f"dashboard missing uid: {path.name}")
        if uid in seen_uids:
            fail(f"duplicate Grafana dashboard uid {uid!r} in {path.name}")
        seen_uids.add(uid)
        if not isinstance(panels, list) or not panels:
            fail(f"dashboard has no panels: {path.name}")
    print(f"PASS Grafana dashboards ({len(files)} dashboards)")


def verify_deny_toml() -> None:
    deny_path = ROOT / "deny.toml"
    if not deny_path.is_file():
        fail("deny.toml is missing")
    parsed = tomllib.loads(deny_path.read_text(encoding="utf-8"))
    for section in ("advisories", "licenses", "bans", "sources"):
        if section not in parsed:
            fail(f"deny.toml missing [{section}] section")
    print("PASS deny.toml schema")


def verify_github_configs(yaml_mod) -> None:
    ruleset_path = ROOT / ".github" / "rulesets" / "main-protection.json"
    if not ruleset_path.is_file():
        fail(".github/rulesets/main-protection.json is missing")
    ruleset = json.loads(ruleset_path.read_text(encoding="utf-8"))
    if ruleset.get("target") != "branch":
        fail("main-protection.json must target branch")

    required_contexts: set[str] = set()
    for rule in ruleset.get("rules", []):
        if rule.get("type") == "required_status_checks":
            for check in rule.get("parameters", {}).get("required_status_checks", []):
                required_contexts.add(check["context"])

    workflows_dir = ROOT / ".github" / "workflows"
    workflow_files = sorted(workflows_dir.glob("*.yml"))
    if not workflow_files:
        fail("no GitHub workflow files found under .github/workflows/")

    discovered_job_names: set[str] = set()
    for wf_path in workflow_files:
        text = wf_path.read_text(encoding="utf-8")
        if yaml_mod is not None:
            doc = yaml_mod.safe_load(text)
            if not isinstance(doc, dict):
                fail(f"workflow is not a YAML mapping: {wf_path.name}")
            if "permissions" not in doc:
                fail(f"workflow missing top-level permissions: {wf_path.name}")
            jobs = doc.get("jobs")
            if not isinstance(jobs, dict) or not jobs:
                fail(f"workflow has no jobs: {wf_path.name}")
            for job_id, job in jobs.items():
                name = job.get("name", job_id)
                if "timeout-minutes" not in job:
                    fail(f"job {job_id!r} in {wf_path.name} missing timeout-minutes")
                discovered_job_names.add(name)
                if "${{ matrix.language }}" in str(name):
                    for lang in job.get("strategy", {}).get("matrix", {}).get("language", []):
                        discovered_job_names.add(str(name).replace("${{ matrix.language }}", lang))
        else:
            if "permissions:" not in text or "jobs:" not in text:
                fail(f"workflow missing permissions or jobs: {wf_path.name}")

    if yaml_mod is not None:
        missing_contexts = sorted(required_contexts - discovered_job_names)
        if missing_contexts:
            fail(
                "main-protection.json required_status_checks do not match workflow job names: "
                + ", ".join(missing_contexts)
            )

        dependabot_path = ROOT / ".github" / "dependabot.yml"
        if not dependabot_path.is_file():
            fail(".github/dependabot.yml is missing")
        dep = yaml_mod.safe_load(dependabot_path.read_text(encoding="utf-8"))
        if dep.get("version") != 2 or not dep.get("updates"):
            fail(".github/dependabot.yml must declare version 2 and updates")

    print(f"PASS GitHub workflows and ruleset ({len(workflow_files)} workflows)")


def verify_prometheus_yaml(yaml_mod) -> None:
    prom_dir = ROOT / "deployment" / "prometheus"
    for name in ("prometheus.yml", "alerts.yml", "alerts.test.yml"):
        path = prom_dir / name
        if not path.is_file():
            fail(f"missing Prometheus file: {path}")
        if yaml_mod is not None:
            parsed = yaml_mod.safe_load(path.read_text(encoding="utf-8"))
            if not isinstance(parsed, dict):
                fail(f"invalid YAML in {path}")
    print("PASS Prometheus configuration files")


def main() -> None:
    yaml_mod = load_yaml_module()
    verify_migrations()
    verify_openapi(yaml_mod)
    verify_grafana_dashboards()
    verify_deny_toml()
    verify_prometheus_yaml(yaml_mod)
    verify_github_configs(yaml_mod)


if __name__ == "__main__":
    main()
