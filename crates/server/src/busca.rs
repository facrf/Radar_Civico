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
    pub tipo: String, // "POLITICO", "FORNECEDOR", "DOADOR"
    pub id: Option<i64>,
    pub identificador: String,
    pub titulo: String,
    pub subtitulo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RespostaBusca {
    pub query: String,
    pub total: usize,
    pub resultados: Vec<ItemBuscaUnificada>,
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
    let fts_query = sanitizar_fts_query(termo);
    let mut resultados = Vec::new();

    if fts_query.is_empty() {
        return Ok(resultados);
    }

    // 1. Busca em politicos_fts
    {
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

            Ok(ItemBuscaUnificada {
                tipo: "POLITICO".to_string(),
                id,
                identificador: sq,
                titulo: nome_completo,
                subtitulo: Some(format!("Nome de urna: {}", nome_urna)),
            })
        })?;

        for r in rows {
            if let Ok(item) = r {
                resultados.push(item);
            }
        }
    }

    // 2. Busca em fornecedores_fts
    {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT fornecedor_cpf_cnpj, fornecedor_nome
             FROM fornecedores_fts
             WHERE fornecedores_fts MATCH ?1
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(storage::rusqlite::params![fts_query, limite], |row| {
            let doc: String = row.get(0)?;
            let nome: String = row.get(1)?;

            Ok(ItemBuscaUnificada {
                tipo: "FORNECEDOR".to_string(),
                id: None,
                identificador: doc,
                titulo: nome,
                subtitulo: Some("Fornecedor contratado/declarado".to_string()),
            })
        })?;

        for r in rows {
            if let Ok(item) = r {
                resultados.push(item);
            }
        }
    }

    // 3. Busca em receitas_campanha (Doadores)
    {
        let like_query = format!("%{}%", termo.trim());
        let mut stmt = conn.prepare(
            "SELECT DISTINCT doador_cpf_cnpj, doador_nome
             FROM receitas_campanha
             WHERE doador_nome LIKE ?1 OR doador_cpf_cnpj LIKE ?1
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(storage::rusqlite::params![like_query, limite], |row| {
            let doc: String = row.get(0)?;
            let nome: String = row.get(1)?;

            Ok(ItemBuscaUnificada {
                tipo: "DOADOR".to_string(),
                id: None,
                identificador: doc,
                titulo: nome,
                subtitulo: Some("Doador eleitoral".to_string()),
            })
        })?;

        for r in rows {
            if let Ok(item) = r {
                resultados.push(item);
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
}
