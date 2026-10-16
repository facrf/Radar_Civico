use axum::{
    body::Body,
    extract::{Multipart, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use storage::{rusqlite, DbPool};
pub const DEFAULT_ICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" width="32" height="32">
  <rect width="32" height="32" rx="8" fill="#0f172a"/>
  <path d="M16 4L6 8v7c0 6.6 4.3 12.8 10 14 5.7-1.2 10-7.4 10-14V8l-10-4z" fill="none" stroke="#10b981" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
  <circle cx="16" cy="15" r="4" fill="none" stroke="#10b981" stroke-width="1.8"/>
  <circle cx="16" cy="15" r="1.5" fill="#10b981"/>
  <line x1="16" y1="15" x2="20" y2="12" stroke="#10b981" stroke-width="1.8" stroke-linecap="round"/>
</svg>"##;

#[derive(Debug, Deserialize)]
pub struct ObterIconeParams {
    pub tipo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RemoverIconeParams {
    pub alvo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalvarIconeResponse {
    pub status: String,
    pub mensagem: String,
    pub alvo: String,
    pub mime_type: String,
    pub tamanho_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentidadeVisualResponse {
    pub tem_icone_customizado: bool,
    pub tem_favicon_customizado: bool,
    pub icone_url: String,
    pub favicon_url: String,
}

pub async fn obter_icone_handler(
    State(pool): State<DbPool>,
    Query(params): Query<ObterIconeParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    tokio::task::spawn_blocking(move || {
        let tipo = params
            .tipo
            .unwrap_or_else(|| "app".to_string())
            .to_lowercase();
        let chave = if tipo == "favicon" {
            "favicon"
        } else {
            "app_icon"
        };

        if let Ok(conn) = pool.get() {
            let res: std::result::Result<(Vec<u8>, String), _> = conn.query_row(
                "SELECT valor_blob, mime_type FROM configuracoes_sistema WHERE chave = ?1",
                [chave],
                |r| Ok((r.get(0)?, r.get(1)?)),
            );

            if let Ok((blob, mime)) = res {
                if !blob.is_empty() {
                    let mut headers = HeaderMap::new();
                    if let Ok(mime_val) = HeaderValue::from_str(&mime) {
                        headers.insert(header::CONTENT_TYPE, mime_val);
                    } else {
                        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
                    }
                    headers.insert(
                        header::X_CONTENT_TYPE_OPTIONS,
                        HeaderValue::from_static("nosniff"),
                    );
                    if mime == "image/svg+xml" {
                        headers.insert(
                            header::CONTENT_SECURITY_POLICY,
                            HeaderValue::from_static(
                                "default-src 'none'; style-src 'unsafe-inline'",
                            ),
                        );
                    }
                    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
                    return Ok((headers, Body::from(blob)));
                }
            }

            if chave == "favicon" {
                let res_app: std::result::Result<(Vec<u8>, String), _> = conn.query_row(
                "SELECT valor_blob, mime_type FROM configuracoes_sistema WHERE chave = 'app_icon'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            );
                if let Ok((blob, mime)) = res_app {
                    if !blob.is_empty() {
                        let mut headers = HeaderMap::new();
                        if let Ok(mime_val) = HeaderValue::from_str(&mime) {
                            headers.insert(header::CONTENT_TYPE, mime_val);
                        } else {
                            headers.insert(
                                header::CONTENT_TYPE,
                                HeaderValue::from_static("image/png"),
                            );
                        }
                        headers.insert(
                            header::X_CONTENT_TYPE_OPTIONS,
                            HeaderValue::from_static("nosniff"),
                        );
                        if mime == "image/svg+xml" {
                            headers.insert(
                                header::CONTENT_SECURITY_POLICY,
                                HeaderValue::from_static(
                                    "default-src 'none'; style-src 'unsafe-inline'",
                                ),
                            );
                        }
                        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
                        return Ok((headers, Body::from(blob)));
                    }
                }
            }
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("image/svg+xml"),
        );
        headers.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        headers.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'"),
        );
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        Ok((headers, Body::from(DEFAULT_ICON_SVG)))
    })
    .await
    .map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro":error.to_string()})),
        )
    })?
}

pub async fn obter_favicon_handler(
    State(pool): State<DbPool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    obter_icone_handler(
        State(pool),
        Query(ObterIconeParams {
            tipo: Some("favicon".to_string()),
        }),
    )
    .await
}

fn validar_arquivo_imagem(bytes: &[u8], mime_type: &str) -> Result<(), &'static str> {
    match mime_type {
        "image/png" => {
            if bytes.len() >= 8 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                Ok(())
            } else {
                Err("Assinatura de arquivo PNG inválida")
            }
        }
        "image/jpeg" => {
            if bytes.len() >= 3 && bytes.starts_with(&[0xff, 0xd8, 0xff]) {
                Ok(())
            } else {
                Err("Assinatura de arquivo JPEG inválida")
            }
        }
        "image/webp" => {
            if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
                Ok(())
            } else {
                Err("Assinatura de arquivo WebP inválida")
            }
        }
        "image/x-icon" | "image/vnd.microsoft.icon" => {
            if bytes.len() >= 4
                && (bytes.starts_with(&[0x00, 0x00, 0x01, 0x00])
                    || bytes.starts_with(&[0x00, 0x00, 0x02, 0x00]))
            {
                Ok(())
            } else {
                Err("Assinatura de arquivo ICO inválida")
            }
        }
        "image/svg+xml" => {
            let svg_str =
                std::str::from_utf8(bytes).map_err(|_| "Arquivo SVG não contém UTF-8 válido")?;
            let lower = svg_str.to_lowercase();
            if lower.contains("<script")
                || lower.contains("javascript:")
                || lower.contains("<foreignobject")
                || lower.contains("onload=")
                || lower.contains("onerror=")
                || lower.contains("onclick=")
                || lower.contains("onmouseover=")
            {
                return Err("Arquivo SVG contém scripts ou elementos executáveis não permitidos");
            }
            if !lower.contains("<svg") {
                return Err("Arquivo não é um SVG válido (tag <svg> ausente)");
            }
            Ok(())
        }
        _ => Err("Tipo MIME de imagem não permitido"),
    }
}

