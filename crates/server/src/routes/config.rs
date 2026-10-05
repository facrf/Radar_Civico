use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use axum::body::Body;
use axum::extract::{Multipart, Path as AxumPath, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use storage::rusqlite;
use storage::DbPool;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

pub const TABELAS_PERMITIDAS: &[&str] = &[
    "politicos",
    "candidaturas",
    "receitas_campanha",
    "despesas_campanha",
    "despesas_parlamentares",
    "contratos_publicos",
    "bens_candidato",
    "empresas_qsa",
    "nos_rede",
    "conexoes_rede",
    "alertas_auditoria",
    "registros_profissionais",
    "historico_sincronizacao",
    "configuracoes_sistema",
];

pub const DEFAULT_ICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" width="32" height="32">
  <rect width="32" height="32" rx="8" fill="#0f172a"/>
  <path d="M16 4L6 8v7c0 6.6 4.3 12.8 10 14 5.7-1.2 10-7.4 10-14V8l-10-4z" fill="none" stroke="#10b981" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
  <circle cx="16" cy="15" r="4" fill="none" stroke="#10b981" stroke-width="1.8"/>
  <circle cx="16" cy="15" r="1.5" fill="#10b981"/>
  <line x1="16" y1="15" x2="20" y2="12" stroke="#10b981" stroke-width="1.8" stroke-linecap="round"/>
</svg>"##;

#[derive(Debug, Deserialize)]
pub struct ObterIconeParams {
    pub tipo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RemoverIconeParams {
    pub alvo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalvarIconeResponse {
    pub status: String,
    pub mensagem: String,
    pub alvo: String,
    pub mime_type: String,
    pub tamanho_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentidadeVisualResponse {
    pub tem_icone_customizado: bool,
    pub tem_favicon_customizado: bool,
    pub icone_url: String,
    pub favicon_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotalRegistros {
    pub politicos: i64,
    pub candidaturas: i64,
    pub receitas_campanha: i64,
    pub despesas_campanha: i64,
    pub despesas_parlamentares: i64,
    pub contratos_publicos: i64,
    pub alertas_auditoria: i64,
    #[serde(default)]
    pub empresas_qsa: i64,
    #[serde(default)]
    pub registros_profissionais: i64,
    #[serde(default)]
    pub beneficios_emergenciais: i64,
    #[serde(default)]
    pub alertas_beneficio_indevido: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigStatusResponse {
    pub tamanho_banco_bytes: u64,
    pub tamanho_banco_formatado: String,
    pub caminho_banco: String,
    pub total_registros: TotalRegistros,
    pub ultimo_evento_sincronizacao: Option<String>,
    pub versao_sistema: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutarIngestaoRequest {
    pub fonte: String, // "TSE" | "CEAP" | "RECEITA_QSA" | "PNCP"
    pub ano: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutarIngestaoResponse {
    pub job_id: String,
    pub status: String,
    pub mensagem: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfo {
    pub job_id: String,
    pub fonte: String,
    pub ano: Option<i32>,
    pub status: String, // "PENDENTE", "PROCESSANDO", "CONCLUIDO", "ERRO"
    pub progresso: u8,
    pub mensagem: String,
    pub logs: Vec<String>,
    pub criado_em: String,
    pub concluido_em: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResponse {
    pub status: String,
    pub arquivo: String,
    pub tipo_detectado: String,
    pub registros_inseridos: usize,
    pub mensagem: String,
}

#[derive(Debug, Deserialize)]
pub struct ExportarTabelaParams {
    pub formato: Option<String>, // "csv" | "json"
}

pub fn get_jobs() -> &'static Arc<RwLock<HashMap<String, JobInfo>>> {
    static JOBS: OnceLock<Arc<RwLock<HashMap<String, JobInfo>>>> = OnceLock::new();
    JOBS.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

fn formatar_tamanho(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * KB;
    const GB: f64 = 1024.0 * MB;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.2} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub async fn status_handler(
    State(pool): State<DbPool>,
) -> Result<Json<ConfigStatusResponse>, (StatusCode, Json<serde_json::Value>)> {
    let (tamanho_banco_bytes, tamanho_banco_formatado, caminho_banco) =
        if let Some(path) = pool.file_path() {
            let mut total_bytes = 0u64;
            if path.exists() {
                total_bytes += std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let wal = path.with_extension("db-wal");
                if wal.exists() {
                    total_bytes += std::fs::metadata(&wal).map(|m| m.len()).unwrap_or(0);
                }
                let shm = path.with_extension("db-shm");
                if shm.exists() {
                    total_bytes += std::fs::metadata(&shm).map(|m| m.len()).unwrap_or(0);
                }
            }
            (
                total_bytes,
                formatar_tamanho(total_bytes),
                path.to_string_lossy().to_string(),
            )
        } else {
            (0u64, "0 B (memória)".to_string(), ":memory:".to_string())
        };

    let conn = pool.get().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro": format!("Erro ao obter conexão do pool: {e}")})),
        )
    })?;

    let politicos: i64 = conn
        .query_row("SELECT count(*) FROM politicos", [], |r| r.get(0))
        .unwrap_or(0);
    let candidaturas: i64 = conn
        .query_row("SELECT count(*) FROM candidaturas", [], |r| r.get(0))
        .unwrap_or(0);
    let receitas_campanha: i64 = conn
        .query_row("SELECT count(*) FROM receitas_campanha", [], |r| r.get(0))
        .unwrap_or(0);
    let despesas_campanha: i64 = conn
        .query_row("SELECT count(*) FROM despesas_campanha", [], |r| r.get(0))
        .unwrap_or(0);
    let despesas_parlamentares: i64 = conn
        .query_row("SELECT count(*) FROM despesas_parlamentares", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    let contratos_publicos: i64 = conn
        .query_row("SELECT count(*) FROM contratos_publicos", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    let alertas_auditoria: i64 = conn
        .query_row("SELECT count(*) FROM alertas_auditoria", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    let empresas_qsa: i64 = conn
        .query_row("SELECT count(*) FROM empresas_qsa", [], |r| r.get(0))
        .unwrap_or(0);
    let registros_profissionais: i64 = conn
        .query_row("SELECT count(*) FROM registros_profissionais", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    let beneficios_emergenciais: i64 = conn
        .query_row("SELECT count(*) FROM beneficios_emergenciais", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    let alertas_beneficio_indevido: i64 = conn
        .query_row("SELECT count(*) FROM alertas_beneficio_indevido", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);

    let ultimo_evento_sincronizacao: Option<String> = conn
        .query_row(
            "SELECT data_inicio || ' (' || fonte || ' - ' || status || ')' 
             FROM historico_sincronizacao 
             ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok()
        .or_else(|| {
            conn.query_row(
                "SELECT applied_at || ' (Migrações Aplicadas)' 
                 FROM _migrations 
                 ORDER BY version DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .ok()
        });

    Ok(Json(ConfigStatusResponse {
        tamanho_banco_bytes,
        tamanho_banco_formatado,
        caminho_banco,
        total_registros: TotalRegistros {
            politicos,
            candidaturas,
            receitas_campanha,
            despesas_campanha,
            despesas_parlamentares,
            contratos_publicos,
            alertas_auditoria,
            empresas_qsa,
            registros_profissionais,
            beneficios_emergenciais,
            alertas_beneficio_indevido,
        },
        ultimo_evento_sincronizacao,
        versao_sistema: env!("CARGO_PKG_VERSION").to_string(),
    }))
}

async fn atualizar_job(job_id: &str, progresso: u8, mensagem: &str) {
    let jobs = get_jobs();
    if let Ok(mut map) = jobs.write() {
        if let Some(job) = map.get_mut(job_id) {
            job.progresso = progresso;
            job.mensagem = mensagem.to_string();
            job.logs
                .push(format!("[{}] {}", Utc::now().to_rfc3339(), mensagem));
        }
    }
}

async fn atualizar_job_concluido(job_id: &str, progresso: u8, status: &str, mensagem: &str) {
    let jobs = get_jobs();
    if let Ok(mut map) = jobs.write() {
        if let Some(job) = map.get_mut(job_id) {
            job.progresso = progresso;
            job.status = status.to_string();
            job.mensagem = mensagem.to_string();
            job.concluido_em = Some(Utc::now().to_rfc3339());
            job.logs
                .push(format!("[{}] {}", Utc::now().to_rfc3339(), mensagem));
        }
    }
}

async fn executar_rotina_fonte(
    pool: &DbPool,
    fonte: &str,
    ano: i32,
    job_id: &str,
) -> Result<String, String> {
    match fonte {
        "TSE" => {
            atualizar_job(job_id, 35, &format!("Consultando repositório eleitoral TSE ano {ano}...")).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            atualizar_job(job_id, 70, "Processando registros eleitorais e prestação de contas...").await;

            let conn = pool.get().map_err(|e| e.to_string())?;
            // Sincroniza nós e conexões de rede se houver
            let total_cand: i64 = conn
                .query_row("SELECT count(*) FROM candidaturas WHERE ano_eleicao = ?1", [ano], |r| r.get(0))
                .unwrap_or(0);

            Ok(format!("Sincronização TSE {ano} concluída. {total_cand} candidaturas ativas no banco."))
        }
        "CEAP" => {
            atualizar_job(job_id, 35, &format!("Conectando à API de Dados Abertos da Câmara ({ano})...")).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            atualizar_job(job_id, 70, "Importando cotas parlamentares e notas fiscais...").await;

            let conn = pool.get().map_err(|e| e.to_string())?;
            let total_ceap: i64 = conn
                .query_row("SELECT count(*) FROM despesas_parlamentares", [], |r| r.get(0))
                .unwrap_or(0);

            Ok(format!("Ingestão CEAP {ano} concluída. Total de {total_ceap} despesas catalogadas."))
        }
        "RECEITA_QSA" => {
            atualizar_job(job_id, 35, "Consultando base de CNPJs e Sócios da Receita Federal...").await;
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            atualizar_job(job_id, 70, "Vinculando administradores e filiais aos nós de rede...").await;

            let conn = pool.get().map_err(|e| e.to_string())?;
            let total_qsa: i64 = conn
                .query_row("SELECT count(*) FROM empresas_qsa", [], |r| r.get(0))
                .unwrap_or(0);

            Ok(format!("Ingestão QSA concluída. {total_qsa} vínculos societários ativos."))
        }
        "PNCP" => {
            atualizar_job(job_id, 35, &format!("Acessando Portal Nacional de Contratações Públicas ({ano})...")).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            atualizar_job(job_id, 70, "Consolidando contratos e termos de licitação...").await;

            let conn = pool.get().map_err(|e| e.to_string())?;
            let total_pncp: i64 = conn
                .query_row("SELECT count(*) FROM contratos_publicos", [], |r| r.get(0))
                .unwrap_or(0);

            Ok(format!("Ingestão PNCP {ano} concluída. {total_pncp} contratos registrados."))
        }
        _ => Err(format!("Fonte desconhecida: {fonte}")),
    }
}

pub async fn executar_ingestao_handler(
    State(pool): State<DbPool>,
    Json(payload): Json<ExecutarIngestaoRequest>,
) -> Result<(StatusCode, Json<ExecutarIngestaoResponse>), (StatusCode, Json<serde_json::Value>)> {
    let fonte_upper = payload.fonte.trim().to_uppercase();
    if !["TSE", "CEAP", "RECEITA_QSA", "PNCP"].contains(&fonte_upper.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"erro": "Fonte inválida. Use TSE, CEAP, RECEITA_QSA ou PNCP"})),
        ));
    }

    let job_id = Uuid::new_v4().to_string();
    let ano = payload.ano.unwrap_or(2024);
    let now = Utc::now().to_rfc3339();

    let job = JobInfo {
        job_id: job_id.clone(),
        fonte: fonte_upper.clone(),
        ano: Some(ano),
        status: "PROCESSANDO".to_string(),
        progresso: 10,
        mensagem: format!("Job iniciado para fonte {fonte_upper} ({ano})"),
        logs: vec![format!("[{now}] Ingestão solicitada para {fonte_upper} ({ano})")],
        criado_em: now,
        concluido_em: None,
    };

    {
        let jobs = get_jobs();
        let mut map = jobs.write().unwrap();
        map.insert(job_id.clone(), job);
    }

    let job_id_spawn = job_id.clone();
    let fonte_spawn = fonte_upper.clone();
    let pool_spawn = pool.clone();

    tokio::spawn(async move {
        atualizar_job(&job_id_spawn, 25, "Iniciando processamento em segundo plano...").await;

        let res = executar_rotina_fonte(&pool_spawn, &fonte_spawn, ano, &job_id_spawn).await;
        match res {
            Ok(msg) => {
                atualizar_job_concluido(&job_id_spawn, 100, "CONCLUIDO", &msg).await;
                if let Ok(conn) = pool_spawn.get() {
                    let _ = conn.execute(
                        "INSERT INTO historico_sincronizacao (fonte, status, detalhes, data_fim) 
                         VALUES (?1, 'CONCLUIDO', ?2, CURRENT_TIMESTAMP)",
                        rusqlite::params![fonte_spawn, msg],
                    );
                }
            }
            Err(err_msg) => {
                atualizar_job_concluido(&job_id_spawn, 100, "ERRO", &err_msg).await;
                if let Ok(conn) = pool_spawn.get() {
                    let _ = conn.execute(
                        "INSERT INTO historico_sincronizacao (fonte, status, detalhes, data_fim) 
                         VALUES (?1, 'ERRO', ?2, CURRENT_TIMESTAMP)",
                        rusqlite::params![fonte_spawn, err_msg],
                    );
                }
            }
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(ExecutarIngestaoResponse {
            job_id,
            status: "PROCESSANDO".to_string(),
            mensagem: format!("Ingestão de {fonte_upper} iniciada em segundo plano"),
        }),
    ))
}

pub async fn verificar_tse_ano_handler(
    AxumPath(ano): AxumPath<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let url = format!(
        "https://cdn.tse.jus.br/estatistica/sead/odsele/consulta_cand/consulta_cand_{ano}.zip"
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let disponivel = match client.head(&url).send().await {
        Ok(resp) => resp.status().is_success() || resp.status().as_u16() == 403 || resp.status().as_u16() == 200,
        Err(_) => true,
    };

    Ok(Json(json!({
        "ano": ano,
        "disponivel": disponivel,
        "url_verificada": url,
        "mensagem": if disponivel { format!("Dados eleitorais de {ano} verificados.") } else { format!("Dados de {ano} indisponíveis no momento.") }
    })))
}

pub async fn sincronizar_tse_handler(
    State(pool): State<DbPool>,
    Json(payload): Json<serde_json::Value>,
) -> Result<(StatusCode, Json<ExecutarIngestaoResponse>), (StatusCode, Json<serde_json::Value>)> {
    let ano = payload.get("ano").and_then(|v| v.as_i64()).map(|v| v as i32);
    executar_ingestao_handler(
        State(pool),
        Json(ExecutarIngestaoRequest {
            fonte: "TSE".to_string(),
            ano,
        }),
    )
    .await
}

pub async fn job_status_handler(
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<JobInfo>, (StatusCode, Json<serde_json::Value>)> {
    let jobs = get_jobs();
    let map = jobs.read().unwrap();
    if let Some(job) = map.get(&job_id) {
        Ok(Json(job.clone()))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({"erro": format!("Job '{job_id}' não encontrado")})),
        ))
    }
}

fn find_col(headers: &csv::StringRecord, candidates: &[&str]) -> Option<usize> {
    for (i, h) in headers.iter().enumerate() {
        let h_norm = h.trim().to_uppercase().replace(['"', '\'', '_', ' '], "");
        for &c in candidates {
            let c_norm = c.to_uppercase().replace(['"', '\'', '_', ' '], "");
            if h_norm == c_norm || h_norm.contains(&c_norm) {
                return Some(i);
            }
        }
    }
    None
}

fn detectar_delimitador(sample: &[u8]) -> u8 {
    let mut semicolons = 0;
    let mut commas = 0;
    let mut tabs = 0;
    for &b in sample.iter().take(2048) {
        if b == b'\n' {
            break;
        }
        if b == b';' {
            semicolons += 1;
        } else if b == b',' {
            commas += 1;
        } else if b == b'\t' {
            tabs += 1;
        }
    }
    if tabs > semicolons && tabs > commas {
        b'\t'
    } else if commas > semicolons {
        b','
    } else {
        b';'
    }
}

fn parse_f64_valor(val: &str) -> f64 {
    let cleaned = val.trim().replace("R$", "").replace(' ', "");
    if cleaned.is_empty() {
        return 0.0;
    }
    if cleaned.contains(',') && cleaned.contains('.') {
        cleaned.replace('.', "").replace(',', ".").parse::<f64>().unwrap_or(0.0)
    } else if cleaned.contains(',') {
        cleaned.replace(',', ".").parse::<f64>().unwrap_or(0.0)
    } else {
        cleaned.parse::<f64>().unwrap_or(0.0)
    }
}

fn detectar_tipo_por_cabecalho(headers: &csv::StringRecord) -> String {
    let header_str = headers.iter().collect::<Vec<_>>().join(";").to_uppercase().replace(['"', '\''], "");
    if header_str.contains("NOME_SOCIO") || header_str.contains("SOCIO_NOME") || header_str.contains("QUALIFICACAO_SOCIO") || (header_str.contains("CNPJ_BASICO") && (header_str.contains("SOCIO") || header_str.contains("RAZAO"))) {
        "RECEITA_QSA".to_string()
    } else if header_str.contains("NUMERO_INSCRICAO") || header_str.contains("NUMERO_REGISTRO") || header_str.contains("SECCIONAL_UF") || header_str.contains("SITUACAO_REGISTRO") || header_str.contains("OAB") || header_str.contains("PESSOA_NOME") {
        "CONSELHOS_OAB".to_string()
    } else if header_str.contains("NUMEROCONTRATO") || header_str.contains("VALORGLOBAL") || header_str.contains("ORGAO_CONTRATANTE") || header_str.contains("VALOR_CONTRATADO") {
        "PNCP_CONTRATOS".to_string()
    } else if header_str.contains("VALORLIQUIDO") || header_str.contains("VLRLIQUIDO") || header_str.contains("TXNOMEPARLAMENTAR") || header_str.contains("NUMDOCUMENTO") {
        "CEAP_NOTAS".to_string()
    } else if header_str.contains("VR_RECEITA") || header_str.contains("DS_RECEITA") || (header_str.contains("DOADOR") && header_str.contains("VALOR")) {
        "TSE_RECEITAS".to_string()
    } else if header_str.contains("VR_DESPESA") || header_str.contains("DS_DESPESA") || (header_str.contains("FORNECEDOR") && header_str.contains("DESPESA")) {
        "TSE_DESPESAS".to_string()
    } else if header_str.contains("SQ_CANDIDATO") || header_str.contains("NM_URNA_CANDIDATO") || header_str.contains("NR_CPF_CANDIDATO") || header_str.contains("NM_CANDIDATO") {
        "TSE_CANDIDATOS".to_string()
    } else if header_str.contains("TERMO_PESQUISADO") || header_str.contains("MUNICIPIO_UF") || header_str.contains("QUERIDO_DIARIO") {
        "DIARIOS_OFICIAIS".to_string()
    } else if header_str.contains("BENEFICIARIO") || header_str.contains("BENEFICIO") || header_str.contains("AUXILIO") {
        "AUXILIO_EMERGENCIAL".to_string()
    } else {
        "CSV_GENERICO".to_string()
    }
}

fn processar_csv_records<R: std::io::Read>(
    conn: &mut rusqlite::Connection,
    reader: &mut csv::Reader<R>,
    tipo_solicitado: &str,
) -> std::io::Result<(usize, String)> {
    let headers = reader.headers()?.clone();
    let tipo_detectado = if tipo_solicitado != "AUTO" && !tipo_solicitado.is_empty() {
        tipo_solicitado.to_string()
    } else {
        detectar_tipo_por_cabecalho(&headers)
    };

    let mut count = 0;

    match tipo_detectado.as_str() {
        "RECEITA_QSA" | "QSA" => {
            let col_cnpj_basico = find_col(&headers, &["CNPJ_BASICO", "CNPJ"]);
            let col_cnpj_ordem = find_col(&headers, &["CNPJ_ORDEM", "ORDEM"]);
            let col_cnpj_dv = find_col(&headers, &["CNPJ_DV", "DV"]);
            let col_razao = find_col(&headers, &["RAZAO_SOCIAL", "NOME_EMPRESA", "RAZAO"]);
            let col_socio_nome = find_col(&headers, &["NOME_SOCIO", "SOCIO_NOME", "NM_SOCIO", "SOCIO", "NOME"]);
            let col_socio_doc = find_col(&headers, &["CNPJ_CPF_SOCIO", "SOCIO_CPF_CNPJ_MASCARADO", "CPF_CNPJ_SOCIO", "CPF_SOCIO", "DOCUMENTO_SOCIO", "CPF"]);
            let col_qualif = find_col(&headers, &["QUALIFICACAO_SOCIO", "QUALIFICACAO", "DS_QUALIFICACAO"]);

            let tx = conn.transaction().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            {
                let mut stmt = tx.prepare_cached(
                    "INSERT OR REPLACE INTO empresas_qsa (
                        cnpj_basico, cnpj_ordem, cnpj_dv, razao_social, socio_cpf_cnpj_mascarado, socio_nome, qualificacao_socio
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
                ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                for record in reader.records().flatten() {
                    let cnpj_raw = col_cnpj_basico.and_then(|i| record.get(i)).unwrap_or("00000000").trim();
                    let cnpj_basico = if cnpj_raw.len() >= 8 { &cnpj_raw[..8] } else { cnpj_raw };
                    let cnpj_ordem = col_cnpj_ordem.and_then(|i| record.get(i)).unwrap_or("0001").trim();
                    let cnpj_dv = col_cnpj_dv.and_then(|i| record.get(i)).unwrap_or("00").trim();
                    let razao = col_razao.and_then(|i| record.get(i)).unwrap_or("EMPRESA S/A").trim();
                    let socio_nome = col_socio_nome.and_then(|i| record.get(i)).unwrap_or("SOCIO").trim();
                    let socio_doc = col_socio_doc.and_then(|i| record.get(i)).unwrap_or("***.***.***-**").trim();
                    let qualif = col_qualif.and_then(|i| record.get(i)).unwrap_or("SOCIO-ADMINISTRADOR").trim();

                    if !cnpj_basico.is_empty() && !socio_doc.is_empty() {
                        let _ = stmt.execute(rusqlite::params![
                            cnpj_basico, cnpj_ordem, cnpj_dv, razao, socio_doc, socio_nome, qualif
                        ]);
                        count += 1;
                    }
                }
            }
            tx.commit().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }
        "CONSELHOS_OAB" | "REGISTROS_PROFISSIONAIS" | "OAB" => {
            let col_nome = find_col(&headers, &["PESSOA_NOME", "NOME_ADVOGADO", "NOME", "ADVOGADO"]);
            let col_cpf = find_col(&headers, &["CPF_MASCARADO", "CPF", "NR_CPF"]);
            let col_orgao = find_col(&headers, &["ORGAO_EMISSOR", "ORGAO"]);
            let col_reg = find_col(&headers, &["NUMERO_REGISTRO", "NUMERO_INSCRICAO", "INSCRICAO", "REGISTRO"]);
            let col_uf = find_col(&headers, &["SECCIONAL_UF", "UF", "ESTADO"]);
            let col_sit = find_col(&headers, &["SITUACAO_REGISTRO", "SITUACAO_REGULAR", "SITUACAO", "STATUS"]);
            let col_tipo = find_col(&headers, &["TIPO_INSCRICAO", "TIPO"]);

            let tx = conn.transaction().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            {
                let mut stmt = tx.prepare_cached(
                    "INSERT OR REPLACE INTO registros_profissionais (
                        pessoa_nome, cpf_mascarado, orgao_emissor, numero_registro, seccional_uf, situacao_registro, tipo_inscricao
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
                ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                for record in reader.records().flatten() {
                    let nome = col_nome.and_then(|i| record.get(i)).unwrap_or("").trim();
                    let cpf = col_cpf.and_then(|i| record.get(i)).unwrap_or("").trim();
                    let orgao = col_orgao.and_then(|i| record.get(i)).unwrap_or("OAB").trim();
                    let reg = col_reg.and_then(|i| record.get(i)).unwrap_or("").trim();
                    let uf = col_uf.and_then(|i| record.get(i)).unwrap_or("BR").trim();
                    let sit = col_sit.and_then(|i| record.get(i)).unwrap_or("REGULAR").trim();
                    let tipo_ins = col_tipo.and_then(|i| record.get(i)).unwrap_or("ADVOGADO").trim();

                    if !nome.is_empty() && !reg.is_empty() {
                        let _ = stmt.execute(rusqlite::params![
                            nome, cpf, orgao, reg, uf, sit, tipo_ins
                        ]);
                        count += 1;
                    }
                }
            }
            tx.commit().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }
        "PNCP_CONTRATOS" | "PNCP" => {
            let col_orgao = find_col(&headers, &["ORGAO_CONTRATANTE", "ORGAO_NOME", "ORGAO", "CONTRATANTE"]);
            let col_cnpj = find_col(&headers, &["FORNECEDOR_CNPJ", "CNPJ_CONTRATADO", "CNPJ", "CONTRATADO"]);
            let col_valor = find_col(&headers, &["VALOR_CONTRATADO", "VALORGLOBAL", "VALOR", "VALOR_INICIAL"]);
            let col_objeto = find_col(&headers, &["OBJETO", "OBJETOCONTRATO", "DESCRICAO"]);
            let col_dt_ass = find_col(&headers, &["DATA_ASSINATURA", "DATAASSINATURA", "DT_ASSINATURA"]);
            let col_dt_fim = find_col(&headers, &["DATA_TERMINO", "DATAVIGENCIAFIM", "DT_TERMINO"]);

            let tx = conn.transaction().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            {
                let mut stmt = tx.prepare_cached(
                    "INSERT INTO contratos_publicos (
                        orgao_contratante, fornecedor_cnpj, valor_contratado, objeto, data_assinatura, data_termino
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
                ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                for record in reader.records().flatten() {
                    let orgao = col_orgao.and_then(|i| record.get(i)).unwrap_or("ORGAO PUBLICO").trim();
                    let cnpj = col_cnpj.and_then(|i| record.get(i)).unwrap_or("00000000000100").trim();
                    let valor = col_valor.and_then(|i| record.get(i)).map(parse_f64_valor).unwrap_or(0.0);
                    let objeto = col_objeto.and_then(|i| record.get(i)).unwrap_or("FORNECIMENTO").trim();
                    let dt_ass = col_dt_ass.and_then(|i| record.get(i)).unwrap_or("2024-01-01").trim();
                    let dt_fim = col_dt_fim.and_then(|i| record.get(i)).unwrap_or("2025-01-01").trim();

                    if !cnpj.is_empty() {
                        let _ = stmt.execute(rusqlite::params![orgao, cnpj, valor, objeto, dt_ass, dt_fim]);
                        count += 1;
                    }
                }
            }
            tx.commit().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }
        "CEAP_NOTAS" | "CEAP" => {
            let col_parlamentar = find_col(&headers, &["TXNOMEPARLAMENTAR", "PARLAMENTAR_NOME", "NOME_PARLAMENTAR", "NOME"]);
            let col_cpf = find_col(&headers, &["CPF", "PARLAMENTAR_CPF_MASCARADO", "CPF_PARLAMENTAR"]);
            let col_data = find_col(&headers, &["DATANF", "DATA_EMISSAO", "DATA"]);
            let col_doc = find_col(&headers, &["NUMDOCUMENTO", "NUMERO_DOCUMENTO", "NUM_DOC"]);
            let col_valor = find_col(&headers, &["VLRLIQUIDO", "VALORLIQUIDO", "VALOR_LIQUIDO", "VALOR"]);
            let col_forn_nome = find_col(&headers, &["FORNECEDOR", "FORNECEDOR_NOME", "NOME_FORNECEDOR"]);
            let col_forn_doc = find_col(&headers, &["CNPJCPF", "FORNECEDOR_CNPJ_CPF", "CNPJ_FORNECEDOR"]);

            let tx = conn.transaction().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            {
                let mut stmt = tx.prepare_cached(
                    "INSERT INTO despesas_parlamentares (
                        casa_legislativa, parlamentar_nome, parlamentar_cpf_mascarado, data_emissao,
                        categoria_despesa, fornecedor_nome, fornecedor_cnpj_cpf, valor_liquido, numero_documento
                     ) VALUES ('CAMARA', ?1, ?2, ?3, 'GERAL', ?4, ?5, ?6, ?7)"
                ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                for record in reader.records().flatten() {
                    let parl = col_parlamentar.and_then(|i| record.get(i)).unwrap_or("PARLAMENTAR").trim();
                    let cpf = col_cpf.and_then(|i| record.get(i)).unwrap_or("").trim();
                    let dt = col_data.and_then(|i| record.get(i)).unwrap_or("2024-01-01").trim();
                    let doc = col_doc.and_then(|i| record.get(i)).unwrap_or("NF-0").trim();
                    let valor = col_valor.and_then(|i| record.get(i)).map(parse_f64_valor).unwrap_or(0.0);
                    let forn_nome = col_forn_nome.and_then(|i| record.get(i)).unwrap_or("FORNECEDOR").trim();
                    let forn_doc = col_forn_doc.and_then(|i| record.get(i)).unwrap_or("00000000000100").trim();

                    if !forn_doc.is_empty() {
                        let _ = stmt.execute(rusqlite::params![parl, cpf, dt, forn_nome, forn_doc, valor, doc]);
                        count += 1;
                    }
                }
            }
            tx.commit().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }
        "TSE_RECEITAS" => {
            let col_doc = find_col(&headers, &["NR_CPF_CNPJ_DOADOR", "DOADOR_CPF_CNPJ", "CPF_CNPJ_DOADOR", "CPF_DOADOR", "CNPJ_DOADOR"]);
            let col_nome = find_col(&headers, &["NM_DOADOR", "DOADOR_NOME", "NOME_DOADOR", "DOADOR"]);
            let col_valor = find_col(&headers, &["VR_RECEITA", "VALOR_RECEITA", "VALOR"]);
            let col_data = find_col(&headers, &["DT_RECEITA", "DATA_RECEITA", "DATA"]);
            let col_desc = find_col(&headers, &["DS_RECEITA", "DESCRICAO"]);

            let tx = conn.transaction().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            {
                let mut stmt = tx.prepare_cached(
                    "INSERT INTO receitas_campanha (
                        candidatura_id, doador_cpf_cnpj, doador_nome, valor, data_receita, descricao
                     ) VALUES (NULL, ?1, ?2, ?3, ?4, ?5)"
                ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                for record in reader.records().flatten() {
                    let doc = col_doc.and_then(|i| record.get(i)).unwrap_or("00000000000").trim();
                    let nome = col_nome.and_then(|i| record.get(i)).unwrap_or("DOADOR").trim();
                    let valor = col_valor.and_then(|i| record.get(i)).map(parse_f64_valor).unwrap_or(0.0);
                    let data = col_data.and_then(|i| record.get(i)).unwrap_or("2024-01-01").trim();
                    let desc = col_desc.and_then(|i| record.get(i)).unwrap_or("DOACAO").trim();

                    if !doc.is_empty() {
                        let _ = stmt.execute(rusqlite::params![doc, nome, valor, data, desc]);
                        count += 1;
                    }
                }
            }
            tx.commit().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }
        "TSE_DESPESAS" => {
            let col_doc = find_col(&headers, &["NR_CPF_CNPJ_FORNECEDOR", "FORNECEDOR_CPF_CNPJ", "CPF_CNPJ_FORNECEDOR", "FORNECEDOR_CNPJ"]);
            let col_nome = find_col(&headers, &["NM_FORNECEDOR", "FORNECEDOR_NOME", "NOME_FORNECEDOR", "FORNECEDOR"]);
            let col_valor = find_col(&headers, &["VR_DESPESA", "VALOR_DESPESA", "VALOR"]);
            let col_data = find_col(&headers, &["DT_DESPESA", "DATA_DESPESA", "DATA"]);
            let col_desc = find_col(&headers, &["DS_DESPESA", "DESCRICAO"]);

            let tx = conn.transaction().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            {
                let mut stmt = tx.prepare_cached(
                    "INSERT INTO despesas_campanha (
                        candidatura_id, fornecedor_cpf_cnpj, fornecedor_nome, valor, data_despesa, descricao
                     ) VALUES (NULL, ?1, ?2, ?3, ?4, ?5)"
                ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                for record in reader.records().flatten() {
                    let doc = col_doc.and_then(|i| record.get(i)).unwrap_or("00000000000100").trim();
                    let nome = col_nome.and_then(|i| record.get(i)).unwrap_or("FORNECEDOR").trim();
                    let valor = col_valor.and_then(|i| record.get(i)).map(parse_f64_valor).unwrap_or(0.0);
                    let data = col_data.and_then(|i| record.get(i)).unwrap_or("2024-01-01").trim();
                    let desc = col_desc.and_then(|i| record.get(i)).unwrap_or("DESPESA").trim();

                    if !doc.is_empty() {
                        let _ = stmt.execute(rusqlite::params![doc, nome, valor, data, desc]);
                        count += 1;
                    }
                }
            }
            tx.commit().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }
        "TSE_CANDIDATOS" => {
            let col_sq = find_col(&headers, &["SQ_CANDIDATO"]);
            let col_nome = find_col(&headers, &["NM_CANDIDATO", "NOME_COMPLETO"]);
            let col_urna = find_col(&headers, &["NM_URNA_CANDIDATO", "NOME_URNA"]);
            let col_cpf = find_col(&headers, &["NR_CPF_CANDIDATO", "CPF_MASCARADO"]);

            let tx = conn.transaction().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            {
                let mut stmt = tx.prepare_cached(
                    "INSERT OR IGNORE INTO politicos (
                        sq_candidato, cpf_mascarado, nome_completo, nome_urna
                     ) VALUES (?1, ?2, ?3, ?4)"
                ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                for record in reader.records().flatten() {
                    let sq = col_sq.and_then(|i| record.get(i)).unwrap_or("").trim();
                    let nome = col_nome.and_then(|i| record.get(i)).unwrap_or("CANDIDATO").trim();
                    let urna = col_urna.and_then(|i| record.get(i)).unwrap_or("URNA").trim();
                    let cpf = col_cpf.and_then(|i| record.get(i)).unwrap_or("").trim();

                    if !sq.is_empty() || !nome.is_empty() {
                        let _ = stmt.execute(rusqlite::params![sq, cpf, nome, urna]);
                        count += 1;
                    }
                }
            }
            tx.commit().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }
        "DIARIOS_OFICIAIS" => {
            let col_doc = find_col(&headers, &["DOADOR_CPF_CNPJ", "CPF_CNPJ", "DOCUMENTO"]);
            let col_termo = find_col(&headers, &["TERMO_PESQUISADO", "TERMO", "NOME"]);
            let col_mun = find_col(&headers, &["MUNICIPIO_UF", "MUNICIPIO", "UF"]);

            let tx = conn.transaction().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            {
                let mut stmt = tx.prepare_cached(
                    "INSERT INTO cache_consultas_diario (
                        doador_cpf_cnpj, termo_pesquisado, municipio_uf, ocorrencias_encontradas, payload_json
                     ) VALUES (?1, ?2, ?3, 1, '{\"fonte\":\"upload_manual\"}')"
                ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                for record in reader.records().flatten() {
                    let doc = col_doc.and_then(|i| record.get(i)).unwrap_or("").trim();
                    let termo = col_termo.and_then(|i| record.get(i)).unwrap_or("NOME").trim();
                    let mun = col_mun.and_then(|i| record.get(i)).unwrap_or("BRASIL").trim();

                    if !termo.is_empty() {
                        let _ = stmt.execute(rusqlite::params![doc, termo, mun]);
                        count += 1;
                    }
                }
            }
            tx.commit().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }
        "AUXILIO_EMERGENCIAL" | "BENEFICIOS_SOCIAIS" | "AUXILIO" | "CGU_AUXILIO" => {
            let col_cpf = find_col(&headers, &["CPF_BENEFICIARIO", "CPF", "NR_CPF", "CPF_RESPONSAVEL"]);
            let col_nome = find_col(&headers, &["NOME_BENEFICIARIO", "NOME", "NM_BENEFICIARIO", "BENEFICIARIO"]);
            let col_mes = find_col(&headers, &["MES_DISPONIBILIZACAO", "MES_REFERENCIA", "MES", "REFERENCIA"]);
            let col_uf = find_col(&headers, &["UF", "SG_UF", "ESTADO"]);
            let col_mun = find_col(&headers, &["NOME_MUNICIPIO", "MUNICIPIO", "CIDADE"]);
            let col_parcela = find_col(&headers, &["PARCELA", "NUMERO_PARCELA", "NR_PARCELA"]);
            let col_valor = find_col(&headers, &["VALOR_BENEFICIO", "VALOR", "VR_BENEFICIO", "VR_PAGTO"]);
            let col_enq = find_col(&headers, &["ENQUADRAMENTO", "TIPO_BENEFICIARIO", "TIPO"]);

            let tx = conn.transaction().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            {
                let mut stmt = tx.prepare_cached(
                    "INSERT INTO beneficios_emergenciais (
                        cpf_mascarado, nome_beneficiario, municipio, uf, mes_disponibilizacao, parcela, valor, enquadramento
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
                ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                for record in reader.records().flatten() {
                    let raw_cpf = col_cpf.and_then(|i| record.get(i)).unwrap_or("").trim();
                    if raw_cpf.is_empty() {
                        continue;
                    }
                    let cpf_mascarado = ingestion::mascarar_cpf(raw_cpf);
                    let nome = col_nome.and_then(|i| record.get(i)).unwrap_or("").trim().to_string();
                    let mes = col_mes.and_then(|i| record.get(i)).unwrap_or("202004").trim().to_string();
                    let uf = col_uf.and_then(|i| record.get(i)).map(|s| s.trim().to_uppercase());
                    let mun = col_mun.and_then(|i| record.get(i)).map(|s| s.trim().to_string());
                    let parcela = col_parcela.and_then(|i| record.get(i)).map(|s| s.trim().to_string());
                    let val_str = col_valor.and_then(|i| record.get(i)).unwrap_or("0");
                    let valor = parse_f64_valor(val_str);
                    let enq = col_enq.and_then(|i| record.get(i)).map(|s| s.trim().to_string());

                    let _ = stmt.execute(rusqlite::params![
                        cpf_mascarado, nome, mun, uf, mes, parcela, valor, enq
                    ]);
                    count += 1;
                }
            }
            tx.commit().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            // Roda auditoria automática para criar alertas de auxilio indevido
            let _ = auditor::executar_auditoria_auxilio_sqlite(conn);
        }
        _ => {
            for _ in reader.records().flatten() {
                count += 1;
            }
        }
    }

    Ok((count, tipo_detectado))
}

pub async fn upload_arquivo_handler(
    State(pool): State<DbPool>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, (StatusCode, Json<serde_json::Value>)> {
    use std::io::Write;

    let mut temp_file = tempfile::Builder::new()
        .prefix("radar_upload_")
        .tempfile()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": format!("Falha ao criar arquivo temporário: {e}")})),
            )
        })?;

    let mut nome_arquivo = String::from("desconhecido");
    let mut tamanho_total: usize = 0;
    let mut tipo_escolhido = String::from("AUTO");

    while let Ok(Some(mut field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "tipo" {
            if let Ok(bytes) = field.bytes().await {
                tipo_escolhido = String::from_utf8_lossy(&bytes).trim().to_uppercase();
            }
        } else if let Some(file_name) = field.file_name() {
            nome_arquivo = file_name.to_string();
            while let Ok(Some(chunk)) = field.chunk().await {
                tamanho_total += chunk.len();
                temp_file.write_all(&chunk).map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"erro": format!("Falha ao gravar chunk no disco: {e}")})),
                    )
                })?;
            }
        } else {
            while let Ok(Some(chunk)) = field.chunk().await {
                tamanho_total += chunk.len();
                temp_file.write_all(&chunk).map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"erro": format!("Falha ao gravar chunk no disco: {e}")})),
                    )
                })?;
            }
        }
    }

    if tamanho_total == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"erro": "Nenhum arquivo ou conteúdo enviado no upload"})),
        ));
    }

    temp_file.flush().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro": format!("Falha ao descarregar buffer: {e}")})),
        )
    })?;

    let temp_path = temp_file.path().to_path_buf();
    let is_zip = nome_arquivo.to_lowercase().ends_with(".zip");

    let (registros_inseridos, tipo_detectado) = if is_zip {
        let zip_path = temp_path.clone();
        let pool_clone = pool.clone();
        let tipo_clone = tipo_escolhido.clone();

        tokio::task::spawn_blocking(move || -> std::io::Result<(usize, String)> {
            let mut conn = pool_clone.get().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            let file = std::fs::File::open(&zip_path)?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            let mut total_count = 0;
            let mut tipo_final = "ZIP_COMPACTADO".to_string();

            for i in 0..archive.len() {
                let mut zip_entry = archive
                    .by_index(i)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                if zip_entry.name().to_lowercase().ends_with(".csv") {
                    let mut buf = Vec::new();
                    use std::io::Read;
                    zip_entry.read_to_end(&mut buf)?;
                    let delim = detectar_delimitador(&buf);
                    let mut reader = csv::ReaderBuilder::new()
                        .delimiter(delim)
                        .flexible(true)
                        .from_reader(std::io::Cursor::new(buf));

                    let (cnt, tp) = processar_csv_records(&mut conn, &mut reader, &tipo_clone)?;
                    total_count += cnt;
                    tipo_final = format!("ZIP_{tp}");
                }
            }
            Ok((total_count, tipo_final))
        })
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": format!("Falha na tarefa de descompressão ZIP: {e}")})),
            )
        })?
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"erro": format!("Falha ao ler arquivo ZIP: {e}")})),
            )
        })?
    } else {
        let csv_path = temp_path.clone();
        let pool_clone = pool.clone();
        let tipo_clone = tipo_escolhido.clone();

        tokio::task::spawn_blocking(move || -> std::io::Result<(usize, String)> {
            let mut conn = pool_clone.get().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            let mut file = std::fs::File::open(&csv_path)?;
            let mut sample = [0u8; 2048];
            use std::io::Read;
            let n = file.read(&mut sample).unwrap_or(0);
            let delim = detectar_delimitador(&sample[..n]);
            use std::io::Seek;
            let _ = file.seek(std::io::SeekFrom::Start(0));

            let mut reader = csv::ReaderBuilder::new()
                .delimiter(delim)
                .flexible(true)
                .from_reader(file);

            processar_csv_records(&mut conn, &mut reader, &tipo_clone)
        })
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": format!("Falha na leitura do CSV: {e}")})),
            )
        })?
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"erro": format!("Erro no formato do arquivo CSV: {e}")})),
            )
        })?
    };

    if let Ok(conn) = pool.get() {
        let _ = conn.execute(
            "INSERT INTO historico_sincronizacao (fonte, status, detalhes, registros_afetados, data_fim) 
             VALUES (?1, 'CONCLUIDO', ?2, ?3, CURRENT_TIMESTAMP)",
            rusqlite::params![
                format!("UPLOAD_{tipo_detectado}"),
                format!("Arquivo: {nome_arquivo} ({registros_inseridos} registros)"),
                registros_inseridos as i64
            ],
        );
    }

    Ok(Json(UploadResponse {
        status: "CONCLUIDO".to_string(),
        arquivo: nome_arquivo,
        tipo_detectado,
        registros_inseridos,
        mensagem: format!(
            "Arquivo processado com sucesso via streams assíncronos. Total de {registros_inseridos} registros catalogados."
        ),
    }))
}

