use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;
use chrono::Utc;
use serde::{Deserialize, Serialize};

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
