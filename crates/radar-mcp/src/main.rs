pub mod prompts;
pub mod resources;
pub mod tools;
pub mod utils;

use std::collections::HashMap;
use std::io::Write as _;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use prompts::PromptRegistry;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use resources::ResourceRegistry;
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResponse, GetPromptRequestParams, GetPromptResponse,
        Implementation, InitializeRequestParams, InitializeResult, ListPromptsResult,
        ListResourcesResult, ListToolsResult, PaginatedRequestParams, ReadResourceRequestParams,
        ReadResourceResponse, ServerCapabilities, ServerConfig, Tool,
    },
    service::{serve_directly, RequestContext},
    ErrorData as McpError, RoleServer, ServerHandler,
};
use serde_json::Value;
use thiserror::Error;
use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::{Notify, RwLock};
use tools::{sanitizar_parametro_log, sanitizar_rota_log, ToolRegistry};
use tracing::{debug, error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// URL padrão da instância local do Radar Cívico.
pub const DEFAULT_RADAR_URL: &str = "http://127.0.0.1:28080";

/// Filtro de log padrão quando `RUST_LOG` não está definida definida no ambiente.
pub const DEFAULT_RUST_LOG: &str = "radar_mcp=info,warn";

/// Timeout padrão do cliente HTTP (8 segundos).
pub const HTTP_TIMEOUT_SECS: u64 = 8;

/// Timeout máximo de desligamento gracioso para cancelamento de tarefas em voo (2 segundos).
pub const SHUTDOWN_TIMEOUT_SECS: u64 = 2;

/// Tempo de vida (TTL) padrão do cache em memória do `RadarHttpClient` (45 segundos).
pub const CACHE_TTL_SECS: u64 = 45;

/// Capacidade máxima de entradas no cache em memória do `RadarHttpClient`.
pub const CACHE_MAX_ENTRIES: usize = 128;

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

#[derive(Clone, Debug)]
struct CacheEntry {
    inserted_at: Instant,
    value: Value,
}

/// Cliente HTTP configurado para comunicação com o backend do Radar Cívico,
/// equipado com cache TTL em memória de curta duração (45s) para leituras repetidas.
#[derive(Clone, Debug)]
pub struct RadarHttpClient {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    timeout: Duration,
    cache_ttl: Duration,
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
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
            cache_ttl: Duration::from_secs(CACHE_TTL_SECS),
            cache: Arc::new(RwLock::new(HashMap::new())),
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

    /// Retorna a quantidade atual de entradas válidas no cache em memória.
    pub async fn cache_len(&self) -> usize {
        let guard = self.cache.read().await;
        let ttl = self.cache_ttl;
        guard
            .values()
            .filter(|e| e.inserted_at.elapsed() < ttl)
            .count()
    }

    /// Limpa todas as entradas do cache em memória.
    pub async fn clear_cache(&self) {
        self.cache.write().await.clear();
    }

    fn build_cache_key(&self, url: &str, query: &[(&str, String)]) -> String {
        if query.is_empty() {
            url.to_string()
        } else {
            let mut pairs: Vec<String> = query.iter().map(|(k, v)| format!("{k}={v}")).collect();
            pairs.sort();
            format!("{url}?{}", pairs.join("&"))
        }
    }

    /// Executa uma requisição GET com cabeçalho de API key (quando configurado) e desserializa a resposta JSON.
    /// Utiliza cache em memória com TTL de 45s para respostas `HTTP 200 OK`.
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
        let url = self.endpoint_url(path);
        let cache_key = self.build_cache_key(&url, query);

        if self.cache_ttl > Duration::ZERO {
            let guard = self.cache.read().await;
            if let Some(entry) = guard.get(&cache_key) {
                if entry.inserted_at.elapsed() < self.cache_ttl {
                    let cached_val = entry.value.clone();
                    drop(guard);
                    let elapsed_ms = inicio.elapsed().as_millis() as u64;
                    tracing::Span::current().record("status_http", 200_u16);
                    tracing::Span::current().record("elapsed_ms", elapsed_ms);
                    info!(
                        rota = %rota_segura,
                        status_http = 200_u16,
                        elapsed_ms,
                        cache_hit = true,
                        "Resposta recuperada do cache em memória do radar-mcp"
                    );
                    return Ok(cached_val);
                }
            }
        }

        let query_segura: Vec<(&str, String)> = query
            .iter()
            .map(|(k, v)| (*k, sanitizar_parametro_log(v)))
            .collect();

        debug!(
            rota = %rota_segura,
            ?query_segura,
            "Iniciando chamada HTTP GET à API local"
        );

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

        if status_code == 204 {
            let elapsed_ms = inicio.elapsed().as_millis() as u64;
            tracing::Span::current().record("elapsed_ms", elapsed_ms);
            info!(
                rota = %rota_segura,
                status_http = status_code,
                elapsed_ms,
                "Resposta HTTP 204 (sem conteúdo) recebida da API local"
            );
            return Err(RadarHttpError::HttpStatus {
                status: 204,
                url: url_segura,
                body: String::new(),
            });
        }

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

        if self.cache_ttl > Duration::ZERO {
            let ttl = self.cache_ttl;
            let mut guard = self.cache.write().await;
            if guard.len() >= CACHE_MAX_ENTRIES {
                guard.retain(|_, v| v.inserted_at.elapsed() < ttl);
                if guard.len() >= CACHE_MAX_ENTRIES {
                    if let Some(oldest_key) = guard
                        .iter()
                        .min_by_key(|(_, v)| v.inserted_at)
                        .map(|(k, _)| k.clone())
                    {
                        guard.remove(&oldest_key);
                    }
                }
            }
            guard.insert(
                cache_key,
                CacheEntry {
                    inserted_at: Instant::now(),
                    value: parsed.clone(),
                },
            );
        }

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
    resources: Arc<ResourceRegistry>,
    in_flight: Arc<AtomicUsize>,
    idle_notify: Arc<Notify>,
    cancel_notify: Arc<Notify>,
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

/// Verifica se um bloco de bytes lido do `stdin` contém uma notificação JSON-RPC `"method": "exit"`.
fn contem_notificacao_exit(bytes: &[u8]) -> bool {
    let Ok(texto) = std::str::from_utf8(bytes) else {
        return false;
    };
    for linha in texto.lines() {
        let trimmed = linha.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
            if val.get("method").and_then(Value::as_str) == Some("exit") {
                return true;
            }
        }
    }
    false
}

/// Verifica se um bloco de bytes lido do `stdin` contém chamadas `"tools/call"` ou `"resources/read"`.
fn contem_chamada_tool(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes)
        .map(|s| s.contains("tools/call") || s.contains("resources/read"))
        .unwrap_or(false)
}