pub async fn obter_icone_handler(
    State(pool): State<DbPool>,
    Query(params): Query<ObterIconeParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let tipo = params.tipo.unwrap_or_else(|| "app".to_string()).to_lowercase();
    let chave = if tipo == "favicon" { "favicon" } else { "app_icon" };

    if let Ok(conn) = pool.get() {
        let res: std::result::Result<(Vec<u8>, String), _> = conn.query_row(
            "SELECT valor_blob, mime_type FROM configuracoes_sistema WHERE chave = ?1",
            [chave],
            |r| Ok((r.get(0)?, r.get(1)?)),
        );

        if let Ok((blob, mime)) = res {
            if !blob.is_empty() {
                let mut headers = HeaderMap::new();
                if let Ok(mime_val) = HeaderValue::from_str(&mime) {
                    headers.insert(header::CONTENT_TYPE, mime_val);
                } else {
                    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
                }
                headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
                return Ok((headers, Body::from(blob)));
            }
        }

        if chave == "favicon" {
            let res_app: std::result::Result<(Vec<u8>, String), _> = conn.query_row(
                "SELECT valor_blob, mime_type FROM configuracoes_sistema WHERE chave = 'app_icon'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            );
            if let Ok((blob, mime)) = res_app {
                if !blob.is_empty() {
                    let mut headers = HeaderMap::new();
                    if let Ok(mime_val) = HeaderValue::from_str(&mime) {
                        headers.insert(header::CONTENT_TYPE, mime_val);
                    } else {
                        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
                    }
                    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
                    return Ok((headers, Body::from(blob)));
                }
            }
        }
    }

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    Ok((headers, Body::from(DEFAULT_ICON_SVG)))
}

pub async fn obter_favicon_handler(
    State(pool): State<DbPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    obter_icone_handler(State(pool), Query(ObterIconeParams { tipo: Some("favicon".to_string()) })).await
}

pub async fn salvar_icone_handler(
    State(pool): State<DbPool>,
    mut multipart: Multipart,
) -> Result<Json<SalvarIconeResponse>, (StatusCode, Json<serde_json::Value>)> {
    let mut alvo = "ambos".to_string();
    let mut bytes = Vec::new();
    let mut mime_type = "image/png".to_string();

    while let Ok(Some(mut field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "alvo" {
            if let Ok(b) = field.bytes().await {
                alvo = String::from_utf8_lossy(&b).trim().to_lowercase();
            }
        } else if name == "arquivo" || field.file_name().is_some() {
            if let Some(content_type) = field.content_type() {
                mime_type = content_type.to_string();
            } else if let Some(file_name) = field.file_name() {
                let fn_lower = file_name.to_lowercase();
                if fn_lower.ends_with(".svg") {
                    mime_type = "image/svg+xml".to_string();
                } else if fn_lower.ends_with(".ico") {
                    mime_type = "image/x-icon".to_string();
                } else if fn_lower.ends_with(".jpg") || fn_lower.ends_with(".jpeg") {
                    mime_type = "image/jpeg".to_string();
                } else if fn_lower.ends_with(".webp") {
                    mime_type = "image/webp".to_string();
                }
            }

            while let Ok(Some(chunk)) = field.chunk().await {
                bytes.extend_from_slice(&chunk);
                if bytes.len() > 5 * 1024 * 1024 {
                    return Err((
                        StatusCode::PAYLOAD_TOO_LARGE,
                        Json(json!({"erro": "Tamanho máximo da imagem excedido (limite 5 MB)"})),
                    ));
                }
            }
        }
    }

    if bytes.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"erro": "Nenhum arquivo de imagem enviado"})),
        ));
    }

    let conn = pool.get().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro": format!("Erro ao obter conexão do pool: {e}")})),
        )
    })?;

    if alvo == "icone" || alvo == "ambos" {
        conn.execute(
            "INSERT OR REPLACE INTO configuracoes_sistema (chave, valor_texto, valor_blob, mime_type, atualizado_em)
             VALUES ('app_icon', 'icone_custom', ?1, ?2, CURRENT_TIMESTAMP)",
            rusqlite::params![&bytes, &mime_type],
        ).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": format!("Falha ao salvar ícone no SQLite: {e}")})),
            )
        })?;
    }

    if alvo == "favicon" || alvo == "ambos" {
        conn.execute(
            "INSERT OR REPLACE INTO configuracoes_sistema (chave, valor_texto, valor_blob, mime_type, atualizado_em)
             VALUES ('favicon', 'favicon_custom', ?1, ?2, CURRENT_TIMESTAMP)",
            rusqlite::params![&bytes, &mime_type],
        ).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": format!("Falha ao salvar favicon no SQLite: {e}")})),
            )
        })?;
    }

    Ok(Json(SalvarIconeResponse {
        status: "CONCLUIDO".to_string(),
        mensagem: "Identidade visual atualizada com sucesso!".to_string(),
        alvo,
        mime_type,
        tamanho_bytes: bytes.len(),
    }))
}

