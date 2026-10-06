use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};

use crate::error::{IngestionError, Result};
use crate::importers::sink::BatchSink;
use crate::importers::traits::{ImportContext, ImportProgress, ImportStage, SourceImporter};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImporterSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub stage: ImportStage,
    pub is_running: bool,
    pub current_file: String,
    pub files_processed: usize,
    pub total_files: usize,
    pub records_processed: u64,
    pub percentage: f32,
    pub elapsed_seconds: u64,
    pub message: String,
    pub last_error: Option<String>,
}

#[derive(Clone)]
pub struct ImporterManager {
    importers: Arc<RwLock<HashMap<String, Arc<dyn SourceImporter>>>>,
    contexts: Arc<RwLock<HashMap<String, Arc<ImportContext>>>>,
    sink: Arc<BatchSink>,
}

impl ImporterManager {
    pub fn new(sink: Arc<BatchSink>) -> Self {
        Self {
            importers: Arc::new(RwLock::new(HashMap::new())),
            contexts: Arc::new(RwLock::new(HashMap::new())),
            sink,
        }
    }

    pub fn register(&self, importer: Arc<dyn SourceImporter>) {
        if let Ok(mut map) = self.importers.write() {
            map.insert(importer.id().to_string(), importer);
        }
    }

    pub fn get_importer(&self, id: &str) -> Option<Arc<dyn SourceImporter>> {
        self.importers.read().ok()?.get(id).cloned()
    }

    pub fn get_sink(&self) -> Arc<BatchSink> {
        Arc::clone(&self.sink)
    }

    pub fn list(&self) -> Vec<ImporterSummary> {
        let importers = match self.importers.read() {
            Ok(map) => map.clone(),
            Err(_) => return Vec::new(),
        };

        let contexts = match self.contexts.read() {
            Ok(map) => map.clone(),
            Err(_) => HashMap::new(),
        };

        let mut list = Vec::new();
        for (id, importer) in importers {
            let progress = contexts
                .get(&id)
                .map(|ctx| ctx.get_progress())
                .unwrap_or_else(|| ImportProgress::new(&id));

            list.push(ImporterSummary {
                id: id.clone(),
                name: importer.name().to_string(),
                description: importer.description().to_string(),
                stage: progress.stage,
                is_running: progress.is_running,
                current_file: progress.current_file,
                files_processed: progress.files_processed,
                total_files: progress.total_files,
                records_processed: progress.records_processed,
                percentage: progress.percentage,
                elapsed_seconds: progress.elapsed_seconds,
                message: progress.message,
                last_error: progress.last_error,
            });
        }

        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
    }

    pub fn get_status(&self, id: &str) -> Option<ImportProgress> {
        let contexts = self.contexts.read().ok()?;
        if let Some(ctx) = contexts.get(id) {
            Some(ctx.get_progress())
        } else if self.get_importer(id).is_some() {
            Some(ImportProgress::new(id))
        } else {
            None
        }
    }

    pub fn cancel(&self, id: &str) -> Result<()> {
        let contexts = self.contexts.read().map_err(|_| {
            IngestionError::Custom("Falha ao obter trava de contextos para cancelamento".to_string())
        })?;

        if let Some(ctx) = contexts.get(id) {
            ctx.cancel();
            Ok(())
        } else {
            Err(IngestionError::Custom(format!(
                "Nenhum processo de importação em execução para '{}'",
                id
            )))
        }
    }

    pub fn start(&self, id: &str) -> Result<Arc<ImportContext>> {
        let importer = self.get_importer(id).ok_or_else(|| {
            IngestionError::Custom(format!("Importador '{}' não encontrado", id))
        })?;

        // Verificar se já está rodando
        {
            let contexts = self.contexts.read().map_err(|_| {
                IngestionError::Custom("Falha de trava ao checar contexto existente".to_string())
            })?;
            if let Some(existing_ctx) = contexts.get(id) {
                if existing_ctx.get_progress().is_running {
                    return Err(IngestionError::Custom(format!(
                        "Importador '{}' já está em execução",
                        id
                    )));
                }
            }
        }

        let ctx = Arc::new(ImportContext::new(id));
        {
            let mut contexts = self.contexts.write().map_err(|_| {
                IngestionError::Custom("Falha de trava ao registrar novo contexto".to_string())
            })?;
            contexts.insert(id.to_string(), Arc::clone(&ctx));
        }

        let importer_cloned = Arc::clone(&importer);
        let ctx_cloned = Arc::clone(&ctx);
        let sink_cloned = Arc::clone(&self.sink);

        tokio::spawn(async move {
            ctx_cloned.set_stage(ImportStage::Processando, "Iniciando processamento...");
            match importer_cloned.run(Arc::clone(&ctx_cloned), sink_cloned).await {
                Ok(_) => {
                    if ctx_cloned.is_cancelled() {
                        ctx_cloned.set_stage(ImportStage::Cancelado, "Operação cancelada pelo usuário");
                    } else {
                        ctx_cloned.finish("Importação finalizada com sucesso");
                    }
                }
                Err(err) => {
                    ctx_cloned.set_error(err.to_string());
                }
            }
        });

        Ok(ctx)
    }
}
