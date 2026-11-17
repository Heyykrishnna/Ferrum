#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 3 ]]; then
  echo "usage: package-release.sh <version> <target> <output_dir> [bin_dir]" >&2
  exit 1
fi

VERSION="$1"
TARGET="$2"
OUTPUT_DIR="$3"
BIN_DIR="${4:-target/release}"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ARCHIVE_NAME="ferrum-${VERSION}-${TARGET}"
STAGE_DIR="$(mktemp -d)"
trap 'rm -rf "${STAGE_DIR}"' EXIT

mkdir -p "${STAGE_DIR}/${ARCHIVE_NAME}/bin" "${STAGE_DIR}/${ARCHIVE_NAME}/docs" "${STAGE_DIR}/${ARCHIVE_NAME}/deployment" "${OUTPUT_DIR}"

for bin in ferrum-api ferrum-worker; do
  bin_path="${ROOT_DIR}/${BIN_DIR}/${bin}"
  if [[ ! -x "${bin_path}" ]]; then
    echo "FAIL: missing executable binary at ${bin_path}" >&2
    exit 1
  fi
  cp "${bin_path}" "${STAGE_DIR}/${ARCHIVE_NAME}/bin/${bin}"
done

cp -R "${ROOT_DIR}/migrations" "${STAGE_DIR}/${ARCHIVE_NAME}/migrations"
cp -R "${ROOT_DIR}/deployment/prometheus" "${STAGE_DIR}/${ARCHIVE_NAME}/deployment/prometheus"
cp "${ROOT_DIR}/docs/openapi.yaml" "${STAGE_DIR}/${ARCHIVE_NAME}/docs/openapi.yaml"
cp "${ROOT_DIR}/docs/openapi.json" "${STAGE_DIR}/${ARCHIVE_NAME}/docs/openapi.json"
cp "${ROOT_DIR}/README.md" "${STAGE_DIR}/${ARCHIVE_NAME}/README.md"
cp "${ROOT_DIR}/SECURITY.md" "${STAGE_DIR}/${ARCHIVE_NAME}/SECURITY.md"

ARCHIVE_PATH="${OUTPUT_DIR}/${ARCHIVE_NAME}.tar.gz"
tar -C "${STAGE_DIR}" -czf "${ARCHIVE_PATH}" "${ARCHIVE_NAME}"

if command -v sha256sum >/dev/null 2>&1; then
  (cd "${OUTPUT_DIR}" && sha256sum "${ARCHIVE_NAME}.tar.gz" > "${ARCHIVE_NAME}.tar.gz.sha256")
else
  (cd "${OUTPUT_DIR}" && shasum -a 256 "${ARCHIVE_NAME}.tar.gz" > "${ARCHIVE_NAME}.tar.gz.sha256")
fi

echo "PASS packaged ${ARCHIVE_PATH}"

