pub mod alertas;
pub mod api;
pub mod busca;
pub mod config;
pub mod dossie;
pub mod geo;
pub mod grafo;
pub mod investigar;
pub mod politico;
pub mod politicos_duplicados;
pub mod routes;

pub use politicos_duplicados::*;
pub use geo::{calcular_distancia_km, coordenadas_capital_uf, resolver_coordenadas_despesa, CoordenadaResolvida};
pub use dossie::{
    dossie_cnpj_handler, dossie_cpf_handler, AlertaDossie, BeneficioEmergencialItem,
    CandidaturaItemCpf, CandidaturaSocioTse, CompradorCeapResumo, ContratoPncpItem,
    DiarioItem, DoacaoEleitoralItem, DoacaoSocioTse, DossieCnpjResponse, DossieCpfResponse,
    EmpresaResumo, EmpresaSocioItem, NotaFiscalCeapItem, OrgaoContratanteResumo, PainelCeap,
    PainelPncp, PainelSocietario, PainelTseSocios, RegistroProfissionalItem, SocioItem,
};

pub use alertas::{
    alertas_handler, auxilio_indevido_handler, carregar_alertas, registrar_alerta,
    sincronizar_alertas_handler, sincronizar_alertas_sistema, AlertaAuxilioResponseItem, AlertaItem,
    AlertasQueryParams, AlertasResponse, AuxilioIndevidoQueryParams, AuxilioIndevidoResponse,
    NovoAlerta,
};
pub use busca::{busca_handler, BuscaParams, ItemBuscaUnificada, RespostaBusca};
pub use config::{
    carregar_parametros_auditoria, criar_backup_handler, download_backup_arquivo_handler,
    executar_ingestao_handler, exportar_banco_handler, exportar_tabela_handler, import_status_handler,
    job_status_handler, listar_backups_handler, obter_audit_rules_handler, obter_favicon_handler,
    obter_icone_handler, obter_identidade_handler, remover_icone_handler, salvar_audit_rules_handler,
    salvar_icone_handler, salvar_parametros_auditoria, sincronizar_autoridades_handler, sincronizar_camara_handler,
    sincronizar_tse_handler, sincronizar_tudo_handler, status_handler, testar_webhook_handler, upload_arquivo_handler,
    verificar_tse_ano_handler, versao_handler, BackupItemInfo, ConfigStatusResponse,
    ExecutarIngestaoRequest, ExecutarIngestaoResponse, IdentidadeVisualResponse, ImportProgress,
    JobInfo, ParametrosAuditoria, SalvarAuditRulesResponse, SalvarIconeResponse,
    SincronizarCamaraRequest, SincronizarTseRequest, SincronizarTudoRequest, TestarWebhookRequest, TotalRegistros,
    UploadResponse, VersaoResponse,
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
    buscar_foto_tse_handler, carregar_dossie, listar_politicos_handler, obter_foto_handler,
    politico_despesas_geo_handler, politico_detalhe_handler, politico_dossie_handler,
    politico_evolucao_patrimonial_handler, remover_foto_handler, salvar_foto_manual_handler,
    sincronizar_fotos_camara_handler, AlertaAuxilioItem, BemItem, BuscarFotoResponse,
    CandidaturaItem, DespesaCeapResumoItem, DoadorItem, DossiePolitico, GastoCategoriaItem,
    ItemPoliticoListagem, ListarPoliticosQueryParams, ListarPoliticosResponse,
    PoliticoDespesasGeoResponse, PoliticoDetalheResponse, PontoDespesaGeo,
    PontoEvolucaoPatrimonial, ResumoFinanceiroPolitico, SalvarFotoManualRequest,
};

