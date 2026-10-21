-- Migration 031: Deduplicação e Saneamento de Candidaturas
-- Reponta registros dependentes (bens, receitas, despesas) vinculados a candidaturas fantasmas
-- (códigos numéricos do TSE '11', '12', '13' ou cabeçalhos 'CARGO') para a candidatura canônica correspondente.

UPDATE bens_candidato
SET candidatura_id = (
    SELECT c2.id FROM candidaturas c1
    JOIN candidaturas c2 ON c2.politico_id = c1.politico_id AND c2.ano_eleicao = c1.ano_eleicao
    WHERE c1.id = bens_candidato.candidatura_id
      AND c2.cargo NOT IN ('13', '12', '11', 'CARGO', 'INDEFINIDO')
    LIMIT 1
)
WHERE candidatura_id IN (
    SELECT id FROM candidaturas WHERE cargo IN ('13', '12', '11', 'CARGO', 'INDEFINIDO')
)
AND EXISTS (
    SELECT 1 FROM candidaturas c1
    JOIN candidaturas c2 ON c2.politico_id = c1.politico_id AND c2.ano_eleicao = c1.ano_eleicao
    WHERE c1.id = bens_candidato.candidatura_id
      AND c2.cargo NOT IN ('13', '12', '11', 'CARGO', 'INDEFINIDO')
);

UPDATE receitas_campanha
SET candidatura_id = (
    SELECT c2.id FROM candidaturas c1
    JOIN candidaturas c2 ON c2.politico_id = c1.politico_id AND c2.ano_eleicao = c1.ano_eleicao
    WHERE c1.id = receitas_campanha.candidatura_id
      AND c2.cargo NOT IN ('13', '12', '11', 'CARGO', 'INDEFINIDO')
    LIMIT 1
)
WHERE candidatura_id IN (
    SELECT id FROM candidaturas WHERE cargo IN ('13', '12', '11', 'CARGO', 'INDEFINIDO')
)
AND EXISTS (
    SELECT 1 FROM candidaturas c1
    JOIN candidaturas c2 ON c2.politico_id = c1.politico_id AND c2.ano_eleicao = c1.ano_eleicao
    WHERE c1.id = receitas_campanha.candidatura_id
      AND c2.cargo NOT IN ('13', '12', '11', 'CARGO', 'INDEFINIDO')
);

UPDATE despesas_campanha
SET candidatura_id = (
    SELECT c2.id FROM candidaturas c1
    JOIN candidaturas c2 ON c2.politico_id = c1.politico_id AND c2.ano_eleicao = c1.ano_eleicao
    WHERE c1.id = despesas_campanha.candidatura_id
      AND c2.cargo NOT IN ('13', '12', '11', 'CARGO', 'INDEFINIDO')
    LIMIT 1
)
WHERE candidatura_id IN (
    SELECT id FROM candidaturas WHERE cargo IN ('13', '12', '11', 'CARGO', 'INDEFINIDO')
)
AND EXISTS (
    SELECT 1 FROM candidaturas c1
    JOIN candidaturas c2 ON c2.politico_id = c1.politico_id AND c2.ano_eleicao = c1.ano_eleicao
    WHERE c1.id = despesas_campanha.candidatura_id
      AND c2.cargo NOT IN ('13', '12', '11', 'CARGO', 'INDEFINIDO')
);

-- Remove as candidaturas fantasmas duplicadas cuja versão canônica já existe
DELETE FROM candidaturas
WHERE cargo IN ('13', '12', '11', 'CARGO')
  AND EXISTS (
      SELECT 1 FROM candidaturas c2
      WHERE c2.politico_id = candidaturas.politico_id
        AND c2.ano_eleicao = candidaturas.ano_eleicao
        AND c2.cargo NOT IN ('13', '12', '11', 'CARGO', 'INDEFINIDO')
  );
