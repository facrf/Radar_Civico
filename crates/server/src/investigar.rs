use auditor::{
    auditar_conflito_oab, is_cargo_incompativel, AlertaConflitoOab, OcupanteCargo, RegistroOab,
};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use ingestion::{
    ExcerptDiario, OabConsultaResult, OabScraperClient, QueridoDiarioApiResponse,
    QueridoDiarioClient,
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
    pub fontes: Vec<EstadoFonte>,
    pub avisos: Vec<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EstadoFonte {
    pub fonte: String,
    pub situacao: String,
    pub mensagem: Option<String>,
}

struct CargoVerificado {
    politico_id: i64,
    cargo: String,
    orgao: String,
    data_posse: String,
    fonte_url: String,
}

struct InvestigacaoPreparada {
    doador: DoadorIdentificado,
    municipio: Option<String>,
    uf: Option<String>,
    chave_cache: String,
    forcar: bool,
    diario: Option<QueridoDiarioApiResponse>,
    oab: Vec<ingestion::RegistroOabLocal>,
    cargos: Vec<CargoVerificado>,
}

fn erro_ingestao(error: ingestion::IngestionError) -> storage::StorageError {
    storage::StorageError::Pool(error.to_string())
}

fn preparar(
    conn: &Connection,
    doador: DoadorIdentificado,
    params: InvestigarParams,
) -> storage::Result<InvestigacaoPreparada> {
    let municipio = params.municipio.or_else(|| doador.municipio.clone());
    let uf = params.uf.or_else(|| doador.uf.clone());
    let chave_cache = format!(
        "{}-{}",
        municipio.as_deref().unwrap_or("GERAL"),
        uf.as_deref().unwrap_or("BR")
    );
    let diario = QueridoDiarioClient::buscar_cache(conn, &doador.documento, &chave_cache)
        .map_err(erro_ingestao)?;
    let oab = OabScraperClient::buscar_registros_banco(conn, &doador.nome, uf.as_deref())
        .map_err(erro_ingestao)?;
    let mut cargos = Vec::new();
    if crate::identity::complete(&doador.documento)
        && crate::identity::digits(&doador.documento).len() == 11
    {
        let documento = crate::identity::document_sql("p.cpf_mascarado");
        let mut stmt = conn.prepare(&format!("SELECT c.politico_id,c.cargo,c.orgao,c.data_posse,c.fonte_url,c.conferido_em FROM cargos_autoridades c JOIN politicos p ON p.id=c.politico_id WHERE {documento}=?1 AND c.evidencia_verificada=1 AND date(c.data_posse)<=date('now') AND (NULLIF(c.data_exoneracao,'') IS NULL OR date(c.data_exoneracao)>date('now'))"))?;
        let mut rows = stmt.query([crate::identity::digits(&doador.documento)])?;
        while let Some(row) = rows.next()? {
            let url: Option<String> = row.get(4)?;
            let conferencia: Option<String> = row.get(5)?;
            let Some(url) = url.filter(|v| {
                reqwest::Url::parse(v)
                    .is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some())
            }) else {
                continue;
            };
            if !conferencia.as_deref().is_some_and(|v| {
                chrono::DateTime::parse_from_rfc3339(v)
                    .is_ok_and(|d| d.with_timezone(&chrono::Utc) <= chrono::Utc::now())
            }) {
                continue;
            }
            let cargo: String = row.get(1)?;
            if is_cargo_incompativel(&cargo) {
                cargos.push(CargoVerificado {
                    politico_id: row.get(0)?,
                    cargo,
                    orgao: row.get(2)?,
                    data_posse: row.get(3)?,
                    fonte_url: url,
                });
            }
        }
    }
    Ok(InvestigacaoPreparada {
        doador,
        municipio,
        uf,
        chave_cache,
        forcar: params.forcar.unwrap_or(false),
        diario,
        oab,
        cargos,
    })
}

struct Consultas {
    diario: QueridoDiarioApiResponse,
    diario_novo: bool,
    oab_remota: Vec<OabConsultaResult>,
    fontes: Vec<EstadoFonte>,
}

