pub mod prompts;
pub mod tools;

use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use prompts::PromptRegistry;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResponse, GetPromptRequestParams, GetPromptResponse,
        Implementation, InitializeRequestParams, InitializeResult, ListPromptsResult,
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
    },
    service::{serve_directly, RequestContext},
    ErrorData as McpError, RoleServer, ServerHandler,
};
use serde_json::Value;
use thiserror::Error;
use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::Notify;
use tools::{sanitizar_parametro_log, sanitizar_rota_log, ToolRegistry};
use tracing::{debug, error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// URL padrão da instância local do Radar Cívico.
pub const DEFAULT_RADAR_URL: &str = "http://127.0.0.1:28080";

/// Filtro de log padrão quando `RUST_LOG` não está definida no ambiente.
pub const DEFAULT_RUST_LOG: &str = "radar_mcp=info,warn";

/// Timeout padrão do cliente HTTP (8 segundos).
pub const HTTP_TIMEOUT_SECS: u64 = 8;

/// Argumentos e variáveis de ambiente de configuração do servidor MCP.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "radar-mcp",
    version,
    about = "Servidor MCP (Model Context Protocol) via stdio para o Radar Cívico"
)]
pub struct RadarMcpConfig {
    /// URL base da API HTTP do Radar Cívico
    #[arg(long, env = "RADAR_URL", default_value = DEFAULT_RADAR_URL)]
    pub radar_url: String,

    /// Chave de autenticação opcional para endpoints protegidos do Radar Cívico
    #[arg(long, env = "RADAR_API_KEY")]
    pub radar_api_key: Option<String>,
}

impl RadarMcpConfig {
    /// Carrega a configuração a partir de variáveis de ambiente e argumentos de CLI,
    /// aplicando valores padrão e normalizando entradas vazias.
    pub fn load() -> Self {
        let _ = dotenvy::dotenv();
        let mut cfg = Self::parse();
        cfg.normalize();
        cfg
    }

    /// Constrói a configuração diretamente das variáveis de ambiente `RADAR_URL` e `RADAR_API_KEY`.
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        let radar_url = std::env::var("RADAR_URL")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| DEFAULT_RADAR_URL.to_string());

        let radar_api_key = std::env::var("RADAR_API_KEY")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());

        let mut cfg = Self {
            radar_url,
            radar_api_key,
        };
        cfg.normalize();
        cfg
    }

    fn normalize(&mut self) {
        let trimmed_url = self.radar_url.trim().trim_end_matches('/').to_string();
        self.radar_url = if trimmed_url.is_empty() {
            DEFAULT_RADAR_URL.to_string()
        } else {
            trimmed_url
        };

        self.radar_api_key = self
            .radar_api_key
            .take()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty());
    }
}

/// Erros de comunicação HTTP com a API do Radar Cívico.
#[derive(Debug, Error)]
pub enum RadarHttpError {
    #[error("Falha ao inicializar cliente HTTP: {0}")]
    ClientBuild(#[from] reqwest::Error),

    #[error("Erro na requisição HTTP para '{url}': {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("Resposta HTTP {status} ao acessar '{url}': {body}")]
    HttpStatus {
        status: u16,
        url: String,
        body: String,
    },
}

/// Cliente HTTP configurado para comunicação com o backend do Radar Cívico.
#[derive(Clone, Debug)]
pub struct RadarHttpClient {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    timeout: Duration,
}

impl RadarHttpClient {
    /// Cria um novo cliente HTTP com timeout fixo de 8 segundos e cabeçalhos de autenticação opcionais.
    pub fn new(config: &RadarMcpConfig) -> Result<Self, RadarHttpError> {
        let timeout = Duration::from_secs(HTTP_TIMEOUT_SECS);
        let mut headers = HeaderMap::new();

        if let Some(ref key) = config.radar_api_key {
            if let Ok(val) = HeaderValue::from_str(key) {
                headers.insert("x-api-key", val);
            }
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {key}")) {
                headers.insert(AUTHORIZATION, val);
            }
        }

        let client = reqwest::Client::builder()
            .timeout(timeout)
            .default_headers(headers)
            .user_agent(format!("radar-mcp/{}", env!("CARGO_PKG_VERSION")))
            .build()?;

        Ok(Self {
            client,
            base_url: config.radar_url.trim_end_matches('/').to_string(),
            api_key: config.radar_api_key.clone(),
            timeout,
        })
    }

