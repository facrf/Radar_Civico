use super::*;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn mock(
    fail_february: bool,
) -> (
    PncpClient,
    Arc<Mutex<Vec<String>>>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let requests = seen.clone();
    let task = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = [0; 4096];
            let n = stream.read(&mut buffer).await.unwrap();
            let raw = String::from_utf8_lossy(&buffer[..n]);
            let target = raw.split_whitespace().nth(1).unwrap();
            requests.lock().unwrap().push(target.to_string());
            let parsed = reqwest::Url::parse(&format!("http://localhost{target}")).unwrap();
            let query = parsed
                .query_pairs()
                .collect::<std::collections::HashMap<_, _>>();
            let start = query.get("dataInicial").unwrap();
            let page = query.get("pagina").unwrap();
            assert!(!query.contains_key("niFornecedor"));
            let failure = fail_february && start.as_ref() == "20240201" && page.as_ref() == "2";
            let (data, pages) = if start.as_ref() == "20240101"
                || (fail_february && start.as_ref() == "20240201")
            {
                let value = if page.as_ref() == "1" { 100 } else { 90 };
                (
                    serde_json::json!([{"orgaoEntidade":{"razaoSocial":format!("ORGAO {start}")},"niFornecedor":"11222333000181","valorGlobal":value,"dataAssinatura":format!("{}-{}-01",&start[..4],&start[4..6])}]),
                    2,
                )
            } else if start.as_ref() == "20241201" {
                (
                    serde_json::json!([{"orgaoEntidade":{"razaoSocial":"DEZEMBRO"},"niFornecedor":"11222333000181","valorGlobal":50,"dataAssinatura":"2024-12-01"}]),
                    1,
                )
            } else {
                (serde_json::json!([]), 0)
            };
            let body = serde_json::json!({"data":data,"totalPaginas":pages}).to_string();
            let status = if failure { "400 Bad Request" } else { "200 OK" };
            stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        }
    });
    (PncpClient::with_base_url(url), seen, task)
}
async fn database() -> storage::DbPool {
    let p = storage::DbPool::open_in_memory().unwrap();
    storage::run_migrations(&mut p.get().unwrap()).unwrap();
    p
}
#[tokio::test]
async fn coleta_todas_paginas_e_meses_incluindo_dezembro_e_ano_bissexto() {
    let (client, seen, task) = mock(false).await;
    let pool = database().await;
    let result = client
        .sincronizar_ano(&pool, 2024, None, &CancellationToken::new(), |_| {
            std::future::ready(())
        })
        .await
        .unwrap();
    assert_eq!(result.status(), "CONCLUIDO");
    assert_eq!(result.periodos_concluidos, 12);
    assert_eq!(result.registros_gravados, 3);
    assert!(seen
        .lock()
        .unwrap()
        .iter()
        .any(|s| s.contains("dataFinal=20240229")));
    assert!(seen
        .lock()
        .unwrap()
        .iter()
        .any(|s| s.contains("dataInicial=20241201")));
    task.abort();
}
#[tokio::test]
async fn falha_pagina_preserva_dados_e_cancelamento_para_novas_requisicoes() {
    let (client, seen, task) = mock(true).await;
    let pool = database().await;
    let result = client
        .sincronizar_ano(&pool, 2024, None, &CancellationToken::new(), |_| {
            std::future::ready(())
        })
        .await
        .unwrap();
    assert_eq!(result.status(), "PARCIAL");
    assert_eq!(result.periodos_concluidos, 11);
    assert_eq!(result.registros_gravados, 4);
    assert!(result.falhas[0].contains("2024-02 página 2"));
    let before = seen.lock().unwrap().len();
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let result = client
        .sincronizar_ano(&pool, 2024, None, &cancel, move |_| {
            trigger.cancel();
            std::future::ready(())
        })
        .await
        .unwrap();
    assert_eq!(result.status(), "CANCELADO");
    assert_eq!(seen.lock().unwrap().len(), before + 1);
    task.abort();
}
