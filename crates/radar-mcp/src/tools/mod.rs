pub mod auditoria;
pub mod buscar;
pub mod dossie_cnpj;
pub mod politicos;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use rmcp::{
    model::{CallToolResponse, CallToolResult, ContentBlock, JsonObject, Tool},
    ErrorData as McpError,
};
use serde_json::{json, Value};
use tracing::{debug, error, info, warn};

use crate::{RadarHttpClient, RadarHttpError};

pub use auditoria::{
    ArestaSubgrafoFormatada, ItemAlertaFormatado, ListarAlertasInput, ListarAlertasTool,
    NoSubgrafoFormatado, SaidaListarAlertas, SaidaSubgrafoFormatada, SubgrafoInput, SubgrafoTool,
};
pub use buscar::{BuscarInput, BuscarTool, ItemBuscaFormatado, SaidaBuscaFormatada};
pub use dossie_cnpj::{
    sanitizar_cnpj, AlertaResumidoCnpj, ContratoResumidoCnpj, DossieCnpjFormatado, DossieCnpjInput,
    DossieCnpjTool, ResumoCadastralCnpj, SocioResumidoCnpj,
};
pub use politicos::{
    mascarar_cpf_estrito, sanitizar_cpf, DetalhePoliticoFormatado, DetalhePoliticoInput,
    DetalhePoliticoTool, DossieCpfFormatado, DossieCpfInput, DossieCpfTool, ItemPoliticoCompacto,
    ListarPoliticosInput, ListarPoliticosTool, SaidaListarPoliticos,
};

/// Interface modular para definição e execução de ferramentas MCP do Radar Cívico.
#[async_trait]
pub trait McpTool: Send + Sync {
    /// Metadados e JSON Schema de entrada da ferramenta MCP.
    fn definition(&self) -> Tool;

    /// Executa a ferramenta utilizando o cliente HTTP configurado para a API do Radar Cívico.
    async fn call(
        &self,
        client: &RadarHttpClient,
        arguments: JsonObject,
    ) -> Result<CallToolResult, McpError>;
}

/// Registro centralizado de ferramentas MCP expostas pelo `radar-mcp`.
#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools_order: Vec<String>,
    tools: HashMap<String, Arc<dyn McpTool>>,
}

impl ToolRegistry {
    /// Cria um registro de ferramentas vazio.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cria o registro de ferramentas inicializado com todas as ferramentas padrão do módulo.
    pub fn with_default_tools() -> Self {
        let mut registry = Self::new();
        register_all(&mut registry);
        registry
    }

    /// Registra uma nova ferramenta no catálogo MCP.
    pub fn register<T: McpTool + 'static>(&mut self, tool: T) {
        let def = tool.definition();
        let name = def.name.to_string();
        if !self.tools.contains_key(&name) {
            self.tools_order.push(name.clone());
        }
        self.tools.insert(name, Arc::new(tool));
    }

    /// Lista todas as definições de ferramentas registradas em ordem determinística.
    pub fn list_tools(&self) -> Vec<Tool> {
        self.tools_order
            .iter()
            .filter_map(|name| self.tools.get(name).map(|t| t.definition()))
            .collect()
    }

    /// Obtém a definição de uma ferramenta pelo nome.
    pub fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools.get(name).map(|t| t.definition())
    }

    /// Despacha a execução de uma ferramenta registrada pelo nome.
    #[tracing::instrument(
        name = "mcp.registry.call_tool",
        skip(self, client, arguments),
        fields(ferramenta = %name, elapsed_ms = tracing::field::Empty)
    )]
    pub async fn call_tool(
        &self,
        client: &RadarHttpClient,
        name: &str,
        arguments: Option<JsonObject>,
    ) -> Result<CallToolResponse, McpError> {
        let inicio = Instant::now();
        let Some(tool) = self.tools.get(name) else {
            let elapsed_ms = inicio.elapsed().as_millis() as u64;
            tracing::Span::current().record("elapsed_ms", elapsed_ms);
            warn!(
                ferramenta = %name,
                elapsed_ms,
                "Tentativa de chamada a ferramenta inexistente no registro MCP"
            );
            let msg = format_validation_error_json(
                name,
                &format!("Ferramenta não encontrada no registro MCP: '{name}'"),
            );
            return Ok(CallToolResponse::from(CallToolResult::error(vec![
                ContentBlock::text(msg),
            ])));
        };

        let args = arguments.unwrap_or_default();
        debug!(ferramenta = %name, "Despachando execução da ferramenta MCP");
        let result = tool.call(client, args).await?;
        let elapsed_ms = inicio.elapsed().as_millis() as u64;
        tracing::Span::current().record("elapsed_ms", elapsed_ms);
        info!(ferramenta = %name, elapsed_ms, "Execução da ferramenta MCP finalizada");
        Ok(CallToolResponse::from(result))
    }
}

/// Registra os módulos de ferramentas disponíveis no `ToolRegistry`.
pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(BuscarTool);
    registry.register(DossieCnpjTool);
    registry.register(ListarPoliticosTool);
    registry.register(DetalhePoliticoTool);
    registry.register(DossieCpfTool);
    registry.register(SubgrafoTool);
    registry.register(ListarAlertasTool);
}

