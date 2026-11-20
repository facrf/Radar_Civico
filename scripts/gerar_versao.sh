#!/usr/bin/env bash
set -euo pipefail

# Diretório raiz do projeto
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION_FILE="${ROOT_DIR}/version.json"

# Parsers mínimos para o JSON gerado por este próprio script (evita jq na imagem).
json_string() {
    sed -n "s/^[[:space:]]*\"$2\"[[:space:]]*:[[:space:]]*\"\([^\"]*\)\".*/\1/p" "$1" | head -n 1
}

json_number() {
    sed -n "s/^[[:space:]]*\"$2\"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p" "$1" | head -n 1
}

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
    VERSION="$(json_string "${VERSION_FILE}" version)"
    COMMIT="$(json_string "${VERSION_FILE}" commit)"
    COUNT="$(json_number "${VERSION_FILE}" count)"
else
    VERSION="v0.001"
    COMMIT="unknown"
    COUNT="1"
fi

# Um JSON vazio ou malformado quebraria build.rs e vite.config.ts.
if [ -z "${VERSION}" ]; then
    echo "ERRO: versão vazia ou ilegível em ${VERSION_FILE}" >&2
    exit 1
fi
case "${VERSION}" in
    *[!A-Za-z0-9._/-]*)
        echo "ERRO: versão contém caracteres inválidos: ${VERSION}" >&2
        exit 1
        ;;
esac
if [ -z "${COMMIT}" ]; then
    echo "ERRO: commit vazio; defina APP_COMMIT ou remova ${VERSION_FILE} para usar o Git" >&2
    exit 1
fi
case "${COMMIT}" in
    *[!A-Za-z0-9._-]*)
        echo "ERRO: commit contém caracteres inválidos: ${COMMIT}" >&2
        exit 1
        ;;
esac
case "${COUNT}" in
    ''|*[!0-9]*)
        echo "ERRO: contador de commits inválido: ${COUNT}" >&2
        exit 1
        ;;
esac
COUNT=$((10#${COUNT}))
if [ "${COUNT}" -lt 1 ]; then
    echo "ERRO: contador de commits deve ser positivo: ${COUNT}" >&2
    exit 1
fi

cat <<EOF > "${VERSION_FILE}"
{
  "version": "${VERSION}",
  "commit": "${COMMIT}",
  "count": ${COUNT}
}
EOF

echo "${VERSION} (${COMMIT})"