    /// Retorna a URL base configurada (`RADAR_URL`).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Retorna a chave de API opcional (`RADAR_API_KEY`), se definida.
    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    /// Retorna o timeout configurado no cliente HTTP (8 segundos).
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Retorna a referência ao `reqwest::Client` subjacente.
    pub fn inner(&self) -> &reqwest::Client {
        &self.client
    }

    /// Monta a URL completa para um caminho da API.
    pub fn endpoint_url(&self, path: &str) -> String {
        let clean_path = path.trim_start_matches('/');
        format!("{}/{}", self.base_url, clean_path)
    }

    /// Executa uma requisição GET com cabeçalho de API key (quando configurado) e desserializa a resposta JSON.
    #[tracing::instrument(
        name = "radar_http.get_json",
        skip(self, path, query),
        fields(
            rota = %sanitizar_rota_log(path),
            status_http = tracing::field::Empty,
            elapsed_ms = tracing::field::Empty
        )
    )]
    pub async fn get_json(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Value, RadarHttpError> {
        let inicio = Instant::now();
        let rota_segura = sanitizar_rota_log(path);
        let query_segura: Vec<(&str, String)> = query
            .iter()
            .map(|(k, v)| (*k, sanitizar_parametro_log(v)))
            .collect();

        debug!(
            rota = %rota_segura,
            ?query_segura,
            "Iniciando chamada HTTP GET à API local"
        );

        let url = self.endpoint_url(path);
        let url_segura = sanitizar_rota_log(&url);
        let mut req = self.client.get(&url);
        if !query.is_empty() {
            req = req.query(query);
        }
        if let Some(ref key) = self.api_key {
            req = req
                .header("x-api-key", key)
                .header(AUTHORIZATION, format!("Bearer {key}"));
        }

        let response = req.send().await.map_err(|source| {
            let elapsed_ms = inicio.elapsed().as_millis() as u64;
            tracing::Span::current().record("elapsed_ms", elapsed_ms);
            error!(
                rota = %rota_segura,
                url = %url_segura,
                elapsed_ms,
                erro = %source,
                "Erro de conexão ao comunicar com a API local do Radar Cívico"
            );
            RadarHttpError::Request {
                url: url_segura.clone(),
                source,
            }
        })?;

        let status = response.status();
        let status_code = status.as_u16();
        tracing::Span::current().record("status_http", status_code);

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let elapsed_ms = inicio.elapsed().as_millis() as u64;
            tracing::Span::current().record("elapsed_ms", elapsed_ms);
            warn!(
                rota = %rota_segura,
                url = %url_segura,
                status_http = status_code,
                elapsed_ms,
                "Status HTTP anômalo retornado pela API local do Radar Cívico"
            );
            return Err(RadarHttpError::HttpStatus {
                status: status_code,
                url: url_segura,
                body,
            });
        }

        let parsed = response.json::<Value>().await.map_err(|source| {
            let elapsed_ms = inicio.elapsed().as_millis() as u64;
            tracing::Span::current().record("elapsed_ms", elapsed_ms);
            error!(
                rota = %rota_segura,
                url = %url_segura,
                status_http = status_code,
                elapsed_ms,
                erro = %source,
                "Falha ao desserializar payload JSON da API local"
            );
            RadarHttpError::Request {
                url: url_segura.clone(),
                source,
            }
        })?;

        let elapsed_ms = inicio.elapsed().as_millis() as u64;
        tracing::Span::current().record("elapsed_ms", elapsed_ms);
        info!(
            rota = %rota_segura,
            status_http = status_code,
            elapsed_ms,
            "Resposta HTTP recebida com sucesso da API local"
        );

        Ok(parsed)
    }
}

/// Estrutura principal do servidor MCP do Radar Cívico.
#[derive(Clone)]
pub struct RadarMcpServer {
    http_client: RadarHttpClient,
    registry: Arc<ToolRegistry>,
    prompts: Arc<PromptRegistry>,
    in_flight: Arc<AtomicUsize>,
    idle_notify: Arc<Notify>,
}

struct InFlightGuard {
    in_flight: Arc<AtomicUsize>,
    idle_notify: Arc<Notify>,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        if self.in_flight.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.idle_notify.notify_waiters();
        }
    }
}

/// Wrapper sobre `tokio::io::Stdin` que posterga a sinalização de EOF enquanto houver
/// chamadas de ferramentas em andamento (até o timeout de 8s do cliente HTTP).
struct GracefulStdin {
    inner: tokio::io::Stdin,
    eof_seen: bool,
    in_flight: Arc<AtomicUsize>,
    idle_notify: Arc<Notify>,
    notified: Option<Pin<Box<dyn std::future::Future<Output = ()> + Send>>>,
}

