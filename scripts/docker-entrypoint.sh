#!/bin/sh
set -e

# Assegura que o diretório de dados e subdiretórios existam com propriedade do usuário radar
mkdir -p /app/data /app/data/backups /app/data/radar_civico.job-completions 2>/dev/null || true
chown -R radar:radar /app/data 2>/dev/null || chmod -R 777 /app/data 2>/dev/null || true

# Executa o processo principal com privilégios reduzidos do usuário não-root radar
if [ "$1" = "/app/server" ] || [ "$1" = "server" ]; then
    exec gosu radar "$@"
fi

exec "$@"
