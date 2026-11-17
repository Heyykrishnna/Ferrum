# CI/CD, Security Gates, and Release Verification

Ferrum uses GitHub Actions workflows with least-privilege permissions, explicit job timeouts, and branch protection rules on `main`.

## Workflows Overview

| Workflow                               | File                                         | Purpose                                                                                                                                                                                                                                             | Triggers                                                               |
| -------------------------------------- | -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| **CI**                                 | `.github/workflows/ci.yml`                   | Rust fmt/check/clippy/test/release build, web format/lint/typecheck/test/build, Prometheus `promtool` validation, OpenAPI/dashboard/migration contract checks, `actionlint`, `shellcheck`, and aggregated `CI Gate`                                 | `push` (`main`), `pull_request` (`main`), `merge_group`, manual        |
| **CodeQL**                             | `.github/workflows/codeql.yml`               | Static security analysis for `rust` (`build-mode: none`) and `javascript-typescript` (`build-mode: none`)                                                                                                                                           | `push` (`main`), `pull_request` (`main`), weekly Monday, manual        |
| **Security Supply Chain**              | `.github/workflows/security.yml`             | `cargo-deny` (`deny.toml` advisories, licenses, bans, sources), RustSec `cargo-audit`, and `npm audit` for `web/`                                                                                                                                   | `push` (`main`), `pull_request` (`main`), `merge_group`, daily, manual |
| **PostgreSQL Integration & API Smoke** | `.github/workflows/postgres-integration.yml` | Ephemeral `postgres:17-bookworm` integration tests (`crates/infrastructure/tests/postgres_integration.rs`) and live `ferrum-api` + `ferrum-worker` CLI smoke tests (`scripts/ci/api-postgres-smoke.py`)                                             | `push` (`main`), `pull_request` (`main`), `merge_group`, manual        |
| **Linux Sandbox Security**             | `.github/workflows/sandbox-security.yml`     | Locked-down Debian Linux container checks (`sandbox-checks` & `language-toolchains` via `deployment/phase3.Dockerfile`) plus dedicated Debian 13 Isolate 2.7 host qualification on `[self-hosted, linux, ferrum-isolate]`                           | `push` (`main`), `pull_request` (`main`), `merge_group`, daily, manual |
| **Performance Regression**             | `.github/workflows/performance.yml`          | Release `ferrum-api` intake benchmark using Grafana `k6` (`scripts/perf/intake-regression.js`) validated against `scripts/perf/baseline.json`                                                                                                       | `push` (`main`), targeted `pull_request`, daily, manual                |
| **Release**                            | `.github/workflows/release.yml`              | Preflight quality/security gate, Linux `x86_64` and `aarch64` release packaging, web bundle packaging, CycloneDX + SPDX SBOMs (`anchore/sbom-action`), SLSA build provenance & SBOM attestations (`actions/attest`), and GitHub Release publication | `v*.*.*` tags, manual dry-run                                          |

- **RustSec (`cargo-audit`) and `cargo-deny`:** `deny.toml` explicitly bans `sqlx-sqlite` and `libsqlite3-sys` from the active build graph (Ferrum uses PostgreSQL only). Because Cargo still records optional `sqlx-sqlite` dependencies in `Cargo.lock`, `.cargo/audit.toml` ignores `RUSTSEC-2026-0459` (`libsqlite3-sys 0.37.0`) while `cargo-deny` enforces that `libsqlite3-sys` is never compiled or linked.

## Protected `main` Branch Ruleset

The declarative GitHub Ruleset lives in `.github/rulesets/main-protection.json` alongside `.github/CODEOWNERS`.

Required status checks enforced on `main`:

- `CI Gate`
- `PostgreSQL Integration & API Smoke`
- `Linux Sandbox & Toolchains`
- `Cargo Deny`
- `RustSec Audit`
- `Analyze (rust)`
- `Analyze (javascript-typescript)`

Apply or update the ruleset once changes are pushed to GitHub:

```sh
bash scripts/ci/apply-branch-protection.sh
```

## Dedicated Linux Isolate 2.7 Runner Setup

The `dedicated-isolate-host` job targets a self-hosted Debian 13 (`amd64` or `arm64`) worker with labels `self-hosted`, `linux`, and `ferrum-isolate`, prepared per [docs/phase0-linux-worker.md](phase0-linux-worker.md) and [docs/phase3-isolate.md](phase3-isolate.md).

- Set repository variable `FERRUM_ISOLATE_RUNNER_ENABLED=true` to enable nightly scheduled runs.
- Label a pull request `run-isolate-security` or trigger `.github/workflows/sandbox-security.yml` via `workflow_dispatch` to run qualification on demand.

## Running Checks Locally

Run the core CI gates locally:

```sh
bash scripts/ci/run-local-ci.sh
```

Run PostgreSQL integration, API smoke, and k6 performance regression locally against a disposable PostgreSQL 17 instance:

```sh
export FERRUM_DATABASE_URL="postgres://ferrum:ferrum_ci_password@127.0.0.1:54329/ferrum_ci"
cargo test --locked -p ferrum-infrastructure --test postgres_integration -- --ignored --test-threads=1 --nocapture
python3 -I scripts/ci/api-postgres-smoke.py
python3 -I scripts/perf/run-perf-regression.py --rate 10 --duration 15s
```

Run containerized Linux sandbox and toolchain checks locally:

```sh
docker build --file deployment/phase3.Dockerfile --target sandbox-checks --tag ferrum-sandbox-checks .
docker run --rm --network none --cap-drop ALL --security-opt no-new-privileges ferrum-sandbox-checks

docker build --file deployment/phase3.Dockerfile --target language-toolchains --tag ferrum-language-toolchains .
docker run --rm --network none --cap-drop ALL --security-opt no-new-privileges ferrum-language-toolchains
```

## Verifying Release Provenance and SBOMs

Consumers can cryptographically verify downloaded release archives with the GitHub CLI:

```sh
gh attestation verify ferrum-<version>-x86_64-unknown-linux-gnu.tar.gz --repo Heyykrishnna/Ferrum
sha256sum -c SHA256SUMS.txt
```
