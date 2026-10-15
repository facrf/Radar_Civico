//! Validação dos destinos de requisições HTTP, proteção de cabeçalhos e autenticação configurável.
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub fn public_address(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || a == 0
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 198 && (b == 18 || b == 19))
                || (a == 192 && b == 0 && c == 0))
        }
        std::net::IpAddr::V6(ip) => {
            let s = ip.segments();
            (0x2000..=0x3fff).contains(&s[0])
                && !(s[0] == 0x2001 && (s[1] <= 0x1ff || s[1] == 0xdb8))
                && !(s[0] == 0x2002)
                && !(s[0] == 0x3fff)
        }
    }
}

/// Valida DNS, fixa os endereços e restringe portas para portas HTTP/HTTPS padrão.
pub async fn public_client(url: &str) -> anyhow::Result<reqwest::Client> {
    let parsed = reqwest::Url::parse(url)?;
    anyhow::ensure!(
        matches!(parsed.scheme(), "https" | "http")
            && parsed.username().is_empty()
            && parsed.password().is_none(),
        "URL HTTP inválida"
    );
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("URL sem host"))?;
    let addresses: Vec<_> =
        tokio::net::lookup_host((host, parsed.port_or_known_default().unwrap_or(443)))
            .await?
            .collect();
    anyhow::ensure!(
        !addresses.is_empty() && addresses.iter().all(|a| public_address(a.ip())),
        "Destino interno ou reservado não permitido"
    );

    let port = parsed.port_or_known_default().unwrap_or(443);
    anyhow::ensure!(
        matches!(port, 80 | 443 | 8080 | 8443),
        "Porta {port} não permitida para requisições externas; utilize as portas padrão HTTP/HTTPS (80, 443, 8080, 8443)"
    );

    Ok(reqwest::Client::builder()
        .no_proxy()
        .resolve_to_addrs(host, &addresses)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .user_agent("RadarCivico/1.0")
        .build()?)
}

/// Controle de Autenticação Administrativa Configurável.
/// Quando nenhuma credencial (`RADAR_ADMIN_PASSWORD` ou `RADAR_API_KEY`) for configurada
/// no ambiente, o sistema permanece em modo livre para facilidade de uso local.
#[derive(Clone, Default)]
pub struct AdminAuth {
    secret: Option<String>,
    sessions: Arc<Mutex<HashMap<String, Instant>>>,
}

