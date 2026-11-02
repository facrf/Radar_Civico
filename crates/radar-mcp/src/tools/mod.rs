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

/// Mensagem amigável padrão quando um recurso retorna HTTP 404 ou 204 (vazio/não encontrado).
pub const MENSAGEM_REGISTRO_NAO_ENCONTRADO: &str =
    "Nenhum registro localizado para o identificador fornecido.";

/// Monta a mensagem limpa de falha de infraestrutura para a URL configurada do Radar Cívico.
pub fn mensagem_falha_infraestrutura(radar_url: &str) -> String {
    let url_limpa = radar_url.trim().trim_end_matches('/');
    format!(
        "Não foi possível conectar à API local do Radar em {url_limpa}. Verifique se o serviço está ativo."
    )
}

/// Formata a resposta amigável para resultado vazio ou não encontrado (HTTP 404 / 204).
pub fn format_not_found_json(ferramenta: &str, status: u16, body: &str) -> String {
    let detalhe = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.get("mensagem")
                .or_else(|| v.get("error"))
                .and_then(Value::as_str)
                .map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
        .map(|s| truncar_texto(&s, 200));

    let mut payload = json!({
        "encontrado": false,
        "ferramenta": ferramenta,
        "status_http": status,
        "mensagem": MENSAGEM_REGISTRO_NAO_ENCONTRADO
    });

    if let (Some(det), Some(obj)) = (detalhe, payload.as_object_mut()) {
        obj.insert("detalhe".to_string(), Value::String(det));
    }

    serde_json::to_string(&payload).unwrap_or_else(|_| {
        r#"{"encontrado":false,"mensagem":"Nenhum registro localizado para o identificador fornecido."}"#
            .to_string()
    })
}

/// Formata falhas reais de infraestrutura (Timeout, HTTP 500, Conexão Recusada) em JSON limpo,
/// sem jamais expor stacktraces brutos ou credenciais de headers.
pub fn format_infra_error_json(ferramenta: &str, radar_url: &str, err: &RadarHttpError) -> String {
    let mensagem = mensagem_falha_infraestrutura(radar_url);
    let payload = match err {
        RadarHttpError::HttpStatus { status, .. } => json!({
            "erro": true,
            "ferramenta": ferramenta,
            "status_http": status,
            "mensagem": mensagem
        }),
        RadarHttpError::Request { .. } | RadarHttpError::ClientBuild(_) => json!({
            "erro": true,
            "ferramenta": ferramenta,
            "mensagem": mensagem
        }),
    };

    serde_json::to_string(&payload).unwrap_or_else(|_| {
        format!(
            r#"{{"erro":true,"mensagem":"Não foi possível conectar à API local do Radar em {}. Verifique se o serviço está ativo."}}"#,
            radar_url.trim().trim_end_matches('/')
        )
    })
}

/// Mantida por compatibilidade: delega para `format_infra_error_json` usando a URL padrão quando omitida.
pub fn format_http_error_json(ferramenta: &str, err: &RadarHttpError) -> String {
    format_infra_error_json(ferramenta, crate::DEFAULT_RADAR_URL, err)
}

/// Diferencia estritamente:
/// 1. HTTP 404 / 204 -> `CallToolResult::success` (`is_error: false`) com `{"encontrado": false, "mensagem": "Nenhum registro localizado para o identificador fornecido."}`.
/// 2. Falhas reais de infraestrutura (Timeout, HTTP 500, Conexão Recusada) -> `CallToolResult::error` (`is_error: true`)
///    com `"Não foi possível conectar à API local do Radar em {RADAR_URL}. Verifique se o serviço está ativo."`,
///    sem expor stacktraces brutos ou credenciais.
pub fn tratar_erro_http_tool(
    ferramenta: &str,
    radar_url: &str,
    err: &RadarHttpError,
) -> CallToolResult {
    if let RadarHttpError::HttpStatus {
        status: status @ (404 | 204),
        body,
        ..
    } = err
    {
        let texto = format_not_found_json(ferramenta, *status, body);
        return CallToolResult::success(vec![ContentBlock::text(texto)]);
    }

    let erro_json = format_infra_error_json(ferramenta, radar_url, err);
    CallToolResult::error(vec![ContentBlock::text(erro_json)])
}

