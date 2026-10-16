use super::config::{atualizar_job_concluido, registrar_job, spawn_monitored_job, JobInfo};
use axum::{
    body::Body,
    extract::{Path as AxumPath, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Json},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use storage::DbPool;
use tokio_util::io::ReaderStream;

fn backup_dir(pool: &DbPool) -> std::path::PathBuf {
    pool.file_path()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var("DATA_DIR").unwrap_or_else(|_| "data".into()))
        })
        .join("backups")
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

    let temporary = tokio::task::spawn_blocking(move || {
        pool.backup_to_file(temp_file.path())?;
        Ok::<_, storage::StorageError>(temp_file)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro":e.to_string()})),
        )
    })?
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro":e.to_string()})),
        )
    })?;
    let file = tokio::fs::File::from_std(temporary.reopen().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro":e.to_string()})),
        )
    })?);
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupItemInfo {
    pub nome_arquivo: String,
    pub tamanho_bytes: u64,
    pub tamanho_formatado: String,
    pub criado_em: String,
    pub download_url: String,
    #[serde(default)]
    pub hash_sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TestarWebhookRequest {
    pub url: String,
    pub evento: Option<String>,
}

pub async fn criar_backup_handler(
    State(pool): State<DbPool>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let job_id = uuid::Uuid::new_v4().to_string();
    let nome_arquivo = format!(
        "radar_backup_{}_{}.sqlite",
        Utc::now().format("%Y%m%d_%H%M%S"),
        job_id
    );
    let now = Utc::now().to_rfc3339();
    registrar_job(
        &pool,
        JobInfo {
            job_id: job_id.clone(),
            fonte: "BACKUP".into(),
            ano: None,
            status: "PROCESSANDO".into(),
            progresso: 10,
            mensagem: "Criando snapshot SQLite em segundo plano".into(),
            logs: vec![format!("[{now}] Backup solicitado: {nome_arquivo}")],
            criado_em: now,
            concluido_em: None,
            persistencia_pendente: false,
        },
    )
    .await?;
    let worker_id = job_id.clone();
    let worker_name = nome_arquivo.clone();
    let worker_pool = pool.clone();
    spawn_monitored_job(pool.clone(), job_id.clone(), async move {
        let backup_pool = worker_pool.clone();
        let result = tokio::task::spawn_blocking(move || -> storage::Result<()> {
            let dir = backup_dir(&backup_pool);
            std::fs::create_dir_all(&dir)?;
            let final_path = dir.join(&worker_name);
            let partial = dir.join(format!("{worker_name}.partial"));
            let operation = (|| {
                backup_pool.backup_to_file(&partial)?;
                let (hash, _) = storage::calcular_sha256_file(&partial)?;
                let sidecar = dir.join(format!("{worker_name}.sha256"));
                std::fs::write(&sidecar, format!("{hash}  {worker_name}\n"))?;
                std::fs::rename(&partial, &final_path)?;
                Ok(())
            })();
            if operation.is_err() {
                let _ = std::fs::remove_file(&partial);
                let _ = std::fs::remove_file(dir.join(format!("{worker_name}.sha256")));
            }
            operation
        })
        .await;
        match result {
            Ok(Ok(()))=>atualizar_job_concluido(&worker_pool,&worker_id,100,"CONCLUIDO","Backup concluído, checksum SHA-256 calculado; arquivo disponível na lista de backups.").await,
            Ok(Err(error))=>atualizar_job_concluido(&worker_pool,&worker_id,100,"ERRO",&format!("Backup falhou: {error}")).await,
            Err(error)=>atualizar_job_concluido(&worker_pool,&worker_id,100,"ERRO",&format!("Worker de backup falhou: {error}")).await,
        }
    });
    Ok((
        StatusCode::ACCEPTED,
        Json(
            json!({"status":"PROCESSANDO","job_id":job_id,"nome_arquivo":nome_arquivo,"mensagem":"Backup iniciado; acompanhe a tarefa.","download_url":format!("/api/v1/config/backups/{nome_arquivo}")}),
        ),
    ))
}

pub async fn listar_backups_handler(
    State(pool): State<DbPool>,
) -> Result<Json<Vec<BackupItemInfo>>, (StatusCode, Json<serde_json::Value>)> {
    let backup_dir = backup_dir(&pool);
    if !backup_dir.exists() {
        return Ok(Json(Vec::new()));
    }

    let mut lista = Vec::new();
    let mut entries = tokio::fs::read_dir(&backup_dir).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"status": "erro", "mensagem": format!("Falha ao ler diretório de backups: {e}")})),
        )
    })?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("sqlite")
            && tokio::fs::try_exists(format!("{}.sha256", path.display()))
                .await
                .unwrap_or(false)
        {
            if let Ok(meta) = entry.metadata().await {
                let nome_arquivo = entry.file_name().to_string_lossy().to_string();
                let tamanho_bytes = meta.len();
                let tamanho_formatado = format!("{:.2} MB", (tamanho_bytes as f64) / 1_048_576.0);
                let criado_em = meta
                    .modified()
                    .ok()
                    .map(|sys_time| {
                        let dt: chrono::DateTime<Utc> = sys_time.into();
                        dt.to_rfc3339()
                    })
                    .unwrap_or_else(|| Utc::now().to_rfc3339());

                let sidecar_path = backup_dir.join(format!("{nome_arquivo}.sha256"));
                let hash_sha256 = if let Ok(conteudo) =
                    tokio::fs::read_to_string(&sidecar_path).await
                {
                    conteudo.split_whitespace().next().map(|s| s.to_string())
                } else if let Ok((h, _)) = storage::calcular_sha256_file(&path) {
                    let _ = tokio::fs::write(&sidecar_path, format!("{h}  {nome_arquivo}\n")).await;
                    Some(h)
                } else {
                    None
                };

                lista.push(BackupItemInfo {
                    download_url: format!("/api/v1/config/backups/{nome_arquivo}"),
                    nome_arquivo,
                    tamanho_bytes,
                    tamanho_formatado,
                    criado_em,
                    hash_sha256,
                });
            }
        }
    }

    lista.sort_by(|a, b| b.criado_em.cmp(&a.criado_em));
    Ok(Json(lista))
}

