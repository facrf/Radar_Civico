use axum::body::Body;
use axum::http::{Request, StatusCode};
use ingestion::camara::{
    ingerir_ceap_bulk_em_lotes, processar_ceap_buffer_ou_zip, CamaraApiClient,
};
use serde_json::json;
use server::criar_router;
use std::io::Write;
use storage::{run_migrations, DbPool};
use tower::ServiceExt;

#[tokio::test]
async fn test_pipeline_completo_camara_ceap_dual_ingestion() {
    // 1. Inicializa banco SQLite em memória com migrações completas
    let pool = DbPool::open_in_memory().expect("falha ao inicializar pool sqlite");
    {
        let mut conn = pool.get().expect("falha ao obter conexao");
        run_migrations(&mut conn).expect("falha ao rodar migracoes");
    }

    // 2. Cria arquivo CSV mockado da CEAP anual com padrão oficial
    let mock_ceap_csv = "\
txNomeParlamentar;cpf;datEmissao;txtDescricao;txtFornecedor;txtCNPJCPF;vlrLiquido;txtNumero;urlDocumento;detalhes_litros\n\
DEPUTADO EDUARDO SILVA;12345678901;2024-04-10T00:00:00;COMBUSTÍVEIS E LUBRIFICANTES.;AUTO POSTO CENTRAL LTDA;01.234.567/0001-89;350,00;NF-5544;http://nf.fazenda.sp.gov.br/doc5544;65,0\n\
DEPUTADA BEATRIZ SOUZA;98765432100;2024-04-15T00:00:00;PASSAGEM AÉREA;GOL LINHAS AEREAS;07.575.651/0001-59;1.450,80;BILHETE-1002;http://gol.com/bilhete;;\n\
DEPUTADO EDUARDO SILVA;12345678901;2024-04-20T00:00:00;DIVULGAÇÃO DA ATIVIDADE PARLAMENTAR;AGÊNCIA DIGITAL LTDA;10.987.654/0001-32;4.200,00;NF-8899;http://nf.fazenda.sp.gov.br/doc8899;;\n";

    // Compacta no formato Ano-2024.csv.zip
    let mut zip_bytes = Vec::new();
    {
        let mut zip_writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_bytes));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip_writer
            .start_file("Ano-2024.csv", options)
            .expect("falha ao iniciar arquivo no zip");
        zip_writer
            .write_all(mock_ceap_csv.as_bytes())
            .expect("falha ao escrever csv no zip");
        zip_writer.finish().expect("falha ao finalizar zip");
    }

    // 3. Valida extração em streaming a partir do ZIP (Modo BULK)
    let records_bulk = processar_ceap_buffer_ou_zip(&zip_bytes)
        .await
        .expect("falha ao processar buffer zip");
    assert_eq!(records_bulk.len(), 3);

    // Persiste no SQLite
    {
        let mut conn = pool.get().expect("falha conn");
        let inseridos = ingerir_ceap_bulk_em_lotes(&mut conn, &records_bulk, 100)
            .expect("falha na insercao em lotes");
        assert_eq!(inseridos, 3);
    }

    // Verifica persistência na tabela despesas_parlamentares
    {
        let conn = pool.get().expect("falha conn");
        let total: i64 = conn
            .query_row("SELECT count(*) FROM despesas_parlamentares", [], |r| r.get(0))
            .expect("falha count");
        assert_eq!(total, 3);

        // Valida campos mascarados e limpos
        let (nome, cpf, doc_forn, litros): (String, Option<String>, String, Option<f64>) = conn
            .query_row(
                "SELECT parlamentar_nome, parlamentar_cpf_mascarado, fornecedor_cnpj_cpf, detalhes_litros 
                 FROM despesas_parlamentares WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .expect("falha select");

        assert_eq!(nome, "DEPUTADO EDUARDO SILVA");
        assert_eq!(cpf, Some("***.456.789-**".to_string()));
        assert_eq!(doc_forn, "01234567000189");
        assert_eq!(litros, Some(65.0));
    }

    // 4. Valida cliente da API REST v2 com navegação HATEOAS (Modo API)
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("falha ao abrir porta mock");
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        let mut request_index = 0;
        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                request_index += 1;
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut buf = [0u8; 2048];
                    let _ = socket.read(&mut buf).await;

                    let body = if request_index == 1 {
                        // Página 1 apontando para página 2 via links.next
                        json!({
                            "dados": [
                                {
                                    "ano": 2024,
                                    "tipoDespesa": "LOCAÇÃO OU FRETAMENTO DE VEÍCULOS AUTOMOTORES",
                                    "dataDocumento": "2024-04-18",
                                    "numDocumento": "LOC-1234",
                                    "nomeFornecedor": "LOCADORA DE CARROS BRASIL",
                                    "cnpjCpfFornecedor": "99.888.777/0001-66",
                                    "valorLiquido": 3200.00
                                }
                            ],
                            "links": [
                                {
                                    "rel": "next",
                                    "href": format!("http://127.0.0.1:{}/deputados/999/despesas?pagina=2", port)
                                }
                            ]
                        })
                    } else {
                        // Página 2 (última página)
                        json!({
                            "dados": [
                                {
                                    "ano": 2024,
                                    "tipoDespesa": "SERVIÇOS POSTAIS",
                                    "dataDocumento": "2024-04-22",
                                    "numDocumento": "ECT-55",
                                    "nomeFornecedor": "CORREIOS",
                                    "cnpjCpfFornecedor": "34.028.316/0001-03",
                                    "valorLiquido": 120.50
                                }
                            ],
                            "links": [
                                {
                                    "rel": "first",
                                    "href": format!("http://127.0.0.1:{}/deputados/999/despesas?pagina=1", port)
                                }
                            ]
                        })
                    };

                    let body_str = serde_json::to_string(&body).unwrap();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body_str.len(),
                        body_str
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        }
    });

    let api_client = CamaraApiClient::with_base_url(format!("http://127.0.0.1:{}", port));
    let despesas_api = api_client
        .buscar_despesas_deputado_hateoas(999, 2024, None)
        .await
        .expect("falha ao buscar despesas hateoas");

    assert_eq!(despesas_api.len(), 2);
    assert_eq!(despesas_api[0].valor_liquido, 3200.00);
    assert_eq!(despesas_api[1].valor_liquido, 120.50);

    // Persiste despesas da API
    {
        let mut conn = pool.get().expect("falha conn");
        let inseridos_api = CamaraApiClient::salvar_despesas_api(
            &mut conn,
            "DEPUTADO VIA API",
            Some("55566677788"),
            &despesas_api,
        )
        .expect("falha ao salvar despesas da api");
        assert_eq!(inseridos_api, 2);

        let total_geral: i64 = conn
            .query_row("SELECT count(*) FROM despesas_parlamentares", [], |r| r.get(0))
            .expect("falha count geral");
        assert_eq!(total_geral, 5);
    }

    // 5. Valida endpoints HTTP do servidor Axum
    let app = criar_router(pool.clone());

    // Disparo de sincronização da Câmara
    let req_sync = Request::builder()
        .method("POST")
        .uri("/api/v1/config/camara/sincronizar")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "ano": 2024,
                "modo": "BULK"
            })
            .to_string(),
        ))
        .unwrap();

    let res_sync = app.oneshot(req_sync).await.unwrap();
    assert_eq!(res_sync.status(), StatusCode::ACCEPTED);

    let bytes_sync = axum::body::to_bytes(res_sync.into_body(), usize::MAX)
        .await
        .unwrap();
    let sync_val: serde_json::Value = serde_json::from_slice(&bytes_sync).unwrap();
    let job_id = sync_val["job_id"].as_str().unwrap();

    // Consulta do job de sincronização
    let app = criar_router(pool.clone());
    let req_job = Request::builder()
        .uri(format!("/api/v1/config/ingestao/status/{}", job_id))
        .body(Body::empty())
        .unwrap();
    let res_job = app.oneshot(req_job).await.unwrap();
    assert_eq!(res_job.status(), StatusCode::OK);

    // Consulta de status do banco (verificando métricas das despesas parlamentares)
    let app = criar_router(pool.clone());
    let req_status = Request::builder()
        .uri("/api/v1/config/status")
        .body(Body::empty())
        .unwrap();
    let res_status = app.oneshot(req_status).await.unwrap();
    assert_eq!(res_status.status(), StatusCode::OK);
    let bytes_status = axum::body::to_bytes(res_status.into_body(), usize::MAX)
        .await
        .unwrap();
    let status_val: serde_json::Value = serde_json::from_slice(&bytes_status).unwrap();
    assert_eq!(status_val["total_registros"]["despesas_parlamentares"], 5);

    // Exportação em formato CSV
    let app = criar_router(pool.clone());
    let req_export = Request::builder()
        .uri("/api/v1/config/exportar/tabela/despesas_parlamentares?formato=csv")
        .body(Body::empty())
        .unwrap();
    let res_export = app.oneshot(req_export).await.unwrap();
    assert_eq!(res_export.status(), StatusCode::OK);
    let bytes_export = axum::body::to_bytes(res_export.into_body(), usize::MAX)
        .await
        .unwrap();
    let csv_export_str = String::from_utf8_lossy(&bytes_export);

    assert!(csv_export_str.contains("DEPUTADO EDUARDO SILVA"));
    assert!(csv_export_str.contains("DEPUTADO VIA API"));
    assert!(csv_export_str.contains("COMBUSTÍVEIS E LUBRIFICANTES."));
}