async fn consultar(preparada: &InvestigacaoPreparada) -> Consultas {
    let diario_consulta = async {
        if !preparada.forcar {
            if let Some(cache) = &preparada.diario {
                return (
                    cache.clone(),
                    false,
                    EstadoFonte {
                        fonte: "QUERIDO_DIARIO".into(),
                        situacao: "CACHE_LOCAL".into(),
                        mensagem: None,
                    },
                );
            }
        }
        match QueridoDiarioClient::new()
            .buscar_nomeacoes(&preparada.doador.nome, preparada.municipio.as_deref(), None)
            .await
        {
            Ok(resultado) => (
                resultado,
                true,
                EstadoFonte {
                    fonte: "QUERIDO_DIARIO".into(),
                    situacao: "DISPONIVEL".into(),
                    mensagem: None,
                },
            ),
            Err(error) => (
                preparada
                    .diario
                    .clone()
                    .unwrap_or(QueridoDiarioApiResponse {
                        total_gazettes: 0,
                        gazettes: Vec::new(),
                    }),
                false,
                EstadoFonte {
                    fonte: "QUERIDO_DIARIO".into(),
                    situacao: if preparada.diario.is_some() {
                        "CACHE_APOS_FALHA"
                    } else {
                        "INDISPONIVEL"
                    }
                    .into(),
                    mensagem: Some(error.to_string()),
                },
            ),
        }
    };
    let oab_consulta = async {
        if !preparada.forcar && !preparada.oab.is_empty() {
            return (
                Vec::new(),
                EstadoFonte {
                    fonte: "OAB".into(),
                    situacao: "BASE_LOCAL".into(),
                    mensagem: None,
                },
            );
        }
        match OabScraperClient::new()
            .consultar_cna(&preparada.doador.nome, preparada.uf.as_deref())
            .await
        {
            Ok(registros) => (
                registros,
                EstadoFonte {
                    fonte: "OAB".into(),
                    situacao: "DISPONIVEL".into(),
                    mensagem: None,
                },
            ),
            Err(error) => (
                Vec::new(),
                EstadoFonte {
                    fonte: "OAB".into(),
                    situacao: if preparada.oab.is_empty() {
                        "INDISPONIVEL"
                    } else {
                        "CACHE_APOS_FALHA"
                    }
                    .into(),
                    mensagem: Some(error.to_string()),
                },
            ),
        }
    };
    let ((diario, diario_novo, fonte_diario), (oab_remota, fonte_oab)) =
        tokio::join!(diario_consulta, oab_consulta);
    Consultas {
        diario,
        diario_novo,
        oab_remota,
        fontes: vec![fonte_diario, fonte_oab],
    }
}