pub async fn salvar_icone_handler(
    State(pool): State<DbPool>,
    mut multipart: Multipart,
) -> Result<Json<SalvarIconeResponse>, (StatusCode, Json<serde_json::Value>)> {
    let mut alvo = "ambos".to_string();
    let mut bytes = Vec::new();
    let mut mime_type = "image/png".to_string();

    while let Ok(Some(mut field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "alvo" {
            if let Ok(b) = field.bytes().await {
                alvo = String::from_utf8_lossy(&b).trim().to_lowercase();
            }
        } else if name == "arquivo" || field.file_name().is_some() {
            if let Some(content_type) = field.content_type() {
                mime_type = content_type.to_string();
            } else if let Some(file_name) = field.file_name() {
                let fn_lower = file_name.to_lowercase();
                if fn_lower.ends_with(".svg") {
                    mime_type = "image/svg+xml".to_string();
                } else if fn_lower.ends_with(".ico") {
                    mime_type = "image/x-icon".to_string();
                } else if fn_lower.ends_with(".jpg") || fn_lower.ends_with(".jpeg") {
                    mime_type = "image/jpeg".to_string();
                } else if fn_lower.ends_with(".webp") {
                    mime_type = "image/webp".to_string();
                }
            }

            while let Ok(Some(chunk)) = field.chunk().await {
                bytes.extend_from_slice(&chunk);
                if bytes.len() > 5 * 1024 * 1024 {
                    return Err((
                        StatusCode::PAYLOAD_TOO_LARGE,
                        Json(json!({"erro": "Tamanho máximo da imagem excedido (limite 5 MB)"})),
                    ));
                }
            }
        }
    }

    if bytes.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"erro": "Nenhum arquivo de imagem enviado"})),
        ));
    }

    let alvo_lower = alvo.to_lowercase();
    if !["icone", "favicon", "ambos"].contains(&alvo_lower.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"erro": "Alvo inválido: use icone, favicon ou ambos"})),
        ));
    }
    alvo = alvo_lower;

    let mime_clean = match mime_type.to_lowercase().as_str() {
        "image/svg+xml" | "image/svg" => "image/svg+xml",
        "image/png" => "image/png",
        "image/jpeg" | "image/jpg" => "image/jpeg",
        "image/webp" => "image/webp",
        "image/x-icon" | "image/vnd.microsoft.icon" => "image/x-icon",
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(
                    json!({"erro": "Tipo de imagem não suportado. Formatos aceitos: PNG, JPEG, WebP, ICO, SVG."}),
                ),
            ));
        }
    };

    if let Err(msg) = validar_arquivo_imagem(&bytes, mime_clean) {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"erro": msg}))));
    }
    let mime_final = mime_clean.to_string();

    let conn = pool.get().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro": format!("Erro ao obter conexão do pool: {e}")})),
        )
    })?;

    if alvo == "icone" || alvo == "ambos" {
        conn.execute(
            "INSERT OR REPLACE INTO configuracoes_sistema (chave, valor_texto, valor_blob, mime_type, atualizado_em)
             VALUES ('app_icon', 'icone_custom', ?1, ?2, CURRENT_TIMESTAMP)",
            rusqlite::params![&bytes, &mime_final],
        ).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": format!("Falha ao salvar ícone no SQLite: {e}")})),
            )
        })?;
    }

    if alvo == "favicon" || alvo == "ambos" {
        conn.execute(
            "INSERT OR REPLACE INTO configuracoes_sistema (chave, valor_texto, valor_blob, mime_type, atualizado_em)
             VALUES ('favicon', 'favicon_custom', ?1, ?2, CURRENT_TIMESTAMP)",
            rusqlite::params![&bytes, &mime_final],
        ).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": format!("Falha ao salvar favicon no SQLite: {e}")})),
            )
        })?;
    }

    Ok(Json(SalvarIconeResponse {
        status: "CONCLUIDO".to_string(),
        mensagem: "Identidade visual atualizada com sucesso!".to_string(),
        alvo,
        mime_type: mime_final,
        tamanho_bytes: bytes.len(),
    }))
}

