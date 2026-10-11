use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ImportProgress {
    pub is_running: bool,
    pub current_file: String,
    pub files_processed: usize,
    pub total_files: usize,
    pub records_processed: u64,
    pub percentage: f32,
    pub started_at: Option<String>,
    pub elapsed_seconds: u64,
    pub last_error: Option<String>,
}

pub fn get_import_progress() -> &'static Arc<RwLock<ImportProgress>> {
    static PROGRESS: OnceLock<Arc<RwLock<ImportProgress>>> = OnceLock::new();
    PROGRESS.get_or_init(|| Arc::new(RwLock::new(ImportProgress::default())))
}

pub fn reset_import_progress(total_files: usize) {
    if let Ok(mut lock) = get_import_progress().write() {
        *lock = ImportProgress {
            is_running: true,
            current_file: "Iniciando importação...".to_string(),
            files_processed: 0,
            total_files,
            records_processed: 0,
            percentage: 0.0,
            started_at: Some(Utc::now().to_rfc3339()),
            elapsed_seconds: 0,
            last_error: None,
        };
    }
}

pub fn update_import_progress_batch(
    current_file: &str,
    files_processed: usize,
    total_files: usize,
    new_records: u64,
    start_time: Instant,
) {
    if let Ok(mut lock) = get_import_progress().write() {
        lock.current_file = current_file.to_string();
        lock.files_processed = files_processed;
        lock.total_files = total_files;
        lock.records_processed += new_records;
        lock.elapsed_seconds = start_time.elapsed().as_secs();
        if total_files > 0 {
            let base_pct = (files_processed as f32 / total_files as f32) * 100.0;
            lock.percentage = base_pct.min(99.0);
        }
    }
}

pub fn finish_import_progress(start_time: Instant) {
    if let Ok(mut lock) = get_import_progress().write() {
        lock.is_running = false;
        lock.current_file = "Concluído".to_string();
        lock.percentage = 100.0;
        lock.elapsed_seconds = start_time.elapsed().as_secs();
    }
}

pub fn set_import_error(err: &str, start_time: Instant) {
    if let Ok(mut lock) = get_import_progress().write() {
        lock.is_running = false;
        lock.last_error = Some(err.to_string());
        lock.elapsed_seconds = start_time.elapsed().as_secs();
    }
}

/// Restaura também o progresso dos endpoints antigos de importação.
pub fn restore_import_progress(pool: &storage::DbPool) -> crate::Result<()> {
    let conn = pool.get()?;
    if let Some(payload) = storage::jobs::load(&conn, "legacy-progress", "global")? {
        let progress = serde_json::from_str(&payload)
            .map_err(|e| crate::IngestionError::Parse(e.to_string()))?;
        *get_import_progress().write().map_err(|_| {
            crate::IngestionError::Custom("Trava de progresso indisponível".into())
        })? = progress;
    }
    Ok(())
}

/// Uma cópia por segundo; não escreve novamente quando o estado não mudou.
pub async fn persist_progress_loop(pool: storage::DbPool) {
    let mut previous = String::new();
    let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        timer.tick().await;
        let snapshot = {
            match get_import_progress().read() {
                Ok(progress) => Some(progress.clone()),
                Err(error) => {
                    tracing::error!("Falha ao ler progresso: {error}");
                    None
                }
            }
        };
        let Some(snapshot) = snapshot else {
            continue;
        };
        let payload = match serde_json::to_string(&snapshot) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Falha ao serializar progresso: {e}");
                continue;
            }
        };
        if payload == previous {
            continue;
        }
        let saved_payload = payload.clone();
        match pool
            .run_blocking(move |conn| {
                storage::jobs::save(
                    conn,
                    "legacy-progress",
                    "global",
                    snapshot.is_running,
                    &saved_payload,
                )
            })
            .await
        {
            Ok(()) => previous = payload,
            Err(error) => tracing::error!("Falha ao persistir progresso de importação: {error}"),
        }
    }
}