fn concluir(
    conn: &mut Connection,
    preparada: InvestigacaoPreparada,
    consultas: Consultas,
) -> storage::Result<InvestigacaoNomeacaoResponse> {
    let tx = storage::transaction_immediate(conn)?;
    if consultas.diario_novo {
        let json = serde_json::to_string(&consultas.diario)
            .map_err(|e| storage::StorageError::Pool(e.to_string()))?;
        QueridoDiarioClient::salvar_cache(
            &tx,
            &preparada.doador.documento,
            &preparada.doador.nome,
            &preparada.chave_cache,
            consultas.diario.total_gazettes,
            &json,
        )
        .map_err(erro_ingestao)?;
    }
    // Uma busca por nome não fornece o CPF de quem aparece no resultado.
    // Resultados remotos permanecem como evidência possível, sem atribuição documental.
    let mut registros: Vec<_> = preparada.oab.iter().map(|r| r.registro.clone()).collect();
    for registro in consultas.oab_remota {
        if !registros
            .iter()
            .any(|r| r.uf == registro.uf && r.inscricao == registro.inscricao)
        {
            registros.push(registro);
        }
    }
    let falhou = consultas
        .fontes
        .iter()
        .any(|f| matches!(f.situacao.as_str(), "INDISPONIVEL" | "CACHE_APOS_FALHA"));
    let mut status = if falhou {
        "ANALISE_PARCIAL"
    } else {
        "EVIDENCIA_INSUFICIENTE"
    }
    .to_string();
    if consultas
        .fontes
        .iter()
        .all(|f| f.situacao == "INDISPONIVEL")
    {
        status = "FONTE_INDISPONIVEL".into();
    }
    let mut avisos=vec!["Trechos de diário e coincidência de nomes não comprovam identidade, nomeação ou vigência do cargo.".into()];
    let mut alerta_conflito = None;
    tx.execute("DELETE FROM alertas_auditoria WHERE tipo='CONFLITO_OAB' AND json_valid(detalhes_json) AND json_extract(detalhes_json,'$.doador_id')=?1",[preparada.doador.id])?;
    for cargo in &preparada.cargos {
        for local in &preparada.oab {
            let confirmado = local.recente
                && local.documento.as_deref().is_some_and(|d| {
                    crate::identity::complete(d)
                        && crate::identity::digits(d)
                            == crate::identity::digits(&preparada.doador.documento)
                });
            if !confirmado {
                continue;
            }
            let registro = &local.registro;
            let ocupante = OcupanteCargo {
                id: cargo.politico_id,
                nome: preparada.doador.nome.clone(),
                cpf_mascarado: Some(preparada.doador.documento.clone()),
                cargo: cargo.cargo.clone(),
                orgao: cargo.orgao.clone(),
                municipio: preparada.municipio.clone().unwrap_or_default(),
                uf: preparada.uf.clone().unwrap_or_default(),
                data_nomeacao: cargo.data_posse.clone(),
                ativo: true,
            };
            let oab = RegistroOab {
                pessoa_nome: registro.nome.clone(),
                numero_registro: registro.inscricao.clone(),
                seccional_uf: registro.uf.clone(),
                situacao_registro: registro.situacao.clone(),
            };
            if let Some(alerta) = auditar_conflito_oab(&ocupante, &oab) {
                registrar_alerta(&tx,&NovoAlerta {tipo:"CONFLITO_OAB".into(),severidade:"CRITICA".into(),titulo:format!("Indício de incompatibilidade OAB — {}",preparada.doador.nome),descricao:format!("Cargo verificado de {} em {} e inscrição {}/{} regular. Conferir as fontes primárias.",cargo.cargo,cargo.orgao,registro.uf,registro.inscricao),alvo_nome:preparada.doador.nome.clone(),alvo_documento:Some(preparada.doador.documento.clone()),municipio:preparada.municipio.clone(),uf:preparada.uf.clone(),ano:cargo.data_posse.get(..4).and_then(|y|y.parse().ok()),valor_envolvido:None,fonte_dado:"CARGO_VERIFICADO/OAB_LOCAL".into(),detalhes_json:Some(serde_json::json!({"doador_id":preparada.doador.id,"politico_id":cargo.politico_id,"cargo":cargo.cargo,"orgao":cargo.orgao,"data_nomeacao":cargo.data_posse,"fonte_url":cargo.fonte_url,"oab_numero":registro.inscricao,"oab_uf":registro.uf,"confianca":"CONFIRMADO","revisao_evidencia":"oab-v2"}).to_string())})?;
                alerta_conflito = Some(alerta);
                status = "ALERTA_GERADO".into();
            } else if !falhou
                && matches!(
                    registro.situacao.trim().to_uppercase().as_str(),
                    "LICENCIADO" | "CANCELADO"
                )
                && alerta_conflito.is_none()
            {
                status = "SEM_INDICIO_NAS_FONTES_CONSULTADAS".into();
            }
        }
    }
    if alerta_conflito.is_none() {
        avisos.push("A confirmação exige CPF completo válido nas duas fontes, inscrição consultada há até 30 dias e cargo vigente com ato e conferência registrados.".into());
    }
    tx.commit()?;
    Ok(InvestigacaoNomeacaoResponse {
        doador_id: preparada.doador.id.to_string(),
        doador_nome: preparada.doador.nome,
        doador_cpf_cnpj: preparada.doador.documento,
        municipio: preparada.municipio,
        uf: preparada.uf,
        total_ocorrencias_diario: consultas.diario.total_gazettes.max(0) as usize,
        trechos_diario: consultas.diario.gazettes,
        registros_oab: registros,
        alerta_conflito,
        status,
        fontes: consultas.fontes,
        avisos,
    })
}

