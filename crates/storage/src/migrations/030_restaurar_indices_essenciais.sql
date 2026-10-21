-- Migração 30: Restauração de índices essenciais para consultas de prestação de contas, bens e candidaturas
CREATE INDEX IF NOT EXISTS idx_bens_candidatura ON bens_candidato(candidatura_id);
CREATE INDEX IF NOT EXISTS idx_receitas_candidatura ON receitas_campanha(candidatura_id);
CREATE INDEX IF NOT EXISTS idx_despesas_candidatura ON despesas_campanha(candidatura_id);
CREATE INDEX IF NOT EXISTS idx_candidaturas_eleicao ON candidaturas(ano_eleicao, cargo, uf);
