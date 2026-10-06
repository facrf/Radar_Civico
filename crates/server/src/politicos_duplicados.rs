use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use storage::{rusqlite::Connection, DbPool};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParametrosConsultaDuplicados {
    pub criterio: Option<String>,
    pub q: Option<String>,
    pub page: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemPoliticoDuplicado {
    pub id: i64,
    pub sq_candidato: Option<String>,
    pub cpf_mascarado: Option<String>,
    pub nome_completo: String,
    pub nome_urna: String,
    pub data_nascimento: Option<String>,
    pub sigla_partido: String,
    pub uf: String,
    pub cargo: String,
    pub ano_eleicao: Option<i32>,
    pub mandatos: Vec<String>,
    pub total_despesas_ceap: f64,
    pub total_itens_ceap: i64,
    pub total_bens: f64,
    pub foto_base64: Option<String>,
    pub foto_mime: Option<String>,
    pub foto_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrupoPoliticosDuplicados {
    pub id_grupo: String,
    pub criterio: String,
    pub confianca: String,
    pub motivo: String,
    pub sugestao_canonico_id: i64,
    pub politicos: Vec<ItemPoliticoDuplicado>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumoDuplicadosResponse {
    pub total_grupos: usize,
    pub total_registros_duplicados: usize,
    pub grupos_nascimento_exato: usize,
    pub grupos_ceap_tse: usize,
    pub explicacao_tecnica: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatorioDuplicadosResponse {
    pub total_grupos: usize,
    pub page: usize,
    pub limit: usize,
    pub total_paginas: usize,
    pub resumo: ResumoDuplicadosResponse,
    pub grupos: Vec<GrupoPoliticosDuplicados>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MesclarPoliticosRequest {
    pub id_canonico: i64,
    pub id_duplicado: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MesclarPoliticosResponse {
    pub sucesso: bool,
    pub mensagem: String,
    pub id_canonico: i64,
    pub id_removido: i64,
    pub candidaturas_migradas: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MesclarAutomaticoResponse {
    pub sucesso: bool,
    pub mensagem: String,
    pub grupos_processados: usize,
    pub registros_unificados: usize,
}

/// Carrega os dados enriquecidos de uma lista de IDs de políticos
fn carregar_itens_politicos(conn: &Connection, ids: &[i64]) -> Vec<ItemPoliticoDuplicado> {
    if ids.is_empty() {
        return Vec::new();
    }

    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT p.id, p.sq_candidato, p.cpf_mascarado, p.nome_completo, p.nome_urna,
                p.data_nascimento,
                COALESCE(c.sigla_partido, 'S/P'), COALESCE(c.uf, 'BR'),
                COALESCE(c.cargo, 'PARLAMENTAR'),
                COALESCE(c.total_bens_declarados, 0.0),
                p.foto_blob, p.foto_mime, p.foto_url,
                GROUP_CONCAT(DISTINCT c.cargo || CASE WHEN c.ano_eleicao IS NOT NULL AND c.ano_eleicao > 0 THEN ' (' || c.ano_eleicao || ')' ELSE '' END) as mandatos_str,
                MAX(c.ano_eleicao) as ano_eleicao
         FROM politicos p
         LEFT JOIN candidaturas c ON c.politico_id = p.id
         WHERE p.id IN ({})
         GROUP BY p.id
         ORDER BY (CASE WHEN p.foto_blob IS NOT NULL OR p.foto_url IS NOT NULL THEN 1 ELSE 2 END) ASC,
                  (CASE WHEN p.sq_candidato IS NOT NULL THEN 1 ELSE 2 END) ASC,
                  p.id ASC",
        placeholders
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let params: Vec<&dyn storage::rusqlite::ToSql> = ids.iter().map(|id| id as &dyn storage::rusqlite::ToSql).collect();

    let rows = match stmt.query_map(params.as_slice(), |row| {
        let id: i64 = row.get(0)?;
        let foto_blob: Option<Vec<u8>> = row.get(10)?;
        let foto_base64 = foto_blob.map(|b| BASE64.encode(b));
        let cargo: String = row.get(8)?;
        let mandatos_str: Option<String> = row.get(13)?;
        let mandatos = mandatos_str
            .map(|s| {
                s.split(',')
                    .map(|m| m.trim().to_string())
                    .filter(|m| !m.is_empty())
                    .collect()
            })
            .unwrap_or_else(|| vec![cargo.clone()]);
        let ano_eleicao: Option<i32> = row.get(14).ok();

        Ok(ItemPoliticoDuplicado {
            id,
            sq_candidato: row.get(1)?,
            cpf_mascarado: row.get(2)?,
            nome_completo: row.get(3)?,
            nome_urna: row.get(4)?,
            data_nascimento: row.get(5)?,
            sigla_partido: row.get(6)?,
            uf: row.get(7)?,
            cargo,
            ano_eleicao,
            mandatos,
            total_despesas_ceap: 0.0,
            total_itens_ceap: 0,
            total_bens: row.get(9)?,
            foto_base64,
            foto_mime: row.get(11)?,
            foto_url: row.get(12)?,
        })
    }) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut itens: Vec<ItemPoliticoDuplicado> = rows.filter_map(|r| r.ok()).collect();

    // Preenche despesas CEAP indexadas
    for item in &mut itens {
        if let Ok((tot, qtd)) = conn.query_row(
            "SELECT COALESCE(SUM(valor_liquido), 0.0), COUNT(id)
             FROM despesas_parlamentares
             WHERE parlamentar_nome = ?1 OR parlamentar_nome = ?2",
            [&item.nome_urna, &item.nome_completo],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ) {
            item.total_despesas_ceap = tot;
            item.total_itens_ceap = qtd;
        }
    }

    itens
}

/// GET /api/politicos/duplicados/resumo
pub async fn resumo_politicos_duplicados_handler(
    State(pool): State<DbPool>,
) -> Result<Json<ResumoDuplicadosResponse>, (StatusCode, String)> {
    let conn = pool.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 1. Grupos com mesmo Nome Completo e Data de Nascimento (100% de certeza)
    let grupos_nascimento: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM (
                SELECT nome_completo, data_nascimento
                FROM politicos
                WHERE data_nascimento IS NOT NULL AND data_nascimento != ''
                  AND nome_completo IS NOT NULL AND length(nome_completo) > 5
                GROUP BY UPPER(TRIM(nome_completo)), data_nascimento
                HAVING COUNT(*) > 1
             )",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // 2. Registros redundantes totais dentro desses grupos
    let registros_nascimento_total: usize = conn
        .query_row(
            "SELECT COALESCE(SUM(c), 0) FROM (
                SELECT COUNT(*) as c
                FROM politicos
                WHERE data_nascimento IS NOT NULL AND data_nascimento != ''
                  AND nome_completo IS NOT NULL AND length(nome_completo) > 5
                GROUP BY UPPER(TRIM(nome_completo)), data_nascimento
                HAVING c > 1
             )",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let registros_redundantes = if registros_nascimento_total > grupos_nascimento {
        registros_nascimento_total - grupos_nascimento
    } else {
        0
    };

    // 3. Deputados CEAP que possuem correspondente com sq_candidato no TSE
    let grupos_ceap: usize = conn
        .query_row(
            "SELECT COUNT(DISTINCT p.id)
             FROM politicos p
             JOIN politicos_fts f ON f.politico_id != p.id AND politicos_fts MATCH '\"' || REPLACE(p.nome_urna, '\"', '') || '\"'
             JOIN politicos p_tse ON p_tse.id = f.politico_id AND p_tse.sq_candidato IS NOT NULL
             WHERE p.sq_candidato IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let explicacao = "Cadastros repetidos ocorrem principalmente porque: \
        (1) No TSE, cada eleição ou substituição gera um novo 'sq_candidato', fazendo com que a ingestão crie uma pessoa física nova para cada pleito em vez de vincular as candidaturas ao mesmo político; \
        (2) A sincronização da Câmara dos Deputados (CEAP) cria parlamentares com nomes encurtados sem 'sq_candidato'; \
        (3) As bases recentes do TSE mascaram o CPF como '-4' devido à LGPD, impedindo o uso exclusivo do CPF como chave primária.".to_string();

    Ok(Json(ResumoDuplicadosResponse {
        total_grupos: grupos_nascimento + grupos_ceap,
        total_registros_duplicados: registros_redundantes + grupos_ceap,
        grupos_nascimento_exato: grupos_nascimento,
        grupos_ceap_tse: grupos_ceap,
        explicacao_tecnica: explicacao,
    }))
}

/// GET /api/politicos/duplicados
pub async fn listar_politicos_duplicados_handler(
    State(pool): State<DbPool>,
    Query(params): Query<ParametrosConsultaDuplicados>,
) -> Result<Json<RelatorioDuplicadosResponse>, (StatusCode, String)> {
    let conn = pool.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(15).clamp(1, 50);
    let offset = (page - 1) * limit;

    let criterio = params.criterio.as_deref().unwrap_or("todos");
    let q_term = params.q.as_deref().unwrap_or("").trim().to_uppercase();

    let mut grupos = Vec::new();

    // Critério 1: Mesmo Nome Completo e Mesma Data de Nascimento (Certeza 100%)
    if criterio == "todos" || criterio == "nascimento" {
        let (where_filtro, param_q) = if !q_term.is_empty() {
            (
                "AND UPPER(nome_completo) LIKE ?",
                Some(format!("%{}%", q_term)),
            )
        } else {
            ("", None)
        };

        let sql = format!(
            "SELECT UPPER(TRIM(nome_completo)) as nome_norm, data_nascimento, COUNT(*) as qtd, GROUP_CONCAT(id) as ids_str
             FROM politicos
             WHERE data_nascimento IS NOT NULL AND data_nascimento != ''
               AND nome_completo IS NOT NULL AND length(nome_completo) > 5
               {}
             GROUP BY UPPER(TRIM(nome_completo)), data_nascimento
             HAVING qtd > 1
             ORDER BY qtd DESC, nome_norm ASC
             LIMIT {} OFFSET {}",
            where_filtro, limit, offset
        );

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let params_dyn: Vec<&dyn storage::rusqlite::ToSql> = match param_q {
            Some(ref q) => vec![q],
            None => vec![],
        };

        let rows = stmt
            .query_map(params_dyn.as_slice(), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, usize>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        for row in rows.flatten() {
            let (nome_norm, data_nasc, _qtd, ids_str) = row;
            let ids: Vec<i64> = ids_str
                .split(',')
                .filter_map(|s| s.trim().parse::<i64>().ok())
                .collect();

            let politicos = carregar_itens_politicos(&conn, &ids);
            if politicos.len() >= 2 {
                let id_sugerido = politicos
                    .iter()
                    .find(|p| p.foto_base64.is_some() || p.foto_url.is_some())
                    .or_else(|| politicos.iter().find(|p| p.sq_candidato.is_some()))
                    .map(|p| p.id)
                    .unwrap_or(politicos[0].id);

                let id_grupo = format!("NASC-{}-{}", nome_norm.replace(' ', "_"), data_nasc.replace('/', ""));

                grupos.push(GrupoPoliticosDuplicados {
                    id_grupo,
                    criterio: "NOME_E_NASCIMENTO".to_string(),
                    confianca: "ALTA (100% Certeza)".to_string(),
                    motivo: format!(
                        "Mesmo nome civil ('{}') e mesma data de nascimento ({}). O TSE gerou sequenciais distintos em cada pleito/cargo.",
                        nome_norm, data_nasc
                    ),
                    sugestao_canonico_id: id_sugerido,
                    politicos,
                });
            }
        }
    }

    // Contagem de grupos para paginação
    let total_grupos: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM (
                SELECT nome_completo, data_nascimento
                FROM politicos
                WHERE data_nascimento IS NOT NULL AND data_nascimento != ''
                  AND nome_completo IS NOT NULL AND length(nome_completo) > 5
                GROUP BY UPPER(TRIM(nome_completo)), data_nascimento
                HAVING COUNT(*) > 1
             )",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let total_paginas = if total_grupos == 0 {
        1
    } else {
        (total_grupos + limit - 1) / limit
    };

    let resumo = ResumoDuplicadosResponse {
        total_grupos,
        total_registros_duplicados: total_grupos * 2,
        grupos_nascimento_exato: total_grupos,
        grupos_ceap_tse: 0,
        explicacao_tecnica: "A análise detecta duplicações provocadas pela rotação de SQ_CANDIDATO do TSE entre eleições e cruzamentos com notas da Câmara.".to_string(),
    };

    Ok(Json(RelatorioDuplicadosResponse {
        total_grupos,
        page,
        limit,
        total_paginas,
        resumo,
        grupos,
    }))
}

/// Executa a mesclagem atômica de dois registros no SQLite
pub fn mesclar_politicos_db(
    conn: &mut Connection,
    id_canonico: i64,
    id_duplicado: i64,
) -> Result<usize, storage::StorageError> {
    if id_canonico == id_duplicado {
        return Ok(0);
    }

    let tx = conn.transaction()?;

    // 1. Migra candidaturas: para candidaturas que coincidem no mesmo (ano_eleicao, cargo), transfere bens e apaga a redundante
    {
        let mut stmt_cands = tx.prepare(
            "SELECT id, ano_eleicao, cargo FROM candidaturas WHERE politico_id = ?1",
        )?;

        let cands_duplicadas = stmt_cands
            .query_map([id_duplicado], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();

        for (cand_id, ano, cargo) in cands_duplicadas {
            let cand_existente: Option<i64> = tx
                .query_row(
                    "SELECT id FROM candidaturas WHERE politico_id = ?1 AND ano_eleicao = ?2 AND cargo = ?3",
                    storage::rusqlite::params![id_canonico, ano, cargo],
                    |r| r.get(0),
                )
                .ok();

            if let Some(cand_existente_id) = cand_existente {
                // Move bens e receitas para a candidatura já existente no canônico
                tx.execute(
                    "UPDATE bens_candidato SET candidatura_id = ?1 WHERE candidatura_id = ?2",
                    [cand_existente_id, cand_id],
                )?;
                tx.execute(
                    "UPDATE receitas_campanha SET candidatura_id = ?1 WHERE candidatura_id = ?2",
                    [cand_existente_id, cand_id],
                )?;
                tx.execute("DELETE FROM candidaturas WHERE id = ?1", [cand_id])?;
            } else {
                // Transfere a candidatura para o ID canônico
                tx.execute(
                    "UPDATE candidaturas SET politico_id = ?1 WHERE id = ?2",
                    [id_canonico, cand_id],
                )?;
            }
        }
    }

    // 2. Migra alertas de benefício indevido
    tx.execute(
        "UPDATE alertas_beneficio_indevido SET politico_id = ?1 WHERE politico_id = ?2",
        [id_canonico, id_duplicado],
    )?;

    // 3. Copia foto se o canônico não possuir
    let (foto_blob_dup, foto_mime_dup, foto_url_dup): (Option<Vec<u8>>, Option<String>, Option<String>) = tx
        .query_row(
            "SELECT foto_blob, foto_mime, foto_url FROM politicos WHERE id = ?1",
            [id_duplicado],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((None, None, None));

    let tem_foto_can: bool = tx
        .query_row(
            "SELECT (foto_blob IS NOT NULL OR foto_url IS NOT NULL) FROM politicos WHERE id = ?1",
            [id_canonico],
            |r| r.get(0),
        )
        .unwrap_or(false);

    if !tem_foto_can && (foto_blob_dup.is_some() || foto_url_dup.is_some()) {
        tx.execute(
            "UPDATE politicos SET foto_blob = ?1, foto_mime = ?2, foto_url = ?3 WHERE id = ?4",
            storage::rusqlite::params![foto_blob_dup, foto_mime_dup, foto_url_dup, id_canonico],
        )?;
    }

    // 4. Se o canônico não possuir sq_candidato mas o duplicado possuir, copia
    let sq_dup: Option<String> = tx
        .query_row(
            "SELECT sq_candidato FROM politicos WHERE id = ?1",
            [id_duplicado],
            |r| r.get(0),
        )
        .ok()
        .flatten();

    let sq_can: Option<String> = tx
        .query_row(
            "SELECT sq_candidato FROM politicos WHERE id = ?1",
            [id_canonico],
            |r| r.get(0),
        )
        .ok()
        .flatten();

    if sq_can.is_none() && sq_dup.is_some() {
        tx.execute(
            "UPDATE politicos SET sq_candidato = ?1 WHERE id = ?2",
            storage::rusqlite::params![sq_dup, id_canonico],
        )?;
    }

    // 5. Remove o registro redundante
    let removidos = tx.execute("DELETE FROM politicos WHERE id = ?1", [id_duplicado])?;

    tx.commit()?;

    Ok(removidos)
}

/// POST /api/politicos/duplicados/mesclar
pub async fn mesclar_politicos_handler(
    State(pool): State<DbPool>,
    Json(payload): Json<MesclarPoliticosRequest>,
) -> Result<Json<MesclarPoliticosResponse>, (StatusCode, String)> {
    if payload.id_canonico == payload.id_duplicado {
        return Err((
            StatusCode::BAD_REQUEST,
            "O ID canônico e o ID duplicado devem ser diferentes.".to_string(),
        ));
    }

    let mut conn = pool.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match mesclar_politicos_db(&mut conn, payload.id_canonico, payload.id_duplicado) {
        Ok(qtd) => Ok(Json(MesclarPoliticosResponse {
            sucesso: true,
            mensagem: format!(
                "Político #{} unificado com sucesso no registro canônico #{}. Candidaturas consolidadas.",
                payload.id_duplicado, payload.id_canonico
            ),
            id_canonico: payload.id_canonico,
            id_removido: payload.id_duplicado,
            candidaturas_migradas: qtd,
        })),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Falha ao mesclar políticos: {e}"))),
    }
}

/// POST /api/politicos/duplicados/mesclar-automatico
pub async fn mesclar_automatico_handler(
    State(pool): State<DbPool>,
) -> Result<Json<MesclarAutomaticoResponse>, (StatusCode, String)> {
    let mut conn = pool.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Busca grupos com mesmo nome civil e data de nascimento
    let mut stmt = conn
        .prepare(
            "SELECT UPPER(TRIM(nome_completo)), data_nascimento, GROUP_CONCAT(id)
             FROM politicos
             WHERE data_nascimento IS NOT NULL AND data_nascimento != ''
               AND nome_completo IS NOT NULL AND length(nome_completo) > 5
             GROUP BY UPPER(TRIM(nome_completo)), data_nascimento
             HAVING count(*) > 1
             LIMIT 100",
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let grupos = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();

    drop(stmt);

    let mut processados = 0;
    let mut unificados = 0;

    for (_nome, _nasc, ids_str) in grupos {
        let mut ids: Vec<i64> = ids_str
            .split(',')
            .filter_map(|s| s.trim().parse::<i64>().ok())
            .collect();

        if ids.len() < 2 {
            continue;
        }

        // Escolhe o canônico (menor ID) e unifica os demais nele
        ids.sort();
        let id_canonico = ids[0];

        for &id_dup in &ids[1..] {
            if mesclar_politicos_db(&mut conn, id_canonico, id_dup).is_ok() {
                unificados += 1;
            }
        }
        processados += 1;
    }

    Ok(Json(MesclarAutomaticoResponse {
        sucesso: true,
        mensagem: format!(
            "Deduplicação automática concluída: {processados} grupos processados e {unificados} registros redundantes consolidados."
        ),
        grupos_processados: processados,
        registros_unificados: unificados,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::{get, post},
        Router,
    };
    use storage::migrations::run_migrations;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_deteccao_e_mesclagem_politicos_repetidos() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // 1. Cria 2 registros da mesma pessoa (Adiel dos Santos Tavares)
        conn.execute(
            "INSERT INTO politicos (sq_candidato, nome_completo, nome_urna, data_nascimento)
             VALUES ('SQ_2024_VICE', 'ADIEL DOS SANTOS TAVARES', 'ADIEL DOS SANTOS', '23/06/1992')",
            [],
        )
        .unwrap();
        let id1 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf)
             VALUES (?1, 2024, 'VICE-PREFEITO', 'PL', 'RJ')",
            [id1],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO politicos (sq_candidato, nome_completo, nome_urna, data_nascimento)
             VALUES ('SQ_2024_VEREADOR', 'ADIEL DOS SANTOS TAVARES', 'ADIEL TAVARES', '23/06/1992')",
            [],
        )
        .unwrap();
        let id2 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf)
             VALUES (?1, 2024, 'VEREADOR', 'PL', 'RJ')",
            [id2],
        )
        .unwrap();

        let app = Router::new()
            .route("/api/politicos/duplicados", get(listar_politicos_duplicados_handler))
            .route("/api/politicos/duplicados/resumo", get(resumo_politicos_duplicados_handler))
            .route("/api/politicos/duplicados/mesclar", post(mesclar_politicos_handler))
            .with_state(pool.clone());

        // 2. Testa detecção via endpoint
        let req_dup = Request::builder()
            .uri("/api/politicos/duplicados")
            .body(Body::empty())
            .unwrap();

        let res_dup = app.clone().oneshot(req_dup).await.unwrap();
        assert_eq!(res_dup.status(), StatusCode::OK);
        let bytes_dup = axum::body::to_bytes(res_dup.into_body(), usize::MAX).await.unwrap();
        let relatorio: RelatorioDuplicadosResponse = serde_json::from_slice(&bytes_dup).unwrap();

        assert_eq!(relatorio.total_grupos, 1);
        assert_eq!(relatorio.grupos[0].politicos.len(), 2);
        assert_eq!(relatorio.grupos[0].politicos[0].nome_completo, "ADIEL DOS SANTOS TAVARES");

        // 3. Testa mesclagem: unifica id2 em id1
        let payload = MesclarPoliticosRequest {
            id_canonico: id1,
            id_duplicado: id2,
        };

        let req_mesclar = Request::builder()
            .uri("/api/politicos/duplicados/mesclar")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();

        let res_mesclar = app.clone().oneshot(req_mesclar).await.unwrap();
        assert_eq!(res_mesclar.status(), StatusCode::OK);

        // 4. Verifica no banco: id2 deve ter sido removido e id1 agora possui 2 candidaturas (Vice e Vereador)
        let count_id2: usize = conn
            .query_row("SELECT count(*) FROM politicos WHERE id = ?1", [id2], |r| r.get(0))
            .unwrap();
        assert_eq!(count_id2, 0);

        let cands_id1: usize = conn
            .query_row("SELECT count(*) FROM candidaturas WHERE politico_id = ?1", [id1], |r| r.get(0))
            .unwrap();
        assert_eq!(cands_id1, 2);
    }

    #[tokio::test]
    async fn test_mesclar_automatico_grupos_duplicados() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // Cria 3 registros de Rosania Santos Souza
        conn.execute(
            "INSERT INTO politicos (sq_candidato, nome_completo, nome_urna, data_nascimento)
             VALUES ('SQ_1', 'ROSANIA SANTOS SOUZA', 'ROSE', '03/04/1973')",
            [],
        )
        .unwrap();
        let id1 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf)
             VALUES (?1, 2020, 'VEREADOR', 'PT', 'BA')",
            [id1],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO politicos (sq_candidato, nome_completo, nome_urna, data_nascimento)
             VALUES ('SQ_2', 'ROSANIA SANTOS SOUZA', 'ROSE', '03/04/1973')",
            [],
        )
        .unwrap();
        let id2 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf)
             VALUES (?1, 2024, 'VEREADOR', 'PT', 'BA')",
            [id2],
        )
        .unwrap();

        let app = Router::new()
            .route("/api/politicos/duplicados/mesclar-automatico", post(mesclar_automatico_handler))
            .with_state(pool.clone());

        let req = Request::builder()
            .uri("/api/politicos/duplicados/mesclar-automatico")
            .method("POST")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // id2 deve ter sido mesclado em id1
        let count_id2: usize = conn
            .query_row("SELECT count(*) FROM politicos WHERE id = ?1", [id2], |r| r.get(0))
            .unwrap();
        assert_eq!(count_id2, 0);

        let count_cands_id1: usize = conn
            .query_row("SELECT count(*) FROM candidaturas WHERE politico_id = ?1", [id1], |r| r.get(0))
            .unwrap();
        assert_eq!(count_cands_id1, 2);
    }
}

