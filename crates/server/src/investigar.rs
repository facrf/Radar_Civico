use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use auditor::{auditar_conflito_oab, is_cargo_incompativel, AlertaConflitoOab, OcupanteCargo, RegistroOab};
use ingestion::{
    ExcerptDiario, OabConsultaResult, OabScraperClient, QueridoDiarioApiResponse, QueridoDiarioClient,
};
use serde::{Deserialize, Serialize};
use storage::rusqlite::Connection;
use storage::DbPool;

use crate::alertas::{registrar_alerta, NovoAlerta};

#[derive(Debug, Clone, Deserialize)]
pub struct InvestigarParams {
    pub municipio: Option<String>,
    pub uf: Option<String>,
    pub forcar: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InvestigacaoNomeacaoResponse {
    pub doador_id: String,
    pub doador_nome: String,
    pub doador_cpf_cnpj: String,
    pub municipio: Option<String>,
    pub uf: Option<String>,
    pub total_ocorrencias_diario: usize,
    pub trechos_diario: Vec<ExcerptDiario>,
    pub registros_oab: Vec<OabConsultaResult>,
    pub alerta_conflito: Option<AlertaConflitoOab>,
    pub status: String,
}

pub struct DoadorIdentificado {
    pub id: i64,
    pub nome: String,
    pub documento: String,
    pub municipio: Option<String>,
    pub uf: Option<String>,
}

pub fn identificar_doador(conn: &Connection, termo: &str) -> Option<DoadorIdentificado> {
    let termo_trim = termo.trim();

    // 1. Tenta por ID em receitas_campanha
    if let Ok(id_num) = termo_trim.parse::<i64>() {
        let mut stmt = conn
            .prepare(
                "SELECT id, doador_nome, doador_cpf_cnpj
                 FROM receitas_campanha WHERE id = ?1 LIMIT 1",
            )
            .ok()?;
        if let Ok(item) = stmt.query_row([id_num], |row| {
            Ok(DoadorIdentificado {
                id: row.get(0)?,
                nome: row.get(1)?,
                documento: row.get(2)?,
                municipio: None,
                uf: None,
            })
        }) {
            return Some(item);
        }
    }

    // 2. Tenta por ID ou UUID em nos_rede
    if let Ok(id_num) = termo_trim.parse::<i64>() {
        let mut stmt = conn
            .prepare(
                "SELECT id, nome, COALESCE(documento, ''), municipio, uf
                 FROM nos_rede WHERE id = ?1 LIMIT 1",
            )
            .ok()?;
        if let Ok(item) = stmt.query_row([id_num], |row| {
            Ok(DoadorIdentificado {
                id: row.get(0)?,
                nome: row.get(1)?,
                documento: row.get(2)?,
                municipio: row.get(3)?,
                uf: row.get(4)?,
            })
        }) {
            return Some(item);
        }
    }

    // Tenta por UUID em nos_rede
    {
        let mut stmt = conn
            .prepare(
                "SELECT id, nome, COALESCE(documento, ''), municipio, uf
                 FROM nos_rede WHERE uuid = ?1 LIMIT 1",
            )
            .ok()?;
        if let Ok(item) = stmt.query_row([termo_trim], |row| {
            Ok(DoadorIdentificado {
                id: row.get(0)?,
                nome: row.get(1)?,
                documento: row.get(2)?,
                municipio: row.get(3)?,
                uf: row.get(4)?,
            })
        }) {
            return Some(item);
        }
    }

    // 3. Tenta por CPF/CNPJ em receitas_campanha
    {
        let mut stmt = conn
            .prepare(
                "SELECT id, doador_nome, doador_cpf_cnpj
                 FROM receitas_campanha
                 WHERE doador_cpf_cnpj = ?1 OR doador_nome = ?1
                 LIMIT 1",
            )
            .ok()?;
        if let Ok(item) = stmt.query_row([termo_trim], |row| {
            Ok(DoadorIdentificado {
                id: row.get(0)?,
                nome: row.get(1)?,
                documento: row.get(2)?,
                municipio: None,
                uf: None,
            })
        }) {
            return Some(item);
        }
    }

    None
}

fn extrair_cargo_de_trechos(trechos: &[ExcerptDiario]) -> Option<(String, String, String)> {
    let cargos_alvo = [
        "SECRETÁRIO MUNICIPAL",
        "SECRETARIO MUNICIPAL",
        "SECRETÁRIO DE ESTADO",
        "SECRETARIO DE ESTADO",
        "SECRETÁRIO",
        "SECRETARIO",
        "DIRETOR-GERAL",
        "DIRETOR",
        "PRESIDENTE",
        "SUPERINTENDENTE",
        "CHEFE DE GABINETE",
        "PROCURADOR-GERAL",
    ];

    for t in trechos {
        let texto_upper = t.excerpt.to_uppercase();
        for &cargo in &cargos_alvo {
            if texto_upper.contains(cargo) {
                let orgao = t
                    .territory_name
                    .clone()
                    .unwrap_or_else(|| "PREFEITURA / GOVERNO".to_string());
                let data = t.date.clone().unwrap_or_else(|| "2024-01-01".to_string());
                return Some((cargo.to_string(), orgao, data));
            }
        }
    }

    None
}

pub async fn executar_investigacao(
    conn: &mut Connection,
    doador: DoadorIdentificado,
    params: InvestigarParams,
) -> Result<InvestigacaoNomeacaoResponse, storage::StorageError> {
    let municipio = params.municipio.or(doador.municipio);
    let uf = params.uf.or(doador.uf);
    let municipio_uf = format!(
        "{}-{}",
        municipio.as_deref().unwrap_or("GERAL"),
        uf.as_deref().unwrap_or("BR")
    );

    let forcar = params.forcar.unwrap_or(false);

    // 1. Consulta Querido Diário (com cache em SQLite)
    let qd_cache = if !forcar {
        QueridoDiarioClient::buscar_cache(conn, &doador.documento, &municipio_uf).unwrap_or(None)
    } else {
        None
    };

    let qd_res = if let Some(cached) = qd_cache {
        cached
    } else {
        let client_qd = QueridoDiarioClient::new();
        match client_qd
            .buscar_nomeacoes(&doador.nome, municipio.as_deref(), None)
            .await
        {
            Ok(api_res) => {
                let payload = serde_json::to_string(&api_res).unwrap_or_default();
                let _ = QueridoDiarioClient::salvar_cache(
                    conn,
                    &doador.documento,
                    &doador.nome,
                    &municipio_uf,
                    api_res.total_gazettes,
                    &payload,
                );
                api_res
            }
            Err(_) => {
                // Fallback gracioso para cache ou vazio
                QueridoDiarioClient::buscar_cache(conn, &doador.documento, &municipio_uf)
                    .unwrap_or(None)
                    .unwrap_or(QueridoDiarioApiResponse {
                        total_gazettes: 0,
                        gazettes: Vec::new(),
                    })
            }
        }
    };

    // 2. Consulta OAB / CNA (com registros em SQLite)
    let oab_local = if !forcar {
        OabScraperClient::buscar_registro_banco(conn, &doador.nome, uf.as_deref())
            .unwrap_or(None)
            .map(|r| vec![r])
    } else {
        None
    };

    let oab_registros = if let Some(local) = oab_local {
        local
    } else {
        let client_oab = OabScraperClient::new();
        match client_oab.consultar_cna(&doador.nome, uf.as_deref()).await {
            Ok(resultados) => {
                for r in &resultados {
                    let _ = OabScraperClient::salvar_registro_oab(conn, r, Some(&doador.documento));
                }
                resultados
            }
            Err(_) => {
                // Fallback para registro pré-existente no banco
                OabScraperClient::buscar_registro_banco(conn, &doador.nome, uf.as_deref())
                    .unwrap_or(None)
                    .map(|r| vec![r])
                    .unwrap_or_default()
            }
        }
    };

    // 3. Auditoria de Conflito de Interesses (Art. 28 OAB)
    let mut alerta_conflito = None;
    let mut status = "REGULAR_SEM_ALERTA".to_string();

    if let Some((cargo, orgao, data_nomeacao)) = extrair_cargo_de_trechos(&qd_res.gazettes) {
        if is_cargo_incompativel(&cargo) {
            for reg in &oab_registros {
                let reg_oab = RegistroOab {
                    pessoa_nome: reg.nome.clone(),
                    numero_registro: reg.inscricao.clone(),
                    seccional_uf: reg.uf.clone(),
                    situacao_registro: reg.situacao.clone(),
                };

                let ocupante = OcupanteCargo {
                    id: doador.id,
                    nome: doador.nome.clone(),
                    cpf_mascarado: Some(doador.documento.clone()),
                    cargo: cargo.clone(),
                    orgao: orgao.clone(),
                    municipio: municipio.clone().unwrap_or_default(),
                    uf: uf.clone().unwrap_or_default(),
                    data_nomeacao: data_nomeacao.clone(),
                    ativo: true,
                };

                if let Some(alerta) = auditar_conflito_oab(&ocupante, &reg_oab) {
                    status = "ALERTA_GERADO".to_string();

                    // Registra na tabela de alertas de auditoria consolidada
                    let ano = data_nomeacao.get(0..4).and_then(|y| y.parse::<i32>().ok());
                    let detalhes = serde_json::json!({
                        "doador_id": doador.id,
                        "oab_numero": reg.inscricao,
                        "oab_uf": reg.uf,
                        "cargo": cargo,
                        "orgao": orgao,
                        "data_nomeacao": data_nomeacao,
                    });

                    let _ = registrar_alerta(
                        conn,
                        &NovoAlerta {
                            tipo: "CONFLITO_OAB".to_string(),
                            severidade: "CRITICA".to_string(),
                            titulo: format!(
                                "Incompatibilidade da Advocacia (Art. 28 OAB) - {}",
                                doador.nome
                            ),
                            descricao: format!(
                                "Nomeação em cargo incompatível ('{}' em '{}') com OAB/{}-{} regular.",
                                cargo, orgao, reg.uf, reg.inscricao
                            ),
                            alvo_nome: doador.nome.clone(),
                            alvo_documento: Some(doador.documento.clone()),
                            municipio: municipio.clone(),
                            uf: uf.clone(),
                            ano,
                            valor_envolvido: None,
                            fonte_dado: "QUERIDO_DIARIO/OAB".to_string(),
                            detalhes_json: Some(detalhes.to_string()),
                        },
                    );

                    alerta_conflito = Some(alerta);
                    break;
                }
            }
        }
    }

    Ok(InvestigacaoNomeacaoResponse {
        doador_id: doador.id.to_string(),
        doador_nome: doador.nome,
        doador_cpf_cnpj: doador.documento,
        municipio,
        uf,
        total_ocorrencias_diario: qd_res.total_gazettes as usize,
        trechos_diario: qd_res.gazettes,
        registros_oab: oab_registros,
        alerta_conflito,
        status,
    })
}

pub async fn investigar_nomeacao_handler(
    State(pool): State<DbPool>,
    Path(doador_id): Path<String>,
    Query(params): Query<InvestigarParams>,
) -> Result<Json<InvestigacaoNomeacaoResponse>, StatusCode> {
    let mut conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let doador = identificar_doador(&conn, &doador_id).ok_or(StatusCode::NOT_FOUND)?;

    let resp = executar_investigacao(&mut conn, doador, params)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(resp))
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
    async fn test_investigar_nomeacao_com_conflito_oab() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // 1. Cadastra político e receita do doador advogado
        conn.execute(
            "INSERT INTO politicos (nome_completo, nome_urna) VALUES ('PREFEITO ELEITO', 'PREFEITO')",
            [],
        ).unwrap();
        let pol_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf, municipio)
             VALUES (?1, 2024, 'PREFEITO', 'PARTIDO X', 'SP', 'Campinas')",
            [pol_id],
        ).unwrap();
        let cand_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO receitas_campanha (candidatura_id, doador_cpf_cnpj, doador_nome, valor, data_receita)
             VALUES (?1, '11122233344', 'DR ADVOGADO NOMEADO', 5000.0, '2024-09-01')",
            [cand_id],
        ).unwrap();
        let receita_id = conn.last_insert_rowid();

        // 2. Pré-alimenta o cache do Querido Diário com extrato de nomeação como SECRETÁRIO
        let gazette = ExcerptDiario {
            date: Some("2025-01-02".to_string()),
            territory_name: Some("Campinas".to_string()),
            territory_id: Some("3509502".to_string()),
            url: Some("https://diario.campinas.sp.gov.br/edicao/123".to_string()),
            excerpt: "RESOLVE: NOMEAR DR ADVOGADO NOMEADO PARA O CARGO DE SECRETÁRIO MUNICIPAL DE FINANÇAS.".to_string(),
        };
        let qd_api = QueridoDiarioApiResponse {
            total_gazettes: 1,
            gazettes: vec![gazette],
        };
        QueridoDiarioClient::salvar_cache(
            &mut conn,
            "11122233344",
            "DR ADVOGADO NOMEADO",
            "Campinas-SP",
            1,
            &serde_json::to_string(&qd_api).unwrap(),
        ).unwrap();

        // 3. Pré-alimenta registro profissional da OAB ativo/regular
        let oab_rec = OabConsultaResult {
            nome: "DR ADVOGADO NOMEADO".to_string(),
            inscricao: "123456".to_string(),
            uf: "SP".to_string(),
            situacao: "REGULAR".to_string(),
            tipo: "ADVOGADO".to_string(),
            payload_json: "{}".to_string(),
        };
        OabScraperClient::salvar_registro_oab(&mut conn, &oab_rec, Some("11122233344")).unwrap();

        // 4. Executa a investigação
        let doador = identificar_doador(&conn, &receita_id.to_string()).unwrap();
        assert_eq!(doador.nome, "DR ADVOGADO NOMEADO");

        let resultado = executar_investigacao(
            &mut conn,
            doador,
            InvestigarParams {
                municipio: Some("Campinas".to_string()),
                uf: Some("SP".to_string()),
                forcar: Some(false),
            },
        ).await.unwrap();

        assert_eq!(resultado.status, "ALERTA_GERADO");
        assert_eq!(resultado.total_ocorrencias_diario, 1);
        assert_eq!(resultado.registros_oab.len(), 1);
        assert!(resultado.alerta_conflito.is_some());

        let alerta = resultado.alerta_conflito.unwrap();
        assert_eq!(alerta.oab_registro, "123456");
        assert_eq!(alerta.situacao_oab, "REGULAR");
        assert!(alerta.cargo.contains("SECRETÁRIO"));

        // Valida que o alerta foi gravado em alertas_auditoria
        let total_alertas: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM alertas_auditoria WHERE tipo = 'CONFLITO_OAB'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(total_alertas, 1);
    }

    #[tokio::test]
    async fn test_investigar_nomeacao_endpoint_http() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, documento, nome, municipio, uf)
             VALUES ('doador-999', 'PESSOA_FISICA', '55566677788', 'DOADOR COMUM', 'Santos', 'SP')",
            [],
        ).unwrap();

        let qd_empty = QueridoDiarioApiResponse {
            total_gazettes: 0,
            gazettes: Vec::new(),
        };
        QueridoDiarioClient::salvar_cache(
            &mut conn,
            "55566677788",
            "DOADOR COMUM",
            "Santos-SP",
            0,
            &serde_json::to_string(&qd_empty).unwrap(),
        ).unwrap();

        let oab_rec = OabConsultaResult {
            nome: "DOADOR COMUM".to_string(),
            inscricao: "999999".to_string(),
            uf: "SP".to_string(),
            situacao: "REGULAR".to_string(),
            tipo: "ADVOGADO".to_string(),
            payload_json: "{}".to_string(),
        };
        OabScraperClient::salvar_registro_oab(&mut conn, &oab_rec, Some("55566677788")).unwrap();

        let app = Router::new()
            .route("/api/v1/investigar/nomeacao/:doador_id", get(investigar_nomeacao_handler))
            .with_state(pool);

        let req = Request::builder()
            .uri("/api/v1/investigar/nomeacao/doador-999")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
        let json: InvestigacaoNomeacaoResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.doador_nome, "DOADOR COMUM");
        assert_eq!(json.status, "REGULAR_SEM_ALERTA");
    }
}
