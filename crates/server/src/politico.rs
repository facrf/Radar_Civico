use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use storage::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BemItem {
    pub id: i64,
    pub candidatura_id: i64,
    pub ano_eleicao: i32,
    pub tipo_bem: Option<String>,
    pub descricao: Option<String>,
    pub valor_declarado: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DoadorItem {
    pub id: i64,
    pub candidatura_id: i64,
    pub ano_eleicao: i32,
    pub doador_cpf_cnpj: String,
    pub doador_nome: String,
    pub valor: f64,
    pub data_receita: Option<String>,
    pub tipo_origem: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidaturaItem {
    pub id: i64,
    pub ano_eleicao: i32,
    pub cargo: String,
    pub numero_urna: Option<i32>,
    pub sigla_partido: String,
    pub uf: String,
    pub municipio: Option<String>,
    pub situacao_totalizacao: Option<String>,
    pub total_bens_declarados: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DossiePolitico {
    pub id: i64,
    pub sq_candidato: Option<String>,
    pub cpf_mascarado: Option<String>,
    pub nome_completo: String,
    pub nome_urna: String,
    pub data_nascimento: Option<String>,
    pub grau_instrucao: Option<String>,
    pub ocupacao: Option<String>,
    pub foto_base64: Option<String>,
    pub foto_mime: Option<String>,
    pub candidaturas: Vec<CandidaturaItem>,
    pub historico_bens: Vec<BemItem>,
    pub doadores: Vec<DoadorItem>,
}

pub fn carregar_dossie(pool: &DbPool, politico_id: i64) -> Result<Option<DossiePolitico>, storage::StorageError> {
    let conn = pool.get()?;

    // 1. Dados cadastrais do Político
    let mut stmt = conn.prepare(
        "SELECT id, sq_candidato, cpf_mascarado, nome_completo, nome_urna,
                data_nascimento, grau_instrucao, ocupacao, foto_blob, foto_mime
         FROM politicos WHERE id = ?1",
    )?;

    let politico_opt = stmt
        .query_row([politico_id], |row| {
            let foto_blob: Option<Vec<u8>> = row.get(8)?;
            let foto_base64 = foto_blob.map(|b| BASE64.encode(b));

            Ok(DossiePolitico {
                id: row.get(0)?,
                sq_candidato: row.get(1)?,
                cpf_mascarado: row.get(2)?,
                nome_completo: row.get(3)?,
                nome_urna: row.get(4)?,
                data_nascimento: row.get(5)?,
                grau_instrucao: row.get(6)?,
                ocupacao: row.get(7)?,
                foto_base64,
                foto_mime: row.get(9)?,
                candidaturas: Vec::new(),
                historico_bens: Vec::new(),
                doadores: Vec::new(),
            })
        })
        .ok();

    let mut dossie = match politico_opt {
        Some(d) => d,
        None => return Ok(None),
    };

    // 2. Candidaturas
    {
        let mut stmt_cand = conn.prepare(
            "SELECT id, ano_eleicao, cargo, numero_urna, sigla_partido, uf,
                    municipio, situacao_totalizacao, total_bens_declarados
             FROM candidaturas
             WHERE politico_id = ?1
             ORDER BY ano_eleicao DESC",
        )?;

        let rows = stmt_cand.query_map([politico_id], |row| {
            Ok(CandidaturaItem {
                id: row.get(0)?,
                ano_eleicao: row.get(1)?,
                cargo: row.get(2)?,
                numero_urna: row.get(3)?,
                sigla_partido: row.get(4)?,
                uf: row.get(5)?,
                municipio: row.get(6)?,
                situacao_totalizacao: row.get(7)?,
                total_bens_declarados: row.get(8)?,
            })
        })?;

        for r in rows {
            if let Ok(cand) = r {
                dossie.candidaturas.push(cand);
            }
        }
    }

    // 3. Histórico de Bens
    {
        let mut stmt_bens = conn.prepare(
            "SELECT b.id, b.candidatura_id, c.ano_eleicao, b.tipo_bem, b.descricao, b.valor_declarado
             FROM bens_candidato b
             JOIN candidaturas c ON b.candidatura_id = c.id
             WHERE c.politico_id = ?1
             ORDER BY c.ano_eleicao DESC, b.valor_declarado DESC",
        )?;

        let rows = stmt_bens.query_map([politico_id], |row| {
            Ok(BemItem {
                id: row.get(0)?,
                candidatura_id: row.get(1)?,
                ano_eleicao: row.get(2)?,
                tipo_bem: row.get(3)?,
                descricao: row.get(4)?,
                valor_declarado: row.get(5)?,
            })
        })?;

        for r in rows {
            if let Ok(bem) = r {
                dossie.historico_bens.push(bem);
            }
        }
    }

    // 4. Doadores de Campanha
    {
        let mut stmt_doadores = conn.prepare(
            "SELECT r.id, r.candidatura_id, c.ano_eleicao, r.doador_cpf_cnpj, r.doador_nome,
                    r.valor, r.data_receita, r.tipo_origem
             FROM receitas_campanha r
             JOIN candidaturas c ON r.candidatura_id = c.id
             WHERE c.politico_id = ?1
             ORDER BY r.valor DESC",
        )?;

        let rows = stmt_doadores.query_map([politico_id], |row| {
            Ok(DoadorItem {
                id: row.get(0)?,
                candidatura_id: row.get(1)?,
                ano_eleicao: row.get(2)?,
                doador_cpf_cnpj: row.get(3)?,
                doador_nome: row.get(4)?,
                valor: row.get(5)?,
                data_receita: row.get(6)?,
                tipo_origem: row.get(7)?,
            })
        })?;

        for r in rows {
            if let Ok(doador) = r {
                dossie.doadores.push(doador);
            }
        }
    }

    Ok(Some(dossie))
}

pub async fn politico_dossie_handler(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<Json<DossiePolitico>, StatusCode> {
    match carregar_dossie(&pool, id) {
        Ok(Some(dossie)) => Ok(Json(dossie)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
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
    async fn test_politico_dossie_endpoint() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        let foto_bytes = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic bytes
        conn.execute(
            "INSERT INTO politicos (sq_candidato, cpf_mascarado, nome_completo, nome_urna, ocupacao, foto_blob, foto_mime)
             VALUES ('SQ9988', '***.555.666-**', 'MARCOS PONTE', 'ASTRONAUTA', 'ENGENHEIRO', ?1, 'image/png')",
            [&foto_bytes],
        ).unwrap();
        let pol_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf, total_bens_declarados)
             VALUES (?1, 2022, 'SENADOR', 'PL', 'SP', 2500000.0)",
            [pol_id],
        ).unwrap();
        let cand_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO bens_candidato (candidatura_id, tipo_bem, descricao, valor_declarado)
             VALUES (?1, 'IMOVEL', 'APARTAMENTO RESIDENCIAL', 1200000.0)",
            [cand_id],
        ).unwrap();

        conn.execute(
            "INSERT INTO receitas_campanha (candidatura_id, doador_cpf_cnpj, doador_nome, valor)
             VALUES (?1, '11122233344', 'DOADOR DESTAQUE', 50000.0)",
            [cand_id],
        ).unwrap();

        let app = Router::new()
            .route("/api/v1/politico/:id", get(politico_dossie_handler))
            .with_state(pool);

        // 1. Testa retorno com sucesso 200 OK
        let req = Request::builder()
            .uri(format!("/api/v1/politico/{}", pol_id))
            .body(Body::empty())
            .unwrap();

        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let dossie: DossiePolitico = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(dossie.nome_completo, "MARCOS PONTE");
        assert_eq!(dossie.nome_urna, "ASTRONAUTA");
        assert!(dossie.foto_base64.is_some());
        assert_eq!(dossie.candidaturas.len(), 1);
        assert_eq!(dossie.historico_bens.len(), 1);
        assert_eq!(dossie.doadores.len(), 1);
        assert_eq!(dossie.doadores[0].doador_nome, "DOADOR DESTAQUE");

        // 2. Testa 404 NOT_FOUND para ID inexistente
        let req_404 = Request::builder()
            .uri("/api/v1/politico/99999")
            .body(Body::empty())
            .unwrap();

        let res_404 = app.oneshot(req_404).await.unwrap();
        assert_eq!(res_404.status(), StatusCode::NOT_FOUND);
    }
}
