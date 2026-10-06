#!/usr/bin/env bash
# ==============================================================================
# Radar Cívico - Wrapper de Ingestão de Auxílio Emergencial no SQLite
# ==============================================================================
# Uso:
#   ./scripts/ingerir_auxilio_completo.sh [OPÇÕES]
#
# Exemplos:
#   # Ingerir o arquivo completo (30 GB) de uma vez:
#   ./scripts/ingerir_auxilio_completo.sh
#
#   # Ingerir apenas de uma cidade específica:
#   ./scripts/ingerir_auxilio_completo.sh --municipio "Campinas" --uf "SP"
#
#   # Ingerir apenas correspondências de políticos cadastrados:
#   ./scripts/ingerir_auxilio_completo.sh --apenas-candidatos
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PYTHON_SCRIPT="${SCRIPT_DIR}/ingerir_auxilio_completo.py"

python3 "$PYTHON_SCRIPT" "$@"