/// Wrapper sobre `tokio::io::Stdin` que monitora continuamente o canal de leitura:
/// - Quando `stdin` atinge EOF (0 bytes lidos) ou recebe `"method": "exit"`, inicia o encerramento gracioso.
/// - Caso haja chamadas de ferramentas em voo no instante do EOF, aguarda até `SHUTDOWN_TIMEOUT_SECS` (2s)
///   e cancela tarefas pendentes se o limite expirar, garantindo ausência de loops infinitos.
struct GracefulStdin {
    inner: tokio::io::Stdin,
    eof_seen: bool,
    exit_seen: bool,
    saw_tool_call: bool,
    eof_done_notify: Arc<Notify>,
    in_flight: Arc<AtomicUsize>,
    idle_notify: Arc<Notify>,
    cancel_notify: Arc<Notify>,
    wait_fut: Option<Pin<Box<dyn std::future::Future<Output = ()> + Send>>>,
}

impl GracefulStdin {
    fn new(
        inner: tokio::io::Stdin,
        eof_done_notify: Arc<Notify>,
        in_flight: Arc<AtomicUsize>,
        idle_notify: Arc<Notify>,
        cancel_notify: Arc<Notify>,
    ) -> Self {
        Self {
            inner,
            eof_seen: false,
            exit_seen: false,
            saw_tool_call: false,
            eof_done_notify,
            in_flight,
            idle_notify,
            cancel_notify,
            wait_fut: None,
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
            if self.exit_seen {
                self.eof_seen = true;
            } else {
                let before = buf.filled().len();
                match Pin::new(&mut self.inner).poll_read(cx, buf) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(e)) => {
                        self.eof_seen = true;
                        self.eof_done_notify.notify_waiters();
                        return Poll::Ready(Err(e));
                    }
                    Poll::Ready(Ok(())) => {
                        let after = buf.filled().len();
                        if after > before {
                            let novos_bytes = &buf.filled()[before..after];
                            if contem_chamada_tool(novos_bytes) {
                                self.saw_tool_call = true;
                            }
                            if contem_notificacao_exit(novos_bytes) {
                                info!("Notificação JSON-RPC 'exit' recebida no stdin; agendando encerramento gracioso");
                                self.exit_seen = true;
                            }
                            return Poll::Ready(Ok(()));
                        }
                        // Leitura retornou 0 bytes: EOF detectado no pipe stdin
                        info!("EOF (0 bytes) detectado no stdin; iniciando encerramento gracioso");
                        self.eof_seen = true;
                    }
                }
            }
        }

        // Se nenhuma chamada tools/call foi vista no lote e não há tarefas em voo, encerra imediatamente
        if !self.saw_tool_call && self.in_flight.load(Ordering::Acquire) == 0 {
            self.wait_fut = None;
            self.eof_done_notify.notify_waiters();
            return Poll::Ready(Ok(()));
        }

        // Aguarda tarefas em voo com timeout estrito de SHUTDOWN_TIMEOUT_SECS (2s) e cancela excedentes
        if self.wait_fut.is_none() {
            let in_flight = self.in_flight.clone();
            let idle_notify = self.idle_notify.clone();
            let cancel_notify = self.cancel_notify.clone();
            let saw_tool_call = self.saw_tool_call;
            self.wait_fut = Some(Box::pin(async move {
                if saw_tool_call && in_flight.load(Ordering::Acquire) == 0 {
                    // Cede brevemente ao runtime para que tasks recém-despachadas pelo parser rmcp entrem em call_tool
                    tokio::time::sleep(Duration::from_millis(15)).await;
                }

                let aguardar_conclusao = async {
                    while in_flight.load(Ordering::Acquire) > 0 {
                        let notified = idle_notify.notified();
                        if in_flight.load(Ordering::Acquire) == 0 {
                            break;
                        }
                        notified.await;
                    }
                };

                if tokio::time::timeout(
                    Duration::from_secs(SHUTDOWN_TIMEOUT_SECS),
                    aguardar_conclusao,
                )
                .await
                .is_err()
                {
                    warn!(
                        timeout_secs = SHUTDOWN_TIMEOUT_SECS,
                        tarefas_ativas = in_flight.load(Ordering::Acquire),
                        "Timeout de desligamento (2s) atingido após EOF; cancelando requisições em voo"
                    );
                    cancel_notify.notify_waiters();
                } else {
                    // Permite que o canal de escrita do rmcp envie a última linha JSON-RPC antes do EOF
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }));
        }

        if let Some(fut) = self.wait_fut.as_mut() {
            match fut.as_mut().poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(()) => {
                    self.wait_fut = None;
                }
            }
        }

        self.eof_done_notify.notify_waiters();
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
            resources: Arc::new(ResourceRegistry::new()),
            in_flight: Arc::new(AtomicUsize::new(0)),
            idle_notify: Arc::new(Notify::new()),
            cancel_notify: Arc::new(Notify::new()),
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

    /// Retorna referência ao registro de recursos MCP.
    pub fn resources(&self) -> &ResourceRegistry {
        &self.resources
    }
}

