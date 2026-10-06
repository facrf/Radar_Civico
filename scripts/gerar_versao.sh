#!/usr/bin/env bash
set -euo pipefail

# Diretório raiz do projeto
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION_FILE="${ROOT_DIR}/version.json"

if [ -n "${APP_VERSION:-}" ]; then
    VERSION="${APP_VERSION}"
    COMMIT="${APP_COMMIT:-}"
    COUNT="${APP_COUNT:-0}"
elif command -v git >/dev/null 2>&1 && [ -d "${ROOT_DIR}/.git" ]; then
    COUNT=$(git -C "${ROOT_DIR}" rev-list --count HEAD 2>/dev/null || echo "1")
    COMMIT=$(git -C "${ROOT_DIR}" rev-parse --short HEAD 2>/dev/null || echo "unknown")
    if [ "$COUNT" -lt 1000 ]; then
        PADDED=$(printf "%03d" "$COUNT")
    else
        PADDED="$COUNT"
    fi
    VERSION="v0.${PADDED}"
elif [ -f "${VERSION_FILE}" ]; then
    VERSION=$(grep -o '"version"[^,]*' "${VERSION_FILE}" | cut -d'"' -f4)
    COMMIT=$(grep -o '"commit"[^,]*' "${VERSION_FILE}" | cut -d'"' -f4)
    COUNT=$(grep -o '"count"[^,]*' "${VERSION_FILE}" | cut -d':' -f2 | tr -d ' }')
else
    VERSION="v0.001"
    COMMIT="unknown"
    COUNT="1"
fi

cat <<EOF > "${VERSION_FILE}"
{
  "version": "${VERSION}",
  "commit": "${COMMIT}",
  "count": ${COUNT}
}
EOF

echo "${VERSION} (${COMMIT})"
