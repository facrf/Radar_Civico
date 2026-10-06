use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use storage::DbPool;

#[derive(Debug, Deserialize)]
pub struct BuscaParams {
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub limite: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemBuscaUnificada {
    pub tipo: String, // "POLITICO", "FORNECEDOR", "DOADOR", "SOCIO", "EMPRESA_QSA"
    pub id: Option<i64>,
    pub identificador: String,
    pub titulo: String,
    pub subtitulo: Option<String>,
    #[serde(default)]
    pub nome: String,
    #[serde(default)]
    pub detalhe: Option<String>,
    #[serde(default)]
    pub documento: Option<String>,
}

impl ItemBuscaUnificada {
    pub fn novo(tipo: &str, id: Option<i64>, identificador: &str, titulo: &str, subtitulo: Option<String>) -> Self {
        Self {
            tipo: tipo.to_string(),
            id,
            identificador: identificador.to_string(),
            titulo: titulo.to_string(),
            subtitulo: subtitulo.clone(),
            nome: titulo.to_string(),
            detalhe: subtitulo,
            documento: None,
        }
    }

    pub fn com_documento(mut self, doc: &str) -> Self {
        self.documento = Some(doc.to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RespostaBusca {
    pub query: String,
    pub total: usize,
    pub resultados: Vec<ItemBuscaUnificada>,
    #[serde(default)]
    pub itens: Vec<ItemBuscaUnificada>,
}

fn sanitizar_fts_query(q: &str) -> String {
    let clean: String = q
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    let words: Vec<&str> = clean.split_whitespace().collect();
    if words.is_empty() {
        return String::new();
    }
    words
        .into_iter()
        .map(|w| format!("{}*", w))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn executar_busca(pool: &DbPool, termo: &str, limite: usize) -> Result<Vec<ItemBuscaUnificada>, storage::StorageError> {
    let conn = pool.get()?;
    let termo_trim = termo.trim();
    let fts_query = sanitizar_fts_query(termo_trim);
    let mut resultados = Vec::new();

    let digits: String = termo_trim.chars().filter(|c| c.is_ascii_digit()).collect();

    // 1. Busca em politicos_fts (se houver texto para FTS)
    if !fts_query.is_empty() {
        let mut stmt = conn.prepare(
            "SELECT politico_id, nome_completo, nome_urna, sq_candidato
             FROM politicos_fts
             WHERE politicos_fts MATCH ?1
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(storage::rusqlite::params![fts_query, limite], |row| {
            let id: Option<i64> = row.get(0)?;
            let nome_completo: String = row.get(1)?;
            let nome_urna: String = row.get(2)?;
            let sq: String = row.get(3)?;

            Ok(ItemBuscaUnificada::novo(
                "POLITICO",
                id,
                &sq,
                &nome_completo,
                Some(format!("Nome de urna: {}", nome_urna)),
            ).com_documento(&sq))
        })?;

        for r in rows.flatten() {
            resultados.push(r);
        }
    }

    // 2. Busca direta em politicos por CPF ou SQ candidato
    if (!digits.is_empty()) && resultados.len() < limite {
        let rem_limite = limite - resultados.len();
        let mut stmt = conn.prepare(
            "SELECT id, nome_completo, nome_urna, sq_candidato, cpf_mascarado
             FROM politicos
             WHERE sq_candidato = ?1
                OR (cpf_mascarado IS NOT NULL AND cpf_mascarado != '-4' AND (cpf_mascarado LIKE ?2 OR cpf_mascarado LIKE ?3))
             LIMIT ?4",
        )?;

        let like_digits = format!("%{}%", digits);
        let like_termo = format!("%{}%", termo_trim);

        let rows = stmt.query_map(
            storage::rusqlite::params![termo_trim, like_digits, like_termo, rem_limite],
            |row| {
                let id: Option<i64> = row.get(0)?;
                let nome_completo: String = row.get(1)?;
                let nome_urna: String = row.get(2)?;
                let sq: String = row.get(3)?;
                let cpf: Option<String> = row.get(4)?;

                let mut item = ItemBuscaUnificada::novo(
                    "POLITICO",
                    id,
                    &sq,
                    &nome_completo,
                    Some(format!(
                        "Nome de urna: {}{}",
                        nome_urna,
                        cpf.as_ref().map(|c| format!(" • CPF: {}", c)).unwrap_or_default()
                    )),
                );
                if let Some(ref c) = cpf {
                    item = item.com_documento(c);
                } else {
                    item = item.com_documento(&sq);
                }
                Ok(item)
            },
        )?;

        for r in rows.flatten() {
            if !resultados.iter().any(|existing| existing.identificador == r.identificador) {
                resultados.push(r);
            }
        }
    }

    // 3. Busca em fornecedores_fts
    if (!fts_query.is_empty() || !digits.is_empty()) && resultados.len() < limite {
        let rem_limite = limite - resultados.len();
        let query_fts = if !fts_query.is_empty() {
            fts_query.clone()
        } else {
            format!("{}*", digits)
        };

        if let Ok(mut stmt) = conn.prepare(
            "SELECT DISTINCT fornecedor_cpf_cnpj, fornecedor_nome
             FROM fornecedores_fts
             WHERE fornecedores_fts MATCH ?1
             LIMIT ?2",
        ) {
            let rows = stmt.query_map(storage::rusqlite::params![query_fts, rem_limite], |row| {
                let doc: String = row.get(0)?;
                let nome: String = row.get(1)?;

                Ok(ItemBuscaUnificada::novo(
                    "FORNECEDOR",
                    None,
                    &doc,
                    &nome,
                    Some("Fornecedor contratado/declarado".to_string()),
                ).com_documento(&doc))
            })?;

            for r in rows.flatten() {
                if !resultados.iter().any(|existing| existing.identificador == r.identificador && existing.titulo == r.titulo) {
                    resultados.push(r);
                }
            }
        }
    }

    // 4. Busca em receitas_campanha (Doadores)
    if (!termo_trim.is_empty()) && resultados.len() < limite {
        let rem_limite = limite - resultados.len();
        let like_query = format!("%{}%", termo_trim);
        let mut stmt = conn.prepare(
            "SELECT DISTINCT doador_cpf_cnpj, doador_nome
             FROM receitas_campanha
             WHERE doador_nome LIKE ?1 OR doador_cpf_cnpj LIKE ?1
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(storage::rusqlite::params![like_query, rem_limite], |row| {
            let doc: String = row.get(0)?;
            let nome: String = row.get(1)?;

            Ok(ItemBuscaUnificada::novo(
                "DOADOR",
                None,
                &doc,
                &nome,
                Some("Doador eleitoral".to_string()),
            ).com_documento(&doc))
        })?;

        for r in rows.flatten() {
            if !resultados.iter().any(|existing| existing.identificador == r.identificador && existing.titulo == r.titulo) {
                resultados.push(r);
            }
        }
    }

    // 5. Busca na base de Sócios e Administradores (empresas_qsa)
    // Suporta CPF completo (11 dígitos), 6 dígitos do miolo, CPF mascarado (***123456**) e CNPJ (8 ou 14 dígitos)
    if resultados.len() < limite {
        let mut qsa_cpf_candidates = Vec::new();

        if termo_trim.contains("***") {
            qsa_cpf_candidates.push(termo_trim.to_string());
        }

        if digits.len() == 11 {
            // Em dados abertos da RFB, o CPF do sócio é mascarado exibindo os 6 dígitos centrais (índice 3 a 9)
            let miolo = &digits[3..9];
            qsa_cpf_candidates.push(format!("***{}**", miolo));
            qsa_cpf_candidates.push(format!("***.{}.{}-**", &miolo[0..3], &miolo[3..6]));
            qsa_cpf_candidates.push(digits.clone());
        } else if digits.len() == 6 {
            qsa_cpf_candidates.push(format!("***{}**", digits));
            qsa_cpf_candidates.push(format!("***.{}.{}-**", &digits[0..3], &digits[3..6]));
        }

        for mask in qsa_cpf_candidates {
            if resultados.len() >= limite {
                break;
            }
            let rem_limite = limite - resultados.len();
            if let Ok(mut stmt) = conn.prepare(
                "SELECT cnpj_basico, cnpj_ordem, cnpj_dv, razao_social, socio_cpf_cnpj_mascarado, socio_nome, qualificacao_socio
                 FROM empresas_qsa
                 WHERE socio_cpf_cnpj_mascarado = ?1
                 LIMIT ?2",
            ) {
                let rows = stmt.query_map(storage::rusqlite::params![mask, rem_limite], |row| {
                    let cnpj_b: String = row.get(0)?;
                    let cnpj_o: String = row.get(1)?;
                    let cnpj_d: String = row.get(2)?;
                    let razao: String = row.get(3)?;
                    let socio_doc: String = row.get(4)?;
                    let socio_nome: String = row.get(5)?;
                    let qualif: Option<String> = row.get(6)?;

                    let cnpj_fmt = format!(
                        "{}.{}.{}/{}-{}",
                        if cnpj_b.len() >= 2 { &cnpj_b[0..2] } else { &cnpj_b },
                        if cnpj_b.len() >= 5 { &cnpj_b[2..5] } else { "" },
                        if cnpj_b.len() >= 8 { &cnpj_b[5..8] } else { "" },
                        cnpj_o,
                        cnpj_d
                    );

                    let titulo = if socio_nome.is_empty() || socio_nome == socio_doc {
                        format!("Sócio {}", socio_doc)
                    } else {
                        socio_nome
                    };

                    let subtitulo = Some(format!(
                        "Sócio em {} (CNPJ: {}){}",
                        razao,
                        cnpj_fmt,
                        qualif.map(|q| format!(" - {}", q)).unwrap_or_default()
                    ));

                    Ok(ItemBuscaUnificada::novo(
                        "SOCIO",
                        None,
                        &cnpj_fmt,
                        &titulo,
                        subtitulo,
                    ).com_documento(&socio_doc))
                })?;

                for r in rows.flatten() {
                    if !resultados.iter().any(|existing| existing.identificador == r.identificador && existing.titulo == r.titulo) {
                        resultados.push(r);
                    }
                }
            }
        }

        // Busca por CNPJ básico (8 ou 14 dígitos) em empresas_qsa
        if (digits.len() == 8 || digits.len() == 14) && resultados.len() < limite {
            let cnpj_b = &digits[0..8];
            let rem_limite = limite - resultados.len();
            if let Ok(mut stmt) = conn.prepare(
                "SELECT cnpj_basico, cnpj_ordem, cnpj_dv, razao_social, socio_cpf_cnpj_mascarado, socio_nome, qualificacao_socio
                 FROM empresas_qsa
                 WHERE cnpj_basico = ?1
                 LIMIT ?2",
            ) {
                let rows = stmt.query_map(storage::rusqlite::params![cnpj_b, rem_limite], |row| {
                    let b: String = row.get(0)?;
                    let o: String = row.get(1)?;
                    let d: String = row.get(2)?;
                    let razao: String = row.get(3)?;
                    let s_doc: String = row.get(4)?;
                    let s_nome: String = row.get(5)?;
                    let qualif: Option<String> = row.get(6)?;

                    let cnpj_fmt = format!(
                        "{}.{}.{}/{}-{}",
                        if b.len() >= 2 { &b[0..2] } else { &b },
                        if b.len() >= 5 { &b[2..5] } else { "" },
                        if b.len() >= 8 { &b[5..8] } else { "" },
                        o,
                        d
                    );

                    let titulo = razao;
                    let subtitulo = Some(format!(
                        "Empresa com sócio {} ({}){}",
                        s_nome,
                        s_doc,
                        qualif.map(|q| format!(" - {}", q)).unwrap_or_default()
                    ));

                    Ok(ItemBuscaUnificada::novo(
                        "EMPRESA_QSA",
                        None,
                        &cnpj_fmt,
                        &titulo,
                        subtitulo,
                    ).com_documento(&cnpj_fmt))
                })?;

                for r in rows.flatten() {
                    if !resultados.iter().any(|existing| existing.identificador == r.identificador && existing.titulo == r.titulo) {
                        resultados.push(r);
                    }
                }
            }
        }
    }

    Ok(resultados)
}

pub async fn busca_handler(
    State(pool): State<DbPool>,
    Query(params): Query<BuscaParams>,
) -> Result<Json<RespostaBusca>, StatusCode> {
    let limite = params.limite.unwrap_or(20).min(100);
    let termo = params.q.clone();

    let resultados = match executar_busca(&pool, &termo, limite) {
        Ok(res) => res,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    Ok(Json(RespostaBusca {
        query: termo,
        total: resultados.len(),
        itens: resultados.clone(),
        resultados,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::Router;
    use storage::run_migrations;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_busca_unificada_fts5_endpoint() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // 1. Insere Político (vai para politicos_fts via trigger)
        conn.execute(
            "INSERT INTO politicos (sq_candidato, nome_completo, nome_urna)
             VALUES ('SQ100', 'FERNANDO HADDAD', 'HADDAD')",
            [],
        ).unwrap();

        // 2. Insere Despesa (vai para fornecedores_fts via trigger)
        conn.execute(
            "INSERT INTO despesas_campanha (fornecedor_cpf_cnpj, fornecedor_nome, valor)
             VALUES ('12345678000199', 'GRAFICA HADDAD E IRMAOS', 5000.0)",
            [],
        ).unwrap();

        // 3. Insere Doador em receitas_campanha
        conn.execute(
            "INSERT INTO receitas_campanha (doador_cpf_cnpj, doador_nome, valor)
             VALUES ('99988877766', 'FERNANDO SILVEIRA', 1000.0)",
            [],
        ).unwrap();

        let app = Router::new()
            .route("/api/v1/busca", get(busca_handler))
            .with_state(pool);

        let req = Request::builder()
            .uri("/api/v1/busca?q=HADDAD")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let resposta: RespostaBusca = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(resposta.query, "HADDAD");
        assert!(resposta.total >= 2);

        let tipos: Vec<String> = resposta.resultados.iter().map(|r| r.tipo.clone()).collect();
        assert!(tipos.contains(&"POLITICO".to_string()));
        assert!(tipos.contains(&"FORNECEDOR".to_string()));
    }

    #[tokio::test]
    async fn test_busca_cpf_e_qsa_socio_endpoint() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // Insere vínculo societário em empresas_qsa com CPF mascarado no formato oficial da Receita (***456789**)
        conn.execute(
            "INSERT INTO empresas_qsa (cnpj_basico, cnpj_ordem, cnpj_dv, razao_social, socio_cpf_cnpj_mascarado, socio_nome, qualificacao_socio)
             VALUES ('12345678', '0001', '90', 'ALPHA SERVICOS DIGITAIS LTDA', '***456789**', 'MARIO SERGIO SILVA', '49-Sócio-Administrador')",
            [],
        ).unwrap();

        let app = Router::new()
            .route("/api/v1/busca", get(busca_handler))
            .with_state(pool);

        // 1. Testa busca pelo CPF completo formatado (123.456.789-00)
        let req1 = Request::builder()
            .uri("/api/v1/busca?q=123.456.789-00")
            .body(Body::empty())
            .unwrap();

        let res1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(res1.status(), StatusCode::OK);

        let bytes1 = axum::body::to_bytes(res1.into_body(), usize::MAX).await.unwrap();
        let resposta1: RespostaBusca = serde_json::from_slice(&bytes1).unwrap();

        assert_eq!(resposta1.total, 1);
        assert_eq!(resposta1.itens.len(), 1); // Garante compatibilidade com o front
        let item = &resposta1.resultados[0];
        assert_eq!(item.tipo, "SOCIO");
        assert_eq!(item.titulo, "MARIO SERGIO SILVA");
        assert_eq!(item.nome, "MARIO SERGIO SILVA"); // Garante campo .nome
        assert!(item.subtitulo.as_ref().unwrap().contains("ALPHA SERVICOS"));

        // 2. Testa busca pelo miolo do CPF (456789)
        let req2 = Request::builder()
            .uri("/api/v1/busca?q=456789")
            .body(Body::empty())
            .unwrap();

        let res2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(res2.status(), StatusCode::OK);

        let bytes2 = axum::body::to_bytes(res2.into_body(), usize::MAX).await.unwrap();
        let resposta2: RespostaBusca = serde_json::from_slice(&bytes2).unwrap();
        assert_eq!(resposta2.total, 1);
        assert_eq!(resposta2.resultados[0].tipo, "SOCIO");
    }
}