/// Formata um erro HTTP (incluindo status não-200) em uma estrutura JSON clara sem causar panic.
pub fn format_http_error_json(ferramenta: &str, err: &RadarHttpError) -> String {
    let payload = match err {
        RadarHttpError::HttpStatus { status, url, body } => {
            let detalhe_json = serde_json::from_str::<Value>(body).ok();
            let mensagem_servidor = detalhe_json
                .as_ref()
                .and_then(|v| {
                    v.get("mensagem")
                        .or_else(|| v.get("error"))
                        .and_then(Value::as_str)
                })
                .map(ToString::to_string)
                .unwrap_or_else(|| truncar_texto(body.trim(), 300));

            json!({
                "erro": true,
                "ferramenta": ferramenta,
                "status_http": status,
                "url": url,
                "mensagem": format!("A API do Radar Cívico retornou status HTTP {} para '{}'", status, ferramenta),
                "detalhe": mensagem_servidor
            })
        }
        RadarHttpError::Request { url, source } => {
            json!({
                "erro": true,
                "ferramenta": ferramenta,
                "status_http": null,
                "url": url,
                "mensagem": format!("Falha de comunicação com a API do Radar Cívico ao executar '{}'", ferramenta),
                "detalhe": source.to_string()
            })
        }
        RadarHttpError::ClientBuild(source) => {
            json!({
                "erro": true,
                "ferramenta": ferramenta,
                "status_http": null,
                "mensagem": "Falha ao construir cliente HTTP",
                "detalhe": source.to_string()
            })
        }
    };

    serde_json::to_string(&payload).unwrap_or_else(|_| {
        r#"{"erro":true,"mensagem":"Falha ao serializar erro HTTP"}"#.to_string()
    })
}

/// Formata um erro de validação de parâmetros de entrada em JSON.
pub fn format_validation_error_json(ferramenta: &str, mensagem: &str) -> String {
    let payload = json!({
        "erro": true,
        "ferramenta": ferramenta,
        "status_http": 400,
        "mensagem": mensagem
    });
    serde_json::to_string(&payload).unwrap_or_else(|_| {
        r#"{"erro":true,"mensagem":"Parâmetros de entrada inválidos"}"#.to_string()
    })
}

/// Trunca um texto longo preservando fronteiras de caracteres UTF-8.
pub fn truncar_texto(texto: &str, max_chars: usize) -> String {
    let total_chars = texto.chars().count();
    if total_chars <= max_chars {
        return texto.to_string();
    }
    let prefixo: String = texto.chars().take(max_chars).collect();
    format!("{prefixo}...")
}

/// Sanitiza um parâmetro de entrada antes de registrá-lo em logs/spans `tracing`,
/// mascarando CPFs íntegros (11 dígitos ou prefixados com `cpf:`) para `***.XXX.XXX-**`.
pub fn sanitizar_parametro_log(valor: &str) -> String {
    let trimmed = valor.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some(rest) = trimmed
        .strip_prefix("cpf:")
        .or_else(|| trimmed.strip_prefix("CPF:"))
    {
        return format!("cpf:{}", mascarar_cpf_estrito(rest));
    }

    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    let apenas_caracteres_doc = trimmed
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '*' | ' '));

    if apenas_caracteres_doc && digits.len() == 11 {
        return mascarar_cpf_estrito(trimmed);
    }

    trimmed.to_string()
}

/// Sanitiza caminhos de rota ou URLs para logging seguro, mascarando CPFs presentes
/// em rotas `/api/v1/dossie/cpf/{cpf}` ou `/api/v1/grafo/cpf:...`.
pub fn sanitizar_rota_log(rota: &str) -> String {
    if let Some((prefixo, sufixo)) = rota.split_once("/api/v1/dossie/cpf/") {
        let (doc_part, query_part) = match sufixo.split_once('?') {
            Some((d, q)) => (d, Some(q)),
            None => (sufixo, None),
        };
        let doc_mascarado = mascarar_cpf_estrito(doc_part);
        return match query_part {
            Some(q) => format!("{prefixo}/api/v1/dossie/cpf/{doc_mascarado}?{q}"),
            None => format!("{prefixo}/api/v1/dossie/cpf/{doc_mascarado}"),
        };
    }

    if let Some((prefixo, sufixo)) = rota.split_once("/api/v1/grafo/") {
        let decoded = urlencoding::decode(sufixo)
            .map(|c| c.into_owned())
            .unwrap_or_else(|_| sufixo.to_string());
        let id_seguro = sanitizar_parametro_log(&decoded);
        return format!("{prefixo}/api/v1/grafo/{id_seguro}");
    }

    rota.to_string()
}

/// Ferramenta base de verificação de conectividade e status da instância do Radar Cívico.
pub struct RadarStatusTool;

#[async_trait]
impl McpTool for RadarStatusTool {
    fn definition(&self) -> Tool {
        let schema_val = json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        });
        let schema_obj = match schema_val {
            Value::Object(map) => map,
            _ => JsonObject::new(),
        };
        Tool::new(
            "radar_status",
            "Verifica a conectividade e retorna o status operacional da API do Radar Cívico.",
            Arc::new(schema_obj),
        )
    }

    #[tracing::instrument(
        name = "tool.radar_status",
        skip(self, client, _arguments),
        fields(ferramenta = "radar_status", elapsed_ms = tracing::field::Empty)
    )]
    async fn call(
        &self,
        client: &RadarHttpClient,
        _arguments: JsonObject,
    ) -> Result<CallToolResult, McpError> {
        let inicio = Instant::now();
        match client.get_json("/api/versao", &[]).await {
            Ok(payload) => {
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                info!(elapsed_ms, "Status da API consultado com sucesso");
                let text = crate::utils::compact::serializar_json_compacto(&payload);
                Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
            }
            Err(err) => {
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                error!(elapsed_ms, erro = %err, "Falha ao consultar status da API");
                let erro_json = format_http_error_json("radar_status", &err);
                Ok(CallToolResult::error(vec![ContentBlock::text(erro_json)]))
            }
        }
    }
}
