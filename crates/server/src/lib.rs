pub mod alertas;
pub mod busca;
pub mod grafo;
pub mod investigar;
pub mod politico;

pub use alertas::{
    alertas_handler, carregar_alertas, registrar_alerta, sincronizar_alertas_sistema, AlertaItem,
    AlertasQueryParams, AlertasResponse, NovoAlerta,
};
pub use busca::{busca_handler, BuscaParams, ItemBuscaUnificada, RespostaBusca};
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
use axum::routing::get;
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
        );

    let web_dir = std::env::var("WEB_DIR").unwrap_or_else(|_| "web/build".to_string());
    if Path::new(&web_dir).exists() {
        let index_file = format!("{}/index.html", web_dir);
        let serve_dir = ServeDir::new(&web_dir).fallback(ServeFile::new(index_file));
        router = router.fallback_service(serve_dir);
    }

    router
        .layer(CorsLayer::permissive())
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(pool)
}
