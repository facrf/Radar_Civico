//! Execução das regras que exigem evidência temporal e identidade completas.
use crate::alertas::{registrar_alerta, NovoAlerta};
use chrono::{DateTime, NaiveDate, Utc};
use storage::{
    rusqlite::{params, Connection, OptionalExtension},
    Result,
};

fn date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.get(..10)?, "%Y-%m-%d").ok()
}
fn save(conn: &Connection, mut alerta: NovoAlerta, key: String, revision: &str) -> Result<()> {
    let mut details: serde_json::Value =
        serde_json::from_str(alerta.detalhes_json.as_deref().unwrap_or("{}"))
            .map_err(|e| storage::StorageError::Pool(e.to_string()))?;
    details["chave_analitica"] = key.clone().into();
    details["revisao_motor"] = revision.into();
    alerta.detalhes_json = Some(details.to_string());
    let existing: Option<i64> = conn.query_row("SELECT id FROM alertas_auditoria WHERE tipo=?1 AND json_extract(detalhes_json,'$.chave_analitica')=?2", params![alerta.tipo,key],|r|r.get(0)).optional()?;
    if let Some(id) = existing {
        conn.execute("UPDATE alertas_auditoria SET severidade=?2,titulo=?3,descricao=?4,alvo_nome=?5,alvo_documento=?6,ano=?7,valor_envolvido=?8,fonte_dado=?9,detalhes_json=?10 WHERE id=?1",params![id,alerta.severidade,alerta.titulo,alerta.descricao,alerta.alvo_nome,alerta.alvo_documento,alerta.ano,alerta.valor_envolvido,alerta.fonte_dado,alerta.detalhes_json])?;
    } else {
        registrar_alerta(conn, &alerta)?;
    }
    Ok(())
}

