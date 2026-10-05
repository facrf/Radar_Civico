pub mod alertas;
pub mod busca;
pub mod config;
pub mod grafo;
pub mod investigar;
pub mod politico;
pub mod routes;

pub use alertas::{
    alertas_handler, carregar_alertas, registrar_alerta, sincronizar_alertas_sistema, AlertaItem,
    AlertasQueryParams, AlertasResponse, NovoAlerta,
};
pub use busca::{busca_handler, BuscaParams, ItemBuscaUnificada, RespostaBusca};
pub use config::{
    executar_ingestao_handler, exportar_banco_handler, exportar_tabela_handler, job_status_handler,
    obter_favicon_handler, obter_icone_handler, obter_identidade_handler, remover_icone_handler,
    salvar_icone_handler, status_handler, upload_arquivo_handler, ConfigStatusResponse,
    ExecutarIngestaoRequest, ExecutarIngestaoResponse, IdentidadeVisualResponse, JobInfo,
    SalvarIconeResponse, TotalRegistros, UploadResponse,
};
pub use grafo::{
    formatar_subgrafo, grafo_subgrafo_handler, CytoscapeEdge, CytoscapeEdgeData,
    CytoscapeElements, CytoscapeNode, CytoscapeNodeData, EchartsCategory, EchartsGraph,
    EchartsLink, EchartsNode, GrafoParams, SubgrafoResponse,
};
pub use investigar::{
    executar_investigacao, identificar_doador, investigar_nomeacao_handler, DoadorIdentificado,
    InvestigacaoNomeacaoResponse, InvestigarParams,
};
pub use politico::{
    carregar_dossie, politico_dossie_handler, BemItem, CandidaturaItem, DoadorItem, DossiePolitico,
};

use std::path::Path;
use axum::routing::{get, post};
use axum::Router;
use storage::DbPool;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

pub fn criar_router(pool: DbPool) -> Router {
    let mut router = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/api/v1/busca", get(busca_handler))
        .route("/api/v1/politico/:id", get(politico_dossie_handler))
        .route("/api/v1/grafo/:id", get(grafo_subgrafo_handler))
        .route("/api/v1/auditoria/alertas", get(alertas_handler))
        .route(
            "/api/v1/investigar/nomeacao/:doador_id",
            get(investigar_nomeacao_handler),
        )
        .route("/api/v1/config/status", get(status_handler))
        .route("/api/v1/config/ingestao/executar", post(executar_ingestao_handler))
        .route("/api/v1/config/ingestao/status/:job_id", get(job_status_handler))
        .route("/api/v1/config/jobs/:job_id", get(job_status_handler))
        .route("/api/v1/config/tse/verificar/:ano", get(config::verificar_tse_ano_handler))
        .route("/api/v1/config/tse/sincronizar", post(config::sincronizar_tse_handler))
        .route("/api/v1/config/ingestao/upload", post(upload_arquivo_handler))
        .route("/api/v1/config/exportar/banco", get(exportar_banco_handler))
        .route("/api/v1/config/exportar/tabela/:nome_tabela", get(exportar_tabela_handler))
        .route(
            "/api/v1/config/icone",
            get(obter_icone_handler)
                .post(salvar_icone_handler)
                .delete(remover_icone_handler),
        )
        .route("/api/v1/config/favicon", get(obter_favicon_handler))
        .route("/api/v1/config/identidade", get(obter_identidade_handler));

    let web_dir = std::env::var("WEB_DIR").unwrap_or_else(|_| "web/build".to_string());
    if Path::new(&web_dir).exists() {
        tracing::info!("Servindo arquivos estáticos da interface web a partir de: {}", web_dir);
        let index_file = format!("{}/index.html", web_dir);
        let serve_dir = ServeDir::new(&web_dir).fallback(ServeFile::new(index_file));
        router = router.fallback_service(serve_dir);
    } else {
        tracing::warn!("Diretório web '{}' não encontrado. Servidor executando apenas API REST.", web_dir);
    }

    router
        .layer(CorsLayer::permissive())
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(pool)
}
