use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::importers::sink::BatchSink;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ImportStage {
    Idle,
    Conectando,
    Baixando,
    Descompactando,
    Processando,
    Finalizando,
    Concluido,
    Cancelado,
    Erro,
}

impl Default for ImportStage {
    fn default() -> Self {
        ImportStage::Idle
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportProgress {
    pub importer_id: String,
    pub stage: ImportStage,
    pub is_running: bool,
    pub current_file: String,
    pub files_processed: usize,
    pub total_files: usize,
    pub records_processed: u64,
    pub percentage: f32,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub elapsed_seconds: u64,
    pub message: String,
    pub last_error: Option<String>,
}

impl ImportProgress {
    pub fn new(importer_id: impl Into<String>) -> Self {
        Self {
            importer_id: importer_id.into(),
            stage: ImportStage::Idle,
            is_running: false,
            current_file: String::new(),
            files_processed: 0,
            total_files: 0,
            records_processed: 0,
            percentage: 0.0,
            started_at: None,
            finished_at: None,
            elapsed_seconds: 0,
            message: "Aguardando início".to_string(),
            last_error: None,
        }
    }
}

#[derive(Clone)]
pub struct ImportContext {
    pub importer_id: String,
    pub cancellation_token: CancellationToken,
    progress: Arc<RwLock<ImportProgress>>,
    started_instant: Instant,
}

impl ImportContext {
    pub fn new(importer_id: impl Into<String>) -> Self {
        let id = importer_id.into();
        let mut initial_progress = ImportProgress::new(&id);
        initial_progress.is_running = true;
        initial_progress.stage = ImportStage::Conectando;
        initial_progress.started_at = Some(Utc::now().to_rfc3339());
        initial_progress.message = "Inicializando importador...".to_string();

        Self {
            importer_id: id,
            cancellation_token: CancellationToken::new(),
            progress: Arc::new(RwLock::new(initial_progress)),
            started_instant: Instant::now(),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    pub fn cancel(&self) {
        self.cancellation_token.cancel();
        if let Ok(mut p) = self.progress.write() {
            p.message = "Cancelamento solicitado; aguardando término do processamento atual".into();
        }
    }

    pub fn set_stage(&self, stage: ImportStage, message: impl Into<String>) {
        if let Ok(mut p) = self.progress.write() {
            p.stage = stage;
            p.message = message.into();
            p.elapsed_seconds = self.started_instant.elapsed().as_secs();
            if stage == ImportStage::Concluido
                || stage == ImportStage::Cancelado
                || stage == ImportStage::Erro
            {
                p.is_running = false;
                p.finished_at = Some(Utc::now().to_rfc3339());
            }
        }
    }

    pub fn update_progress(
        &self,
        current_file: impl Into<String>,
        files_processed: usize,
        total_files: usize,
        records_processed: u64,
    ) {
        if let Ok(mut p) = self.progress.write() {
            p.current_file = current_file.into();
            p.files_processed = files_processed;
            p.total_files = total_files;
            p.records_processed = records_processed;
            p.percentage = if total_files > 0 {
                (files_processed as f32 / total_files as f32) * 100.0
            } else {
                0.0
            };
            p.elapsed_seconds = self.started_instant.elapsed().as_secs();
        }
    }

    pub fn add_records(&self, count: u64) {
        if let Ok(mut p) = self.progress.write() {
            p.records_processed += count;
            p.elapsed_seconds = self.started_instant.elapsed().as_secs();
        }
    }

    pub fn set_error(&self, error: impl Into<String>) {
        let err_msg = error.into();
        if let Ok(mut p) = self.progress.write() {
            p.stage = ImportStage::Erro;
            p.is_running = false;
            p.last_error = Some(err_msg.clone());
            p.message = format!("Erro: {}", err_msg);
            p.finished_at = Some(Utc::now().to_rfc3339());
            p.elapsed_seconds = self.started_instant.elapsed().as_secs();
        }
    }

    pub fn finish(&self, message: impl Into<String>) {
        if let Ok(mut p) = self.progress.write() {
            p.stage = ImportStage::Concluido;
            p.is_running = false;
            p.percentage = 100.0;
            p.message = message.into();
            p.finished_at = Some(Utc::now().to_rfc3339());
            p.elapsed_seconds = self.started_instant.elapsed().as_secs();
        }
    }

    pub fn get_progress(&self) -> ImportProgress {
        match self.progress.read() {
            Ok(p) => {
                let mut current = p.clone();
                if current.is_running {
                    current.elapsed_seconds = self.started_instant.elapsed().as_secs();
                }
                current
            }
            Err(_) => ImportProgress::new(&self.importer_id),
        }
    }
}

#[async_trait::async_trait]
pub trait SourceImporter: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn run(&self, ctx: Arc<ImportContext>, sink: Arc<BatchSink>) -> Result<()>;
}