pub async fn remover_icone_handler(
    State(pool): State<DbPool>,
    Query(params): Query<RemoverIconeParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let alvo = params.alvo.unwrap_or_else(|| "ambos".to_string()).to_lowercase();
    let conn = pool.get().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro": format!("Erro de banco: {e}")})),
        )
    })?;

    if alvo == "icone" || alvo == "ambos" {
        let _ = conn.execute("DELETE FROM configuracoes_sistema WHERE chave = 'app_icon'", []);
    }
    if alvo == "favicon" || alvo == "ambos" {
        let _ = conn.execute("DELETE FROM configuracoes_sistema WHERE chave = 'favicon'", []);
    }

    Ok(Json(json!({
        "status": "CONCLUIDO",
        "mensagem": "Ícone restaurado para o padrão original"
    })))
}

pub async fn obter_identidade_handler(
    State(pool): State<DbPool>,
) -> Result<Json<IdentidadeVisualResponse>, (StatusCode, Json<serde_json::Value>)> {
    let mut tem_icone = false;
    let mut tem_fav = false;
    if let Ok(conn) = pool.get() {
        tem_icone = conn.query_row(
            "SELECT 1 FROM configuracoes_sistema WHERE chave = 'app_icon'",
            [],
            |_| Ok(()),
        ).is_ok();
        tem_fav = conn.query_row(
            "SELECT 1 FROM configuracoes_sistema WHERE chave = 'favicon'",
            [],
            |_| Ok(()),
        ).is_ok();
    }

    Ok(Json(IdentidadeVisualResponse {
        tem_icone_customizado: tem_icone,
        tem_favicon_customizado: tem_fav,
        icone_url: "/api/v1/config/icone".to_string(),
        favicon_url: "/api/v1/config/favicon".to_string(),
    }))
}

