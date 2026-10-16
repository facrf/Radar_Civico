use super::config::sqlite_http_error;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::Deserialize;
use serde_json::json;
use storage::DbPool;
pub async fn listar_arquivos_ingeridos_handler(
    State(pool): State<DbPool>,
) -> Result<Json<Vec<storage::RegistroArquivoIngerido>>, (StatusCode, Json<serde_json::Value>)> {
    let lista = pool
        .run_blocking(|conn| storage::listar_arquivos_ingeridos(conn, Some(100)))
        .await
        .map_err(sqlite_http_error)?;
    Ok(Json(lista))
}

#[derive(Debug, Deserialize)]
pub struct VerificarHashParams {
    pub hash: String,
    pub fonte: Option<String>,
    pub nome: Option<String>,
}

pub async fn verificar_arquivo_hash_handler(
    State(pool): State<DbPool>,
    Query(params): Query<VerificarHashParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    pool.run_blocking(move |conn| {
    let fonte = params.fonte.as_deref().unwrap_or("UPLOAD_MANUAL");
    let existente=storage::buscar_arquivo_por_hash(conn,fonte,&params.hash)?;
    let retificacao=params.nome.as_deref().map(|nome|storage::verificar_retificacao(conn,fonte,nome,&params.hash)).transpose()?.flatten();
    Ok(Json(json!({"hash":params.hash,"fonte":fonte,"ja_ingerido":existente.is_some(),"arquivo":existente,"retificacao_de_hash":retificacao})))
    }).await.map_err(sqlite_http_error)
}
