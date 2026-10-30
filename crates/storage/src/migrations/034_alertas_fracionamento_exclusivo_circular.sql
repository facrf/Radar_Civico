-- Migration 034: Suporte a FRACIONAMENTO_DESPESAS, FORNECEDOR_EXCLUSIVO e DOACAO_CIRCULAR na chave de origem

DROP INDEX IF EXISTS idx_alertas_origem;

ALTER TABLE alertas_auditoria DROP COLUMN chave_origem;

ALTER TABLE alertas_auditoria ADD COLUMN chave_origem TEXT GENERATED ALWAYS AS (
    CASE WHEN json_valid(detalhes_json) THEN
        CASE
            WHEN tipo IN ('COMBUSTIVEL', 'COMBUSTIVEL_SOBREPRECO', 'EMPRESA_RECEM_CRIADA', 'FORNECEDOR_FANTASMA')
                AND json_extract(detalhes_json, '$.despesa_id') IS NOT NULL
                THEN json_array(json_extract(detalhes_json, '$.despesa_id'))
            WHEN tipo = 'CONFLITO_OAB'
                THEN CASE
                    WHEN json_extract(detalhes_json, '$.conflito_id') IS NOT NULL
                        THEN json_array(json_extract(detalhes_json, '$.conflito_id'))
                    WHEN json_extract(detalhes_json, '$.doador_id') IS NOT NULL
                        THEN json_array(json_extract(detalhes_json, '$.doador_id'), json_extract(detalhes_json, '$.cargo'))
                    ELSE NULL
                END
            WHEN tipo = 'AUXILIO_EMERGENCIAL' AND json_extract(detalhes_json, '$.auxilio_alerta_id') IS NOT NULL
                THEN json_array(json_extract(detalhes_json, '$.auxilio_alerta_id'))
            WHEN tipo = 'DOADOR_INCOMPATIVEL' AND json_extract(detalhes_json, '$.receita_id') IS NOT NULL
                THEN json_array(json_extract(detalhes_json, '$.receita_id'))
            WHEN tipo = 'EVOLUCAO_PATRIMONIAL'
                THEN json_array(json_extract(detalhes_json, '$.politico_id'), json_extract(detalhes_json, '$.ano_recente'))
            WHEN tipo = 'CONLUIO_LICITACAO'
                THEN json_array(json_extract(detalhes_json, '$.socio_documento'), json_extract(detalhes_json, '$.orgao'))
            WHEN tipo = 'CAPITAL_DESPROPORCIONAL' AND json_extract(detalhes_json, '$.cnpj') IS NOT NULL
                THEN json_array(json_extract(detalhes_json, '$.cnpj'))
            WHEN tipo = 'POSSIVEL_PARENTESCO'
                THEN json_array(json_extract(detalhes_json, '$.politico_id'), json_extract(detalhes_json, '$.alvo_doc'))
            WHEN tipo = 'FORNECEDOR_HUB' AND json_extract(detalhes_json, '$.fornecedor_id') IS NOT NULL
                THEN json_array(json_extract(detalhes_json, '$.fornecedor_id'))
            WHEN tipo IN ('TRIANGULACAO', 'UBIQUIDADE') AND json_extract(detalhes_json, '$.chave_analitica') IS NOT NULL
                THEN json_array(json_extract(detalhes_json, '$.chave_analitica'))
            WHEN tipo = 'FRACIONAMENTO_DESPESAS'
                THEN json_array(json_extract(detalhes_json, '$.parlamentar'), json_extract(detalhes_json, '$.fornecedor_doc'), json_extract(detalhes_json, '$.data_inicio'))
            WHEN tipo = 'FORNECEDOR_EXCLUSIVO'
                THEN json_array(json_extract(detalhes_json, '$.fornecedor_doc'), json_extract(detalhes_json, '$.parlamentar'))
            WHEN tipo = 'DOACAO_CIRCULAR'
                THEN json_array(json_extract(detalhes_json, '$.politico_id'), json_extract(detalhes_json, '$.fornecedor_doc'), json_extract(detalhes_json, '$.ano_eleicao'))
        END
    END
) VIRTUAL;

DELETE FROM alertas_auditoria
WHERE chave_origem IS NOT NULL AND id NOT IN (
    SELECT MIN(id) FROM alertas_auditoria WHERE chave_origem IS NOT NULL GROUP BY tipo, chave_origem
);

CREATE UNIQUE INDEX idx_alertas_origem ON alertas_auditoria(tipo, chave_origem)
WHERE chave_origem IS NOT NULL;