impl GracefulStdin {
    fn new(inner: tokio::io::Stdin, in_flight: Arc<AtomicUsize>, idle_notify: Arc<Notify>) -> Self {
        Self {
            inner,
            eof_seen: false,
            in_flight,
            idle_notify,
            notified: None,
        }
    }
}

impl AsyncRead for GracefulStdin {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if !self.eof_seen {
            let before = buf.filled().len();
            match Pin::new(&mut self.inner).poll_read(cx, buf) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Ready(Ok(())) => {
                    if buf.filled().len() > before {
                        return Poll::Ready(Ok(()));
                    }
                    self.eof_seen = true;
                }
            }
        }

        while self.in_flight.load(Ordering::Acquire) > 0 {
            if self.notified.is_none() {
                let notify = self.idle_notify.clone();
                self.notified = Some(Box::pin(async move {
                    tokio::select! {
                        _ = notify.notified() => {}
                        _ = tokio::time::sleep(Duration::from_millis(50)) => {}
                    }
                }));
            }
            if self.in_flight.load(Ordering::Acquire) == 0 {
                self.notified = None;
                break;
            }
            if let Some(fut) = self.notified.as_mut() {
                match fut.as_mut().poll(cx) {
                    Poll::Ready(()) => {
                        self.notified = None;
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }
        }

        Poll::Ready(Ok(()))
    }
}

impl RadarMcpServer {
    /// Cria uma nova instância do servidor MCP com o registro padrão de ferramentas.
    pub fn new(http_client: RadarHttpClient) -> Self {
        Self::with_registry(http_client, ToolRegistry::with_default_tools())
    }

    /// Cria uma instância do servidor MCP com um `ToolRegistry` customizado.
    pub fn with_registry(http_client: RadarHttpClient, registry: ToolRegistry) -> Self {
        Self {
            http_client,
            registry: Arc::new(registry),
            prompts: Arc::new(PromptRegistry::new()),
            in_flight: Arc::new(AtomicUsize::new(0)),
            idle_notify: Arc::new(Notify::new()),
        }
    }

    /// Retorna referência ao cliente HTTP configurado.
    pub fn http_client(&self) -> &RadarHttpClient {
        &self.http_client
    }

    /// Retorna referência ao registro de ferramentas.
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// Retorna referência ao registro de prompts nativos.
    pub fn prompts(&self) -> &PromptRegistry {
        &self.prompts
    }
}

impl ServerHandler for RadarMcpServer {
    fn get_info(&self) -> ServerConfig {
        let capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_prompts()
            .build();
        ServerConfig::new(capabilities)
            .with_server_info(Implementation::new("radar-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Servidor MCP do Radar Cívico para consulta e auditoria de dados públicos.",
            )
    }

    #[tracing::instrument(
        name = "mcp.initialize",
        skip(self, request, context),
        fields(client_name = %request.client_info.name, protocol_version = %request.protocol_version)
    )]
    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        info!(
            client_name = %request.client_info.name,
            protocol_version = %request.protocol_version,
            "Handshake initialize recebido no servidor MCP"
        );
        let init_result = self.negotiate_initialize(&request)?;
        let mut peer_info = request;
        peer_info.protocol_version = init_result.protocol_version.clone();
        context.peer.set_peer_info(peer_info);
        Ok(init_result)
    }

    #[tracing::instrument(name = "mcp.list_tools", skip(self, _request, _context))]
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let tools = self.registry.list_tools();
        debug!(
            tools_count = tools.len(),
            "Listando ferramentas MCP registradas"
        );
        Ok(ListToolsResult::with_all_items(tools))
    }

    #[tracing::instrument(name = "mcp.list_prompts", skip(self, _request, _context))]
    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        let prompts = self.prompts.list_prompts();
        debug!(
            prompts_count = prompts.len(),
            "Listando prompts nativos MCP"
        );
        Ok(ListPromptsResult::with_all_items(prompts))
    }

    #[tracing::instrument(
        name = "mcp.get_prompt",
        skip(self, request, _context),
        fields(prompt_name = %request.name)
    )]
    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        self.prompts.get_prompt(request)
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.registry.get_tool(name)
    }

    #[tracing::instrument(
        name = "mcp.call_tool",
        skip(self, request, _context),
        fields(ferramenta = %request.name)
    )]
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        self.in_flight.fetch_add(1, Ordering::AcqRel);
        let _guard = InFlightGuard {
            in_flight: self.in_flight.clone(),
            idle_notify: self.idle_notify.clone(),
        };
        self.registry
            .call_tool(&self.http_client, &request.name, request.arguments)
            .await
    }
}

