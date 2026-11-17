#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RULESET_FILE="${ROOT_DIR}/.github/rulesets/main-protection.json"
REPO="${1:-$(gh repo view --json nameWithOwner -q .nameWithOwner)}"

existing_id="$(gh api "repos/${REPO}/rulesets" --jq '.[] | select(.name == "Protect main") | .id' | head -n 1 || true)"

if [[ -n "${existing_id}" ]]; then
  gh api \
    --method PUT \
    -H "Accept: application/vnd.github+json" \
    "repos/${REPO}/rulesets/${existing_id}" \
    --input "${RULESET_FILE}" >/dev/null
  echo "PASS updated ruleset ${existing_id} on ${REPO}"
else
  gh api \
    --method POST \
    -H "Accept: application/vnd.github+json" \
    "repos/${REPO}/rulesets" \
    --input "${RULESET_FILE}" >/dev/null
  echo "PASS created ruleset on ${REPO}"
fi

