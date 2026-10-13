//! Projeção transacional das bases importadas para nós e relações navegáveis.
use storage::{rusqlite::Connection, Result};

/// Reutiliza apenas a projeção compatível e sem alterações nas fontes.
pub fn refresh_if_needed(conn: &Connection) -> Result<usize> {
    let cached: Option<usize> = conn.query_row(
        "SELECT CASE WHEN dirty=0 AND version=2 THEN relations END FROM projection_state WHERE id=1",
        [], |row| row.get(0),
    )?;
    match cached {
        Some(count) => Ok(count),
        None => refresh(conn),
    }
}

pub fn refresh(conn: &Connection) -> Result<usize> {
    crate::identity::register_sql(conn)?;
    if conn.is_autocommit() {
        let tx = storage::transaction_immediate_ref(conn)?;
        let result = refresh(&tx)?;
        tx.commit()?;
        return Ok(result);
    }
    let tx = conn;
    tx.execute_batch("DROP TABLE IF EXISTS temp.projection_roots; CREATE TEMP TABLE projection_roots(root TEXT PRIMARY KEY);")?;
    tx.execute_batch(r#"
        INSERT OR IGNORE INTO temp.projection_roots(root)
        SELECT substr(doc,1,8) FROM (
            SELECT REPLACE(REPLACE(REPLACE(fornecedor_cnpj_cpf,'.',''),'/',''),'-','') AS doc FROM despesas_parlamentares
            UNION ALL SELECT REPLACE(REPLACE(REPLACE(fornecedor_cnpj,'.',''),'/',''),'-','') FROM contratos_publicos
            UNION ALL SELECT REPLACE(REPLACE(REPLACE(doador_cpf_cnpj,'.',''),'/',''),'-','') FROM receitas_campanha
            UNION ALL SELECT REPLACE(REPLACE(REPLACE(fornecedor_cpf_cnpj,'.',''),'/',''),'-','') FROM despesas_campanha
        ) WHERE length(doc)=14 AND valid_document(doc);
    "#)?;
    // Apenas relações geradas por esta projeção são substituídas. IDs de nós são estáveis.
    tx.execute(
        "DELETE FROM conexoes_rede WHERE json_extract(metadata_json,'$.projection') IN ('radar','qsa_root')",
        [],
    )?;

    tx.execute_batch(r#"
        INSERT INTO nos_rede(uuid,tipo,documento,nome,metadata_json)
        SELECT 'politico_'||id,'POLITICO',cpf_mascarado,nome_completo,json_object('projection','radar','politico_id',id) FROM politicos WHERE 1
        ON CONFLICT(uuid) DO UPDATE SET documento=excluded.documento,nome=excluded.nome,metadata_json=excluded.metadata_json;
        INSERT INTO nos_rede(uuid,tipo,documento,nome,metadata_json)
        SELECT 'cnpj_'||cnpj_basico,'EMPRESA',cnpj_basico,MIN(razao_social),json_object('projection','radar') FROM temp.projection_roots roots CROSS JOIN empresas_qsa ON empresas_qsa.cnpj_basico=roots.root GROUP BY cnpj_basico
        ON CONFLICT(uuid) DO UPDATE SET nome=excluded.nome;
        INSERT INTO nos_rede(uuid,tipo,documento,nome,metadata_json)
        SELECT 'cnpj_'||substr(doc,1,8),'EMPRESA',substr(doc,1,8),MIN(name),json_object('projection','radar') FROM (
            SELECT REPLACE(REPLACE(REPLACE(fornecedor_cnpj_cpf,'.',''),'/',''),'-','') AS doc,fornecedor_nome AS name FROM despesas_parlamentares
            UNION ALL SELECT REPLACE(REPLACE(REPLACE(fornecedor_cnpj,'.',''),'/',''),'-',''),fornecedor_cnpj FROM contratos_publicos
            UNION ALL SELECT REPLACE(REPLACE(REPLACE(doador_cpf_cnpj,'.',''),'/',''),'-',''),doador_nome FROM receitas_campanha
            UNION ALL SELECT REPLACE(REPLACE(REPLACE(fornecedor_cpf_cnpj,'.',''),'/',''),'-',''),fornecedor_nome FROM despesas_campanha
        ) WHERE length(doc)=14 AND valid_document(doc) GROUP BY substr(doc,1,8)
        ON CONFLICT(uuid) DO NOTHING;
        INSERT INTO nos_rede(uuid,tipo,documento,nome,metadata_json)
        SELECT 'doc_'||doc,'PESSOA_FISICA',doc,MIN(name),json_object('projection','radar','confianca','CONFIRMADO') FROM (
            SELECT REPLACE(REPLACE(REPLACE(doador_cpf_cnpj,'.',''),'/',''),'-','') AS doc,doador_nome AS name FROM receitas_campanha
            UNION ALL SELECT REPLACE(REPLACE(REPLACE(fornecedor_cpf_cnpj,'.',''),'/',''),'-',''),fornecedor_nome FROM despesas_campanha
            UNION ALL SELECT REPLACE(REPLACE(REPLACE(socio_cpf_cnpj_mascarado,'.',''),'/',''),'-',''),socio_nome FROM temp.projection_roots roots CROSS JOIN empresas_qsa ON empresas_qsa.cnpj_basico=roots.root
        ) WHERE length(doc)=11 AND valid_document(doc) GROUP BY doc
        ON CONFLICT(uuid) DO UPDATE SET nome=excluded.nome;
    "#)?;
    tx.execute_batch(r#"
        INSERT INTO nos_rede(uuid,tipo,documento,nome,metadata_json)
        SELECT 'cnpj_'||substr(doc,1,8),'EMPRESA',substr(doc,1,8),MIN(socio_nome),json_object('projection','radar')
        FROM (SELECT socio_nome,REPLACE(REPLACE(REPLACE(socio_cpf_cnpj_mascarado,'.',''),'/',''),'-','') AS doc FROM temp.projection_roots roots CROSS JOIN empresas_qsa q ON q.cnpj_basico=roots.root)
        WHERE length(doc)=14 AND valid_document(doc) GROUP BY substr(doc,1,8) ON CONFLICT(uuid) DO NOTHING;
    "#)?;
    tx.execute("UPDATE nos_rede SET metadata_json=json_set(metadata_json,'$.confianca','POSSIVEL') WHERE uuid LIKE 'doc_%' AND json_extract(metadata_json,'$.projection') IN ('radar','qsa_root') AND NOT valid_document(documento)", [])?;
    // Sócios mascarados ou inválidos são nós próprios por empresa e nome.
    tx.execute_batch(r#"
        INSERT INTO nos_rede(uuid,tipo,documento,nome,metadata_json)
        SELECT 'socio_'||cnpj_basico||'_'||socio_cpf_cnpj_mascarado||'_'||socio_nome,'PESSOA_FISICA',socio_cpf_cnpj_mascarado,socio_nome,json_object('projection','radar','confianca','POSSIVEL')
        FROM temp.projection_roots roots CROSS JOIN empresas_qsa ON empresas_qsa.cnpj_basico=roots.root WHERE NOT valid_document(socio_cpf_cnpj_mascarado)
        ON CONFLICT(uuid) DO NOTHING;
        WITH projection_edges(source,target,relation,cents,year,event,origin,evidence) AS (SELECT CASE WHEN length(doc)=11 AND valid_document(doc) THEN 'doc_'||doc
                    WHEN length(doc)=14 AND valid_document(doc) THEN 'cnpj_'||substr(doc,1,8)
                    ELSE 'socio_'||cnpj_basico||'_'||socio_cpf_cnpj_mascarado||'_'||socio_nome END,
               'cnpj_'||cnpj_basico,'SOCIO_ADMINISTRADOR',0,0,NULL,'RECEITA_FEDERAL',json_object('projection','radar','confianca',CASE WHEN valid_document(doc) THEN 'CONFIRMADO' ELSE 'POSSIVEL' END)
        FROM (SELECT *, REPLACE(REPLACE(REPLACE(socio_cpf_cnpj_mascarado,'.',''),'/',''),'-','') AS doc FROM temp.projection_roots roots CROSS JOIN empresas_qsa ON empresas_qsa.cnpj_basico=roots.root)) INSERT INTO conexoes_rede(origem_id,destino_id,tipo_relacao,valor,ano,data_evento,fonte_dado,metadata_json) SELECT s.id,t.id,e.relation,e.cents/100.0,COALESCE(e.year,0),e.event,e.origin,e.evidence FROM projection_edges e JOIN nos_rede s ON s.uuid=e.source JOIN nos_rede t ON t.uuid=e.target;
        WITH projection_edges(source,target,relation,cents,year,event,origin,evidence) AS (SELECT CASE length(doc) WHEN 14 THEN 'cnpj_'||substr(doc,1,8) ELSE 'doc_'||doc END,'politico_'||politico_id,'DOACAO',valor_centavos,ano_eleicao,data_receita,'TSE',json_object('projection','radar','receita_id',id)
        FROM (SELECT r.*,c.politico_id,c.ano_eleicao,REPLACE(REPLACE(REPLACE(doador_cpf_cnpj,'.',''),'/',''),'-','') AS doc FROM receitas_campanha r JOIN candidaturas c ON c.id=r.candidatura_id)
        WHERE valid_document(doc)) INSERT INTO conexoes_rede(origem_id,destino_id,tipo_relacao,valor,ano,data_evento,fonte_dado,metadata_json) SELECT s.id,t.id,e.relation,e.cents/100.0,COALESCE(e.year,0),e.event,e.origin,e.evidence FROM projection_edges e JOIN nos_rede s ON s.uuid=e.source JOIN nos_rede t ON t.uuid=e.target;
        WITH projection_edges(source,target,relation,cents,year,event,origin,evidence) AS (SELECT 'politico_'||politico_id,CASE length(doc) WHEN 14 THEN 'cnpj_'||substr(doc,1,8) ELSE 'doc_'||doc END,'DESPESA_CAMPANHA',valor_centavos,ano_eleicao,data_despesa,'TSE',json_object('projection','radar','despesa_id',id)
        FROM (SELECT d.*,c.politico_id,c.ano_eleicao,REPLACE(REPLACE(REPLACE(fornecedor_cpf_cnpj,'.',''),'/',''),'-','') AS doc FROM despesas_campanha d JOIN candidaturas c ON c.id=d.candidatura_id)
        WHERE valid_document(doc)) INSERT INTO conexoes_rede(origem_id,destino_id,tipo_relacao,valor,ano,data_evento,fonte_dado,metadata_json) SELECT s.id,t.id,e.relation,e.cents/100.0,COALESCE(e.year,0),e.event,e.origin,e.evidence FROM projection_edges e JOIN nos_rede s ON s.uuid=e.source JOIN nos_rede t ON t.uuid=e.target;
    "#)?;
    // Despesas CEAP só ligam a um político se o nome tem uma única identidade no cadastro.
    tx.execute_batch(r#"
        WITH projection_edges(source,target,relation,cents,year,event,origin,evidence) AS (SELECT 'politico_'||COALESCE(d.parlamentar_id,p.id),'cnpj_'||substr(REPLACE(REPLACE(REPLACE(d.fornecedor_cnpj_cpf,'.',''),'/',''),'-',''),1,8),'CEAP',d.valor_liquido_centavos,CAST(substr(d.data_emissao,1,4) AS INTEGER),d.data_emissao,'CAMARA',json_object('projection','radar','despesa_id',d.id,'confianca',CASE WHEN d.parlamentar_id IS NULL THEN 'POSSIVEL' ELSE 'CONFIRMADO' END)
        FROM despesas_parlamentares d LEFT JOIN (
            SELECT name,MIN(id) AS id FROM (SELECT id,nome_completo AS name FROM politicos UNION SELECT id,nome_urna FROM politicos) GROUP BY name HAVING COUNT(DISTINCT id)=1
        ) p ON p.name=d.parlamentar_nome
        WHERE COALESCE(d.parlamentar_id,p.id) IS NOT NULL AND length(REPLACE(REPLACE(REPLACE(d.fornecedor_cnpj_cpf,'.',''),'/',''),'-',''))=14 AND valid_document(d.fornecedor_cnpj_cpf)) INSERT INTO conexoes_rede(origem_id,destino_id,tipo_relacao,valor,ano,data_evento,fonte_dado,metadata_json) SELECT s.id,t.id,e.relation,e.cents/100.0,COALESCE(e.year,0),e.event,e.origin,e.evidence FROM projection_edges e JOIN nos_rede s ON s.uuid=e.source JOIN nos_rede t ON t.uuid=e.target;
        INSERT INTO nos_rede(uuid,tipo,nome,metadata_json)
        SELECT 'orgao_'||orgao_contratante,'ORGAO',orgao_contratante,json_object('projection','radar') FROM contratos_publicos GROUP BY orgao_contratante ON CONFLICT(uuid) DO NOTHING;
        WITH projection_edges(source,target,relation,cents,year,event,origin,evidence) AS (SELECT 'orgao_'||orgao_contratante,'cnpj_'||substr(REPLACE(REPLACE(REPLACE(fornecedor_cnpj,'.',''),'/',''),'-',''),1,8),'CONTRATO',valor_contratado_centavos,CAST(substr(data_assinatura,1,4) AS INTEGER),data_assinatura,'PNCP',json_object('projection','radar','contrato_id',id)
        FROM contratos_publicos WHERE length(REPLACE(REPLACE(REPLACE(fornecedor_cnpj,'.',''),'/',''),'-',''))=14 AND valid_document(fornecedor_cnpj)) INSERT INTO conexoes_rede(origem_id,destino_id,tipo_relacao,valor,ano,data_evento,fonte_dado,metadata_json) SELECT s.id,t.id,e.relation,e.cents/100.0,COALESCE(e.year,0),e.event,e.origin,e.evidence FROM projection_edges e JOIN nos_rede s ON s.uuid=e.source JOIN nos_rede t ON t.uuid=e.target;
    "#)?;
    let count = tx.query_row("SELECT COUNT(*) FROM conexoes_rede WHERE json_extract(metadata_json,'$.projection')='radar'",[],|r|r.get(0))?;
    tx.execute_batch("DROP TABLE temp.projection_roots")?;
    tx.execute(
        "UPDATE projection_state SET dirty=0,version=2,relations=?1 WHERE id=1",
        [count],
    )?;
    Ok(count)
}

/// Carrega um QSA isolado sem varrer a base nacional. Limite explícito por raiz.
pub fn ensure_company(conn: &Connection, root: &str) -> Result<()> {
    if root.len() != 8 || !root.bytes().all(|b| b.is_ascii_digit()) {
        return Ok(());
    }
    let tx = storage::transaction_immediate_ref(conn)?;
    let uuid = format!("cnpj_{root}");
    let loaded:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM conexoes_rede e JOIN nos_rede n ON n.id=e.destino_id WHERE n.uuid=?1 AND e.tipo_relacao='SOCIO_ADMINISTRADOR')",[&uuid],|r|r.get(0))?;
    if loaded {
        tx.commit()?;
        return Ok(());
    }
    let mut stmt=tx.prepare("SELECT socio_nome,socio_cpf_cnpj_mascarado,razao_social FROM empresas_qsa WHERE cnpj_basico=?1 LIMIT 10001")?;
    let mut rows = stmt.query([root])?;
    let mut count = 0;
    while let Some(row) = rows.next()? {
        count += 1;
        if count > 10000 {
            return Err(storage::StorageError::Pool(
                "QSA excede 10000 sócios".into(),
            ));
        }
        let name: String = row.get(0)?;
        let doc: String = row.get(1)?;
        let company: String = row.get(2)?;
        tx.execute("INSERT INTO nos_rede(uuid,tipo,documento,nome,metadata_json) VALUES (?1,'EMPRESA',?2,?3,json_object('projection','qsa_root')) ON CONFLICT(uuid) DO NOTHING",storage::rusqlite::params![uuid,root,company])?;
        let complete = crate::identity::complete(&doc);
        let partner = if complete {
            let digits = crate::identity::digits(&doc);
            if digits.len() == 14 {
                format!("cnpj_{}", &digits[..8])
            } else {
                format!("doc_{digits}")
            }
        } else {
            format!("socio_{root}_{doc}_{name}")
        };
        let kind = if crate::identity::digits(&doc).len() == 14 {
            "EMPRESA"
        } else {
            "PESSOA_FISICA"
        };
        let confidence = if complete { "CONFIRMADO" } else { "POSSIVEL" };
        tx.execute("INSERT INTO nos_rede(uuid,tipo,documento,nome,metadata_json) VALUES (?1,?2,?3,?4,json_object('projection','qsa_root','confianca',?5)) ON CONFLICT(uuid) DO NOTHING",storage::rusqlite::params![partner,kind,doc,name,confidence])?;
        tx.execute("INSERT INTO conexoes_rede(origem_id,destino_id,tipo_relacao,valor,ano,fonte_dado,metadata_json) SELECT s.id,t.id,'SOCIO_ADMINISTRADOR',0,0,'RECEITA_FEDERAL',json_object('projection','qsa_root','confianca',?3) FROM nos_rede s JOIN nos_rede t ON t.uuid=?2 WHERE s.uuid=?1",storage::rusqlite::params![partner,uuid,confidence])?;
    }
    drop(rows);
    drop(stmt);
    tx.commit()?;
    Ok(())
}