/// Inicializa o subsistema `tracing-subscriber` redirecionando 100% dos logs para `std::io::stderr`
/// e aplicando o filtro da variável `RUST_LOG` (default: `"radar_mcp=info,warn"`).
pub fn init_stderr_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(DEFAULT_RUST_LOG));

    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(false),
        )
        .try_init();
}

#[tokio::main]
async fn main() -> Result<()> {
    // 100% dos logs vão estritamente para std::io::stderr para isolamento total do transporte stdio JSON-RPC.
    init_stderr_tracing();

    let config = RadarMcpConfig::load();

    let http_client = RadarHttpClient::new(&config)
        .context("Falha ao inicializar cliente HTTP do Radar Cívico")?;

    let server = RadarMcpServer::new(http_client);

    info!(
        radar_url = %server.http_client().base_url(),
        api_key_configured = server.http_client().api_key().is_some(),
        timeout_secs = server.http_client().timeout().as_secs(),
        tools_count = server.registry().list_tools().len(),
        "Iniciando servidor radar-mcp via transporte stdio"
    );

    let stdin = GracefulStdin::new(
        tokio::io::stdin(),
        server.in_flight.clone(),
        server.idle_notify.clone(),
    );
    let running = serve_directly(server, (stdin, tokio::io::stdout()), None);
    running
        .waiting()
        .await
        .context("Erro no loop de eventos stdio do servidor MCP")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn config_usa_valores_padrao_e_normaliza_url() {
        let mut cfg = RadarMcpConfig {
            radar_url: "http://127.0.0.1:28080/".to_string(),
            radar_api_key: Some("   ".to_string()),
        };
        cfg.normalize();

        assert_eq!(cfg.radar_url, DEFAULT_RADAR_URL);
        assert_eq!(cfg.radar_api_key, None);
    }

    #[test]
    fn cliente_http_configura_timeout_8_segundos_e_api_key() {
        let cfg = RadarMcpConfig {
            radar_url: "http://localhost:28080".to_string(),
            radar_api_key: Some("chave-secreta-teste".to_string()),
        };

        let client = RadarHttpClient::new(&cfg).expect("deve criar cliente HTTP");
        assert_eq!(client.base_url(), "http://localhost:28080");
        assert_eq!(client.api_key(), Some("chave-secreta-teste"));
        assert_eq!(client.timeout(), Duration::from_secs(8));
        assert_eq!(
            client.endpoint_url("/api/v1/config/status"),
            "http://localhost:28080/api/v1/config/status"
        );
    }

    #[test]
    fn servidor_mcp_registra_ferramentas_buscar_e_dossie_cnpj() {
        let cfg = RadarMcpConfig::from_env();
        let client = RadarHttpClient::new(&cfg).expect("deve criar cliente HTTP");
        let registry = ToolRegistry::with_default_tools();
        let server = RadarMcpServer::with_registry(client, registry);

        let tools = server.registry().list_tools();
        assert!(tools.len() >= 2);

        let buscar = server.get_tool("buscar").expect("deve conter 'buscar'");
        assert_eq!(
            buscar.description.as_deref(),
            Some(
                "Busca entidades (políticos, empresas, contratos) por nome, termo ou documento. Retorna identificadores e tipos encontrados."
            )
        );

        let dossie = server
            .get_tool("dossie_cnpj")
            .expect("deve conter 'dossie_cnpj'");
        assert_eq!(
            dossie.description.as_deref(),
            Some(
                "Consulta o dossiê consolidado de uma pessoa jurídica pelo CNPJ. Retorna resumo cadastral, sócios e alertas detectados por heurística (não são acusações formais)."
            )
        );

        let listar_pol = server
            .get_tool("listar_politicos")
            .expect("deve conter 'listar_politicos'");
        assert_eq!(
            listar_pol.description.as_deref(),
            Some(
                "Lista políticos cadastrados com suporte a filtros por estado (UF) e partido. Retorna identificadores e nomes para cruzamento."
            )
        );

        let detalhe_pol = server
            .get_tool("detalhe_politico")
            .expect("deve conter 'detalhe_politico'");
        assert_eq!(
            detalhe_pol.description.as_deref(),
            Some(
                "Consulta o perfil completo de um político por ID, incluindo histórico de candidaturas e vínculos conhecidos."
            )
        );

        let dossie_cpf = server
            .get_tool("dossie_cpf")
            .expect("deve conter 'dossie_cpf'");
        assert_eq!(
            dossie_cpf.description.as_deref(),
            Some(
                "Consulta o dossiê consolidado de pessoa física por CPF. Retorna sociedades empresariais, doações e alertas vinculados."
            )
        );

        let subgrafo = server.get_tool("subgrafo").expect("deve conter 'subgrafo'");
        assert_eq!(
            subgrafo.description.as_deref(),
            Some(
                "Obtém as conexões imediatas (raio 1) de uma entidade no grafo de relacionamentos. Retorna nós conectados e tipos de aresta."
            )
        );

        let listar_alertas = server
            .get_tool("listar_alertas")
            .expect("deve conter 'listar_alertas'");
        assert_eq!(
            listar_alertas.description.as_deref(),
            Some(
                "Lista indícios e incongruências detectadas por rotinas automáticas de auditoria. ATENÇÃO: Os alertas são apontamentos heurísticos para triagem investigativa, não constituem comprovação de irregularidade ou condenação."
            )
        );

        // Valida se todos os schemas JSON-RPC gerados são objetos JSON Schema válidos
        let list_result = ListToolsResult::with_all_items(tools.clone());
        let list_json =
            serde_json::to_value(&list_result).expect("deve serializar ListToolsResult");
        let tools_arr = list_json["tools"]
            .as_array()
            .expect("deve conter array 'tools'");
        assert_eq!(tools_arr.len(), tools.len());
        for t in tools_arr {
            assert!(t["name"].as_str().is_some_and(|s| !s.is_empty()));
            assert!(t["description"].as_str().is_some_and(|s| !s.is_empty()));
            assert_eq!(t["inputSchema"]["type"], "object");
            assert!(t["inputSchema"]["properties"].is_object());
        }
    }

    #[test]
    fn sanitizacao_de_logs_mascara_cpf_e_preserva_cnpj() {
        assert_eq!(DEFAULT_RUST_LOG, "radar_mcp=info,warn");
        assert_eq!(sanitizar_parametro_log("12345678901"), "***.456.789-**");
        assert_eq!(sanitizar_parametro_log("123.456.789-01"), "***.456.789-**");
        assert_eq!(
            sanitizar_parametro_log("cpf:12345678901"),
            "cpf:***.456.789-**"
        );
        assert_eq!(sanitizar_parametro_log("12345678000190"), "12345678000190");
        assert_eq!(
            sanitizar_rota_log("/api/v1/dossie/cpf/12345678901"),
            "/api/v1/dossie/cpf/***.456.789-**"
        );
        assert_eq!(
            sanitizar_rota_log("/api/v1/grafo/cpf%3A12345678901"),
            "/api/v1/grafo/cpf:***.456.789-**"
        );
    }

    #[tokio::test]
    async fn ferramentas_tratam_http_nao_200_e_enviam_radar_api_key_sem_panic() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Servidor HTTP mockado: valida o header x-api-key e devolve HTTP 404 com JSON de erro
        let server_handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            let n = socket.read(&mut buf).await.unwrap();
            let req_str = String::from_utf8_lossy(&buf[..n]).to_string();

            assert!(
                req_str
                    .to_ascii_lowercase()
                    .contains("x-api-key: segredo-mcp-123"),
                "Requisição deve incluir o header x-api-key: {req_str}"
            );
            assert!(
                req_str.contains("GET /api/v1/dossie/cnpj/12345678000190 "),
                "CNPJ deve chegar sanitizado na rota: {req_str}"
            );

            let body = r#"{"status":"erro","mensagem":"CNPJ não localizado na base"}"#;
            let resp = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(resp.as_bytes()).await.unwrap();
        });

        let cfg = RadarMcpConfig {
            radar_url: format!("http://{addr}"),
            radar_api_key: Some("segredo-mcp-123".to_string()),
        };
        let client = RadarHttpClient::new(&cfg).unwrap();
        let registry = ToolRegistry::with_default_tools();

        let args = json!({ "cnpj": "12.345.678/0001-90" }).as_object().cloned();
        let response = registry
            .call_tool(&client, "dossie_cnpj", args)
            .await
            .expect("não deve retornar erro de protocolo nem dar panic");

        server_handle.await.unwrap();

        let CallToolResponse::Complete(result) = response else {
            panic!("esperado CallToolResponse::Complete");
        };
        let serialized = serde_json::to_value(&result).unwrap();
        assert_eq!(serialized["isError"], true);
        let text = serialized["content"][0]["text"].as_str().unwrap();
        let err_json: Value = serde_json::from_str(text).unwrap();
        assert_eq!(err_json["erro"], true);
        assert_eq!(err_json["status_http"], 404);
        assert_eq!(err_json["detalhe"], "CNPJ não localizado na base");
    }
}