impl ServerHandler for RadarMcpServer {
    fn get_info(&self) -> ServerConfig {
        let capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_prompts()
            .enable_resources()
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

    #[tracing::instrument(name = "mcp.list_resources", skip(self, _request, _context))]
    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let resources = self.resources.list_resources();
        debug!(
            resources_count = resources.len(),
            "Listando recursos nativos MCP"
        );
        Ok(ListResourcesResult::with_all_items(resources))
    }

    #[tracing::instrument(
        name = "mcp.read_resource",
        skip(self, request, _context),
        fields(uri = %request.uri)
    )]
    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        self.in_flight.fetch_add(1, Ordering::AcqRel);
        let _guard = InFlightGuard {
            in_flight: self.in_flight.clone(),
            idle_notify: self.idle_notify.clone(),
        };

        let cancel_notified = self.cancel_notify.notified();
        let uri = request.uri.clone();
        let resource_fut = self.resources.read_resource(&self.http_client, request);

        tokio::select! {
            res = resource_fut => res,
            _ = cancel_notified => {
                warn!(
                    uri = %uri,
                    "Leitura de recurso cancelada durante desligamento do servidor MCP"
                );
                Err(McpError::internal_error(
                    "Servidor MCP em processo de desligamento",
                    None,
                ))
            }
        }
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

        let cancel_notified = self.cancel_notify.notified();
        let tool_name = request.name.to_string();
        let tool_fut = self
            .registry
            .call_tool(&self.http_client, &tool_name, request.arguments);

        tokio::select! {
            res = tool_fut => res,
            _ = cancel_notified => {
                warn!(
                    ferramenta = %tool_name,
                    "Tarefa assíncrona de ferramenta cancelada durante desligamento do servidor MCP"
                );
                Err(McpError::internal_error(
                    "Servidor MCP em processo de desligamento",
                    None,
                ))
            }
        }
    }
}

/// Aguarda sinais de encerramento do sistema operacional (`SIGINT` via `ctrl_c` ou `SIGTERM` no Unix).
async fn aguardar_sinal_sistema() -> &'static str {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => "SIGINT",
                    _ = sigterm.recv() => "SIGTERM",
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
                "SIGINT"
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        "SIGINT"
    }
}

