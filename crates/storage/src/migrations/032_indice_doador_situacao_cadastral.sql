-- Migration 032: Otimização de Busca de Doadores e Situação Cadastral de Empresas
-- 1. Índice para aceleração de consultas e autocompletes por nome do doador
CREATE INDEX IF NOT EXISTS idx_receitas_campanha_doador_nome ON receitas_campanha(doador_nome);

-- 2. Coluna de situação cadastral na tabela empresas_qsa para auditoria de empresas inaptas/baixadas
ALTER TABLE empresas_qsa ADD COLUMN situacao_cadastral TEXT DEFAULT 'ATIVA';
CREATE INDEX IF NOT EXISTS idx_empresas_qsa_situacao ON empresas_qsa(situacao_cadastral);
