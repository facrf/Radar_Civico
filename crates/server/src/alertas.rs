use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use storage::rusqlite::{params, Connection, ToSql};
use storage::DbPool;

#[derive(Debug, Clone, Deserialize)]
pub struct AlertasQueryParams {
    pub municipio: Option<String>,
    pub ano: Option<i32>,
    pub severidade: Option<String>,
    pub tipo: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NovoAlerta {
    pub tipo: String,
    pub severidade: String,
    pub titulo: String,
    pub descricao: String,
    pub alvo_nome: String,
    pub alvo_documento: Option<String>,
    pub municipio: Option<String>,
    pub uf: Option<String>,
    pub ano: Option<i32>,
    pub valor_envolvido: Option<f64>,
    pub fonte_dado: String,
    pub detalhes_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaItem {
    pub id: i64,
    pub tipo: String,
    pub severidade: String,
    pub titulo: String,
    pub descricao: String,
    pub alvo_nome: String,
    pub alvo_documento: Option<String>,
    pub municipio: Option<String>,
    pub uf: Option<String>,
    pub ano: Option<i32>,
    pub valor_envolvido: Option<f64>,
    pub fonte_dado: String,
    pub detalhes: Option<serde_json::Value>,
    pub data_criacao: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertasResponse {
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub alertas: Vec<AlertaItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuxilioIndevidoQueryParams {
    pub politico_id: Option<i64>,
    pub motivo: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaAuxilioResponseItem {
    pub id: i64,
    pub politico_id: i64,
    pub politico_nome: String,
    pub cpf_mascarado: String,
    pub beneficio_id: Option<i64>,
    pub motivo: String,
    pub detalhes: Option<String>,
    pub valor_recebido: f64,
    pub total_bens: Option<f64>,
    pub cargo_ou_mandato: Option<String>,
    pub ano_exercicio: Option<i32>,
    pub status_analise: String,
    pub mes_disponibilizacao: Option<String>,
    pub parcela: Option<String>,
    pub data_alerta: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuxilioIndevidoResponse {
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub alertas: Vec<AlertaAuxilioResponseItem>,
}

pub fn registrar_alerta(conn: &Connection, alerta: &NovoAlerta) -> Result<i64, storage::StorageError> {
    conn.execute(
        "INSERT INTO alertas_auditoria (
            tipo, severidade, titulo, descricao, alvo_nome, alvo_documento,
            municipio, uf, ano, valor_envolvido, fonte_dado, detalhes_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            alerta.tipo,
            alerta.severidade.to_uppercase(),
            alerta.titulo,
            alerta.descricao,
            alerta.alvo_nome,
            alerta.alvo_documento,
            alerta.municipio,
            alerta.uf,
            alerta.ano,
            alerta.valor_envolvido,
            alerta.fonte_dado,
            alerta.detalhes_json
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn sincronizar_alertas_sistema(conn: &Connection) -> Result<usize, storage::StorageError> {
    let mut novos_inseridos = 0;

    // 1. Sincroniza anomalias de combustível da CEAP (> 80 litros ou flag_anomalia = 1)
    {
        let mut stmt = conn.prepare(
            "SELECT id, parlamentar_nome, parlamentar_cpf_mascarado, fornecedor_nome,
                    data_emissao, valor_liquido, detalhes_litros, flag_anomalia
             FROM despesas_parlamentares
             WHERE detalhes_litros > 80.0 OR flag_anomalia = 1",
        )?;

        let despesas = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, Option<f64>>(6)?,
                row.get::<_, bool>(7)?,
            ))
        })?;

        for d in despesas {
            let (id, parl, cpf, forn, dt, val, litros, _flag) = d?;
            let litros_val = litros.unwrap_or(0.0);

            // Verifica se já foi sincronizado
            let chave_detalhes = format!("\"despesa_id\":{}", id);
            let ja_existe: bool = conn
                .query_row(
                    "SELECT 1 FROM alertas_auditoria WHERE tipo = 'COMBUSTIVEL' AND detalhes_json LIKE ?1 LIMIT 1",
                    [format!("%{}%", chave_detalhes)],
                    |_| Ok(true),
                )
                .unwrap_or(false);

            if !ja_existe {
                let ano = dt.get(0..4).and_then(|y| y.parse::<i32>().ok());
                let severidade = if litros_val > 150.0 { "CRITICA" } else { "ALTA" };
                let detalhes = serde_json::json!({
                    "despesa_id": id,
                    "litros": litros_val,
                    "fornecedor": forn,
                    "data": dt,
                });

                registrar_alerta(
                    conn,
                    &NovoAlerta {
                        tipo: "COMBUSTIVEL".to_string(),
                        severidade: severidade.to_string(),
                        titulo: format!("Abastecimento anômalo ({:.1}L) - {}", litros_val, parl),
                        descricao: format!(
                            "Volume faturado de {:.1}L em {} supera capacidade física de veículo leve.",
                            litros_val, forn
                        ),
                        alvo_nome: parl,
                        alvo_documento: cpf,
                        municipio: None,
                        uf: None,
                        ano,
                        valor_envolvido: Some(val),
                        fonte_dado: "CEAP".to_string(),
                        detalhes_json: Some(detalhes.to_string()),
                    },
                )?;
                novos_inseridos += 1;
            }
        }
    }

    // 2. Sincroniza conflitos OAB da tabela alertas_incompatibilidade
    {
        let mut stmt = conn.prepare(
            "SELECT a.id, r.pessoa_nome, r.cpf_mascarado, r.seccional_uf,
                    a.cargo_ocupado, a.orgao_lotacao, a.data_nomeacao, a.motivo_incompatibilidade
             FROM alertas_incompatibilidade a
             JOIN registros_profissionais r ON a.registro_profissional_id = r.id",
        )?;

        let conflitos = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        })?;

        for c in conflitos {
            let (id, nome, cpf, uf, cargo, orgao, dt, motivo) = c?;
            let chave_detalhes = format!("\"conflito_id\":{}", id);
            let ja_existe: bool = conn
                .query_row(
                    "SELECT 1 FROM alertas_auditoria WHERE tipo = 'CONFLITO_OAB' AND detalhes_json LIKE ?1 LIMIT 1",
                    [format!("%{}%", chave_detalhes)],
                    |_| Ok(true),
                )
                .unwrap_or(false);

            if !ja_existe {
                let ano = dt.get(0..4).and_then(|y| y.parse::<i32>().ok());
                let detalhes = serde_json::json!({
                    "conflito_id": id,
                    "cargo": cargo,
                    "orgao": orgao,
                    "data_nomeacao": dt,
                });

                registrar_alerta(
                    conn,
                    &NovoAlerta {
                        tipo: "CONFLITO_OAB".to_string(),
                        severidade: "CRITICA".to_string(),
                        titulo: format!("Incompatibilidade com Advocacia (Art. 28 OAB) - {}", nome),
                        descricao: format!("{}. Cargo: {} ({})", motivo, cargo, orgao),
                        alvo_nome: nome,
                        alvo_documento: cpf,
                        municipio: None,
                        uf: Some(uf),
                        ano,
                        valor_envolvido: None,
                        fonte_dado: "OAB/DIARIO_OFICIAL".to_string(),
                        detalhes_json: Some(detalhes.to_string()),
                    },
                )?;
                novos_inseridos += 1;
            }
        }
    }

    // 3. Sincroniza alertas de auxílio indevido da tabela alertas_beneficio_indevido
    {
        let mut stmt = conn.prepare(
            "SELECT a.id, p.nome_completo, p.cpf_mascarado, a.motivo, a.detalhes,
                    a.valor_recebido, a.cargo_ou_mandato, a.ano_exercicio, b.uf, b.municipio
             FROM alertas_beneficio_indevido a
             JOIN politicos p ON a.politico_id = p.id
             LEFT JOIN beneficios_emergenciais b ON a.beneficio_id = b.id",
        )?;

        let aux_alertas = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<i32>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
            ))
        })?;

        for a in aux_alertas {
            let (id, nome, cpf, motivo, detalhes, valor, cargo, ano, uf, mun) = a?;
            let chave_detalhes = format!("\"auxilio_alerta_id\":{}", id);
            let ja_existe: bool = conn
                .query_row(
                    "SELECT 1 FROM alertas_auditoria WHERE tipo = 'AUXILIO_EMERGENCIAL' AND detalhes_json LIKE ?1 LIMIT 1",
                    [format!("%{}%", chave_detalhes)],
                    |_| Ok(true),
                )
                .unwrap_or(false);

            if !ja_existe {
                let severidade = if motivo.contains("MANDATO") { "CRITICA" } else { "ALTA" };
                let detalhes_obj = serde_json::json!({
                    "auxilio_alerta_id": id,
                    "motivo": motivo,
                    "detalhes": detalhes,
                    "cargo_ou_mandato": cargo,
                });

                registrar_alerta(
                    conn,
                    &NovoAlerta {
                        tipo: "AUXILIO_EMERGENCIAL".to_string(),
                        severidade: severidade.to_string(),
                        titulo: format!("Auxílio Emergencial Indevido - {}", nome),
                        descricao: detalhes.unwrap_or_else(|| format!("Recebimento irregular de benefício (motivo: {})", motivo)),
                        alvo_nome: nome,
                        alvo_documento: cpf,
                        municipio: mun,
                        uf,
                        ano,
                        valor_envolvido: Some(valor),
                        fonte_dado: "CGU/BRASIL_IO".to_string(),
                        detalhes_json: Some(detalhes_obj.to_string()),
                    },
                )?;
                novos_inseridos += 1;
            }
        }
    }

    Ok(novos_inseridos)
}

