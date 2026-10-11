use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use ingestion::importers::{
    BatchSink, CamaraCeapImporter, CnaOabImporter, ImporterManager, ImporterSummary, PncpImporter,
    QueridoDiarioImporter, ReceitaFederalImporter, TseImporter,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use storage::DbPool;

static GLOBAL_IMPORTER_MANAGER: RwLock<Option<Arc<ImporterManager>>> = RwLock::new(None);

pub fn get_or_init_importer_manager(pool: &DbPool) -> Arc<ImporterManager> {
    if let Ok(guard) = GLOBAL_IMPORTER_MANAGER.read() {
        if let Some(ref m) = *guard {
            return m.clone();
        }
    }
    let mut guard = GLOBAL_IMPORTER_MANAGER
        .write()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(ref m) = *guard {
        return m.clone();
    }
    let sink = Arc::new(BatchSink::new(pool.clone()));
    let manager = ImporterManager::new(sink);
    if let Err(error) = manager.restore() {
        tracing::error!("Falha ao restaurar importadores: {error}");
    }
    manager.register(Arc::new(TseImporter::new_with_id(
        2024,
        "tse",
        "Tribunal Superior Eleitoral (2024 - Municipais)",
    )));
    manager.register(Arc::new(TseImporter::new_with_id(
        2022,
        "tse-2022",
        "Tribunal Superior Eleitoral (2022 - Presidência e Governos)",
    )));
    manager.register(Arc::new(ReceitaFederalImporter::new()));
    manager.register(Arc::new(CamaraCeapImporter::new(2024)));
    manager.register(Arc::new(PncpImporter::new(2024)));
    manager.register(Arc::new(QueridoDiarioImporter::new("", "")));
    manager.register(Arc::new(CnaOabImporter::new()));
    let arc = Arc::new(manager);
    *guard = Some(arc.clone());
    arc
}

pub fn set_importer_manager(manager: Arc<ImporterManager>) {
    let mut guard = GLOBAL_IMPORTER_MANAGER
        .write()
        .unwrap_or_else(|e| e.into_inner());
    *guard = Some(manager);
}

#[derive(Serialize, Deserialize)]
pub struct ApiMessageResponse {
    pub status: String,
    pub importer_id: String,
    pub mensagem: String,
}

#[derive(Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub erro: String,
}

pub async fn listar_importers_handler(State(pool): State<DbPool>) -> Json<Vec<ImporterSummary>> {
    let manager = get_or_init_importer_manager(&pool);
    Json(manager.list())
}

pub async fn obter_status_handler(
    State(pool): State<DbPool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let manager = get_or_init_importer_manager(&pool);
    match manager.get_status(&id) {
        Some(status) => (StatusCode::OK, Json(serde_json::to_value(status).unwrap())),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "erro": format!("Importador '{}' não encontrado", id)
            })),
        ),
    }
}

pub async fn iniciar_importer_handler(
    State(pool): State<DbPool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let manager = get_or_init_importer_manager(&pool);
    match manager.start(&id) {
        Ok(_) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({
                "status": "INICIADO",
                "importer_id": id,
                "mensagem": "Processo de importação iniciado com sucesso em background"
            })),
        ),
        Err(e) => {
            let erro_str = e.to_string();
            let status_code = if erro_str.contains("não encontrado") {
                StatusCode::NOT_FOUND
            } else if erro_str.contains("já está em execução") {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_REQUEST
            };
            (
                status_code,
                Json(serde_json::json!({
                    "erro": erro_str
                })),
            )
        }
    }
}

pub async fn cancelar_importer_handler(
    State(pool): State<DbPool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let manager = get_or_init_importer_manager(&pool);
    match manager.cancel(&id) {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "CANCELADO",
                "importer_id": id,
                "mensagem": "Sinal de cancelamento enviado com sucesso"
            })),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "erro": e.to_string()
            })),
        ),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use async_trait::async_trait;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use ingestion::importers::{ImportContext, SourceImporter};
    use storage::run_migrations;
    use tower::ServiceExt;

    struct DummyImporter {
        id: String,
        name: String,
    }

    #[async_trait]
    impl SourceImporter for DummyImporter {
        fn id(&self) -> &str {
            &self.id
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Importador dummy para testes"
        }

        async fn run(
            &self,
            ctx: Arc<ImportContext>,
            _sink: Arc<BatchSink>,
        ) -> ingestion::error::Result<()> {
            ctx.set_stage(
                ingestion::importers::ImportStage::Processando,
                "Rodando dummy",
            );
            for _ in 0..10 {
                if ctx.is_cancelled() {
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_api_importers_rotas_crud_e_ciclo_de_vida() {
        let pool = DbPool::open_in_memory().expect("pool");
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }

        let sink = Arc::new(BatchSink::new(pool.clone()));
        let manager = Arc::new(ImporterManager::new(sink));
        manager.register(Arc::new(DummyImporter {
            id: "teste_dummy".to_string(),
            name: "Teste Dummy".to_string(),
        }));
        set_importer_manager(manager);

        let app = crate::criar_router(pool.clone());

        // 1. GET /api/importers
        let req = Request::builder()
            .uri("/api/importers")
            .method("GET")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let lista: Vec<ImporterSummary> = serde_json::from_slice(&body).unwrap();
        assert!(!lista.is_empty());
        assert!(lista.iter().any(|i| i.id == "teste_dummy"));

        // 2. GET /api/importers/teste_dummy/status
        let req = Request::builder()
            .uri("/api/importers/teste_dummy/status")
            .method("GET")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // 3. POST /api/importers/teste_dummy/start
        let req = Request::builder()
            .uri("/api/importers/teste_dummy/start")
            .method("POST")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::ACCEPTED);

        // 4. POST /api/importers/teste_dummy/cancel
        let req = Request::builder()
            .uri("/api/importers/teste_dummy/cancel")
            .method("POST")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
