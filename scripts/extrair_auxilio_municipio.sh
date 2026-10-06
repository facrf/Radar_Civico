#!/usr/bin/env bash
# ==============================================================================
# Radar Cívico - Extrator Cirúrgico de Auxílio Emergencial por Município
# ==============================================================================
# Uso:
#   ./scripts/extrair_auxilio_municipio.sh "NOME_DO_MUNICIPIO" ["UF"] ["CAMINHO_DO_CSV"]
#
# Exemplo:
#   ./scripts/extrair_auxilio_municipio.sh "Campinas" "SP"
#   ./scripts/extrair_auxilio_municipio.sh "Uberlândia" "MG"
# ==============================================================================

set -euo pipefail

MUNICIPIO="${1:-}"
UF="${2:-}"
CSV_ORIGEM="${3:-/home/facrf/Downloads/basedados/auxilio_emergencial.csv}"

if [ -z "$MUNICIPIO" ]; then
    echo "❌ Erro: Informe o nome do município."
    echo "Uso: $0 \"NOME_DO_MUNICIPIO\" [\"UF\"] [\"CAMINHO_DO_CSV\"]"
    exit 1
fi

if [ ! -f "$CSV_ORIGEM" ]; then
    echo "❌ Arquivo não encontrado: $CSV_ORIGEM"
    exit 1
fi

CIDADE_SLUG=$(echo -n "$MUNICIPIO" | iconv -f UTF-8 -t ASCII//TRANSLIT | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]/_/g')
DIR_SAIDA=$(dirname "$CSV_ORIGEM")
CSV_DESTINO="${DIR_SAIDA}/auxilio_${CIDADE_SLUG}.csv"

echo "======================================================================"
echo "🎯 RADAR CÍVICO - EXTRAÇÃO CIRÚRGICA DE AUXÍLIO EMERGENCIAL"
echo "======================================================================"
echo "📂 Arquivo de Origem: $CSV_ORIGEM ($(du -h "$CSV_ORIGEM" | cut -f1))"
echo "🏙️ Município Alvo:    $MUNICIPIO"
[ -n "$UF" ] && echo "📍 UF:               $UF"
echo "💾 Arquivo de Saída:  $CSV_DESTINO"
echo "----------------------------------------------------------------------"
echo "⏳ Extraindo cabeçalho e filtrando registros em alta velocidade..."

# 1. Extrai o cabeçalho original
head -n 1 "$CSV_ORIGEM" > "$CSV_DESTINO"

# 2. Executa a filtragem em streaming usando awk otimizado (case-insensitive para município e UF)
MUNICIPIO_UPPER=$(echo "$MUNICIPIO" | tr '[:lower:]' '[:upper:]')
UF_UPPER=$(echo "$UF" | tr '[:lower:]' '[:upper:]')

if [ -n "$UF_UPPER" ]; then
    # Coluna 2 = UF, Coluna 4 = Município
    LC_ALL=C awk -F',' -v mun="$MUNICIPIO_UPPER" -v uf="$UF_UPPER" '
        NR > 1 {
            curr_uf = toupper($2)
            curr_mun = toupper($4)
            if (curr_uf == uf && curr_mun == mun) {
                print $0
            }
        }
    ' "$CSV_ORIGEM" >> "$CSV_DESTINO"
else
    LC_ALL=C awk -F',' -v mun="$MUNICIPIO_UPPER" '
        NR > 1 {
            curr_mun = toupper($4)
            if (curr_mun == mun) {
                print $0
            }
        }
    ' "$CSV_ORIGEM" >> "$CSV_DESTINO"
fi

TOTAL_REGISTROS=$(($(wc -l < "$CSV_DESTINO") - 1))
TAMANHO_SAIDA=$(du -h "$CSV_DESTINO" | cut -f1)

echo "----------------------------------------------------------------------"
echo "✅ Concluído com Sucesso!"
echo "📊 Total de registros extraídos para $MUNICIPIO: $TOTAL_REGISTROS"
echo "📦 Tamanho final do arquivo: $TAMANHO_SAIDA (redução drástica para carga)"
echo "🚀 Pronto para importar no Radar Cívico em Configurações > Carga Manual!"
echo "======================================================================"