/// Retorna a instrução corretiva clara de tipos e parâmetros esperados para cada ferramenta MCP.
pub fn instrucao_corretiva_ferramenta(ferramenta: &str) -> &'static str {
    match ferramenta {
        "buscar" => {
            "Correção: informe 'q' como string não vazia e 'limite' (opcional) como número inteiro entre 1 e 25 (não envie string em campo numérico). Exemplo: {\"q\": \"Silva\", \"limite\": 10}."
        }
        "dossie_cnpj" => {
            "Correção: informe 'cnpj' como string contendo os 14 caracteres do CNPJ (com ou sem pontuação). Exemplo: {\"cnpj\": \"00.000.000/0001-91\"}."
        }
        "listar_politicos" => {
            "Correção: informe 'uf' (opcional, string de 2 letras), 'partido' (opcional, string), 'pagina' (opcional, número inteiro >= 1) e 'limite' (opcional, número inteiro entre 1 e 50). Não envie strings em 'pagina' ou 'limite'. Exemplo: {\"uf\": \"SP\", \"pagina\": 1, \"limite\": 20}."
        }
        "detalhe_politico" => {
            "Correção: informe 'id' como número inteiro ou string não vazia identificando o político. Exemplo: {\"id\": 42} ou {\"id\": \"280000123456\"}."
        }
        "dossie_cpf" => {
            "Correção: informe 'cpf' como string contendo os dígitos do CPF (com ou sem pontuação). Exemplo: {\"cpf\": \"123.456.789-01\"}."
        }
        "subgrafo" => {
            "Correção: informe 'id' como string não vazia (ex.: 'cnpj:...' ou 'cpf:...') e 'limite_arestas' (opcional) como número inteiro entre 1 e 50 (não envie string em campo numérico). Exemplo: {\"id\": \"cnpj:00000000000191\", \"limite_arestas\": 30}."
        }
        "listar_alertas" => {
            "Correção: informe 'tipo' (opcional, string), 'severidade' (opcional: 'baixa', 'media' ou 'alta') e 'limite' (opcional) como número inteiro entre 1 e 50 (não envie string em campo numérico). Exemplo: {\"severidade\": \"alta\", \"limite\": 15}."
        }
        _ => {
            "Correção: verifique os nomes e tipos dos parâmetros no inputSchema da ferramenta (campos numéricos exigem números inteiros sem aspas e campos textuais exigem strings)."
        }
    }
}

/// Formata um erro de validação de parâmetros de entrada em JSON com instrução corretiva clara.
pub fn format_validation_error_json(ferramenta: &str, mensagem: &str) -> String {
    let instrucao = instrucao_corretiva_ferramenta(ferramenta);
    let mensagem_completa = if mensagem.contains("Correção:") {
        mensagem.to_string()
    } else {
        format!("{mensagem} {instrucao}")
    };

    let payload = json!({
        "erro": true,
        "ferramenta": ferramenta,
        "status_http": 400,
        "mensagem": mensagem_completa,
        "instrucao_corretiva": instrucao
    });
    serde_json::to_string(&payload).unwrap_or_else(|_| {
        r#"{"erro":true,"mensagem":"Parâmetros de entrada inválidos"}"#.to_string()
    })
}