pub async fn remover_icone_handler(
    State(pool): State<DbPool>,
    Query(params): Query<RemoverIconeParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tokio::task::spawn_blocking(move || {
        let alvo = params
            .alvo
            .unwrap_or_else(|| "ambos".to_string())
            .to_lowercase();
        let conn = pool.get().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": format!("Erro de banco: {e}")})),
            )
        })?;

        if alvo == "icone" || alvo == "ambos" {
            let _ = conn.execute(
                "DELETE FROM configuracoes_sistema WHERE chave = 'app_icon'",
                [],
            );
        }
        if alvo == "favicon" || alvo == "ambos" {
            let _ = conn.execute(
                "DELETE FROM configuracoes_sistema WHERE chave = 'favicon'",
                [],
            );
        }

        Ok(Json(json!({
            "status": "CONCLUIDO",
            "mensagem": "Ícone restaurado para o padrão original"
        })))
    })
    .await
    .map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro":error.to_string()})),
        )
    })?
}

pub async fn obter_identidade_handler(
    State(pool): State<DbPool>,
) -> Result<Json<IdentidadeVisualResponse>, (StatusCode, Json<serde_json::Value>)> {
    tokio::task::spawn_blocking(move || {
        let mut tem_icone = false;
        let mut tem_fav = false;
        if let Ok(conn) = pool.get() {
            tem_icone = conn
                .query_row(
                    "SELECT 1 FROM configuracoes_sistema WHERE chave = 'app_icon'",
                    [],
                    |_| Ok(()),
                )
                .is_ok();
            tem_fav = conn
                .query_row(
                    "SELECT 1 FROM configuracoes_sistema WHERE chave = 'favicon'",
                    [],
                    |_| Ok(()),
                )
                .is_ok();
        }

        Ok(Json(IdentidadeVisualResponse {
            tem_icone_customizado: tem_icone,
            tem_favicon_customizado: tem_fav,
            icone_url: "/api/v1/config/icone".to_string(),
            favicon_url: "/api/v1/config/favicon".to_string(),
        }))
    })
    .await
    .map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro":error.to_string()})),
        )
    })?
}