/// Cancela tarefas assíncronas ativas com timeout de desligamento de 2 segundos,
/// força o flush de `stdout` e `stderr` e encerra o processo com código 0.
async fn encerrar_com_limpeza(
    in_flight: &AtomicUsize,
    idle_notify: &Notify,
    cancel_notify: &Notify,
) -> ! {
    let ativas = in_flight.load(Ordering::Acquire);
    if ativas > 0 {
        info!(
            tarefas_ativas = ativas,
            timeout_secs = SHUTDOWN_TIMEOUT_SECS,
            "Aguardando conclusão ou cancelamento de tarefas assíncronas em voo"
        );
        let aguardar_ativas = async {
            while in_flight.load(Ordering::Acquire) > 0 {
                let notified = idle_notify.notified();
                if in_flight.load(Ordering::Acquire) == 0 {
                    break;
                }
                notified.await;
            }
        };

        if tokio::time::timeout(Duration::from_secs(SHUTDOWN_TIMEOUT_SECS), aguardar_ativas)
            .await
            .is_err()
        {
            warn!(
                timeout_secs = SHUTDOWN_TIMEOUT_SECS,
                tarefas_restantes = in_flight.load(Ordering::Acquire),
                "Timeout de 2s esgotado; cancelando tarefas em voo"
            );
            cancel_notify.notify_waiters();
        }
    } else {
        cancel_notify.notify_waiters();
    }

    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    std::process::exit(0);
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

    let eof_done_notify = Arc::new(Notify::new());
    let in_flight = server.in_flight.clone();
    let idle_notify = server.idle_notify.clone();
    let cancel_notify = server.cancel_notify.clone();

    let stdin = GracefulStdin::new(
        tokio::io::stdin(),
        eof_done_notify.clone(),
        in_flight.clone(),
        idle_notify.clone(),
        cancel_notify.clone(),
    );
    let running = serve_directly(server, (stdin, tokio::io::stdout()), None);
    let waiting_fut = running.waiting();
    tokio::pin!(waiting_fut);

    let eof_failsafe = async {
        eof_done_notify.notified().await;
        // Concede uma breve janela para o loop de escrita do rmcp fazer flush das respostas finais no stdout
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    tokio::pin!(eof_failsafe);

    tokio::select! {
        res = &mut waiting_fut => {
            match res {
                Ok(_reason) => {
                    info!("Loop stdio concluído após EOF; encerrando processo");
                }
                Err(err) => {
                    warn!(erro = %err, "Loop stdio finalizado com condição de transporte; encerrando processo");
                }
            }
        }
        _ = &mut eof_failsafe => {
            info!("Canal stdin atingiu EOF; finalizando loop principal");
        }
        sinal = aguardar_sinal_sistema() => {
            info!(sinal, "Sinal de sistema recebido; iniciando desligamento gracioso");
        }
    }

    encerrar_com_limpeza(&in_flight, &idle_notify, &cancel_notify).await;
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
        assert_eq!(serialized["isError"], false);
        let text = serialized["content"][0]["text"].as_str().unwrap();
        let payload_404: Value = serde_json::from_str(text).unwrap();
        assert_eq!(payload_404["encontrado"], false);
        assert_eq!(
            payload_404["mensagem"],
            "Nenhum registro localizado para o identificador fornecido."
        );
        assert_eq!(payload_404["status_http"], 404);
        assert_eq!(payload_404["detalhe"], "CNPJ não localizado na base");
    }

    #[tokio::test]
    async fn cliente_http_armazena_em_cache_ttl_apenas_respostas_200_ok() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Servidor aceita apenas UMA conexão TCP: a segunda chamada deve ser servida pelo cache em memória
        let server_handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 1024];
            let _ = socket.read(&mut buf).await.unwrap();
            let body = r#"{"versao":"1.0.0","status":"ok"}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(resp.as_bytes()).await.unwrap();
        });

        let cfg = RadarMcpConfig {
            radar_url: format!("http://{addr}"),
            radar_api_key: None,
        };
        let client = RadarHttpClient::new(&cfg).unwrap();
        assert_eq!(client.cache_len().await, 0);

        let r1 = client.get_json("/api/versao", &[]).await.unwrap();
        assert_eq!(r1["versao"], "1.0.0");
        assert_eq!(client.cache_len().await, 1);

        server_handle.await.unwrap();

        // Segunda chamada para a mesma rota (mesmo com o listener TCP já encerrado!) deve vir do cache
        let r2 = client.get_json("/api/versao", &[]).await.unwrap();
        assert_eq!(r2["versao"], "1.0.0");
    }
}