/// Formata falhas de desserialização de parâmetros de entrada (ex.: string em campo numérico)
/// em uma instrução corretiva clara sem causar panic.
pub fn format_deserialization_error_json(ferramenta: &str, err: &serde_json::Error) -> String {
    let instrucao = instrucao_corretiva_ferramenta(ferramenta);
    let mensagem = format!(
        "Parâmetros de entrada com tipo ou formato inválido para '{ferramenta}' ({err}). {instrucao}"
    );
    format_validation_error_json(ferramenta, &mensagem)
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
                Ok(tratar_erro_http_tool(
                    "radar_status",
                    client.base_url(),
                    &err,
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RadarMcpConfig;

    #[test]
    fn test_tratar_erro_http_404_e_204_retornam_sucesso_com_encontrado_false() {
        for status in [404u16, 204u16] {
            let err = RadarHttpError::HttpStatus {
                status,
                url: "http://127.0.0.1:28080/api/v1/dossie/cnpj/99999999000199".to_string(),
                body: r#"{"mensagem":"Registro inexistente"}"#.to_string(),
            };

            let result = tratar_erro_http_tool("dossie_cnpj", "http://127.0.0.1:28080", &err);
            let serialized = serde_json::to_value(&result).unwrap();

            assert_eq!(
                serialized["isError"], false,
                "HTTP {status} não deve marcar isError=true"
            );
            let text = serialized["content"][0]["text"].as_str().unwrap();
            let payload: Value = serde_json::from_str(text).unwrap();
            assert_eq!(payload["encontrado"], false);
            assert_eq!(
                payload["mensagem"],
                "Nenhum registro localizado para o identificador fornecido."
            );
            assert_eq!(payload["status_http"], status);
        }
    }

    #[test]
    fn test_tratar_erro_infraestrutura_500_usa_is_error_true_e_oculta_stacktrace() {
        let stacktrace_sensivel = "thread 'tokio-runtime-worker' panicked at crates/server/src/main.rs:102\nAuthorization: Bearer segredo-123";
        let err = RadarHttpError::HttpStatus {
            status: 500,
            url: "http://127.0.0.1:28080/api/v1/busca".to_string(),
            body: stacktrace_sensivel.to_string(),
        };

        let result = tratar_erro_http_tool("buscar", "http://127.0.0.1:28080", &err);
        let serialized = serde_json::to_value(&result).unwrap();

        assert_eq!(serialized["isError"], true);
        let text = serialized["content"][0]["text"].as_str().unwrap();
        assert!(
            !text.contains("panicked at") && !text.contains("segredo-123"),
            "Nunca deve vazar stacktrace ou credencial: {text}"
        );

        let payload: Value = serde_json::from_str(text).unwrap();
        assert_eq!(payload["erro"], true);
        assert_eq!(
            payload["mensagem"],
            "Não foi possível conectar à API local do Radar em http://127.0.0.1:28080. Verifique se o serviço está ativo."
        );
    }

    #[tokio::test]
    async fn test_validacao_parametros_tipos_invalidos_retorna_instrucao_corretiva_sem_panic() {
        let cfg = RadarMcpConfig {
            radar_url: "http://127.0.0.1:28080".to_string(),
            radar_api_key: None,
        };
        let client = RadarHttpClient::new(&cfg).unwrap();
        let registry = ToolRegistry::with_default_tools();

        // String em campo numérico `limite` em `buscar`, `listar_politicos`, `listar_alertas` e `limite_arestas` em `subgrafo`
        let casos = [
            ("buscar", json!({ "q": "Silva", "limite": "dez" })),
            (
                "listar_politicos",
                json!({ "uf": "SP", "pagina": "primeira", "limite": "vinte" }),
            ),
            (
                "subgrafo",
                json!({ "id": "cnpj:00000000000191", "limite_arestas": "trinta" }),
            ),
            (
                "listar_alertas",
                json!({ "severidade": "alta", "limite": "quinze" }),
            ),
            ("dossie_cnpj", json!({ "cnpj": 12345678000190u64 })),
            ("dossie_cpf", json!({ "cpf": 12345678901u64 })),
            ("detalhe_politico", json!({ "id": ["array_invalido"] })),
        ];

        for (tool_name, invalid_args) in casos {
            let args_obj = invalid_args.as_object().cloned();
            let resp = registry
                .call_tool(&client, tool_name, args_obj)
                .await
                .expect(
                "validação de tipos inválidos não deve retornar erro de protocolo nem dar panic",
            );

            let CallToolResponse::Complete(result) = resp else {
                panic!("esperado CallToolResponse::Complete para {tool_name}");
            };
            let serialized = serde_json::to_value(&result).unwrap();
            assert_eq!(
                serialized["isError"], true,
                "Tipo inválido em '{tool_name}' deve retornar isError=true"
            );

            let text = serialized["content"][0]["text"].as_str().unwrap();
            let payload: Value = serde_json::from_str(text).unwrap();
            assert_eq!(payload["erro"], true);
            assert_eq!(payload["ferramenta"], tool_name);
            assert!(
                payload["mensagem"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("Correção:"),
                "Mensagem de erro de '{tool_name}' deve conter instrução corretiva clara: {payload}"
            );
            assert!(
                payload["instrucao_corretiva"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty()),
                "Campo 'instrucao_corretiva' deve estar preenchido para '{tool_name}'"
            );
        }
    }
}
