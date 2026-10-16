use serde_json::{json, Value};
use std::collections::HashSet;
use storage::{
    rusqlite::{params, Connection, OptionalExtension},
    Result,
};

fn node(
    conn: &Connection,
    uuid: &str,
    kind: &str,
    doc: Option<&str>,
    name: &str,
    metadata: Value,
) -> Result<i64> {
    conn.execute("INSERT INTO nos_rede(uuid,tipo,documento,nome,metadata_json) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(uuid) DO UPDATE SET documento=excluded.documento,nome=excluded.nome,metadata_json=excluded.metadata_json WHERE documento IS NOT excluded.documento OR nome IS NOT excluded.nome OR metadata_json IS NOT excluded.metadata_json",params![uuid,kind,doc,name,metadata.to_string()])?;
    Ok(
        conn.query_row("SELECT id FROM nos_rede WHERE uuid=?1", [uuid], |r| {
            r.get(0)
        })?,
    )
}
fn politician(conn: &Connection, id: i64) -> Result<Option<i64>> {
    let person: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT nome_completo,cpf_mascarado FROM politicos WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    person
        .map(|(name, doc)| {
            node(
                conn,
                &format!("politico_{id}"),
                "POLITICO",
                doc.as_deref(),
                &name,
                json!({"projection":"radar","politico_id":id}),
            )
        })
        .transpose()
}
fn document(
    conn: &Connection,
    doc: &str,
    name: &str,
    roots: &mut HashSet<String>,
) -> Result<Option<i64>> {
    if !crate::identity::complete(doc) {
        return Ok(None);
    }
    let digits = crate::identity::digits(doc);
    if digits.len() == 14 {
        let root = &digits[..8];
        roots.insert(root.to_string());
        Ok(Some(node(
            conn,
            &format!("cnpj_{root}"),
            "EMPRESA",
            Some(root),
            name,
            json!({"projection":"radar"}),
        )?))
    } else {
        Ok(Some(node(
            conn,
            &format!("doc_{digits}"),
            "PESSOA_FISICA",
            Some(&digits),
            name,
            json!({"projection":"radar","confianca":"CONFIRMADO"}),
        )?))
    }
}
struct Edge<'a> {
    source: i64,
    target: i64,
    kind: &'a str,
    cents: i64,
    year: i32,
    date: Option<String>,
    origin: &'a str,
    details: Value,
}
fn edge(conn: &Connection, e: Edge<'_>) -> Result<()> {
    conn.execute("INSERT INTO conexoes_rede(origem_id,destino_id,tipo_relacao,valor,ano,data_evento,fonte_dado,metadata_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![e.source,e.target,e.kind,e.cents as f64/100.0,e.year,e.date,e.origin,e.details.to_string()])?;
    Ok(())
}
fn remove_event(conn: &Connection, kind: &str, id: i64) -> Result<i64> {
    Ok(conn.execute("DELETE FROM conexoes_rede WHERE tipo_relacao=?1 AND COALESCE(json_extract(metadata_json,'$.receita_id'),json_extract(metadata_json,'$.despesa_id'),json_extract(metadata_json,'$.contrato_id'))=?2 AND json_extract(metadata_json,'$.projection')='radar'",params![kind,id])? as i64)
}
fn qsa(conn: &Connection, root: &str) -> Result<i64> {
    let company: Option<i64> = conn
        .query_row(
            "SELECT id FROM nos_rede WHERE uuid=?1",
            [format!("cnpj_{root}")],
            |r| r.get(0),
        )
        .optional()?;
    let Some(company) = company else { return Ok(0) };
    let old:i64=conn.query_row("SELECT COUNT(*) FROM conexoes_rede WHERE destino_id=?1 AND tipo_relacao='SOCIO_ADMINISTRADOR' AND json_extract(metadata_json,'$.projection')='radar'",[company],|r|r.get(0))?;
    conn.execute("DELETE FROM conexoes_rede WHERE destino_id=?1 AND tipo_relacao='SOCIO_ADMINISTRADOR' AND json_extract(metadata_json,'$.projection') IN ('radar','qsa_root')",[company])?;
    let mut stmt=conn.prepare("SELECT socio_nome,socio_cpf_cnpj_mascarado,razao_social FROM empresas_qsa WHERE cnpj_basico=?1")?;
    let mut rows = stmt.query([root])?;
    let mut added = 0;
    while let Some(row) = rows.next()? {
        let name: String = row.get(0)?;
        let doc: String = row.get(1)?;
        let company_name: String = row.get(2)?;
        conn.execute(
            "UPDATE nos_rede SET nome=?2 WHERE id=?1 AND nome IS NOT ?2",
            params![company, company_name],
        )?;
        let mut ignored = HashSet::new();
        let partner = if let Some(partner) = document(conn, &doc, &name, &mut ignored)? {
            partner
        } else {
            node(
                conn,
                &format!("socio_{root}_{doc}_{name}"),
                "PESSOA_FISICA",
                Some(&doc),
                &name,
                json!({"projection":"radar","confianca":"POSSIVEL"}),
            )?
        };
        edge(
            conn,
            Edge {
                source: partner,
                target: company,
                kind: "SOCIO_ADMINISTRADOR",
                cents: 0,
                year: 0,
                date: None,
                origin: "RECEITA_FEDERAL",
                details: json!({"projection":"radar","confianca":if crate::identity::complete(&doc){"CONFIRMADO"}else{"POSSIVEL"}}),
            },
        )?;
        added += 1;
    }
    Ok(added - old)
}

pub(super) fn batch(conn: &Connection, limit: usize) -> Result<(usize, i64)> {
    let mut stmt = conn.prepare(
        "SELECT kind,record_key FROM projection_changes ORDER BY kind,record_key LIMIT ?1",
    )?;
    let changes = stmt
        .query_map([limit], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);
    let mut delta = 0;
    let mut roots = HashSet::new();
    for (kind, key) in &changes {
        if kind == "qsa" {
            roots.insert(key.clone());
            continue;
        }
        let id = key
            .parse::<i64>()
            .map_err(|e| storage::StorageError::Pool(e.to_string()))?;
        match kind.as_str() {
            "politico" => {
                if politician(conn, id)?.is_none() {
                    let uuid = format!("politico_{id}");
                    let node_id: Option<i64> = conn
                        .query_row("SELECT id FROM nos_rede WHERE uuid=?1", [&uuid], |r| {
                            r.get(0)
                        })
                        .optional()?;
                    if let Some(node_id) = node_id {
                        delta-=conn.execute("DELETE FROM conexoes_rede WHERE (origem_id=?1 OR destino_id=?1) AND json_extract(metadata_json,'$.projection')='radar'",[node_id])? as i64;
                        conn.execute("DELETE FROM nos_rede WHERE id=?1 AND NOT EXISTS(SELECT 1 FROM conexoes_rede WHERE origem_id=?1 OR destino_id=?1)",[node_id])?;
                    }
                }
            }
            "receita" | "despesa" => {
                let relation = if kind == "receita" {
                    "DOACAO"
                } else {
                    "DESPESA_CAMPANHA"
                };
                delta -= remove_event(conn, relation, id)?;
                let sql = if kind == "receita" {
                    "SELECT r.doador_cpf_cnpj,r.doador_nome,r.valor_centavos,c.politico_id,c.ano_eleicao,r.data_receita FROM receitas_campanha r JOIN candidaturas c ON c.id=r.candidatura_id WHERE r.id=?1"
                } else {
                    "SELECT d.fornecedor_cpf_cnpj,d.fornecedor_nome,d.valor_centavos,c.politico_id,c.ano_eleicao,d.data_despesa FROM despesas_campanha d JOIN candidaturas c ON c.id=d.candidatura_id WHERE d.id=?1"
                };
                let row = conn
                    .query_row(sql, [id], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, i64>(3)?,
                            r.get::<_, i32>(4)?,
                            r.get::<_, Option<String>>(5)?,
                        ))
                    })
                    .optional()?;
                if let Some((doc, name, cents, person, year, date)) = row {
                    if let (Some(doc), Some(person)) = (
                        document(conn, &doc, &name, &mut roots)?,
                        politician(conn, person)?,
                    ) {
                        let (source, target) = if kind == "receita" {
                            (doc, person)
                        } else {
                            (person, doc)
                        };
                        let mut details = json!({"projection":"radar"});
                        details[if kind == "receita" {
                            "receita_id"
                        } else {
                            "despesa_id"
                        }] = id.into();
                        edge(
                            conn,
                            Edge {
                                source,
                                target,
                                kind: relation,
                                cents,
                                year,
                                date,
                                origin: "TSE",
                                details,
                            },
                        )?;
                        delta += 1;
                    }
                }
            }
            "ceap" => {
                delta -= remove_event(conn, "CEAP", id)?;
                let row=conn.query_row("SELECT parlamentar_id,parlamentar_nome,fornecedor_cnpj_cpf,fornecedor_nome,valor_liquido_centavos,data_emissao FROM despesas_parlamentares WHERE id=?1",[id],|r|Ok((r.get::<_,Option<i64>>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,i64>(4)?,r.get::<_,String>(5)?))).optional()?;
                if let Some((explicit, name, doc, supplier, cents, date)) = row {
                    let person=match explicit { Some(p)=>Some(p),None=>conn.query_row("SELECT MIN(id) FROM politicos WHERE nome_completo=?1 OR nome_urna=?1 HAVING COUNT(*)=1",[&name],|r|r.get::<_,i64>(0)).optional()? };
                    if let Some(person) = person {
                        if let (Some(source), Some(target)) = (
                            politician(conn, person)?,
                            document(conn, &doc, &supplier, &mut roots)?,
                        ) {
                            let year = date.get(..4).and_then(|s| s.parse().ok()).unwrap_or(0);
                            edge(
                                conn,
                                Edge {
                                    source,
                                    target,
                                    kind: "CEAP",
                                    cents,
                                    year,
                                    date: Some(date),
                                    origin: "CAMARA",
                                    details: json!({"projection":"radar","despesa_id":id,"confianca":if explicit.is_some(){"CONFIRMADO"}else{"POSSIVEL"}}),
                                },
                            )?;
                            delta += 1;
                        }
                    }
                }
            }
            "contrato" => {
                delta -= remove_event(conn, "CONTRATO", id)?;
                let row=conn.query_row("SELECT orgao_contratante,fornecedor_cnpj,valor_contratado_centavos,data_assinatura FROM contratos_publicos WHERE id=?1",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,Option<String>>(3)?))).optional()?;
                if let Some((org, doc, cents, date)) = row {
                    if let Some(target) = document(conn, &doc, &doc, &mut roots)? {
                        let source = node(
                            conn,
                            &format!("orgao_{org}"),
                            "ORGAO",
                            None,
                            &org,
                            json!({"projection":"radar"}),
                        )?;
                        let year = date
                            .as_deref()
                            .and_then(|s| s.get(..4))
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        edge(
                            conn,
                            Edge {
                                source,
                                target,
                                kind: "CONTRATO",
                                cents,
                                year,
                                date,
                                origin: "PNCP",
                                details: json!({"projection":"radar","contrato_id":id}),
                            },
                        )?;
                        delta += 1;
                    }
                }
            }
            _ => {
                return Err(storage::StorageError::Pool(format!(
                    "Evento de projeção desconhecido: {kind}"
                )))
            }
        }
    }
    for root in roots {
        delta += qsa(conn, &root)?;
    }
    for (kind, key) in &changes {
        conn.execute(
            "DELETE FROM projection_changes WHERE kind=?1 AND record_key=?2",
            params![kind, key],
        )?;
    }
    conn.execute("UPDATE projection_state SET relations=MAX(0,relations+?1),version=3,dirty=EXISTS(SELECT 1 FROM projection_changes) WHERE id=1",[delta])?;
    Ok((
        changes.len(),
        conn.query_row(
            "SELECT relations FROM projection_state WHERE id=1",
            [],
            |r| r.get(0),
        )?,
    ))
}
