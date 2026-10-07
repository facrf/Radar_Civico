use std::io::Write;
use std::sync::Arc;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use ingestion::importers::{
    BatchSink, CnaOabImporter, ImporterManager, ImporterSummary, PncpImporter,
    QueridoDiarioImporter, ReceitaFederalImporter, TseImporter,
};
use server::api::importers::set_importer_manager;
use server::criar_router;
use storage::{run_migrations, DbPool};
use tower::ServiceExt;

#[tokio::test]
async fn test_pipeline_unificado_de_ingestao_em_lote_e_api_status() {
    // 1. Inicializa SQLite em memória com PRAGMAs e migrações
    let pool = DbPool::open_in_memory().expect("falha ao instanciar sqlite em memoria");
    {
        let mut conn = pool.get().expect("falha conexao pool");
        run_migrations(&mut conn).expect("falha ao rodar migracoes");
    }

    // 2. Prepara dados mockados para o TSE (ZIP com consulta_cand em ISO-8859-1)
    let csv_tse = "\
ANO_ELEICAO;SG_UF;DS_CARGO;SQ_CANDIDATO;NR_CANDIDATO;NM_CANDIDATO;NM_URNA_CANDIDATO;NR_CPF_CANDIDATO;SG_PARTIDO;NM_MUNICIPIO;DS_SIT_TOT_TURNO;DS_OCUPACAO;DS_GRAU_INSTRUCAO;DT_NASCIMENTO\n\
2024;SP;PREFEITO;900001;45;FERNANDO SANTOS;FERNANDO;11122233344;PSDB;SANTOS;ELEITO;ENGENHEIRO;SUPERIOR COMPLETO;15/07/1978\n";
    let (bytes_latin1, _, _) = encoding_rs::WINDOWS_1252.encode(csv_tse);

    let mut zip_buffer = Vec::new();
    {
        let mut zip_writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buffer));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip_writer
            .start_file("consulta_cand_2024_BRASIL.csv", options)
            .unwrap();
        zip_writer.write_all(&bytes_latin1).unwrap();
        zip_writer.finish().unwrap();
    }

    // 3. Prepara dados mockados para Receita Federal (QSA)
    let csv_qsa = "\
CNPJ_BASICO;CNPJ_ORDEM;CNPJ_DV;RAZAO_SOCIAL;SOCIO_CPF_CNPJ_MASCARADO;SOCIO_NOME;QUALIFICACAO_SOCIO\n\
98765432;0001;10;SERVICOS TECNICOS S/A;***.111.222-**;FERNANDO SANTOS;SOCIO-DIRETOR\n";

    // 4. Instancia BatchSink e ImporterManager
    let sink = Arc::new(BatchSink::new(pool.clone()));
    let manager = Arc::new(ImporterManager::new(sink));

    manager.register(Arc::new(TseImporter::with_mock_data(2024, zip_buffer)));
    manager.register(Arc::new(ReceitaFederalImporter::with_mock_csv(csv_qsa)));
    manager.register(Arc::new(PncpImporter::new(2024)));
    manager.register(Arc::new(QueridoDiarioImporter::default()));
    manager.register(Arc::new(CnaOabImporter::default()));

    set_importer_manager(manager);

    let app = criar_router(pool.clone());

    // 5. Consulta inicial via API REST: GET /api/importers
    let req = Request::builder()
        .uri("/api/importers")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let lista_importers: Vec<ImporterSummary> = serde_json::from_slice(&body).unwrap();
    assert_eq!(lista_importers.len(), 5);

    // 6. Dispara ingestão concorrente em background via API REST
    // Inicia TSE
    let req_tse = Request::builder()
        .uri("/api/importers/tse/start")
        .method("POST")
        .body(Body::empty())
        .unwrap();
    let res_tse = app.clone().oneshot(req_tse).await.unwrap();
    assert_eq!(res_tse.status(), StatusCode::ACCEPTED);

    // Inicia Receita QSA
    let req_qsa = Request::builder()
        .uri("/api/importers/receita_qsa/start")
        .method("POST")
        .body(Body::empty())
        .unwrap();
    let res_qsa = app.clone().oneshot(req_qsa).await.unwrap();
    assert_eq!(res_qsa.status(), StatusCode::ACCEPTED);

    // Inicia PNCP
    let req_pncp = Request::builder()
        .uri("/api/importers/pncp/start")
        .method("POST")
        .body(Body::empty())
        .unwrap();
    let res_pncp = app.clone().oneshot(req_pncp).await.unwrap();
    assert_eq!(res_pncp.status(), StatusCode::ACCEPTED);

    // 7. Validação de isolamento das worker threads e responsividade da API
    // Enquanto as tarefas pesadas rodam em background (spawn_blocking), o servidor atende requisições
    for _ in 0..5 {
        let req_health = Request::builder()
            .uri("/health")
            .method("GET")
            .body(Body::empty())
            .unwrap();
        let res_health = app.clone().oneshot(req_health).await.unwrap();
        assert_eq!(res_health.status(), StatusCode::OK);

        let req_status = Request::builder()
            .uri("/api/importers/tse/status")
            .method("GET")
            .body(Body::empty())
            .unwrap();
        let res_status = app.clone().oneshot(req_status).await.unwrap();
        assert_eq!(res_status.status(), StatusCode::OK);

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // 8. Aguarda processamento das tarefas assíncronas com polling resiliente
    let mut lista_final: Vec<ImporterSummary> = Vec::new();
    for _ in 0..60 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let req_list = Request::builder()
            .uri("/api/importers")
            .method("GET")
            .body(Body::empty())
            .unwrap();
        let res_list = app.clone().oneshot(req_list).await.unwrap();
        assert_eq!(res_list.status(), StatusCode::OK);
        let body_list = to_bytes(res_list.into_body(), usize::MAX).await.unwrap();
        lista_final = serde_json::from_slice(&body_list).unwrap();

        let todos_concluidos = ["tse", "receita_qsa", "pncp"].iter().all(|id| {
            lista_final
                .iter()
                .find(|i| &i.id == id)
                .map(|i| i.stage == ingestion::importers::ImportStage::Concluido)
                .unwrap_or(false)
        });
        if todos_concluidos {
            break;
        }
    }

    let tse_status = lista_final.iter().find(|i| i.id == "tse").unwrap();
    assert_eq!(tse_status.stage, ingestion::importers::ImportStage::Concluido);
    assert_eq!(tse_status.records_processed, 1);

    let qsa_status = lista_final.iter().find(|i| i.id == "receita_qsa").unwrap();
    assert_eq!(qsa_status.stage, ingestion::importers::ImportStage::Concluido);
    assert_eq!(qsa_status.records_processed, 1);

    let pncp_status = lista_final.iter().find(|i| i.id == "pncp").unwrap();
    assert_eq!(pncp_status.stage, ingestion::importers::ImportStage::Concluido);
    assert_eq!(pncp_status.records_processed, 1);

    // 10. Validação da integridade dos dados inseridos no banco SQLite
    let conn = pool.get().expect("conexao para validacao final");

    let total_politicos: i64 = conn
        .query_row("SELECT COUNT(*) FROM politicos WHERE nome_completo = 'FERNANDO SANTOS'", [], |r| r.get(0))
        .expect("query politicos");
    assert_eq!(total_politicos, 1);

    let total_qsa: i64 = conn
        .query_row("SELECT COUNT(*) FROM empresas_qsa WHERE cnpj_basico = '98765432'", [], |r| r.get(0))
        .expect("query empresas_qsa");
    assert_eq!(total_qsa, 1);

    let total_contratos: i64 = conn
        .query_row("SELECT COUNT(*) FROM contratos_publicos", [], |r| r.get(0))
        .expect("query contratos_publicos");
    assert_eq!(total_contratos, 1);
}