pub async fn executar_investigacao(
    conn: &mut Connection,
    doador: DoadorIdentificado,
    params: InvestigarParams,
) -> storage::Result<InvestigacaoNomeacaoResponse> {
    let preparada = preparar(conn, doador, params)?;
    let consultas = consultar(&preparada).await;
    concluir(conn, preparada, consultas)
}

pub async fn investigar_nomeacao_handler(
    State(pool): State<DbPool>,
    Path(doador_id): Path<String>,
    Query(params): Query<InvestigarParams>,
) -> Result<Json<InvestigacaoNomeacaoResponse>, StatusCode> {
    let preparada = pool
        .run_blocking(move |conn| {
            let doador = identificar_doador(conn, &doador_id)
                .ok_or_else(|| storage::StorageError::Pool("Doador não encontrado".into()))?;
            preparar(conn, doador, params)
        })
        .await
        .map_err(|e| {
            if e.to_string().contains("Doador não encontrado") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        })?;
    // Nenhuma conexão permanece emprestada durante as requisições externas.
    let consultas = consultar(&preparada).await;
    pool.run_blocking(move |conn| concluir(conn, preparada, consultas))
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
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
             VALUES (?1, '52998224725', 'DR ADVOGADO NOMEADO', 5000.0, '2024-09-01')",
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
            &conn,
            "52998224725",
            "DR ADVOGADO NOMEADO",
            "Campinas-SP",
            1,
            &serde_json::to_string(&qd_api).unwrap(),
        )
        .unwrap();

        // 3. Pré-alimenta registro profissional da OAB ativo/regular
        let oab_rec = OabConsultaResult {
            nome: "DR ADVOGADO NOMEADO".to_string(),
            inscricao: "123456".to_string(),
            uf: "SP".to_string(),
            situacao: "REGULAR".to_string(),
            tipo: "ADVOGADO".to_string(),
            payload_json: "{}".to_string(),
        };
        OabScraperClient::salvar_registro_oab(&mut conn, &oab_rec, Some("52998224725")).unwrap();

        conn.execute("INSERT INTO politicos(nome_completo,nome_urna,cpf_mascarado) VALUES('DR ADVOGADO NOMEADO','ADVOGADO','52998224725')",[]).unwrap();
        let autoridade = conn.last_insert_rowid();
        conn.execute("INSERT INTO cargos_autoridades(politico_id,orgao,cargo,esfera,data_posse,origem_dado,fonte_url,conferido_em,evidencia_verificada) VALUES(?1,'PREFEITURA','SECRETÁRIO MUNICIPAL','MUNICIPAL','2025-01-02','ATO_OFICIAL','https://diario.campinas.sp.gov.br/edicao/123',?2,1)",storage::rusqlite::params![autoridade,chrono::Utc::now().to_rfc3339()]).unwrap();
        conn.execute("UPDATE registros_profissionais SET cpf_mascarado='52998224725' WHERE numero_registro='123456'",[]).unwrap();

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
        )
        .await
        .unwrap();

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
        )
        .unwrap();

        let qd_empty = QueridoDiarioApiResponse {
            total_gazettes: 0,
            gazettes: Vec::new(),
        };
        QueridoDiarioClient::salvar_cache(
            &conn,
            "55566677788",
            "DOADOR COMUM",
            "Santos-SP",
            0,
            &serde_json::to_string(&qd_empty).unwrap(),
        )
        .unwrap();

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
            .route(
                "/api/v1/investigar/nomeacao/:doador_id",
                get(investigar_nomeacao_handler),
            )
            .with_state(pool);

        let req = Request::builder()
            .uri("/api/v1/investigar/nomeacao/doador-999")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: InvestigacaoNomeacaoResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.doador_nome, "DOADOR COMUM");
        assert_eq!(json.status, "EVIDENCIA_INSUFICIENTE");
    }
}

