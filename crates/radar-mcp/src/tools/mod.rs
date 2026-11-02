use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use rmcp::{
    ErrorData as McpError,
    model::{CallToolResponse, CallToolResult, ContentBlock, JsonObject, Tool},
};
use serde_json::{Value, json};

use crate::RadarHttpClient;

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

    /// Cria o registro de ferramentas inicializado com as ferramentas registradas do módulo.
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
    pub async fn call_tool(
        &self,
        client: &RadarHttpClient,
        name: &str,
        arguments: Option<JsonObject>,
    ) -> Result<CallToolResponse, McpError> {
        let Some(tool) = self.tools.get(name) else {
            return Err(McpError::invalid_params(
                format!("Ferramenta não encontrada: '{name}'"),
                None,
            ));
        };

        let args = arguments.unwrap_or_default();
        let result = tool.call(client, args).await?;
        Ok(CallToolResponse::from(result))
    }
}

/// Registra os módulos de ferramentas disponíveis no `ToolRegistry`.
pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(RadarStatusTool);
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

    async fn call(
        &self,
        client: &RadarHttpClient,
        _arguments: JsonObject,
    ) -> Result<CallToolResult, McpError> {
        match client.get_json("/api/versao", &[]).await {
            Ok(payload) => {
                let text = serde_json::to_string_pretty(&payload)
                    .unwrap_or_else(|_| payload.to_string());
                Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
            }
            Err(err) => Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "Falha ao consultar status do Radar Cívico em {}: {}",
                client.base_url(),
                err
            ))])),
        }
    }
}
