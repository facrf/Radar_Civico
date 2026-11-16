-- Preserva a evidência anterior antes de retirar resultados produzidos pelas
-- regras cuja agregação/cobertura ou confirmação de identidade foi corrigida.
INSERT INTO qualidade_dados_eventos(entidade,registro_id,campo,valor_anterior,motivo)
SELECT 'alertas_auditoria',id,'resultado',json_object('tipo',tipo,'severidade',severidade,'titulo',titulo,'descricao',descricao,'alvo_nome',alvo_nome,'alvo_documento',alvo_documento,'municipio',municipio,'uf',uf,'ano',ano,'valor_envolvido',valor_envolvido,'fonte_dado',fonte_dado,'detalhes_json',detalhes_json,'data_criacao',data_criacao),
       'Motor revisado: resultado anterior preservado para rastreabilidade; requer nova auditoria'
FROM alertas_auditoria
WHERE tipo IN ('DOACAO_CIRCULAR','FRACIONAMENTO_DESPESAS','FORNECEDOR_EXCLUSIVO','FORNECEDOR_HUB','POSSIVEL_PARENTESCO','EMPRESA_RECEM_CRIADA','FORNECEDOR_FANTASMA')
   OR (tipo='CONFLITO_OAB' AND json_valid(detalhes_json) AND json_extract(detalhes_json,'$.doador_id') IS NOT NULL);

DELETE FROM alertas_auditoria
WHERE tipo IN ('DOACAO_CIRCULAR','FRACIONAMENTO_DESPESAS','FORNECEDOR_EXCLUSIVO','FORNECEDOR_HUB','POSSIVEL_PARENTESCO','EMPRESA_RECEM_CRIADA','FORNECEDOR_FANTASMA')
   OR (tipo='CONFLITO_OAB' AND json_valid(detalhes_json) AND json_extract(detalhes_json,'$.doador_id') IS NOT NULL);

UPDATE auditoria_estado SET pendente=1,metodologia='alertas-locais-v2' WHERE id=1;
