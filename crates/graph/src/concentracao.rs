//! Concentração financeira por empresa, com consultas indexadas e entrega incremental.
use storage::rusqlite::Connection;

#[derive(Debug, Clone)]
pub struct FornecedorConcentrado {
    pub id: i64,
    pub uuid: String,
    pub nome: String,
    pub documento: Option<String>,
    pub total_faturamento: f64,
    pub grupo: String,
    pub faturamento_grupo: f64,
    pub percentual: f64,
    pub agentes: usize,
}

/// Não materializa o grafo nem calcula centralidade. A memória usada pelas
/// agregações depende dos pagadores da empresa corrente. Grupos desconhecidos
/// entram no denominador, mas não constituem evidência de um grupo político.
pub fn visitar_fornecedores_concentrados(
    conn: &Connection,
    limite: f64,
    mut visitar: impl FnMut(FornecedorConcentrado) -> storage::Result<()>,
) -> crate::Result<()> {
    if !limite.is_finite() || !(0.0..=100.0).contains(&limite) {
        return Err(crate::GraphError::General("Concentração inválida".into()));
    }
    let mut empresas = conn.prepare("SELECT n.id,n.uuid,n.nome,n.documento FROM nos_rede n WHERE n.tipo='EMPRESA' AND EXISTS(SELECT 1 FROM conexoes_rede e WHERE e.destino_id=n.id AND e.valor_centavos>0) ORDER BY n.id")?;
    let mut totais = conn.prepare_cached("SELECT COALESCE(SUM(valor_centavos),0),COUNT(DISTINCT origem_id) FROM conexoes_rede WHERE destino_id=?1 AND valor_centavos>0 AND tipo_relacao IN ('DESPESA','DESPESA_CAMPANHA','CEAP','CONTRATO')")?;
    let mut grupos = conn.prepare_cached(
        "SELECT COALESCE(NULLIF(TRIM(CASE WHEN json_valid(n.metadata_json) THEN json_extract(n.metadata_json,'$.coligacao') END),''),NULLIF(TRIM(CASE WHEN json_valid(n.metadata_json) THEN json_extract(n.metadata_json,'$.partido') END),''),
             (SELECT CASE WHEN COUNT(DISTINCT NULLIF(TRIM(c.sigla_partido),''))=1 THEN MIN(NULLIF(TRIM(c.sigla_partido),'')) END
              FROM candidaturas c WHERE c.ano_eleicao=e.ano AND c.politico_id=CASE WHEN n.tipo='POLITICO' AND json_valid(n.metadata_json) THEN json_extract(n.metadata_json,'$.politico_id') END)) AS grupo,SUM(e.valor_centavos)
         FROM conexoes_rede e JOIN nos_rede n ON n.id=e.origem_id
         WHERE e.destino_id=?1 AND e.valor_centavos>0 AND e.tipo_relacao IN ('DESPESA','DESPESA_CAMPANHA','CEAP','CONTRATO')
         GROUP BY 1 HAVING grupo IS NOT NULL AND UPPER(grupo) NOT IN ('GRUPO_INDEFINIDO','SEM PARTIDO','#NULO','#NE') ORDER BY 2 DESC,1 LIMIT 1",
    )?;
    let mut rows = empresas.query([])?;
    while let Some(row) = rows.next()? {
        let id = row.get::<_, i64>(0)?;
        let (centavos, agentes): (i64, usize) =
            totais.query_row([id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        if centavos <= 0 || agentes < 2 {
            continue;
        }
        let mut resultados = grupos.query([id])?;
        let Some(grupo) = resultados.next()? else {
            continue;
        };
        let nome_grupo: Option<String> = grupo.get(0)?;
        let Some(nome_grupo) = nome_grupo else {
            continue;
        };
        let grupo_centavos: i64 = grupo.get(1)?;
        let percentual = grupo_centavos as f64 / centavos as f64 * 100.0;
        if percentual >= limite {
            visitar(FornecedorConcentrado {
                id,
                uuid: row.get(1)?,
                nome: row.get(2)?,
                documento: row.get(3)?,
                total_faturamento: centavos as f64 / 100.0,
                grupo: nome_grupo,
                faturamento_grupo: grupo_centavos as f64 / 100.0,
                percentual,
                agentes,
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn preparar() -> storage::DbPool {
        let pool = storage::DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        storage::run_migrations(&mut conn).unwrap();
        conn.execute_batch("INSERT INTO nos_rede(id,uuid,tipo,nome,metadata_json) VALUES(1,'pagador-a','POLITICO','A','{\"coligacao\":\"GRUPO A\"}'),(2,'pagador-b','POLITICO','B','{\"coligacao\":\"GRUPO A\"}'),(3,'pagador-c','POLITICO','C','{\"coligacao\":\"GRUPO B\"}'),(4,'empresa','EMPRESA','EMPRESA',NULL);
        INSERT INTO conexoes_rede(origem_id,destino_id,tipo_relacao,valor,ano,fonte_dado) VALUES(1,4,'DESPESA',45000.01,2024,'TSE'),(2,4,'DESPESA',45000.02,2024,'TSE'),(3,4,'DESPESA',9999.97,2024,'TSE');").unwrap();
        drop(conn);
        pool
    }
    #[test]
    fn agregacao_sql_equivale_a_rede_completa_sem_exigir_centralidade() {
        let pool = preparar();
        let conn = pool.get().unwrap();
        let mut sql = Vec::new();
        visitar_fornecedores_concentrados(&conn, 70.0, |a| {
            sql.push(a);
            Ok(())
        })
        .unwrap();
        let rede = crate::GrafoSincronizado::carregar_do_sqlite(&conn).unwrap();
        let referencia = rede.detectar_fornecedores_hub_com_limite(70.0).unwrap();
        assert_eq!(sql.len(), referencia.len());
        assert_eq!(sql[0].total_faturamento, 100000.0);
        assert_eq!(sql[0].faturamento_grupo, referencia[0].faturamento_grupo);
        assert_eq!(sql[0].agentes, 3);
        assert!(visitar_fornecedores_concentrados(&conn, 70.0, |_| Err(
            storage::StorageError::Pool("falha de escrita".into())
        ))
        .is_err());
    }
    #[test]
    fn grupo_desconhecido_nao_gera_concentracao_artificial() {
        let pool = preparar();
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE nos_rede SET metadata_json=NULL WHERE tipo='POLITICO'",
            [],
        )
        .unwrap();
        let mut encontrados = 0;
        visitar_fornecedores_concentrados(&conn, 70.0, |_| {
            encontrados += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(encontrados, 0);
    }
}

#[cfg(test)]
mod projected_groups {
    #[test]
    fn usa_partido_da_candidatura_do_ano_sem_multiplicar_arestas() {
        let pool = storage::DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        storage::run_migrations(&mut conn).unwrap();
        conn.execute_batch("INSERT INTO politicos(id,nome_completo,nome_urna) VALUES(1,'A','A'),(2,'B','B'); INSERT INTO candidaturas(politico_id,ano_eleicao,cargo,sigla_partido,uf) VALUES(1,2024,'PREFEITO','ABC','SP'),(2,2024,'PREFEITO','ABC','SP'); INSERT INTO nos_rede(id,uuid,tipo,nome,metadata_json) VALUES(1,'politico_1','POLITICO','A','{\"politico_id\":1}'),(2,'politico_2','POLITICO','B','{\"politico_id\":2}'),(3,'empresa','EMPRESA','EMPRESA',NULL); INSERT INTO conexoes_rede(origem_id,destino_id,tipo_relacao,valor,ano,fonte_dado) VALUES(1,3,'DESPESA_CAMPANHA',50000,2024,'TSE'),(2,3,'DESPESA_CAMPANHA',50000,2024,'TSE');").unwrap();
        let mut encontrados = Vec::new();
        super::visitar_fornecedores_concentrados(&conn, 70.0, |a| {
            encontrados.push(a);
            Ok(())
        })
        .unwrap();
        assert_eq!(encontrados.len(), 1);
        assert_eq!(encontrados[0].grupo, "ABC");
        assert_eq!(encontrados[0].total_faturamento, 100000.0);
        conn.execute("INSERT INTO candidaturas(politico_id,ano_eleicao,cargo,sigla_partido,uf) VALUES(1,2024,'VEREADOR','XYZ','SP')",[]).unwrap();
        let mut quantidade = 0;
        super::visitar_fornecedores_concentrados(&conn, 70.0, |_| {
            quantidade += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(quantidade, 0);
    }
}
