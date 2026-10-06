use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use server::criar_router;
use storage::{run_migrations, DbPool};
use tower::ServiceExt;

#[tokio::test]
async fn test_pipeline_completo_configuracao_sincronizacao_e_exportacao() {
    // 1. Inicializa banco e migrações
    let pool = DbPool::open_in_memory().expect("falha ao abrir sqlite");
    {
        let mut conn = pool.get().expect("falha na conexao");
        run_migrations(&mut conn).expect("falha nas migracoes");

        conn.execute(
            "INSERT INTO politicos (sq_candidato, nome_completo, nome_urna) 
             VALUES ('SQ9999', 'ALICE SILVA OLIVEIRA', 'ALICE')",
            [],
        )
        .expect("falha ao inserir politico");
    }

    let app = criar_router(pool.clone());

    // 2. Consulta de status do banco
    let req_status = Request::builder()
        .uri("/api/v1/config/status")
        .body(Body::empty())
        .unwrap();

    let res_status = app.oneshot(req_status).await.unwrap();
    assert_eq!(res_status.status(), StatusCode::OK);

    let bytes_status = axum::body::to_bytes(res_status.into_body(), usize::MAX)
        .await
        .unwrap();
    let status_json: serde_json::Value = serde_json::from_slice(&bytes_status).unwrap();
    assert_eq!(status_json["total_registros"]["politicos"], 1);
    assert!(status_json["versao_sistema"].as_str().unwrap().starts_with("v0."));

    // 2.1 Consulta de versão dedicada
    let app_versao = criar_router(pool.clone());
    let req_versao = Request::builder()
        .uri("/api/v1/config/versao")
        .body(Body::empty())
        .unwrap();
    let res_versao = app_versao.oneshot(req_versao).await.unwrap();
    assert_eq!(res_versao.status(), StatusCode::OK);
    let bytes_versao = axum::body::to_bytes(res_versao.into_body(), usize::MAX)
        .await
        .unwrap();
    let versao_json: serde_json::Value = serde_json::from_slice(&bytes_versao).unwrap();
    assert!(versao_json["versao"].as_str().unwrap().starts_with("v0."));
    assert!(!versao_json["commit"].as_str().unwrap().is_empty());
    assert!(versao_json["count"].as_u64().unwrap() >= 1);

    // 3. Disparo de sincronização em background
    let app = criar_router(pool.clone());
    let req_sync = Request::builder()
        .method("POST")
        .uri("/api/v1/config/ingestao/executar")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "fonte": "TSE",
                "ano": 2024
            })
            .to_string(),
        ))
        .unwrap();

    let res_sync = app.oneshot(req_sync).await.unwrap();
    assert_eq!(res_sync.status(), StatusCode::ACCEPTED);

    let bytes_sync = axum::body::to_bytes(res_sync.into_body(), usize::MAX)
        .await
        .unwrap();
    let sync_json: serde_json::Value = serde_json::from_slice(&bytes_sync).unwrap();
    let job_id = sync_json["job_id"].as_str().unwrap();

    // 4. Polling do status do job
    let mut concluido = false;
    for _ in 0..10 {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        let app = criar_router(pool.clone());
        let req_job = Request::builder()
            .uri(format!("/api/v1/config/ingestao/status/{}", job_id))
            .body(Body::empty())
            .unwrap();

        let res_job = app.oneshot(req_job).await.unwrap();
        assert_eq!(res_job.status(), StatusCode::OK);

        let bytes_job = axum::body::to_bytes(res_job.into_body(), usize::MAX)
            .await
            .unwrap();
        let job_data: serde_json::Value = serde_json::from_slice(&bytes_job).unwrap();
        if job_data["status"] == "CONCLUIDO" {
            concluido = true;
            break;
        }
    }
    assert!(concluido, "Job de sincronização não concluiu a tempo");

    // 5. Exportação do banco completo
    let app = criar_router(pool.clone());
    let req_export_banco = Request::builder()
        .uri("/api/v1/config/exportar/banco")
        .body(Body::empty())
        .unwrap();

    let res_export_banco = app.oneshot(req_export_banco).await.unwrap();
    assert_eq!(res_export_banco.status(), StatusCode::OK);
    let bkp_bytes = axum::body::to_bytes(res_export_banco.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(bkp_bytes.starts_with(b"SQLite format 3\0"));

    // 6. Exportação de tabela em CSV
    let app = criar_router(pool.clone());
    let req_export_tabela = Request::builder()
        .uri("/api/v1/config/exportar/tabela/politicos?formato=csv")
        .body(Body::empty())
        .unwrap();

    let res_export_tabela = app.oneshot(req_export_tabela).await.unwrap();
    assert_eq!(res_export_tabela.status(), StatusCode::OK);
    let csv_bytes = axum::body::to_bytes(res_export_tabela.into_body(), usize::MAX)
        .await
        .unwrap();
    let csv_text = String::from_utf8(csv_bytes.to_vec()).unwrap();
    assert!(csv_text.contains("ALICE SILVA OLIVEIRA"));
}