pub async fn download_backup_arquivo_handler(
    State(pool): State<DbPool>,
    AxumPath(nome_arquivo): AxumPath<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let nome_limpo = std::path::Path::new(&nome_arquivo)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"status": "erro", "mensagem": "Nome de arquivo inválido"})),
            )
        })?;

    if nome_limpo != nome_arquivo
        || !(nome_limpo.ends_with(".sqlite") || nome_limpo.ends_with(".sqlite.sha256"))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"erro":"Nome de backup inválido"})),
        ));
    }
    let arquivo_path = backup_dir(&pool).join(nome_limpo);
    if !arquivo_path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"status": "erro", "mensagem": "Arquivo de backup não encontrado"})),
        ));
    }

    let file = tokio::fs::File::open(&arquivo_path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"status": "erro", "mensagem": format!("Falha ao abrir backup: {e}")})),
        )
    })?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let is_sidecar = nome_limpo.ends_with(".sha256");
    let content_type = if is_sidecar {
        "text/plain; charset=utf-8"
    } else {
        "application/vnd.sqlite3"
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{nome_limpo}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, v);
    }

    if !is_sidecar {
        let sidecar_path = backup_dir(&pool).join(format!("{nome_limpo}.sha256"));
        if let Ok(conteudo) = tokio::fs::read_to_string(&sidecar_path).await {
            if let Some(h) = conteudo.split_whitespace().next() {
                if let Ok(val) = HeaderValue::from_str(h) {
                    headers.insert("x-checksum-sha256", val);
                }
            }
        }
    }

    Ok((headers, body))
}
