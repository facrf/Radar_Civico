#!/usr/bin/env python3
"""
Radar Cívico - Ingestor de Alta Performance para Auxílio Emergencial no SQLite
==============================================================================
Lê dumps massivos do Auxílio Emergencial (30 GB a 60 GB) em streaming e insere
diretamente no banco de dados local com transações em lote e PRAGMAs de alta velocidade.

Uso:
    python3 scripts/ingerir_auxilio_completo.py [OPÇÕES]

Exemplos:
    # 1. Jogar tudo no banco (carga completa):
    python3 scripts/ingerir_auxilio_completo.py --arquivo /home/facrf/Downloads/basedados/auxilio_emergencial.csv

    # 2. Jogar apenas de uma cidade específica:
    python3 scripts/ingerir_auxilio_completo.py --municipio "Campinas" --uf "SP"

    # 3. Jogar apenas registros de pessoas com nomes/CPFs correspondentes a políticos cadastrados:
    python3 scripts/ingerir_auxilio_completo.py --apenas-candidatos
"""

import os
import sys
import time
import sqlite3
import argparse
from typing import Optional, Set

DEFAULT_CSV = "/home/facrf/Downloads/basedados/auxilio_emergencial.csv"
DEFAULT_DB = "./data/radar_civico.db"
BATCH_SIZE = 50_000


def configurar_pragmas(conn: sqlite3.Connection, rapido: bool = True):
    cursor = conn.cursor()
    cursor.execute("PRAGMA journal_mode = WAL;")
    if rapido:
        cursor.execute("PRAGMA synchronous = OFF;")
    else:
        cursor.execute("PRAGMA synchronous = NORMAL;")
    cursor.execute("PRAGMA cache_size = -128000;")  # ~128 MB cache
    cursor.execute("PRAGMA temp_store = MEMORY;")
    cursor.execute("PRAGMA foreign_keys = OFF;")
    cursor.close()


def dropar_indices(conn: sqlite3.Connection):
    cursor = conn.cursor()
    print("⚡ Desativando índices temporariamente para acelerar a carga...")
    cursor.execute("DROP INDEX IF EXISTS idx_beneficios_mes;")
    cursor.execute("DROP INDEX IF EXISTS idx_beneficios_uf_mun;")
    cursor.execute("DROP INDEX IF EXISTS idx_beneficios_nome;")
    cursor.execute("DROP INDEX IF EXISTS idx_beneficios_cpf;")
    conn.commit()
    cursor.close()


def recriar_indices(conn: sqlite3.Connection):
    cursor = conn.cursor()
    print("\n🔨 Reconstruindo índices em B-Tree para buscas instantâneas...")
    inicio = time.time()
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_beneficios_cpf ON beneficios_emergenciais(cpf_mascarado);")
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_beneficios_nome ON beneficios_emergenciais(nome_beneficiario);")
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_beneficios_uf_mun ON beneficios_emergenciais(uf, municipio);")
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_beneficios_mes ON beneficios_emergenciais(mes_disponibilizacao);")
    conn.commit()
    cursor.close()
    print(f"✅ Índices reconstruídos em {time.time() - inicio:.2f}s!")


def carregar_politicos_referencia(conn: sqlite3.Connection) -> tuple[Set[str], Set[str]]:
    cursor = conn.cursor()
    cursor.execute("SELECT cpf_mascarado, UPPER(nome_completo) FROM politicos;")
    cpfs = set()
    nomes = set()
    for cpf, nome in cursor.fetchall():
        if cpf and len(cpf) >= 5 and cpf != "-4":
            cpf_clean = "".join(c for c in cpf if c.isdigit())
            if cpf_clean:
                cpfs.add(cpf_clean)
        if nome:
            nomes.add(nome.strip())
    cursor.close()
    print(f"📋 Políticos carregados para filtro: {len(nomes)} nomes e {len(cpfs)} CPFs mascarados válidos.")
    return cpfs, nomes


