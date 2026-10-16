//! Identidade estável da CEAP e retificações preservando a linha e seu histórico.
use crate::{
    rusqlite::{params, Connection, OptionalExtension},
    NovaDespesaParlamentar, Result, StorageError,
};

pub fn identificador_ceap(
    documento: &str,
    ressarcimento: &str,
    parcela: &str,
    lote: &str,
) -> Option<String> {
    let documento = documento.trim().parse::<u64>().ok().filter(|id| *id > 0)?;
    let normalize = |s: &str| s.trim().parse::<u64>().unwrap_or(0);
    Some(
        serde_json::json!([
            documento,
            normalize(ressarcimento),
            normalize(parcela),
            normalize(lote)
        ])
        .to_string(),
    )
}

/// Chamar dentro da transação do arquivo/lote. Retorna ID e número de linhas novas/retificadas.
pub fn upsert_despesa_parlamentar(
    conn: &Connection,
    d: &NovaDespesaParlamentar,
) -> Result<(i64, usize)> {
    let valor = crate::Money::from_reais(d.valor_liquido)?.reais();
    if let Some(ref key) = d.id_origem {
        let exists:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM despesas_parlamentares WHERE casa_legislativa=?1 AND id_origem=?2)",params![d.casa_legislativa,key],|r|r.get(0))?;
        if !exists {
            if let Some(numero) = d
                .numero_documento
                .as_deref()
                .filter(|n| !n.trim().is_empty())
            {
                let mut stmt=conn.prepare("SELECT id FROM despesas_parlamentares WHERE id_origem IS NULL AND casa_legislativa=?1 AND parlamentar_nome=?2 AND data_emissao=?3 AND fornecedor_cnpj_cpf=?4 AND numero_documento=?5 LIMIT 2")?;
                let candidates = stmt
                    .query_map(
                        params![
                            d.casa_legislativa,
                            d.parlamentar_nome,
                            d.data_emissao,
                            d.fornecedor_cnpj_cpf,
                            numero
                        ],
                        |r| r.get::<_, i64>(0),
                    )?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if candidates.len() > 1 {
                    return Err(StorageError::Pool(format!("CEAP {key}: registros legados ambíguos; reconciliação explícita necessária")));
                }
                if let Some(id) = candidates.first() {
                    conn.execute(
                        "UPDATE despesas_parlamentares SET id_origem=?2 WHERE id=?1",
                        params![id, key],
                    )?;
                }
            }
        }
        let changes=conn.execute(
            "INSERT INTO despesas_parlamentares(casa_legislativa,parlamentar_nome,parlamentar_cpf_mascarado,data_emissao,categoria_despesa,fornecedor_nome,fornecedor_cnpj_cpf,valor_liquido,numero_documento,url_nota_fiscal,detalhes_litros,id_origem)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
             ON CONFLICT(casa_legislativa,id_origem) WHERE id_origem IS NOT NULL DO UPDATE SET
                parlamentar_nome=excluded.parlamentar_nome,parlamentar_cpf_mascarado=excluded.parlamentar_cpf_mascarado,data_emissao=excluded.data_emissao,categoria_despesa=excluded.categoria_despesa,fornecedor_nome=excluded.fornecedor_nome,fornecedor_cnpj_cpf=excluded.fornecedor_cnpj_cpf,valor_liquido=excluded.valor_liquido,numero_documento=excluded.numero_documento,url_nota_fiscal=excluded.url_nota_fiscal,detalhes_litros=excluded.detalhes_litros
             WHERE parlamentar_nome IS NOT excluded.parlamentar_nome OR parlamentar_cpf_mascarado IS NOT excluded.parlamentar_cpf_mascarado OR data_emissao IS NOT excluded.data_emissao OR categoria_despesa IS NOT excluded.categoria_despesa OR fornecedor_nome IS NOT excluded.fornecedor_nome OR fornecedor_cnpj_cpf IS NOT excluded.fornecedor_cnpj_cpf OR valor_liquido IS NOT excluded.valor_liquido OR numero_documento IS NOT excluded.numero_documento OR url_nota_fiscal IS NOT excluded.url_nota_fiscal OR detalhes_litros IS NOT excluded.detalhes_litros",
            params![d.casa_legislativa,d.parlamentar_nome,d.parlamentar_cpf_mascarado,d.data_emissao,d.categoria_despesa,d.fornecedor_nome,d.fornecedor_cnpj_cpf,valor,d.numero_documento,d.url_nota_fiscal,d.detalhes_litros,key],
        )?;
        let id = conn.query_row(
            "SELECT id FROM despesas_parlamentares WHERE casa_legislativa=?1 AND id_origem=?2",
            params![d.casa_legislativa, key],
            |r| r.get(0),
        )?;
        return Ok((id, changes));
    }
    let changes=conn.execute("INSERT OR IGNORE INTO despesas_parlamentares(casa_legislativa,parlamentar_nome,parlamentar_cpf_mascarado,data_emissao,categoria_despesa,fornecedor_nome,fornecedor_cnpj_cpf,valor_liquido,numero_documento,url_nota_fiscal,detalhes_litros) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![d.casa_legislativa,d.parlamentar_nome,d.parlamentar_cpf_mascarado,d.data_emissao,d.categoria_despesa,d.fornecedor_nome,d.fornecedor_cnpj_cpf,valor,d.numero_documento,d.url_nota_fiscal,d.detalhes_litros])?;
    let id=conn.query_row("SELECT id FROM despesas_parlamentares WHERE id_origem IS NULL AND casa_legislativa=?1 AND parlamentar_nome=?2 AND data_emissao=?3 AND fornecedor_cnpj_cpf=?4 AND valor_liquido=?5 AND COALESCE(numero_documento,'')=COALESCE(?6,'')",params![d.casa_legislativa,d.parlamentar_nome,d.data_emissao,d.fornecedor_cnpj_cpf,valor,d.numero_documento],|r|r.get(0)).optional()?.ok_or_else(||StorageError::Pool("CEAP não gravada".into()))?;
    Ok((id, changes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retificacao_preserva_id_historico_e_distingue_documentos_identicos() {
        let pool = crate::DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        crate::run_migrations(&mut conn).unwrap();
        let mut d = NovaDespesaParlamentar {
            id_origem: None,
            casa_legislativa: "CAMARA".into(),
            parlamentar_nome: "MARIA".into(),
            parlamentar_cpf_mascarado: None,
            data_emissao: "2024-01-01".into(),
            categoria_despesa: "SERVICOS".into(),
            fornecedor_nome: "EMPRESA".into(),
            fornecedor_cnpj_cpf: "11222333000181".into(),
            valor_liquido: 100.0,
            numero_documento: Some("NF1".into()),
            url_nota_fiscal: None,
            detalhes_litros: None,
        };
        let (original, _) = upsert_despesa_parlamentar(&conn, &d).unwrap();
        d.id_origem = identificador_ceap("123", "0", "0", "0");
        d.valor_liquido = 90.0;
        assert_eq!(
            upsert_despesa_parlamentar(&conn, &d).unwrap(),
            (original, 1)
        );
        assert_eq!(
            upsert_despesa_parlamentar(&conn, &d).unwrap(),
            (original, 0)
        );
        let values: (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*),SUM(valor_liquido_centavos) FROM despesas_parlamentares",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(values, (1, 9000));
        let history:(f64,f64)=conn.query_row("SELECT json_extract(anterior_json,'$.valor_liquido'),json_extract(atual_json,'$.valor_liquido') FROM despesas_parlamentares_versoes",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(history, (100.0, 90.0));
        {
            let tx = crate::transaction_immediate(&mut conn).unwrap();
            d.valor_liquido = 80.0;
            upsert_despesa_parlamentar(&tx, &d).unwrap();
        }
        assert_eq!(
            conn.query_row(
                "SELECT valor_liquido FROM despesas_parlamentares WHERE id=?1",
                [original],
                |r| r.get::<_, f64>(0)
            )
            .unwrap(),
            90.0
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM despesas_parlamentares_versoes",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        d.valor_liquido = 90.0;
        d.id_origem = identificador_ceap("456", "0", "0", "0");
        assert_ne!(upsert_despesa_parlamentar(&conn, &d).unwrap().0, original);
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM despesas_parlamentares", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
    }
}
