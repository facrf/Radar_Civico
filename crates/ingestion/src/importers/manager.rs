use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

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
            IngestionError::Custom(
                "Falha ao obter trava de contextos para cancelamento".to_string(),
            )
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
        let importer = self
            .get_importer(id)
            .ok_or_else(|| IngestionError::Custom(format!("Importador '{}' não encontrado", id)))?;

        let ctx = {
            let mut contexts = self.contexts.write().map_err(|_| {
                IngestionError::Custom("Falha de trava ao registrar contexto".into())
            })?;
            if contexts
                .get(id)
                .is_some_and(|existing| existing.get_progress().is_running)
            {
                return Err(IngestionError::Custom(format!(
                    "Importador '{}' já está em execução",
                    id
                )));
            }
            let ctx = Arc::new(ImportContext::new(id));
            contexts.insert(id.to_string(), Arc::clone(&ctx));
            ctx
        };

        let importer_cloned = Arc::clone(&importer);
        let ctx_cloned = Arc::clone(&ctx);
        let sink_cloned = Arc::clone(&self.sink);

        tokio::spawn(async move {
            let monitor = ctx_cloned.clone();
            let result = tokio::spawn(async move {
                ctx_cloned.set_stage(ImportStage::Processando, "Iniciando processamento...");
                match importer_cloned
                    .run(Arc::clone(&ctx_cloned), sink_cloned)
                    .await
                {
                    Ok(_) => {
                        if ctx_cloned.is_cancelled() {
                            ctx_cloned.set_stage(
                                ImportStage::Cancelado,
                                "Operação cancelada pelo usuário",
                            );
                        } else {
                            ctx_cloned.finish("Importação finalizada com sucesso");
                        }
                    }
                    Err(err) => {
                        ctx_cloned.set_error(err.to_string());
                    }
                }
            })
            .await;
            if let Err(error) = result {
                monitor.set_error(format!("Worker de importação interrompido: {error}"));
            }
        });

        Ok(ctx)
    }
}

#[cfg(test)]
mod concurrency_regressions {
    use super::*;
    use async_trait::async_trait;
    struct WaitingImporter;
    #[async_trait]
    impl SourceImporter for WaitingImporter {
        fn id(&self) -> &str {
            "waiting"
        }
        fn name(&self) -> &str {
            "Importação controlada"
        }
        fn description(&self) -> &str {
            "Aguarda cancelamento para validar exclusão mútua"
        }
        async fn run(&self, ctx: Arc<ImportContext>, _sink: Arc<BatchSink>) -> Result<()> {
            ctx.cancellation_token.cancelled().await;
            Ok(())
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dois_inicios_simultaneos_criam_apenas_um_worker() {
        let manager = ImporterManager::new(Arc::new(BatchSink::new(
            storage::DbPool::open_in_memory().unwrap(),
        )));
        manager.register(Arc::new(WaitingImporter));
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let mut threads = Vec::new();
        for _ in 0..2 {
            let manager = manager.clone();
            let barrier = barrier.clone();
            let handle = tokio::runtime::Handle::current();
            threads.push(std::thread::spawn(move || {
                let _runtime = handle.enter();
                barrier.wait();
                manager.start("waiting").is_ok()
            }));
        }
        let started = threads
            .into_iter()
            .map(|t| t.join().unwrap())
            .filter(|started| *started)
            .count();
        assert_eq!(started, 1);
        manager.cancel("waiting").unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while manager.get_status("waiting").unwrap().is_running {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            manager.get_status("waiting").unwrap().stage,
            ImportStage::Cancelado
        );
    }
}