def formatar_bytes(num_bytes: int) -> str:
    for unit in ["B", "KB", "MB", "GB", "TB"]:
        if num_bytes < 1024.0:
            return f"{num_bytes:.1f} {unit}"
        num_bytes /= 1024.0
    return f"{num_bytes:.1f} PB"


def main():
    parser = argparse.ArgumentParser(
        description="Radar Cívico - Ingestor de Alta Performance para Auxílio Emergencial"
    )
    parser.add_argument("--arquivo", default=DEFAULT_CSV, help="Caminho do CSV do auxílio")
    parser.add_argument("--banco", default=DEFAULT_DB, help="Caminho do banco SQLite")
    parser.add_argument("--municipio", default=None, help="Filtrar por município específico")
    parser.add_argument("--uf", default=None, help="Filtrar por estado / UF específico (ex: SP)")
    parser.add_argument("--apenas-candidatos", action="store_true", help="Inserir somente registros com match de políticos")
    parser.add_argument("--lote", type=int, default=BATCH_SIZE, help="Tamanho do lote por transação (padrão: 50.000)")
    parser.add_argument("--recriar-indices", action="store_true", default=True, help="Recriar índices após a carga")
    parser.add_argument("--sem-recriar-indices", dest="recriar_indices", action="store_false")

    args = parser.parse_args()

    if not os.path.exists(args.arquivo):
        print(f"❌ Erro: Arquivo CSV não encontrado: {args.arquivo}")
        sys.exit(1)

    if not os.path.exists(args.banco):
        print(f"❌ Erro: Banco SQLite não encontrado: {args.banco}")
        sys.exit(1)

    tamanho_csv = os.path.getsize(args.arquivo)
    print("=" * 70)
    print("🚀 RADAR CÍVICO - INGESTOR DE AUXÍLIO EMERGENCIAL DIRETO NO SQLITE")
    print("=" * 70)
    print(f"📂 Arquivo CSV:     {args.arquivo} ({formatar_bytes(tamanho_csv)})")
    print(f"🗄️ Banco de Dados:  {args.banco} ({formatar_bytes(os.path.getsize(args.banco))})")
    print(f"📦 Tamanho do Lote: {args.lote:,} registros por transação")
    if args.municipio:
        print(f"🏙️ Filtro Município: {args.municipio}")
    if args.uf:
        print(f"📍 Filtro UF:        {args.uf.upper()}")
    if args.apenas_candidatos:
        print("🎯 Filtro Ativo:     Apenas correspondências com Políticos/Candidatos")
    print("-" * 70)

    conn = sqlite3.connect(args.banco)
    configurar_pragmas(conn, rapido=True)

    cpfs_politicos = set()
    nomes_politicos = set()
    if args.apenas_candidatos:
        cpfs_politicos, nomes_politicos = carregar_politicos_referencia(conn)

    # Se for uma carga massiva (maior que 500 MB) e recriar_indices estiver ativo
    reconstruir_depois = False
    if args.recriar_indices and tamanho_csv > 500 * 1024 * 1024 and not args.apenas_candidatos:
        dropar_indices(conn)
        reconstruir_depois = True

    filtro_mun = args.municipio.strip().upper() if args.municipio else None
    filtro_uf = args.uf.strip().upper() if args.uf else None

    sql_insert = """
        INSERT INTO beneficios_emergenciais (
            cpf_mascarado, nome_beneficiario, municipio, uf, mes_disponibilizacao, parcela, valor, enquadramento
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
    """

    cursor = conn.cursor()
    total_linhas = 0
    total_inseridos = 0
    lote = []
    inicio = time.time()
    ultimo_log = inicio

    print("⏳ Iniciando leitura em streaming e inserção em lotes atômicos...")

    try:
        with open(args.arquivo, "r", encoding="utf-8", errors="replace") as f:
            header_line = f.readline()
            headers = [h.strip().upper() for h in header_line.split(",")]

            # Mapeamento dinâmico de colunas
            col_ano_mes = headers.index("ANO_MES") if "ANO_MES" in headers else 0
            col_uf = headers.index("UF") if "UF" in headers else 1
            col_mun = headers.index("MUNICIPIO") if "MUNICIPIO" in headers else 3
            col_cpf = headers.index("CPF_BENEFICIARIO") if "CPF_BENEFICIARIO" in headers else 5
            col_nome = headers.index("BENEFICIARIO") if "BENEFICIARIO" in headers else 6
            col_enq = headers.index("ENQUADRAMENTO") if "ENQUADRAMENTO" in headers else 10
            col_parcela = headers.index("PARCELA") if "PARCELA" in headers else 11
            col_valor = headers.index("VALOR") if "VALOR" in headers else 13

            for line in f:
                total_linhas += 1
                parts = line.rstrip("\r\n").split(",")
                if len(parts) <= max(col_cpf, col_nome, col_valor):
                    continue

                uf_val = parts[col_uf].strip().upper() if col_uf < len(parts) else ""
                mun_val = parts[col_mun].strip() if col_mun < len(parts) else ""

                if filtro_uf and uf_val != filtro_uf:
                    continue
                if filtro_mun and mun_val.upper() != filtro_mun:
                    continue

                raw_cpf = parts[col_cpf].strip()
                nome_val = parts[col_nome].strip().upper()

                if args.apenas_candidatos:
                    cpf_clean = "".join(c for c in raw_cpf if c.isdigit())
                    if cpf_clean not in cpfs_politicos and nome_val not in nomes_politicos:
                        continue

                ano_mes = parts[col_ano_mes].strip() if col_ano_mes < len(parts) else "202004"
                parcela = parts[col_parcela].strip() if col_parcela < len(parts) else "1"
                enq = parts[col_enq].strip() if col_enq < len(parts) else "EXTRA CADUN"
                
                try:
                    valor = float(parts[col_valor].strip())
                except ValueError:
                    valor = 600.0

                lote.append((
                    raw_cpf,
                    nome_val,
                    mun_val if mun_val else None,
                    uf_val if uf_val else None,
                    ano_mes,
                    parcela,
                    valor,
                    enq,
                ))

                if len(lote) >= args.lote:
                    cursor.executemany(sql_insert, lote)
                    conn.commit()
                    total_inseridos += len(lote)
                    lote.clear()

                    agora = time.time()
                    if agora - ultimo_log >= 1.0:
                        taxa = total_linhas / (agora - inicio)
                        print(
                            f"\r🔄 Lidos: {total_linhas:,} linhas | Inseridos: {total_inseridos:,} "
                            f"| Velocidade: {taxa:,.0f} linhas/s | Decorrido: {agora - inicio:.1f}s",
                            end="",
                            flush=True,
                        )
                        ultimo_log = agora

        # Inserir o restante
        if lote:
            cursor.executemany(sql_insert, lote)
            conn.commit()
            total_inseridos += len(lote)
            lote.clear()

    except KeyboardInterrupt:
        print("\n⚠️ Interrupção manual solicitada pelo usuário! Salvando lote atual...")
        if lote:
            cursor.executemany(sql_insert, lote)
            conn.commit()
            total_inseridos += len(lote)

    tempo_total = time.time() - inicio
    print(f"\n\n🏁 Ingestão concluída em {tempo_total:.2f} segundos!")
    print(f"📊 Total de Linhas Lidas:     {total_linhas:,}")
    print(f"💾 Total de Registros Salvos: {total_inseridos:,}")

    if reconstruir_depois:
        recriar_indices(conn)

    # Executa checkpoint do WAL para sincronizar
    print("🧹 Executando PRAGMA wal_checkpoint(PASSIVE)...")
    cursor.execute("PRAGMA wal_checkpoint(PASSIVE);")
    conn.commit()

    cursor.execute("SELECT count(*) FROM beneficios_emergenciais;")
    total_banco = cursor.fetchone()[0]
    cursor.close()
    conn.close()

    print("-" * 70)
    print(f"🎉 SUCESSO! Total atual de benefícios emergenciais no SQLite: {total_banco:,}")
    print("=" * 70)


if __name__ == "__main__":
    main()