impl AdminAuth {
    pub fn from_env() -> Self {
        let secret = std::env::var("RADAR_ADMIN_PASSWORD")
            .ok()
            .or_else(|| std::env::var("RADAR_API_KEY").ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        Self {
            secret,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn new_with_secret(secret: Option<String>) -> Self {
        Self {
            secret: secret.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn is_active(&self) -> bool {
        self.secret.is_some()
    }

    pub fn is_authorized(&self, headers: &header::HeaderMap) -> bool {
        let Some(ref sec) = self.secret else {
            return true; // Se auth não estiver ativada, permite tudo
        };

        // 1. Header X-API-Key
        if let Some(key) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
            if key.trim() == sec {
                return true;
            }
        }

        // 2. Header Authorization (Bearer ou Basic)
        if let Some(auth_val) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
            if let Some(bearer) = auth_val.strip_prefix("Bearer ").or_else(|| auth_val.strip_prefix("bearer ")) {
                if bearer.trim() == sec {
                    return true;
                }
            } else if let Some(basic) = auth_val.strip_prefix("Basic ").or_else(|| auth_val.strip_prefix("basic ")) {
                if let Ok(decoded) = BASE64.decode(basic.trim()) {
                    if let Ok(cred_str) = String::from_utf8(decoded) {
                        if let Some((_user, pass)) = cred_str.split_once(':') {
                            if pass == sec {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        // 3. Cookie de sessão radar_session
        if let Some(cookie_hdr) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
            for c in cookie_hdr.split(';') {
                let part = c.trim();
                if let Some(token) = part.strip_prefix("radar_session=") {
                    if let Ok(mut sessions) = self.sessions.lock() {
                        sessions.retain(|_, expiry| *expiry > Instant::now());
                        if sessions.contains_key(token) {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }
}

/// Middleware que protege rotas de mutação (POST, PUT, DELETE) e exportação sensível
/// quando a autenticação estiver ativada via variável de ambiente.
pub async fn auth_guard(
    State(auth): State<AdminAuth>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if !auth.is_active() {
        return next.run(req).await;
    }

    let path = req.uri().path();
    let method = req.method();

    // Endpoints públicos que nunca exigem autenticação
    let eh_publico = path == "/health"
        || path == "/admin/login"
        || path.starts_with("/_app")
        || (!path.starts_with("/api") && method == Method::GET);

    if eh_publico {
        return next.run(req).await;
    }

    // Rotas de leitura pública na API de consulta
    let eh_leitura_publica = method == Method::GET
        && (path.starts_with("/api/politico")
            || path.starts_with("/api/v1/politico")
            || path.starts_with("/api/busca")
            || path.starts_with("/api/v1/busca")
            || path.starts_with("/api/dossie")
            || path.starts_with("/api/v1/dossie")
            || path.starts_with("/api/grafo")
            || path.starts_with("/api/v1/grafo")
            || path.starts_with("/api/auditoria/alertas")
            || path.starts_with("/api/v1/auditoria/alertas")
            || path.starts_with("/api/auditoria/auxilio-indevido")
            || path.starts_with("/api/v1/auditoria/auxilio-indevido")
            || path.starts_with("/api/versao")
            || path.starts_with("/api/v1/config/versao")
            || path.starts_with("/api/v1/config/status")
            || path.starts_with("/api/v1/config/identidade")
            || path.starts_with("/api/v1/config/icone")
            || path.starts_with("/api/v1/config/favicon")
            || path.starts_with("/api/import/status")
            || path.starts_with("/api/v1/import/status")
            || (path.starts_with("/api/importers") && !path.contains("/start") && !path.contains("/cancel"))
            || (path.starts_with("/api/v1/importers") && !path.contains("/start") && !path.contains("/cancel")));

    if eh_leitura_publica {
        return next.run(req).await;
    }

    if auth.is_authorized(req.headers()) {
        next.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "status": "erro",
                "mensagem": "Autenticação necessária. Forneça cabeçalho Authorization (Bearer/Basic) ou X-API-Key."
            })),
        )
            .into_response()
    }
}

/// Middleware que injeta cabeçalhos de segurança defensivos em todas as respostas HTTP.
pub async fn security_headers_middleware(req: Request<Body>, next: Next) -> Response {
    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("SAMEORIGIN"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn rejects_internal_and_reserved_addresses() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "::1",
            "::ffff:127.0.0.1",
            "fe80::1",
            "2001:db8::1",
        ] {
            assert!(!public_address(ip.parse().unwrap()), "{ip}");
        }
        assert!(public_address("8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn test_admin_auth_inativo_permite_tudo() {
        let auth = AdminAuth::new_with_secret(None);
        assert!(!auth.is_active());
        let headers = HeaderMap::new();
        assert!(auth.is_authorized(&headers));
    }

    #[test]
    fn test_admin_auth_ativo_valida_credenciais() {
        let auth = AdminAuth::new_with_secret(Some("segredo123".to_string()));
        assert!(auth.is_active());

        // Sem cabeçalhos -> não autorizado
        let empty = HeaderMap::new();
        assert!(!auth.is_authorized(&empty));

        // X-API-Key correto
        let mut with_api_key = HeaderMap::new();
        with_api_key.insert("x-api-key", "segredo123".parse().unwrap());
        assert!(auth.is_authorized(&with_api_key));

        // X-API-Key incorreto
        let mut with_wrong_key = HeaderMap::new();
        with_wrong_key.insert("x-api-key", "errado".parse().unwrap());
        assert!(!auth.is_authorized(&with_wrong_key));

        // Bearer token
        let mut with_bearer = HeaderMap::new();
        with_bearer.insert(header::AUTHORIZATION, "Bearer segredo123".parse().unwrap());
        assert!(auth.is_authorized(&with_bearer));

        // Basic auth
        let mut with_basic = HeaderMap::new();
        let encoded = BASE64.encode("admin:segredo123");
        with_basic.insert(
            header::AUTHORIZATION,
            format!("Basic {encoded}").parse().unwrap(),
        );
        assert!(auth.is_authorized(&with_basic));
    }
}