#[cfg(test)]
mod evidence_regressions {
    use super::*;
    fn preparada(conn: &Connection) -> InvestigacaoPreparada {
        preparar(
            conn,
            DoadorIdentificado {
                id: 42,
                nome: "ALBERTO GOLDSZTEJN".into(),
                documento: "52998224725".into(),
                municipio: None,
                uf: None,
            },
            InvestigarParams {
                municipio: None,
                uf: Some("SP".into()),
                forcar: None,
            },
        )
        .unwrap()
    }
    fn consultas(falhou: bool) -> Consultas {
        Consultas {
            diario: QueridoDiarioApiResponse {
                total_gazettes: 1,
                gazettes: vec![ExcerptDiario {
                    date: None,
                    territory_name: None,
                    territory_id: None,
                    url: None,
                    excerpt: "NOMEAR ALBERTO GOLDSZTEJN SECRETÁRIO".into(),
                }],
            },
            diario_novo: false,
            oab_remota: vec![OabConsultaResult {
                nome: "ALBERTO GOLDSZTEJN".into(),
                inscricao: "123".into(),
                uf: "SP".into(),
                situacao: "REGULAR".into(),
                tipo: "ADVOGADO".into(),
                payload_json: "{}".into(),
            }],
            fontes: ["QUERIDO_DIARIO", "OAB"]
                .map(|fonte| EstadoFonte {
                    fonte: fonte.into(),
                    situacao: if falhou { "INDISPONIVEL" } else { "DISPONIVEL" }.into(),
                    mensagem: falhou.then(|| "falha simulada".into()),
                })
                .to_vec(),
        }
    }
    #[test]
    fn falha_das_fontes_e_coincidencia_nominal_nao_atestam_regularidade_nem_nomeacao() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        storage::run_migrations(&mut conn).unwrap();
        for falhou in [true, false] {
            let contexto = preparada(&conn);
            let resultado = concluir(&mut conn, contexto, consultas(falhou)).unwrap();
            assert_eq!(
                resultado.status,
                if falhou {
                    "FONTE_INDISPONIVEL"
                } else {
                    "EVIDENCIA_INSUFICIENTE"
                }
            );
            assert!(resultado.alerta_conflito.is_none());
        }
        assert_eq!(
            conn.query_row("SELECT count(*) FROM alertas_auditoria", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn homonimo_documentado_e_cargo_encerrado_nao_geram_alerta() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        storage::run_migrations(&mut conn).unwrap();
        conn.execute_batch("INSERT INTO politicos(id,nome_completo,nome_urna,cpf_mascarado) VALUES(1,'ALBERTO GOLDSZTEJN','ALBERTO','52998224725'); INSERT INTO registros_profissionais(pessoa_nome,cpf_mascarado,orgao_emissor,numero_registro,seccional_uf,situacao_registro) VALUES('ALBERTO GOLDSZTEJN','12311122207','OAB','123','SP','REGULAR'); INSERT INTO cargos_autoridades(politico_id,orgao,cargo,esfera,data_posse,data_exoneracao,origem_dado,evidencia_verificada,fonte_url,conferido_em) VALUES(1,'PREFEITURA','SECRETÁRIO','MUNICIPAL','2020-01-01','2021-01-01','ATO',1,'https://example.org/ato','2020-01-01T00:00:00Z');").unwrap();
        for ativo in [false, true] {
            if ativo {
                conn.execute("UPDATE cargos_autoridades SET data_exoneracao=NULL", [])
                    .unwrap();
            }
            let contexto = preparada(&conn);
            let resultado = concluir(&mut conn, contexto, consultas(false)).unwrap();
            assert!(resultado.alerta_conflito.is_none());
        }
        conn.execute("UPDATE registros_profissionais SET cpf_mascarado='52998224725',data_consulta='2000-01-01'",[]).unwrap();
        let contexto = preparada(&conn);
        let resultado = concluir(&mut conn, contexto, consultas(false)).unwrap();
        assert!(resultado.alerta_conflito.is_none());
    }
}