use std::path::Path;
use axum::extract::DefaultBodyLimit;
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
        .route("/api/busca", get(busca_handler))
        .route("/api/v1/busca", get(busca_handler))
        .route("/api/politicos", get(listar_politicos_handler))
        .route("/api/v1/politicos", get(listar_politicos_handler))
        .route("/api/politicos/duplicados", get(listar_politicos_duplicados_handler))
        .route("/api/v1/politicos/duplicados", get(listar_politicos_duplicados_handler))
        .route("/api/politicos/duplicados/resumo", get(resumo_politicos_duplicados_handler))
        .route("/api/v1/politicos/duplicados/resumo", get(resumo_politicos_duplicados_handler))
        .route("/api/politicos/duplicados/mesclar", post(mesclar_politicos_handler))
        .route("/api/v1/politicos/duplicados/mesclar", post(mesclar_politicos_handler))
        .route("/api/politicos/duplicados/mesclar-automatico", post(mesclar_automatico_handler))
        .route("/api/v1/politicos/duplicados/mesclar-automatico", post(mesclar_automatico_handler))
        .route("/api/politicos/duplicados/mesclar-automatico-job", post(mesclar_automatico_job_handler))
        .route("/api/v1/politicos/duplicados/mesclar-automatico-job", post(mesclar_automatico_job_handler))
        .route("/api/politicos/sincronizar-fotos-camara", post(sincronizar_fotos_camara_handler))
        .route("/api/v1/politicos/sincronizar-fotos-camara", post(sincronizar_fotos_camara_handler))
        .route("/api/politicos/:id", get(politico_detalhe_handler))
        .route("/api/v1/politicos/:id", get(politico_detalhe_handler))
        .route("/api/politicos/:id/buscar-foto-tse", post(buscar_foto_tse_handler))
        .route("/api/v1/politicos/:id/buscar-foto-tse", post(buscar_foto_tse_handler))
        .route(
            "/api/politicos/:id/foto",
            get(obter_foto_handler)
                .post(salvar_foto_manual_handler)
                .delete(remover_foto_handler),
        )
        .route(
            "/api/v1/politicos/:id/foto",
            get(obter_foto_handler)
                .post(salvar_foto_manual_handler)
                .delete(remover_foto_handler),
        )
        .route("/api/politicos/:id/despesas-geo", get(politico_despesas_geo_handler))
        .route("/api/v1/politicos/:id/despesas-geo", get(politico_despesas_geo_handler))
        .route("/api/politicos/:id/evolucao-patrimonial", get(politico_evolucao_patrimonial_handler))
        .route("/api/v1/politicos/:id/evolucao-patrimonial", get(politico_evolucao_patrimonial_handler))
        .route("/api/politico/:id", get(politico_detalhe_handler))
        .route("/api/v1/politico/:id", get(politico_detalhe_handler))
        .route("/api/dossie/cnpj/:cnpj", get(dossie::dossie_cnpj_handler))
        .route("/api/v1/dossie/cnpj/:cnpj", get(dossie::dossie_cnpj_handler))
        .route("/api/dossie/cpf/:cpf", get(dossie::dossie_cpf_handler))
        .route("/api/v1/dossie/cpf/:cpf", get(dossie::dossie_cpf_handler))
        .route("/api/grafo/:id", get(grafo_subgrafo_handler))
        .route("/api/v1/grafo/:id", get(grafo_subgrafo_handler))
        .route("/api/auditoria/alertas", get(alertas_handler))
        .route("/api/v1/auditoria/alertas", get(alertas_handler))
        .route("/api/auditoria/sincronizar", post(sincronizar_alertas_handler))
        .route("/api/v1/auditoria/sincronizar", post(sincronizar_alertas_handler))
        .route("/api/auditoria/auxilio-indevido", get(auxilio_indevido_handler))
        .route("/api/v1/auditoria/auxilio-indevido", get(auxilio_indevido_handler))
        .route(
            "/api/v1/investigar/nomeacao/:doador_id",
            get(investigar_nomeacao_handler),
        )
        .route(
            "/api/settings/audit-rules",
            get(obter_audit_rules_handler)
                .post(salvar_audit_rules_handler)
                .put(salvar_audit_rules_handler),
        )
        .route(
            "/api/v1/settings/audit-rules",
            get(obter_audit_rules_handler)
                .post(salvar_audit_rules_handler)
                .put(salvar_audit_rules_handler),
        )
        .route(
            "/api/v1/config/audit-rules",
            get(obter_audit_rules_handler)
                .post(salvar_audit_rules_handler)
                .put(salvar_audit_rules_handler),
        )
        .route("/api/v1/config/status", get(status_handler))
        .route("/api/v1/config/versao", get(versao_handler))
        .route("/api/versao", get(versao_handler))
        .route("/api/v1/config/ingestao/executar", post(executar_ingestao_handler))
        .route("/api/import/status", get(import_status_handler))
        .route("/api/v1/import/status", get(import_status_handler))
        .route("/api/v1/config/ingestao/status/:job_id", get(job_status_handler))
        .route("/api/v1/config/jobs/:job_id", get(job_status_handler))
        .route("/api/v1/config/tse/verificar/:ano", get(verificar_tse_ano_handler))
        .route("/api/v1/config/tse/sincronizar", post(sincronizar_tse_handler))
        .route("/api/v1/config/camara/sincronizar", post(config::sincronizar_camara_handler))
        .route("/api/config/sincronizar/autoridades", post(config::sincronizar_autoridades_handler))
        .route("/api/v1/config/sincronizar/autoridades", post(config::sincronizar_autoridades_handler))
        .route("/api/config/autoridades/sincronizar", post(config::sincronizar_autoridades_handler))
        .route("/api/v1/config/autoridades/sincronizar", post(config::sincronizar_autoridades_handler))
        .route("/api/config/sincronizar-tudo", post(config::sincronizar_tudo_handler))
        .route("/api/v1/config/sincronizar-tudo", post(config::sincronizar_tudo_handler))
        .route("/api/v1/config/ingestao/upload", post(upload_arquivo_handler))
        .route("/api/v1/config/exportar/banco", get(exportar_banco_handler))
        .route("/api/config/backup", post(criar_backup_handler))
        .route("/api/v1/config/backup", post(criar_backup_handler))
        .route("/api/config/backups", get(listar_backups_handler))
        .route("/api/v1/config/backups", get(listar_backups_handler))
        .route("/api/v1/config/backups/:nome_arquivo", get(download_backup_arquivo_handler))
        .route("/api/config/webhook/test", post(testar_webhook_handler))
        .route("/api/v1/config/webhook/test", post(testar_webhook_handler))
        .route("/api/settings/webhook/test", post(testar_webhook_handler))
        .route("/api/v1/settings/webhook/test", post(testar_webhook_handler))
        .route("/api/v1/config/exportar/tabela/:nome_tabela", get(exportar_tabela_handler))
        .route(
            "/api/v1/config/icone",
            get(obter_icone_handler)
                .post(salvar_icone_handler)
                .delete(remover_icone_handler),
        )
        .route("/api/v1/config/favicon", get(obter_favicon_handler))
        .route("/api/v1/config/identidade", get(obter_identidade_handler))
        .route("/api/importers", get(api::importers::listar_importers_handler))
        .route("/api/v1/importers", get(api::importers::listar_importers_handler))
        .route("/api/importers/:id/status", get(api::importers::obter_status_handler))
        .route("/api/v1/importers/:id/status", get(api::importers::obter_status_handler))
        .route("/api/importers/:id/start", post(api::importers::iniciar_importer_handler))
        .route("/api/v1/importers/:id/start", post(api::importers::iniciar_importer_handler))
        .route("/api/importers/:id/cancel", post(api::importers::cancelar_importer_handler))
        .route("/api/v1/importers/:id/cancel", post(api::importers::cancelar_importer_handler));

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
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024 * 1024))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(pool)
}