pub fn carregar_alertas(
    conn: &Connection,
    filtros: &AlertasQueryParams,
) -> Result<AlertasResponse, storage::StorageError> {
    let limit = filtros.limit.unwrap_or(50).min(200);
    let offset = filtros.offset.unwrap_or(0);

    let mut sql_base = String::from(
        "FROM alertas_auditoria WHERE 1=1"
    );
    let mut sql_params = Vec::<storage::rusqlite::types::Value>::new();

    if let Some(ref mun) = filtros.municipio {
        let trimmed = mun.trim();
        if !trimmed.is_empty() {
            sql_base.push_str(" AND UPPER(municipio) LIKE UPPER(?)");
            sql_params.push(format!("%{}%", trimmed).into());
        }
    }

    if let Some(ano) = filtros.ano {
        sql_base.push_str(" AND ano = ?");
        sql_params.push(ano.into());
    }

    if let Some(ref sev) = filtros.severidade {
        let trimmed = sev.trim();
        if !trimmed.is_empty() {
            sql_base.push_str(" AND UPPER(severidade) = UPPER(?)");
            sql_params.push(trimmed.to_string().into());
        }
    }

    if let Some(ref tip) = filtros.tipo {
        let trimmed = tip.trim();
        if !trimmed.is_empty() {
            sql_base.push_str(" AND UPPER(tipo) = UPPER(?)");
            sql_params.push(trimmed.to_string().into());
        }
    }

    // Contagem total
    let count_sql = format!("SELECT COUNT(*) {}", sql_base);
    let total: usize = {
        let mut count_stmt = conn.prepare(&count_sql)?;
        let p_refs: Vec<&dyn ToSql> = sql_params.iter().map(|v| v as &dyn ToSql).collect();
        count_stmt.query_row(p_refs.as_slice(), |row| row.get(0))?
    };

    // Consulta com ordenação por severidade e paginação
    let select_sql = format!(
        "SELECT id, tipo, severidade, titulo, descricao, alvo_nome, alvo_documento,
                municipio, uf, ano, valor_envolvido, fonte_dado, detalhes_json, data_criacao
         {}
         ORDER BY
             CASE UPPER(severidade)
                 WHEN 'CRITICA' THEN 1
                 WHEN 'ALTA' THEN 2
                 WHEN 'MEDIA' THEN 3
                 WHEN 'BAIXA' THEN 4
                 ELSE 5
             END ASC,
             COALESCE(valor_envolvido, 0.0) DESC,
             id DESC
         LIMIT ? OFFSET ?",
        sql_base
    );

    let mut select_params = sql_params;
    select_params.push((limit as i64).into());
    select_params.push((offset as i64).into());

    let mut stmt = conn.prepare(&select_sql)?;
    let p_refs: Vec<&dyn ToSql> = select_params.iter().map(|v| v as &dyn ToSql).collect();

    let alertas = stmt
        .query_map(p_refs.as_slice(), |row| {
            let detalhes_str: Option<String> = row.get(12)?;
            let detalhes_json = detalhes_str.and_then(|s| serde_json::from_str(&s).ok());

            Ok(AlertaItem {
                id: row.get(0)?,
                tipo: row.get(1)?,
                severidade: row.get(2)?,
                titulo: row.get(3)?,
                descricao: row.get(4)?,
                alvo_nome: row.get(5)?,
                alvo_documento: row.get(6)?,
                municipio: row.get(7)?,
                uf: row.get(8)?,
                ano: row.get(9)?,
                valor_envolvido: row.get(10)?,
                fonte_dado: row.get(11)?,
                detalhes: detalhes_json,
                data_criacao: row.get(13)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(AlertasResponse {
        total,
        limit,
        offset,
        alertas,
    })
}

pub async fn alertas_handler(
    State(pool): State<DbPool>,
    Query(filtros): Query<AlertasQueryParams>,
) -> Result<Json<AlertasResponse>, StatusCode> {
    let conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Sincroniza se necessário
    let _ = sincronizar_alertas_sistema(&conn);

    let response = carregar_alertas(&conn, &filtros)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(response))
}

pub async fn auxilio_indevido_handler(
    State(pool): State<DbPool>,
    Query(params): Query<AuxilioIndevidoQueryParams>,
) -> Result<Json<AuxilioIndevidoResponse>, StatusCode> {
    let mut conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Se a tabela de alertas de auxilio estiver vazia, mas houver registros em beneficios, executa auditoria
    let count_alertas: i64 = conn
        .query_row("SELECT count(*) FROM alertas_beneficio_indevido", [], |r| r.get(0))
        .unwrap_or(0);
    if count_alertas == 0 {
        let count_ben: i64 = conn
            .query_row("SELECT count(*) FROM beneficios_emergenciais", [], |r| r.get(0))
            .unwrap_or(0);
        if count_ben > 0 {
            let _ = auditor::executar_auditoria_auxilio_sqlite(&mut conn);
        }
    }

    let mut sql = "
        SELECT 
            a.id,
            a.politico_id,
            p.nome_completo,
            p.cpf_mascarado,
            a.beneficio_id,
            a.motivo,
            a.detalhes,
            a.valor_recebido,
            a.total_bens,
            a.cargo_ou_mandato,
            a.ano_exercicio,
            a.status_analise,
            b.mes_disponibilizacao,
            b.parcela,
            a.data_alerta
        FROM alertas_beneficio_indevido a
        JOIN politicos p ON a.politico_id = p.id
        LEFT JOIN beneficios_emergenciais b ON a.beneficio_id = b.id
        WHERE 1=1
    ".to_string();

    let mut sql_params: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(pid) = params.politico_id {
        sql.push_str(&format!(" AND a.politico_id = ?{}", sql_params.len() + 1));
        sql_params.push(Box::new(pid));
    }

    if let Some(ref m) = params.motivo {
        sql.push_str(&format!(" AND a.motivo LIKE ?{}", sql_params.len() + 1));
        sql_params.push(Box::new(format!("%{}%", m)));
    }

    sql.push_str(" ORDER BY a.valor_recebido DESC, a.id DESC");

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    sql.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

    let mut stmt = conn.prepare(&sql).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let param_refs: Vec<&dyn ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(AlertaAuxilioResponseItem {
            id: row.get(0)?,
            politico_id: row.get(1)?,
            politico_nome: row.get(2)?,
            cpf_mascarado: row.get(3)?,
            beneficio_id: row.get(4)?,
            motivo: row.get(5)?,
            detalhes: row.get(6)?,
            valor_recebido: row.get(7)?,
            total_bens: row.get(8)?,
            cargo_ou_mandato: row.get(9)?,
            ano_exercicio: row.get(10)?,
            status_analise: row.get(11)?,
            mes_disponibilizacao: row.get(12)?,
            parcela: row.get(13)?,
            data_alerta: row.get(14)?,
        })
    }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut alertas = Vec::new();
    for r in rows {
        if let Ok(item) = r {
            alertas.push(item);
        }
    }

    let total = alertas.len();
    Ok(Json(AuxilioIndevidoResponse {
        total,
        limit,
        offset,
        alertas,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use storage::run_migrations;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_alertas_ordenacao_severidade_e_filtros() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // Insere alertas manuais com diferentes severidades, anos e municípios
        registrar_alerta(
            &conn,
            &NovoAlerta {
                tipo: "TRIANGULACAO".to_string(),
                severidade: "ALTA".to_string(),
                titulo: "Doador com contrato rápido".to_string(),
                descricao: "Contrato assinado em menos de 180 dias após doação.".to_string(),
                alvo_nome: "EMPRESA X".to_string(),
                alvo_documento: Some("11222333000199".to_string()),
                municipio: Some("São Paulo".to_string()),
                uf: Some("SP".to_string()),
                ano: Some(2024),
                valor_envolvido: Some(500000.0),
                fonte_dado: "PNCP".to_string(),
                detalhes_json: None,
            },
        ).unwrap();

        registrar_alerta(
            &conn,
            &NovoAlerta {
                tipo: "FORNECEDOR_HUB".to_string(),
                severidade: "MEDIA".to_string(),
                titulo: "Concentração partidária".to_string(),
                descricao: "Fornecedor recebeu 75% dos gastos de campanha.".to_string(),
                alvo_nome: "GRAFICA RAPIDA LTDA".to_string(),
                alvo_documento: Some("99888777000100".to_string()),
                municipio: Some("Campinas".to_string()),
                uf: Some("SP".to_string()),
                ano: Some(2024),
                valor_envolvido: Some(150000.0),
                fonte_dado: "TSE".to_string(),
                detalhes_json: None,
            },
        ).unwrap();

        registrar_alerta(
            &conn,
            &NovoAlerta {
                tipo: "CONFLITO_OAB".to_string(),
                severidade: "CRITICA".to_string(),
                titulo: "Secretário Municipal de Obras com OAB ativa".to_string(),
                descricao: "Advogado nomeado em cargo de direção executiva.".to_string(),
                alvo_nome: "DR. BELTRANO ADVOGADO".to_string(),
                alvo_documento: Some("***.123.456-**".to_string()),
                municipio: Some("São Paulo".to_string()),
                uf: Some("SP".to_string()),
                ano: Some(2023),
                valor_envolvido: None,
                fonte_dado: "OAB/CNA".to_string(),
                detalhes_json: None,
            },
        ).unwrap();

        // 1. Teste de ordenação por severidade (CRITICA -> ALTA -> MEDIA)
        let resp_todas = carregar_alertas(
            &conn,
            &AlertasQueryParams {
                municipio: None,
                ano: None,
                severidade: None,
                tipo: None,
                limit: Some(10),
                offset: Some(0),
            },
        ).unwrap();

        assert_eq!(resp_todas.total, 3);
        assert_eq!(resp_todas.alertas[0].severidade, "CRITICA");
        assert_eq!(resp_todas.alertas[1].severidade, "ALTA");
        assert_eq!(resp_todas.alertas[2].severidade, "MEDIA");

        // 2. Teste de filtro por município
        let resp_sp = carregar_alertas(
            &conn,
            &AlertasQueryParams {
                municipio: Some("São Paulo".to_string()),
                ano: None,
                severidade: None,
                tipo: None,
                limit: None,
                offset: None,
            },
        ).unwrap();

        assert_eq!(resp_sp.total, 2);
        for a in &resp_sp.alertas {
            assert_eq!(a.municipio.as_deref(), Some("São Paulo"));
        }

        // 3. Teste de filtro por ano
        let resp_2023 = carregar_alertas(
            &conn,
            &AlertasQueryParams {
                municipio: None,
                ano: Some(2023),
                severidade: None,
                tipo: None,
                limit: None,
                offset: None,
            },
        ).unwrap();

        assert_eq!(resp_2023.total, 1);
        assert_eq!(resp_2023.alertas[0].tipo, "CONFLITO_OAB");

        // 4. Teste sincronização automática de combustível da CEAP
        conn.execute(
            "INSERT INTO despesas_parlamentares (
                casa_legislativa, parlamentar_nome, parlamentar_cpf_mascarado,
                data_emissao, categoria_despesa, fornecedor_nome, fornecedor_cnpj_cpf,
                valor_liquido, detalhes_litros, flag_anomalia
             ) VALUES (
                'CAMARA', 'DEPUTADO X', '***.000.111-**',
                '2024-05-10', 'COMBUSTIVEL', 'POSTO 1', '12345678000100',
                600.0, 95.0, 1
             )",
            [],
        ).unwrap();

        let novos = sincronizar_alertas_sistema(&conn).unwrap();
        assert_eq!(novos, 1);

        let resp_combustivel = carregar_alertas(
            &conn,
            &AlertasQueryParams {
                municipio: None,
                ano: Some(2024),
                severidade: None,
                tipo: Some("COMBUSTIVEL".to_string()),
                limit: None,
                offset: None,
            },
        ).unwrap();

        assert_eq!(resp_combustivel.total, 1);
        assert_eq!(resp_combustivel.alertas[0].alvo_nome, "DEPUTADO X");
        assert_eq!(resp_combustivel.alertas[0].severidade, "ALTA");
    }

    #[tokio::test]
    async fn test_alertas_endpoint_http() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        registrar_alerta(
            &conn,
            &NovoAlerta {
                tipo: "UBIQUIDADE".to_string(),
                severidade: "ALTA".to_string(),
                titulo: "Despesas simultâneas a 900 km de distância".to_string(),
                descricao: "Notas fiscais emitidas no mesmo intervalo de 1h em cidades distantes.".to_string(),
                alvo_nome: "PARLAMENTAR Y".to_string(),
                alvo_documento: None,
                municipio: Some("Curitiba".to_string()),
                uf: Some("PR".to_string()),
                ano: Some(2024),
                valor_envolvido: Some(1200.0),
                fonte_dado: "CEAP".to_string(),
                detalhes_json: None,
            },
        ).unwrap();

        let app = Router::new()
            .route("/api/v1/auditoria/alertas", get(alertas_handler))
            .with_state(pool);

        let req = Request::builder()
            .uri("/api/v1/auditoria/alertas?ano=2024&municipio=Curitiba")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
        let json: AlertasResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.total, 1);
        assert_eq!(json.alertas[0].tipo, "UBIQUIDADE");
    }

    #[tokio::test]
    async fn test_auxilio_indevido_endpoint_http() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        conn.execute(
            "INSERT INTO politicos (nome_completo, nome_urna, cpf_mascarado)
             VALUES ('DEPUTADO BENEFICIARIO', 'BENEFICIARIO', '***.888.999-**')",
            [],
        ).unwrap();
        let pol_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO beneficios_emergenciais (cpf_mascarado, nome_beneficiario, municipio, uf, mes_disponibilizacao, parcela, valor)
             VALUES ('***.888.999-**', 'DEPUTADO BENEFICIARIO', 'SAO PAULO', 'SP', '202004', '1', 600.0)",
            [],
        ).unwrap();
        let ben_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO alertas_beneficio_indevido (politico_id, beneficio_id, motivo, detalhes, valor_recebido, total_bens, cargo_ou_mandato, ano_exercicio)
             VALUES (?1, ?2, 'MANDATO_VIGENTE', 'Recebeu Auxilio ocupando mandato', 600.0, 500000.0, 'DEPUTADO', 2020)",
            (pol_id, ben_id),
        ).unwrap();

        let app = Router::new()
            .route("/api/v1/auditoria/auxilio-indevido", get(auxilio_indevido_handler))
            .with_state(pool);

        let req = Request::builder()
            .uri(format!("/api/v1/auditoria/auxilio-indevido?politico_id={}", pol_id))
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
        let json: AuxilioIndevidoResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.total, 1);
        assert_eq!(json.alertas[0].politico_id, pol_id);
        assert_eq!(json.alertas[0].politico_nome, "DEPUTADO BENEFICIARIO");
        assert_eq!(json.alertas[0].motivo, "MANDATO_VIGENTE");
        assert_eq!(json.alertas[0].valor_recebido, 600.0);
    }
}
