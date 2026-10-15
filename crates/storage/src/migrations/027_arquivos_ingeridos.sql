-- Migration 027: Tabela de auditoria e rastreamento de integridade de arquivos ingeridos com hashes SHA-256
CREATE TABLE IF NOT EXISTS arquivos_ingeridos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fonte TEXT NOT NULL,
    nome_arquivo TEXT NOT NULL,
    hash_sha256 TEXT NOT NULL,
    tamanho_bytes INTEGER NOT NULL,
    status TEXT NOT NULL,
    registros_afetados INTEGER DEFAULT 0,
    detalhes TEXT,
    criado_em TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(fonte, hash_sha256)
);

CREATE INDEX IF NOT EXISTS idx_arquivos_ingeridos_fonte ON arquivos_ingeridos(fonte);
CREATE INDEX IF NOT EXISTS idx_arquivos_ingeridos_nome ON arquivos_ingeridos(nome_arquivo);
CREATE INDEX IF NOT EXISTS idx_arquivos_ingeridos_hash ON arquivos_ingeridos(hash_sha256);