pub fn run(conn: &Connection, window: i64) -> Result<()> {
    let revision = uuid::Uuid::new_v4().to_string();
    let donor = crate::identity::document_sql("r.doador_cpf_cnpj");
    let partner = crate::identity::document_sql("q.socio_cpf_cnpj_mascarado");
    let supplier = crate::identity::document_sql("ct.fornecedor_cnpj");
    let sql = format!(
        r#"SELECT DISTINCT r.id,r.doador_cpf_cnpj,r.doador_nome,r.valor,c.politico_id,p.nome_completo,a.data_posse,q.socio_nome,ct.fornecedor_cnpj,q.razao_social,ct.id,ct.orgao_contratante,ct.valor_contratado,ct.objeto,ct.data_assinatura
        FROM receitas_campanha r JOIN candidaturas c ON c.id=r.candidatura_id JOIN politicos p ON p.id=c.politico_id
        JOIN cargos_autoridades a ON a.politico_id=p.id
        JOIN empresas_qsa q ON {partner}={donor}
        JOIN contratos_publicos ct ON substr({supplier},1,8)=q.cnpj_basico AND TRIM(UPPER(ct.orgao_contratante))=TRIM(UPPER(a.orgao))
        WHERE length({donor}) IN (11,14) AND {donor} NOT GLOB '*[^0-9]*' AND length({supplier})=14 AND {supplier} NOT GLOB '*[^0-9]*'
          AND UPPER(ct.modalidade) IN ('INEXIGIBILIDADE','INEXIGIBILIDADE DE LICITACAO','INEXIGIBILIDADE DE LICITAÇÃO')
          AND date(r.data_receita)<=date(a.data_posse) AND c.ano_eleicao<=CAST(substr(a.data_posse,1,4) AS INTEGER)
          AND (a.data_exoneracao IS NULL OR a.data_exoneracao='' OR date(ct.data_assinatura)<=date(a.data_exoneracao))
          AND julianday(ct.data_assinatura)-julianday(a.data_posse)>=0 AND julianday(ct.data_assinatura)-julianday(a.data_posse)<?1"#
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([window])?;
    while let Some(row) = rows.next()? {
        let receipt_id: i64 = row.get(0)?;
        let Some(posse) = date(&row.get::<_, String>(6)?) else {
            continue;
        };
        let Some(signed) = date(&row.get::<_, String>(14)?) else {
            continue;
        };
        let donor = auditor::DoadorCampanha {
            doador_cpf_cnpj: row.get(1)?,
            doador_nome: row.get(2)?,
            valor_doado: row.get(3)?,
            politico_id: row.get(4)?,
            politico_nome: row.get(5)?,
            data_posse: posse,
        };
        let partner = auditor::SocioEmpresa {
            socio_cpf_cnpj: donor.doador_cpf_cnpj.clone(),
            socio_nome: row.get(7)?,
            empresa_cnpj: row.get(8)?,
            empresa_razao_social: row.get(9)?,
        };
        let contract = auditor::ContratoPublico {
            id: row.get(10)?,
            orgao_contratante: row.get(11)?,
            valor_contratado: row.get(12)?,
            objeto: row.get(13)?,
            data_assinatura: signed,
            fornecedor_cnpj: partner.empresa_cnpj.clone(),
        };
        for alert in auditor::auditar_triangulacao_com_janela(
            std::slice::from_ref(&donor),
            &[partner],
            &[contract],
            window,
        ) {
            let key =
                serde_json::json!([receipt_id, donor.politico_id, alert.contrato_id]).to_string();
            let details = serde_json::json!({"receita_id":receipt_id,"politico_id":donor.politico_id,"contrato_id":alert.contrato_id,"data_posse":posse.to_string(),"dias_apos_posse":alert.dias_apos_posse,"modalidade":"INEXIGIBILIDADE","confianca":"CONFIRMADO","conclusao":"Indício para verificação, não comprovação de irregularidade"});
            save(
                conn,
                NovoAlerta {
                    tipo: "TRIANGULACAO".into(),
                    severidade: "ALTA".into(),
                    titulo: format!(
                        "Doador vinculado a contrato após posse — {}",
                        donor.politico_nome
                    ),
                    descricao: alert.motivo,
                    alvo_nome: donor.politico_nome.clone(),
                    alvo_documento: None,
                    municipio: None,
                    uf: None,
                    ano: Some(chrono::Datelike::year(&posse)),
                    valor_envolvido: Some(alert.valor_contrato),
                    fonte_dado: "TSE/PNCP/RECEITA_FEDERAL/ATO_DE_POSSE".into(),
                    detalhes_json: Some(details.to_string()),
                },
                key,
                &revision,
            )?;
        }
    }
    drop(rows);
    drop(stmt);
    let mut stmt=conn.prepare("SELECT d.id,d.parlamentar_id,p.nome_completo,d.emitido_em,d.municipio_emissao,d.uf_emissao,d.latitude_emissao,d.longitude_emissao,d.valor_liquido,d.categoria_despesa FROM despesas_parlamentares d JOIN politicos p ON p.id=d.parlamentar_id WHERE d.presencial=1 AND d.latitude_emissao BETWEEN -90 AND 90 AND d.longitude_emissao BETWEEN -180 AND 180 AND d.municipio_emissao IS NOT NULL AND d.uf_emissao IS NOT NULL ORDER BY d.parlamentar_id,julianday(d.emitido_em),d.id")?;
    let mut rows = stmt.query([])?;
    let mut previous: Option<(i64, auditor::DespesaPresencial)> = None;
    while let Some(row) = rows.next()? {
        let Ok(timestamp) = DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?) else {
            continue;
        };
        let person: i64 = row.get(1)?;
        let current = auditor::DespesaPresencial {
            id: row.get(0)?,
            parlamentar_nome: row.get(2)?,
            timestamp: timestamp.with_timezone(&Utc),
            municipio: row.get(4)?,
            uf: row.get(5)?,
            latitude: row.get(6)?,
            longitude: row.get(7)?,
            valor: row.get(8)?,
            descricao: row.get(9)?,
        };
        if let Some((id, ref prior)) = previous {
            if id == person {
                for alert in auditor::auditar_ubiquidade(&[prior.clone(), current.clone()]) {
                    let key = serde_json::json!([
                        person,
                        alert.despesa_origem_id,
                        alert.despesa_destino_id
                    ])
                    .to_string();
                    let details = serde_json::json!({"politico_id":person,"despesa_origem_id":alert.despesa_origem_id,"despesa_destino_id":alert.despesa_destino_id,"distancia_km":alert.distancia_km,"tempo_horas":alert.tempo_horas,"velocidade_kmh":if alert.velocidade_kmh.is_finite() {Some(alert.velocidade_kmh)} else {None},"horarios":[prior.timestamp.to_rfc3339(),current.timestamp.to_rfc3339()],"confianca":"CONFIRMADO"});
                    save(
                        conn,
                        NovoAlerta {
                            tipo: "UBIQUIDADE".into(),
                            severidade: "ALTA".into(),
                            titulo: format!(
                                "Despesas presenciais incompatíveis — {}",
                                current.parlamentar_nome
                            ),
                            descricao: alert.motivo,
                            alvo_nome: current.parlamentar_nome.clone(),
                            alvo_documento: None,
                            municipio: Some(current.municipio.clone()),
                            uf: Some(current.uf.clone()),
                            ano: Some(chrono::Datelike::year(&current.timestamp)),
                            valor_envolvido: Some(prior.valor + current.valor),
                            fonte_dado: "CEAP/NOTA_FISCAL".into(),
                            detalhes_json: Some(details.to_string()),
                        },
                        key,
                        &revision,
                    )?;
                }
            }
        }
        previous = Some((person, current));
    }
    conn.execute("DELETE FROM alertas_auditoria WHERE tipo IN ('TRIANGULACAO','UBIQUIDADE') AND json_extract(detalhes_json,'$.revisao_motor') IS NOT NULL AND json_extract(detalhes_json,'$.revisao_motor')<>?1",[revision])?;
    Ok(())
}
