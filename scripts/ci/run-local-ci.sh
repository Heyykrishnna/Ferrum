#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

echo "==> Rust quality gates"
cargo fmt --all --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked

echo "==> Web quality gates"
(
  cd web
  npm run format:check
  npm run lint
  npm run typecheck
  npm test
  npm run build
)

echo "==> Contract and workflow verification"
python3 -I scripts/ci/verify-contracts.py

if command -v cargo-deny >/dev/null 2>&1; then
  echo "==> cargo-deny"
  cargo deny --locked check
else
  echo "SKIP cargo-deny (not installed)"
fi

if command -v cargo-audit >/dev/null 2>&1; then
  echo "==> cargo-audit"
  cargo audit --deny warnings
else
  echo "SKIP cargo-audit (not installed)"
fi

if command -v actionlint >/dev/null 2>&1; then
  echo "==> actionlint"
  actionlint
else
  echo "SKIP actionlint (not installed)"
fi

if [[ -n "${FERRUM_DATABASE_URL:-}" ]]; then
  echo "==> PostgreSQL integration and API smoke"
  export DATABASE_URL="${DATABASE_URL:-${FERRUM_DATABASE_URL}}"
  cargo build --bins --locked
  cargo test --locked -p ferrum-infrastructure --test postgres_integration -- --ignored --test-threads=1
  cargo test --locked -p ferrum-worker -- --ignored --test-threads=1
  python3 -I scripts/ci/api-postgres-smoke.py
else
  echo "SKIP PostgreSQL integration and API smoke (set FERRUM_DATABASE_URL to enable)"
fi

echo "PASS local CI checks completed"
