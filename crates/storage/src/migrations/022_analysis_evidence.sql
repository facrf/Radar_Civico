-- Evidência explícita: uma data sem horário ou uma localização estimada não
-- comprova presença simultânea em dois lugares.
ALTER TABLE despesas_parlamentares ADD COLUMN parlamentar_id INTEGER REFERENCES politicos(id);
ALTER TABLE despesas_parlamentares ADD COLUMN emitido_em TEXT;
ALTER TABLE despesas_parlamentares ADD COLUMN municipio_emissao TEXT;
ALTER TABLE despesas_parlamentares ADD COLUMN uf_emissao TEXT;
ALTER TABLE despesas_parlamentares ADD COLUMN latitude_emissao REAL;
ALTER TABLE despesas_parlamentares ADD COLUMN longitude_emissao REAL;
ALTER TABLE despesas_parlamentares ADD COLUMN presencial INTEGER NOT NULL DEFAULT 0 CHECK(presencial IN (0,1));
CREATE INDEX idx_despesas_presenciais ON despesas_parlamentares(parlamentar_id, emitido_em) WHERE presencial=1;
ALTER TABLE contratos_publicos ADD COLUMN modalidade TEXT;
CREATE INDEX idx_alertas_chave_analitica ON alertas_auditoria(tipo,json_extract(detalhes_json,'$.chave_analitica')) WHERE json_valid(detalhes_json);
