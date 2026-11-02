pub mod tools;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, Implementation, InitializeRequestParams,
        InitializeResult, ListToolsResult, PaginatedRequestParams, ServerCapabilities,
        ServerConfig, Tool,
    },
    service::{RequestContext, serve_directly},
    transport::stdio,
};
use serde_json::Value;
use thiserror::Error;
use tools::ToolRegistry;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// URL padrão da instância local do Radar Cívico.
pub const DEFAULT_RADAR_URL: &str = "http://127.0.0.1:28080";

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

    /// Executa uma requisição GET e desserializa a resposta JSON.
    pub async fn get_json(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Value, RadarHttpError> {
        let url = self.endpoint_url(path);
        let mut req = self.client.get(&url);
        if !query.is_empty() {
            req = req.query(query);
        }

        let response = req.send().await.map_err(|source| RadarHttpError::Request {
            url: url.clone(),
            source,
        })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(RadarHttpError::HttpStatus {
                status: status.as_u16(),
                url,
                body,
            });
        }

        response
            .json::<Value>()
            .await
            .map_err(|source| RadarHttpError::Request { url, source })
    }
}

/// Estrutura principal do servidor MCP do Radar Cívico.
#[derive(Clone)]
pub struct RadarMcpServer {
    http_client: RadarHttpClient,
    registry: Arc<ToolRegistry>,
}

impl RadarMcpServer {
    /// Cria uma nova instância do servidor MCP com o registro padrão de ferramentas.
    pub fn new(http_client: RadarHttpClient) -> Self {
        Self {
            http_client,
            registry: Arc::new(ToolRegistry::with_default_tools()),
        }
    }

    /// Cria uma instância do servidor MCP com um `ToolRegistry` customizado.
    pub fn with_registry(http_client: RadarHttpClient, registry: ToolRegistry) -> Self {
        Self {
            http_client,
            registry: Arc::new(registry),
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
}

impl ServerHandler for RadarMcpServer {
    fn get_info(&self) -> ServerConfig {
        let capabilities = ServerCapabilities::builder().enable_tools().build();
        ServerConfig::new(capabilities)
            .with_server_info(Implementation::new("radar-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Servidor MCP do Radar Cívico para consulta e auditoria de dados públicos.",
            )
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        let init_result = self.negotiate_initialize(&request)?;
        let mut peer_info = request;
        peer_info.protocol_version = init_result.protocol_version.clone();
        context.peer.set_peer_info(peer_info);
        Ok(init_result)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.registry.list_tools()))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.registry.get_tool(name)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        self.registry
            .call_tool(&self.http_client, &request.name, request.arguments)
            .await
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = RadarMcpConfig::load();

    // Logs são enviados exclusivamente para stderr para preservar o transporte JSON-RPC em stdout.
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "radar_mcp=info".into()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(false),
        )
        .init();

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

    let running = serve_directly(server, stdio(), None);
    running
        .waiting()
        .await
        .context("Erro no loop de eventos stdio do servidor MCP")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn servidor_mcp_registra_ferramentas_modulares() {
        let cfg = RadarMcpConfig::from_env();
        let client = RadarHttpClient::new(&cfg).expect("deve criar cliente HTTP");
        let registry = ToolRegistry::with_default_tools();
        let server = RadarMcpServer::with_registry(client, registry);

        let tools = server.registry().list_tools();
        assert!(!tools.is_empty());
        assert!(server.get_tool("radar_status").is_some());
        assert!(server.get_tool("ferramenta_inexistente").is_none());
    }
}