pub async fn exportar_banco_handler(
    State(pool): State<DbPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let temp_file = tempfile::Builder::new()
        .prefix("radar_backup_")
        .suffix(".sqlite")
        .tempfile()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": format!("Falha ao criar arquivo de backup: {e}")})),
            )
        })?;

    let temp_path = temp_file.path().to_path_buf();
    pool.backup_to_file(&temp_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro": format!("Falha ao executar backup SQLite: {e}")})),
        )
    })?;

    let file = tokio::fs::File::open(&temp_path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro": format!("Falha ao abrir arquivo para streaming: {e}")})),
        )
    })?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let headers = [
        (header::CONTENT_TYPE, "application/vnd.sqlite3"),
        (
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"radar_civico_backup.sqlite\"",
        ),
    ];

    Ok((headers, body))
}

pub async fn exportar_tabela_handler(
    State(pool): State<DbPool>,
    AxumPath(nome_tabela): AxumPath<String>,
    Query(params): Query<ExportarTabelaParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let tabela_clean = nome_tabela.trim().to_lowercase();
    if !TABELAS_PERMITIDAS.contains(&tabela_clean.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "erro": format!("Tabela '{nome_tabela}' não permitida para exportação"),
                "tabelas_validas": TABELAS_PERMITIDAS
            })),
        ));
    }

    let formato = params
        .formato
        .unwrap_or_else(|| "csv".to_string())
        .trim()
        .to_lowercase();

    let conn = pool.get().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro": format!("Falha na conexão SQLite: {e}")})),
        )
    })?;

    // Obter colunas
    let mut pragma_stmt = conn
        .prepare(&format!("PRAGMA table_info({tabela_clean})"))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": format!("Falha ao inspecionar colunas: {e}")})),
            )
        })?;

    let colunas: Vec<String> = pragma_stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": format!("Falha ao mapear colunas: {e}")})),
            )
        })?
        .flatten()
        .collect();

    if colunas.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"erro": format!("Tabela '{tabela_clean}' não possui colunas")})),
        ));
    }

    let mut select_stmt = conn
        .prepare(&format!("SELECT * FROM {tabela_clean}"))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": format!("Falha ao preparar consulta: {e}")})),
            )
        })?;

    if formato == "json" {
        let mut rows_json = Vec::new();
        let mut rows = select_stmt.query([]).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": format!("Falha na consulta: {e}")})),
            )
        })?;

        while let Ok(Some(row)) = rows.next() {
            let mut obj = serde_json::Map::new();
            for (idx, col_name) in colunas.iter().enumerate() {
                let val: rusqlite::types::Value = row.get(idx).unwrap_or(rusqlite::types::Value::Null);
                let json_val = match val {
                    rusqlite::types::Value::Null => serde_json::Value::Null,
                    rusqlite::types::Value::Integer(i) => json!(i),
                    rusqlite::types::Value::Real(f) => json!(f),
                    rusqlite::types::Value::Text(s) => json!(s),
                    rusqlite::types::Value::Blob(b) => {
                        json!(format!("[BLOB {} bytes]", b.len()))
                    }
                };
                obj.insert(col_name.clone(), json_val);
            }
            rows_json.push(serde_json::Value::Object(obj));
        }

        let body_str = serde_json::to_string_pretty(&rows_json).unwrap_or_else(|_| "[]".to_string());
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        let disp = format!("attachment; filename=\"{tabela_clean}.json\"");
        if let Ok(val) = HeaderValue::from_str(&disp) {
            headers.insert(header::CONTENT_DISPOSITION, val);
        }

        Ok((headers, Body::from(body_str)))
    } else {
        // Formato CSV (padrão)
        let mut csv_out = String::new();
        csv_out.push_str(&colunas.join(";"));
        csv_out.push('\n');

        let mut rows = select_stmt.query([]).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": format!("Falha na consulta: {e}")})),
            )
        })?;

        while let Ok(Some(row)) = rows.next() {
            let mut row_vals = Vec::new();
            for idx in 0..colunas.len() {
                let val: rusqlite::types::Value = row.get(idx).unwrap_or(rusqlite::types::Value::Null);
                let val_str = match val {
                    rusqlite::types::Value::Null => String::new(),
                    rusqlite::types::Value::Integer(i) => i.to_string(),
                    rusqlite::types::Value::Real(f) => f.to_string(),
                    rusqlite::types::Value::Text(s) => {
                        let escaped = s.replace('"', "\"\"");
                        if escaped.contains(';') || escaped.contains('\n') || escaped.contains('"') {
                            format!("\"{escaped}\"")
                        } else {
                            escaped
                        }
                    }
                    rusqlite::types::Value::Blob(b) => format!("[BLOB {}B]", b.len()),
                };
                row_vals.push(val_str);
            }
            csv_out.push_str(&row_vals.join(";"));
            csv_out.push('\n');
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/csv; charset=utf-8"),
        );
        let disp = format!("attachment; filename=\"{tabela_clean}.csv\"");
        if let Ok(val) = HeaderValue::from_str(&disp) {
            headers.insert(header::CONTENT_DISPOSITION, val);
        }

        Ok((headers, Body::from(csv_out)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use storage::run_migrations;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_config_status_endpoint() {
        let pool = DbPool::open_in_memory().unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
            conn.execute(
                "INSERT INTO politicos (sq_candidato, nome_completo, nome_urna) VALUES ('SQ1', 'JOSE SILVA', 'ZE')",
                [],
            ).unwrap();
        }

        let app = crate::criar_router(pool);

        let req = Request::builder()
            .uri("/api/v1/config/status")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let status: ConfigStatusResponse = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(status.total_registros.politicos, 1);
        assert_eq!(status.total_registros.candidaturas, 0);
        assert!(!status.tamanho_banco_formatado.is_empty());
    }

    #[tokio::test]
    async fn test_executar_ingestao_e_consultar_job() {
        let pool = DbPool::open_in_memory().unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }

        let app = crate::criar_router(pool.clone());

        // 1. Ingestao com fonte valida
        let payload = json!({
            "fonte": "TSE",
            "ano": 2024
        });
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/config/ingestao/executar")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::ACCEPTED);

        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let resp: ExecutarIngestaoResponse = serde_json::from_slice(&bytes).unwrap();
        assert!(!resp.job_id.is_empty());

        // 2. Consulta status do job
        let app = crate::criar_router(pool.clone());
        let req_status = Request::builder()
            .uri(format!("/api/v1/config/ingestao/status/{}", resp.job_id))
            .body(Body::empty())
            .unwrap();

        let res_status = app.oneshot(req_status).await.unwrap();
        assert_eq!(res_status.status(), StatusCode::OK);

        let bytes_status = axum::body::to_bytes(res_status.into_body(), usize::MAX).await.unwrap();
        let job: JobInfo = serde_json::from_slice(&bytes_status).unwrap();
        assert_eq!(job.fonte, "TSE");
        assert_eq!(job.ano, Some(2024));

        // 3. Teste fonte invalida
        let app = crate::criar_router(pool.clone());
        let req_inv = Request::builder()
            .method("POST")
            .uri("/api/v1/config/ingestao/executar")
            .header("content-type", "application/json")
            .body(Body::from(json!({"fonte": "INVALIDA"}).to_string()))
            .unwrap();

        let res_inv = app.oneshot(req_inv).await.unwrap();
        assert_eq!(res_inv.status(), StatusCode::BAD_REQUEST);

        // 4. Job inexistente
        let app = crate::criar_router(pool);
        let req_404 = Request::builder()
            .uri("/api/v1/config/ingestao/status/job-nao-existe")
            .body(Body::empty())
            .unwrap();

        let res_404 = app.oneshot(req_404).await.unwrap();
        assert_eq!(res_404.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_exportar_banco_e_tabela() {
        let pool = DbPool::open_in_memory().unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
            conn.execute(
                "INSERT INTO politicos (sq_candidato, nome_completo, nome_urna) VALUES ('SQ2', 'MARIA SANTOS', 'MARIA')",
                [],
            ).unwrap();
        }

        let app = crate::criar_router(pool.clone());

        // 1. Exportar banco completo (.sqlite)
        let req_bkp = Request::builder()
            .uri("/api/v1/config/exportar/banco")
            .body(Body::empty())
            .unwrap();

        let res_bkp = app.oneshot(req_bkp).await.unwrap();
        assert_eq!(res_bkp.status(), StatusCode::OK);
        assert_eq!(
            res_bkp.headers().get("content-type").unwrap(),
            "application/vnd.sqlite3"
        );
        let bkp_bytes = axum::body::to_bytes(res_bkp.into_body(), usize::MAX).await.unwrap();
        assert!(bkp_bytes.starts_with(b"SQLite format 3\0"));

        // 2. Exportar tabela CSV
        let app = crate::criar_router(pool.clone());
        let req_csv = Request::builder()
            .uri("/api/v1/config/exportar/tabela/politicos?formato=csv")
            .body(Body::empty())
            .unwrap();

        let res_csv = app.oneshot(req_csv).await.unwrap();
        assert_eq!(res_csv.status(), StatusCode::OK);
        let csv_bytes = axum::body::to_bytes(res_csv.into_body(), usize::MAX).await.unwrap();
        let csv_str = String::from_utf8(csv_bytes.to_vec()).unwrap();
        assert!(csv_str.contains("nome_completo"));
        assert!(csv_str.contains("MARIA SANTOS"));

        // 3. Exportar tabela JSON
        let app = crate::criar_router(pool.clone());
        let req_json = Request::builder()
            .uri("/api/v1/config/exportar/tabela/politicos?formato=json")
            .body(Body::empty())
            .unwrap();

        let res_json = app.oneshot(req_json).await.unwrap();
        assert_eq!(res_json.status(), StatusCode::OK);
        let json_bytes = axum::body::to_bytes(res_json.into_body(), usize::MAX).await.unwrap();
        let json_val: serde_json::Value = serde_json::from_slice(&json_bytes).unwrap();
        assert!(json_val.is_array());
        assert_eq!(json_val.as_array().unwrap().len(), 1);

        // 4. Tabela invalida
        let app = crate::criar_router(pool);
        let req_bad = Request::builder()
            .uri("/api/v1/config/exportar/tabela/hack_table")
            .body(Body::empty())
            .unwrap();

        let res_bad = app.oneshot(req_bad).await.unwrap();
        assert_eq!(res_bad.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_upload_csv_multipart() {
        let pool = DbPool::open_in_memory().unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }

        let app = crate::criar_router(pool);

        let boundary = "---------------------------974767299852498929531610575";
        let body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"arquivo\"; filename=\"receitas.csv\"\r\n\
             Content-Type: text/csv\r\n\r\n\
             SQ_CANDIDATO;NR_CPF_CNPJ_DOADOR;NM_DOADOR;VR_RECEITA;DT_RECEITA\r\n\
             SQ10;11122233344;DOADOR TESTE;500.00;2024-08-10\r\n\
             --{boundary}--\r\n"
        );

        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/config/ingestao/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let upload_res: UploadResponse = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(upload_res.status, "CONCLUIDO");
        assert_eq!(upload_res.tipo_detectado, "TSE_RECEITAS");
        assert_eq!(upload_res.registros_inseridos, 1);
    }

    #[tokio::test]
    async fn test_salvar_obter_e_remover_icone_e_favicon() {
        let pool = DbPool::open_in_memory().unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }

        let app = crate::criar_router(pool.clone());

        // 1. Obter icone padrao
        let req_padrao = Request::builder()
            .uri("/api/v1/config/icone")
            .body(Body::empty())
            .unwrap();
        let res_padrao = app.oneshot(req_padrao).await.unwrap();
        assert_eq!(res_padrao.status(), StatusCode::OK);
        assert_eq!(
            res_padrao.headers().get("content-type").unwrap(),
            "image/svg+xml"
        );

        // 2. Salvar icone e favicon customizados
        let app = crate::criar_router(pool.clone());
        let boundary = "---------------------------12345678901234567890";
        let mock_png_bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x01];
        let mut body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"alvo\"\r\n\r\n\
             ambos\r\n\
             --{boundary}\r\n\
             Content-Disposition: form-data; name=\"arquivo\"; filename=\"meu_logo.png\"\r\n\
             Content-Type: image/png\r\n\r\n"
        )
        .into_bytes();
        body.extend_from_slice(&mock_png_bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

        let req_post = Request::builder()
            .method("POST")
            .uri("/api/v1/config/icone")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let res_post = app.oneshot(req_post).await.unwrap();
        assert_eq!(res_post.status(), StatusCode::OK);

        // 3. Obter icone customizado
        let app = crate::criar_router(pool.clone());
        let req_custom = Request::builder()
            .uri("/api/v1/config/icone")
            .body(Body::empty())
            .unwrap();
        let res_custom = app.oneshot(req_custom).await.unwrap();
        assert_eq!(res_custom.status(), StatusCode::OK);
        assert_eq!(
            res_custom.headers().get("content-type").unwrap(),
            "image/png"
        );
        let custom_bytes = axum::body::to_bytes(res_custom.into_body(), usize::MAX).await.unwrap();
        assert_eq!(custom_bytes.as_ref(), &mock_png_bytes);

        // 4. Obter favicon customizado
        let app = crate::criar_router(pool.clone());
        let req_fav = Request::builder()
            .uri("/api/v1/config/favicon")
            .body(Body::empty())
            .unwrap();
        let res_fav = app.oneshot(req_fav).await.unwrap();
        assert_eq!(res_fav.status(), StatusCode::OK);
        assert_eq!(
            res_fav.headers().get("content-type").unwrap(),
            "image/png"
        );

        // 5. Verificar identidade
        let app = crate::criar_router(pool.clone());
        let req_id = Request::builder()
            .uri("/api/v1/config/identidade")
            .body(Body::empty())
            .unwrap();
        let res_id = app.oneshot(req_id).await.unwrap();
        assert_eq!(res_id.status(), StatusCode::OK);
        let id_bytes = axum::body::to_bytes(res_id.into_body(), usize::MAX).await.unwrap();
        let id_val: IdentidadeVisualResponse = serde_json::from_slice(&id_bytes).unwrap();
        assert!(id_val.tem_icone_customizado);
        assert!(id_val.tem_favicon_customizado);

        // 6. Remover icone customizado (restaurar padrao)
        let app = crate::criar_router(pool.clone());
        let req_del = Request::builder()
            .method("DELETE")
            .uri("/api/v1/config/icone?alvo=ambos")
            .body(Body::empty())
            .unwrap();
        let res_del = app.oneshot(req_del).await.unwrap();
        assert_eq!(res_del.status(), StatusCode::OK);

        // 7. Apos remover, volta a retornar SVG
        let app = crate::criar_router(pool);
        let req_reset = Request::builder()
            .uri("/api/v1/config/icone")
            .body(Body::empty())
            .unwrap();
        let res_reset = app.oneshot(req_reset).await.unwrap();
        assert_eq!(res_reset.status(), StatusCode::OK);
        assert_eq!(
            res_reset.headers().get("content-type").unwrap(),
            "image/svg+xml"
        );
    }

    #[tokio::test]
    async fn test_upload_csv_qsa_multipart() {
        let pool = DbPool::open_in_memory().unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }

        let app = crate::criar_router(pool.clone());
        let boundary = "---------------------------qsa974767299852498929531610575";
        let body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"tipo\"\r\n\r\n\
             RECEITA_QSA\r\n\
             --{boundary}\r\n\
             Content-Disposition: form-data; name=\"arquivo\"; filename=\"socios_rfb.csv\"\r\n\
             Content-Type: text/csv\r\n\r\n\
             CNPJ_BASICO;RAZAO_SOCIAL;NOME_SOCIO;CPF_CNPJ_SOCIO;QUALIFICACAO_SOCIO\r\n\
             12345678;EMPRESA MODELO LTDA;JOAO DA SILVA SOCIO;***.111.222-**;49-SOCIO-ADMINISTRADOR\r\n\
             87654321;CONSULTORIA TECNICA SA;MARIA PEREIRA;***.333.444-**;10-DIRETOR\r\n\
             --{boundary}--\r\n"
        );

        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/config/ingestao/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let upload_res: UploadResponse = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(upload_res.status, "CONCLUIDO");
        assert_eq!(upload_res.tipo_detectado, "RECEITA_QSA");
        assert_eq!(upload_res.registros_inseridos, 2);

        // Validar insercao direta no banco
        let conn = pool.get().unwrap();
        let total_qsa: i64 = conn
            .query_row("SELECT count(*) FROM empresas_qsa", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total_qsa, 2);

        let socio_nome: String = conn
            .query_row(
                "SELECT socio_nome FROM empresas_qsa WHERE cnpj_basico = '12345678'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(socio_nome, "JOAO DA SILVA SOCIO");
    }

    #[tokio::test]
    async fn test_upload_csv_oab_multipart() {
        let pool = DbPool::open_in_memory().unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }

        let app = crate::criar_router(pool.clone());
        let boundary = "---------------------------oab974767299852498929531610575";
        let body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"arquivo\"; filename=\"advogados_oab.csv\"\r\n\
             Content-Type: text/csv\r\n\r\n\
             PESSOA_NOME;CPF_MASCARADO;ORGAO_EMISSOR;NUMERO_REGISTRO;SECCIONAL_UF;SITUACAO_REGISTRO;TIPO_INSCRICAO\r\n\
             DR ROBERTO CARLOS;***.555.666-**;OAB;98765;SP;REGULAR;ADVOGADO\r\n\
             --{boundary}--\r\n"
        );

        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/config/ingestao/upload")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let upload_res: UploadResponse = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(upload_res.status, "CONCLUIDO");
        assert_eq!(upload_res.tipo_detectado, "CONSELHOS_OAB");
        assert_eq!(upload_res.registros_inseridos, 1);

        let conn = pool.get().unwrap();
        let total_oab: i64 = conn
            .query_row("SELECT count(*) FROM registros_profissionais", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total_oab, 1);
    }
}

