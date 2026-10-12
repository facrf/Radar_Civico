//! Administração autenticada; consultas públicas continuam disponíveis.
use axum::{
    extract::{Form, Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub struct AdminAuth {
    password: String,
    sessions: Mutex<HashMap<String, Instant>>,
}
impl AdminAuth {
    pub fn new(password: String) -> Self {
        Self {
            password,
            sessions: Mutex::new(HashMap::new()),
        }
    }
    pub fn load(data_dir: &Path) -> anyhow::Result<Arc<Self>> {
        let path = data_dir.join("admin_password");
        let password = match std::env::var("RADAR_ADMIN_PASSWORD") {
            Ok(value) if value.len() >= 16 => value,
            Ok(_) => anyhow::bail!("RADAR_ADMIN_PASSWORD deve ter pelo menos 16 caracteres"),
            Err(_) => {
                std::fs::create_dir_all(data_dir)?;
                if !path.exists() {
                    use std::io::Write;
                    let mut options = std::fs::OpenOptions::new();
                    options.write(true).create_new(true);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::OpenOptionsExt;
                        options.mode(0o600);
                    }
                    let mut file = options.open(&path)?;
                    file.write_all(uuid::Uuid::new_v4().simple().to_string().as_bytes())?;
                }
                std::fs::read_to_string(&path)?.trim().to_string()
            }
        };
        anyhow::ensure!(
            password.len() >= 16,
            "Senha administrativa inválida em {}",
            path.display()
        );
        tracing::info!(
            "Administração protegida. Usuário: admin; senha em RADAR_ADMIN_PASSWORD ou {}",
            path.display()
        );
        Ok(Arc::new(Self::new(password)))
    }
    fn matches(&self, supplied: &str) -> bool {
        let a = self.password.as_bytes();
        let b = supplied.as_bytes();
        let mut difference = a.len() ^ b.len();
        for (i, c) in a.iter().enumerate() {
            difference |= usize::from(*c ^ b.get(i).copied().unwrap_or(0));
        }
        difference == 0
    }
}
fn protected(method: &axum::http::Method, path: &str) -> bool {
    path.starts_with("/api/")
        && (method != axum::http::Method::GET && method != axum::http::Method::HEAD
            || path.contains("/exportar/")
            || path.contains("/backups")
            || path.contains("/investigar/"))
}
pub async fn guard(State(auth): State<Arc<AdminAuth>>, req: Request, next: Next) -> Response {
    if !protected(req.method(), req.uri().path()) {
        return next.run(req).await;
    }
    let basic = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
        .and_then(|v| STANDARD.decode(v).ok())
        .and_then(|v| String::from_utf8(v).ok())
        .is_some_and(|v| v.strip_prefix("admin:").is_some_and(|p| auth.matches(p)));
    let token = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.split(';')
                .find_map(|c| c.trim().strip_prefix("radar_session="))
        });
    let cookie = token.is_some_and(|token| {
        auth.sessions.lock().ok().is_some_and(|mut sessions| {
            sessions.retain(|_, expiry| *expiry > Instant::now());
            sessions.contains_key(token)
        })
    });
    // Cookies só podem autorizar solicitações originadas nesta aplicação.
    let origin_ok = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| reqwest::Url::parse(v).ok())
        .is_some_and(|url| {
            req.headers()
                .get(header::HOST)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|host| {
                    format!(
                        "{}{}",
                        url.host_str().unwrap_or(""),
                        url.port().map(|p| format!(":{p}")).unwrap_or_default()
                    ) == host
                })
        });
    if basic
        || (cookie
            && (req.method() == axum::http::Method::GET
                || req.method() == axum::http::Method::HEAD
                || origin_ok))
    {
        return next.run(req).await;
    }
    (StatusCode::UNAUTHORIZED, axum::Json(serde_json::json!({"erro":"Autenticação administrativa necessária", "login":"/admin/login"}))).into_response()
}
const LOGIN: &str = r#"<!doctype html><html lang="pt-BR"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Administração — Radar Cívico</title><body style="background:#0f172a;color:white;font:18px system-ui;padding:3rem"><h1>Acesso administrativo</h1><form method="post"><label>Usuário <input name="username" value="admin" autocomplete="username" required></label><p><label>Senha <input name="password" type="password" autocomplete="current-password" required></label></p><button>Entrar</button></form><p><a href="/" style="color:#38bdf8">Voltar ao painel público</a></p></body></html>"#;
#[derive(serde::Deserialize)]
struct Credentials {
    username: String,
    password: String,
}
async fn login(State(auth): State<Arc<AdminAuth>>, Form(input): Form<Credentials>) -> Response {
    if input.username != "admin" || !auth.matches(&input.password) {
        tokio::time::sleep(Duration::from_millis(500)).await;
        return (
            StatusCode::UNAUTHORIZED,
            Html(format!("<p>Credenciais inválidas.</p>{LOGIN}")),
        )
            .into_response();
    }
    let token = uuid::Uuid::new_v4().simple().to_string();
    let Ok(mut sessions) = auth.sessions.lock() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    sessions.retain(|_, expiry| *expiry > Instant::now());
    if sessions.len() >= 100 {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    sessions.insert(token.clone(), Instant::now() + Duration::from_secs(28800));
    let mut response = Redirect::to("/configuracoes").into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        format!("radar_session={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=28800")
            .parse()
            .unwrap(),
    );
    response
}
pub fn secure_router(router: Router, auth: Arc<AdminAuth>) -> Router {
    router
        .layer(axum::middleware::from_fn_with_state(auth.clone(), guard))
        .merge(
            Router::new()
                .route("/admin/login", get(|| async { Html(LOGIN) }).post(login))
                .with_state(auth),
        )
}

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
/// Valida DNS e fixa os endereços; sem proxy ou redirects que contornem a validação.
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
    Ok(reqwest::Client::builder()
        .no_proxy()
        .resolve_to_addrs(host, &addresses)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .user_agent("RadarCivico/1.0")
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request},
    };
    use tower::ServiceExt;
    #[tokio::test]
    async fn protects_mutations_exports_and_allows_public_reads() {
        let password = "a-strong-test-password";
        let app = secure_router(
            Router::new()
                .route("/api/test", get(|| async { "ok" }).post(|| async { "ok" }))
                .route("/api/config/exportar/banco", get(|| async { "db" })),
            Arc::new(AdminAuth::new(password.into())),
        );
        for (method, path, status) in [
            (Method::GET, "/api/test", 200),
            (Method::POST, "/api/test", 401),
            (Method::GET, "/api/config/exportar/banco", 401),
        ] {
            let result = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(result.status().as_u16(), status);
        }
        let result = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/test")
                    .header(
                        header::AUTHORIZATION,
                        format!("Basic {}", STANDARD.encode(format!("admin:{password}"))),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(result.status(), StatusCode::OK);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/login")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(format!("username=admin&password={password}")))
                    .unwrap(),
            )
            .await
            .unwrap();
        let cookie = response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/test")
                    .header(header::COOKIE, cookie)
                    .header(header::HOST, "localhost:8080")
                    .header(header::ORIGIN, "https://evil.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
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
}
