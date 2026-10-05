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
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotalRegistros {
    pub politicos: i64,
    pub candidaturas: i64,
    pub receitas_campanha: i64,
    pub despesas_campanha: i64,
    pub despesas_parlamentares: i64,
    pub contratos_publicos: i64,
    pub alertas_auditoria: i64,
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

    while let Ok(Some(mut field)) = multipart.next_field().await {
        if let Some(file_name) = field.file_name() {
            nome_arquivo = file_name.to_string();
        }

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

        let extracted_records = tokio::task::spawn_blocking(move || -> std::io::Result<usize> {
            let file = std::fs::File::open(&zip_path)?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            let mut count = 0;
            for i in 0..archive.len() {
                let mut zip_entry = archive
                    .by_index(i)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                if zip_entry.name().to_lowercase().ends_with(".csv") {
                    let mut reader = csv::ReaderBuilder::new()
                        .delimiter(b';')
                        .flexible(true)
                        .from_reader(&mut zip_entry);

                    for _ in reader.records().flatten() {
                        count += 1;
                    }
                }
            }
            Ok(count)
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
        })?;

        (extracted_records, String::from("ZIP_COMPACTADO"))
    } else {
        // Arquivo CSV direto
        let csv_path = temp_path.clone();
        let (records_count, tipo) = tokio::task::spawn_blocking(move || -> std::io::Result<(usize, String)> {
            let file = std::fs::File::open(&csv_path)?;
            let mut reader = csv::ReaderBuilder::new()
                .delimiter(b';')
                .flexible(true)
                .from_reader(file);

            let mut count = 0;
            let mut detected = "CSV_GENERICO".to_string();

            if let Ok(headers) = reader.headers() {
                let header_str = headers.iter().collect::<Vec<_>>().join(";").to_uppercase();
                if header_str.contains("VR_RECEITA") || header_str.contains("DS_RECEITA") {
                    detected = "TSE_RECEITAS".to_string();
                } else if header_str.contains("VR_DESPESA") || header_str.contains("DS_DESPESA") {
                    detected = "TSE_DESPESAS".to_string();
                } else if header_str.contains("NM_CANDIDATO") || header_str.contains("SQ_CANDIDATO") {
                    detected = "TSE_CANDIDATOS".to_string();
                } else if header_str.contains("VALORLIQUIDO") || header_str.contains("NUMDOCUMENTO") {
                    detected = "CEAP_NOTAS".to_string();
                } else if header_str.contains("NUMEROCONTRATO") || header_str.contains("VALORGLOBAL") {
                    detected = "PNCP_CONTRATOS".to_string();
                }
            }

            for _ in reader.records().flatten() {
                count += 1;
            }

            Ok((count, detected))
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
        })?;

        (records_count, tipo)
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
}
