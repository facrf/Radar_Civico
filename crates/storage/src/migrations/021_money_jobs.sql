ALTER TABLE candidaturas ADD COLUMN total_bens_declarados_centavos_armazenados INTEGER;
ALTER TABLE candidaturas ADD COLUMN total_bens_declarados_centavos INTEGER GENERATED ALWAYS AS (COALESCE(total_bens_declarados_centavos_armazenados,CAST(ROUND(total_bens_declarados*100.0) AS INTEGER))) VIRTUAL;
CREATE TRIGGER candidaturas_total_bens_declarados_centavos_insert AFTER INSERT ON candidaturas BEGIN
 UPDATE candidaturas SET total_bens_declarados_centavos_armazenados=CAST(ROUND(NEW.total_bens_declarados*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TRIGGER candidaturas_total_bens_declarados_centavos_update AFTER UPDATE OF total_bens_declarados ON candidaturas BEGIN
 UPDATE candidaturas SET total_bens_declarados_centavos_armazenados=CAST(ROUND(NEW.total_bens_declarados*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
ALTER TABLE bens_candidato ADD COLUMN valor_declarado_centavos_armazenados INTEGER;
ALTER TABLE bens_candidato ADD COLUMN valor_declarado_centavos INTEGER GENERATED ALWAYS AS (COALESCE(valor_declarado_centavos_armazenados,CAST(ROUND(valor_declarado*100.0) AS INTEGER))) VIRTUAL;
CREATE TRIGGER bens_candidato_valor_declarado_centavos_insert AFTER INSERT ON bens_candidato BEGIN
 UPDATE bens_candidato SET valor_declarado_centavos_armazenados=CAST(ROUND(NEW.valor_declarado*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TRIGGER bens_candidato_valor_declarado_centavos_update AFTER UPDATE OF valor_declarado ON bens_candidato BEGIN
 UPDATE bens_candidato SET valor_declarado_centavos_armazenados=CAST(ROUND(NEW.valor_declarado*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
ALTER TABLE receitas_campanha ADD COLUMN valor_centavos_armazenados INTEGER;
ALTER TABLE receitas_campanha ADD COLUMN valor_centavos INTEGER GENERATED ALWAYS AS (COALESCE(valor_centavos_armazenados,CAST(ROUND(valor*100.0) AS INTEGER))) VIRTUAL;
CREATE TRIGGER receitas_campanha_valor_centavos_insert AFTER INSERT ON receitas_campanha BEGIN
 UPDATE receitas_campanha SET valor_centavos_armazenados=CAST(ROUND(NEW.valor*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TRIGGER receitas_campanha_valor_centavos_update AFTER UPDATE OF valor ON receitas_campanha BEGIN
 UPDATE receitas_campanha SET valor_centavos_armazenados=CAST(ROUND(NEW.valor*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
ALTER TABLE despesas_campanha ADD COLUMN valor_centavos_armazenados INTEGER;
ALTER TABLE despesas_campanha ADD COLUMN valor_centavos INTEGER GENERATED ALWAYS AS (COALESCE(valor_centavos_armazenados,CAST(ROUND(valor*100.0) AS INTEGER))) VIRTUAL;
CREATE TRIGGER despesas_campanha_valor_centavos_insert AFTER INSERT ON despesas_campanha BEGIN
 UPDATE despesas_campanha SET valor_centavos_armazenados=CAST(ROUND(NEW.valor*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TRIGGER despesas_campanha_valor_centavos_update AFTER UPDATE OF valor ON despesas_campanha BEGIN
 UPDATE despesas_campanha SET valor_centavos_armazenados=CAST(ROUND(NEW.valor*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
ALTER TABLE despesas_parlamentares ADD COLUMN valor_liquido_centavos_armazenados INTEGER;
ALTER TABLE despesas_parlamentares ADD COLUMN valor_liquido_centavos INTEGER GENERATED ALWAYS AS (COALESCE(valor_liquido_centavos_armazenados,CAST(ROUND(valor_liquido*100.0) AS INTEGER))) VIRTUAL;
CREATE TRIGGER despesas_parlamentares_valor_liquido_centavos_insert AFTER INSERT ON despesas_parlamentares BEGIN
 UPDATE despesas_parlamentares SET valor_liquido_centavos_armazenados=CAST(ROUND(NEW.valor_liquido*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TRIGGER despesas_parlamentares_valor_liquido_centavos_update AFTER UPDATE OF valor_liquido ON despesas_parlamentares BEGIN
 UPDATE despesas_parlamentares SET valor_liquido_centavos_armazenados=CAST(ROUND(NEW.valor_liquido*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
ALTER TABLE contratos_publicos ADD COLUMN valor_contratado_centavos_armazenados INTEGER;
ALTER TABLE contratos_publicos ADD COLUMN valor_contratado_centavos INTEGER GENERATED ALWAYS AS (COALESCE(valor_contratado_centavos_armazenados,CAST(ROUND(valor_contratado*100.0) AS INTEGER))) VIRTUAL;
CREATE TRIGGER contratos_publicos_valor_contratado_centavos_insert AFTER INSERT ON contratos_publicos BEGIN
 UPDATE contratos_publicos SET valor_contratado_centavos_armazenados=CAST(ROUND(NEW.valor_contratado*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TRIGGER contratos_publicos_valor_contratado_centavos_update AFTER UPDATE OF valor_contratado ON contratos_publicos BEGIN
 UPDATE contratos_publicos SET valor_contratado_centavos_armazenados=CAST(ROUND(NEW.valor_contratado*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
ALTER TABLE conexoes_rede ADD COLUMN valor_centavos_armazenados INTEGER;
ALTER TABLE conexoes_rede ADD COLUMN valor_centavos INTEGER GENERATED ALWAYS AS (COALESCE(valor_centavos_armazenados,CAST(ROUND(valor*100.0) AS INTEGER))) VIRTUAL;
CREATE TRIGGER conexoes_rede_valor_centavos_insert AFTER INSERT ON conexoes_rede BEGIN
 UPDATE conexoes_rede SET valor_centavos_armazenados=CAST(ROUND(NEW.valor*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TRIGGER conexoes_rede_valor_centavos_update AFTER UPDATE OF valor ON conexoes_rede BEGIN
 UPDATE conexoes_rede SET valor_centavos_armazenados=CAST(ROUND(NEW.valor*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
ALTER TABLE alertas_auditoria ADD COLUMN valor_envolvido_centavos_armazenados INTEGER;
ALTER TABLE alertas_auditoria ADD COLUMN valor_envolvido_centavos INTEGER GENERATED ALWAYS AS (COALESCE(valor_envolvido_centavos_armazenados,CAST(ROUND(valor_envolvido*100.0) AS INTEGER))) VIRTUAL;
CREATE TRIGGER alertas_auditoria_valor_envolvido_centavos_insert AFTER INSERT ON alertas_auditoria BEGIN
 UPDATE alertas_auditoria SET valor_envolvido_centavos_armazenados=CAST(ROUND(NEW.valor_envolvido*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TRIGGER alertas_auditoria_valor_envolvido_centavos_update AFTER UPDATE OF valor_envolvido ON alertas_auditoria BEGIN
 UPDATE alertas_auditoria SET valor_envolvido_centavos_armazenados=CAST(ROUND(NEW.valor_envolvido*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
ALTER TABLE beneficios_emergenciais ADD COLUMN valor_centavos_armazenados INTEGER;
ALTER TABLE beneficios_emergenciais ADD COLUMN valor_centavos INTEGER GENERATED ALWAYS AS (COALESCE(valor_centavos_armazenados,CAST(ROUND(valor*100.0) AS INTEGER))) VIRTUAL;
CREATE TRIGGER beneficios_emergenciais_valor_centavos_insert AFTER INSERT ON beneficios_emergenciais BEGIN
 UPDATE beneficios_emergenciais SET valor_centavos_armazenados=CAST(ROUND(NEW.valor*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TRIGGER beneficios_emergenciais_valor_centavos_update AFTER UPDATE OF valor ON beneficios_emergenciais BEGIN
 UPDATE beneficios_emergenciais SET valor_centavos_armazenados=CAST(ROUND(NEW.valor*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
ALTER TABLE alertas_beneficio_indevido ADD COLUMN valor_recebido_centavos_armazenados INTEGER;
ALTER TABLE alertas_beneficio_indevido ADD COLUMN valor_recebido_centavos INTEGER GENERATED ALWAYS AS (COALESCE(valor_recebido_centavos_armazenados,CAST(ROUND(valor_recebido*100.0) AS INTEGER))) VIRTUAL;
CREATE TRIGGER alertas_beneficio_indevido_valor_recebido_centavos_insert AFTER INSERT ON alertas_beneficio_indevido BEGIN
 UPDATE alertas_beneficio_indevido SET valor_recebido_centavos_armazenados=CAST(ROUND(NEW.valor_recebido*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TRIGGER alertas_beneficio_indevido_valor_recebido_centavos_update AFTER UPDATE OF valor_recebido ON alertas_beneficio_indevido BEGIN
 UPDATE alertas_beneficio_indevido SET valor_recebido_centavos_armazenados=CAST(ROUND(NEW.valor_recebido*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
ALTER TABLE alertas_beneficio_indevido ADD COLUMN total_bens_centavos_armazenados INTEGER;
ALTER TABLE alertas_beneficio_indevido ADD COLUMN total_bens_centavos INTEGER GENERATED ALWAYS AS (COALESCE(total_bens_centavos_armazenados,CAST(ROUND(total_bens*100.0) AS INTEGER))) VIRTUAL;
CREATE TRIGGER alertas_beneficio_indevido_total_bens_centavos_insert AFTER INSERT ON alertas_beneficio_indevido BEGIN
 UPDATE alertas_beneficio_indevido SET total_bens_centavos_armazenados=CAST(ROUND(NEW.total_bens*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TRIGGER alertas_beneficio_indevido_total_bens_centavos_update AFTER UPDATE OF total_bens ON alertas_beneficio_indevido BEGIN
 UPDATE alertas_beneficio_indevido SET total_bens_centavos_armazenados=CAST(ROUND(NEW.total_bens*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
ALTER TABLE empresas_qsa ADD COLUMN capital_social_centavos_armazenados INTEGER;
ALTER TABLE empresas_qsa ADD COLUMN capital_social_centavos INTEGER GENERATED ALWAYS AS (COALESCE(capital_social_centavos_armazenados,CAST(ROUND(capital_social*100.0) AS INTEGER))) VIRTUAL;
CREATE TRIGGER empresas_qsa_capital_social_centavos_insert AFTER INSERT ON empresas_qsa BEGIN
 UPDATE empresas_qsa SET capital_social_centavos_armazenados=CAST(ROUND(NEW.capital_social*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TRIGGER empresas_qsa_capital_social_centavos_update AFTER UPDATE OF capital_social ON empresas_qsa BEGIN
 UPDATE empresas_qsa SET capital_social_centavos_armazenados=CAST(ROUND(NEW.capital_social*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
ALTER TABLE emendas_parlamentares ADD COLUMN valor_empenhado_centavos_armazenados INTEGER;
ALTER TABLE emendas_parlamentares ADD COLUMN valor_empenhado_centavos INTEGER GENERATED ALWAYS AS (COALESCE(valor_empenhado_centavos_armazenados,CAST(ROUND(valor_empenhado*100.0) AS INTEGER))) VIRTUAL;
CREATE TRIGGER emendas_parlamentares_valor_empenhado_centavos_insert AFTER INSERT ON emendas_parlamentares BEGIN
 UPDATE emendas_parlamentares SET valor_empenhado_centavos_armazenados=CAST(ROUND(NEW.valor_empenhado*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TRIGGER emendas_parlamentares_valor_empenhado_centavos_update AFTER UPDATE OF valor_empenhado ON emendas_parlamentares BEGIN
 UPDATE emendas_parlamentares SET valor_empenhado_centavos_armazenados=CAST(ROUND(NEW.valor_empenhado*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
ALTER TABLE emendas_parlamentares ADD COLUMN valor_pago_centavos_armazenados INTEGER;
ALTER TABLE emendas_parlamentares ADD COLUMN valor_pago_centavos INTEGER GENERATED ALWAYS AS (COALESCE(valor_pago_centavos_armazenados,CAST(ROUND(valor_pago*100.0) AS INTEGER))) VIRTUAL;
CREATE TRIGGER emendas_parlamentares_valor_pago_centavos_insert AFTER INSERT ON emendas_parlamentares BEGIN
 UPDATE emendas_parlamentares SET valor_pago_centavos_armazenados=CAST(ROUND(NEW.valor_pago*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TRIGGER emendas_parlamentares_valor_pago_centavos_update AFTER UPDATE OF valor_pago ON emendas_parlamentares BEGIN
 UPDATE emendas_parlamentares SET valor_pago_centavos_armazenados=CAST(ROUND(NEW.valor_pago*100.0) AS INTEGER) WHERE rowid=NEW.rowid;
END;
CREATE TABLE tarefas_importacao(namespace TEXT NOT NULL,id TEXT NOT NULL,em_execucao INTEGER NOT NULL CHECK(em_execucao IN (0,1)),payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),atualizado_em TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,PRIMARY KEY(namespace,id));
