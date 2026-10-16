//! Coleta anual paginada, compartilhada pelos jobs individual e unificado.
use ingestion::pncp::RelatorioPncp;
use storage::DbPool;

pub async fn sincronizar(pool: &DbPool, ano: i32, job_id: &str) -> Result<RelatorioPncp, String> {
    let progress_pool = pool.clone();
    let progress_id = job_id.to_string();
    ingestion::PncpClient::new()
        .sincronizar_ano(
            pool,
            ano,
            None,
            &tokio_util::sync::CancellationToken::new(),
            move |p| {
                let pool = progress_pool.clone();
                let id = progress_id.clone();
                async move {
                    super::config::atualizar_job(
                        &pool,
                        &id,
                        (10 + p.mes * 6).min(85) as u8,
                        &format!(
                            "PNCP por publicação: mês {}/12, página {}, {} registros gravados",
                            p.mes, p.pagina, p.registros_gravados
                        ),
                    )
                    .await;
                }
            },
        )
        .await
        .map_err(|e| e.to_string())
}
