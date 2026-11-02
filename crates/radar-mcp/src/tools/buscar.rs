use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use rmcp::{
    model::{CallToolResult, ContentBlock, JsonObject, Tool},
    ErrorData as McpError,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::tools::{
    format_http_error_json, format_validation_error_json, sanitizar_parametro_log, McpTool,
};
use crate::utils::compact::serializar_json_compacto;
use crate::RadarHttpClient;

pub const DEFAULT_LIMITE_BUSCA: usize = 10;
pub const MAX_LIMITE_BUSCA: usize = 25;

/// Parâmetros de entrada da ferramenta MCP `buscar`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuscarInput {
    /// Termo de busca (nome, razão social, termo ou documento CPF/CNPJ).
    pub q: String,
    /// Quantidade máxima de resultados (default: 10, máximo: 25).
    #[serde(default)]
    pub limite: Option<usize>,
}

impl BuscarInput {
    /// Calcula o limite efetivo respeitando o default (10) e o teto máximo (25).
    pub fn limite_efetivo(&self) -> usize {
        match self.limite {
            Some(0) | None => DEFAULT_LIMITE_BUSCA,
            Some(v) => v.min(MAX_LIMITE_BUSCA),
        }
    }
}

/// Item individual resumido retornado pela ferramenta `buscar`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemBuscaFormatado {
    pub id: String,
    pub tipo: String,
    pub nome: String,
    pub relevancia: f64,
}

/// Estrutura de saída formatada da ferramenta `buscar`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SaidaBuscaFormatada {
    pub query: String,
    pub total_retornado: usize,
    pub resultados: Vec<ItemBuscaFormatado>,
}

/// Extrai e resume os resultados do endpoint `/api/v1/busca` contendo apenas
/// `id`, `tipo`, `nome` e `relevancia`, truncados ao `limite` solicitado.
pub fn parse_e_formatar_busca(
    raw: &Value,
    fallback_query: &str,
    limite: usize,
) -> SaidaBuscaFormatada {
    let limite_clamped = if limite == 0 {
        DEFAULT_LIMITE_BUSCA
    } else {
        limite.min(MAX_LIMITE_BUSCA)
    };

    let query = raw
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or(fallback_query)
        .to_string();

    let raw_items = raw
        .get("resultados")
        .and_then(Value::as_array)
        .or_else(|| raw.get("itens").and_then(Value::as_array))
        .or_else(|| raw.as_array());

    let mut resultados = Vec::new();

    if let Some(items) = raw_items {
        for (idx, item) in items.iter().take(limite_clamped).enumerate() {
            let id = extrair_id_entidade(item);
            let tipo = item
                .get("tipo")
                .and_then(Value::as_str)
                .unwrap_or("DESCONHECIDO")
                .to_string();
            let nome = item
                .get("nome")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .or_else(|| {
                    item.get("titulo")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                })
                .unwrap_or(&id)
                .to_string();

            let relevancia = item
                .get("relevancia")
                .and_then(Value::as_f64)
                .or_else(|| item.get("score").and_then(Value::as_f64))
                .unwrap_or_else(|| calcular_relevancia_posicional(idx));

            resultados.push(ItemBuscaFormatado {
                id,
                tipo,
                nome,
                relevancia,
            });
        }
    }

    SaidaBuscaFormatada {
        query,
        total_retornado: resultados.len(),
        resultados,
    }
}

fn extrair_id_entidade(item: &Value) -> String {
    if let Some(id_val) = item.get("id") {
        if let Some(n) = id_val.as_i64() {
            return n.to_string();
        }
        if let Some(u) = id_val.as_u64() {
            return u.to_string();
        }
        if let Some(s) = id_val.as_str() {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    if let Some(ident) = item.get("identificador").and_then(Value::as_str) {
        let trimmed = ident.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    if let Some(doc) = item.get("documento").and_then(Value::as_str) {
        let trimmed = doc.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    String::new()
}

fn calcular_relevancia_posicional(idx: usize) -> f64 {
    let raw_score = (1.0 - (idx as f64 * 0.04)).max(0.04);
    (raw_score * 100.0).round() / 100.0
}

/// Ferramenta MCP `buscar`.
pub struct BuscarTool;

#[async_trait]
impl McpTool for BuscarTool {
    fn definition(&self) -> Tool {
        let schema_val = json!({
            "type": "object",
            "properties": {
                "q": {
                    "type": "string",
                    "description": "Termo de busca: nome, razão social, palavra-chave ou documento (CPF/CNPJ)."
                },
                "limite": {
                    "type": "integer",
                    "description": "Quantidade máxima de resultados (default: 10, máximo: 25).",
                    "default": 10,
                    "minimum": 1,
                    "maximum": 25
                }
            },
            "required": ["q"],
            "additionalProperties": false
        });

        let schema_obj = match schema_val {
            Value::Object(map) => map,
            _ => JsonObject::new(),
        };

        Tool::new(
            "buscar",
            "Busca entidades (políticos, empresas, contratos) por nome, termo ou documento. Retorna identificadores e tipos encontrados.",
            Arc::new(schema_obj),
        )
    }

    #[tracing::instrument(
        name = "tool.buscar",
        skip(self, client, arguments),
        fields(
            ferramenta = "buscar",
            q = tracing::field::Empty,
            limite = tracing::field::Empty,
            elapsed_ms = tracing::field::Empty
        )
    )]
    async fn call(
        &self,
        client: &RadarHttpClient,
        arguments: JsonObject,
    ) -> Result<CallToolResult, McpError> {
        let inicio = Instant::now();
        let input: BuscarInput = match serde_json::from_value(Value::Object(arguments)) {
            Ok(val) => val,
            Err(err) => {
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                warn!(elapsed_ms, erro = %err, "Parâmetros de entrada inválidos para 'buscar'");
                let msg = format_validation_error_json(
                    "buscar",
                    &format!("Parâmetros inválidos para 'buscar': {err}"),
                );
                return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]));
            }
        };

        let termo = input.q.trim();
        if termo.is_empty() {
            let elapsed_ms = inicio.elapsed().as_millis() as u64;
            tracing::Span::current().record("elapsed_ms", elapsed_ms);
            warn!(elapsed_ms, "Parâmetro 'q' vazio em 'buscar'");
            let msg = format_validation_error_json(
                "buscar",
                "O parâmetro obrigatório 'q' não pode ser vazio.",
            );
            return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]));
        }

        let termo_seguro = sanitizar_parametro_log(termo);
        let limite = input.limite_efetivo();
        tracing::Span::current().record("q", termo_seguro.as_str());
        tracing::Span::current().record("limite", limite);

        info!(
            q = %termo_seguro,
            limite,
            "Executando ferramenta MCP 'buscar'"
        );

        let query_params = [("q", termo.to_string()), ("limite", limite.to_string())];

        match client.get_json("/api/v1/busca", &query_params).await {
            Ok(raw_json) => {
                let formatado = parse_e_formatar_busca(&raw_json, termo, limite);
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                info!(
                    q = %termo_seguro,
                    total_retornado = formatado.total_retornado,
                    elapsed_ms,
                    "Busca concluída com sucesso"
                );
                let texto = serializar_json_compacto(&formatado);
                Ok(CallToolResult::success(vec![ContentBlock::text(texto)]))
            }
            Err(err) => {
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                warn!(
                    q = %termo_seguro,
                    elapsed_ms,
                    erro = %err,
                    "Falha na chamada à API durante 'buscar'"
                );
                let erro_json = format_http_error_json("buscar", &err);
                Ok(CallToolResult::error(vec![ContentBlock::text(erro_json)]))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buscar_input_deserializacao_e_limites() {
        // Default limite = 10 quando omitido
        let json_sem_limite = json!({ "q": "Construtora Alfa" });
        let input: BuscarInput = serde_json::from_value(json_sem_limite).unwrap();
        assert_eq!(input.q, "Construtora Alfa");
        assert_eq!(input.limite, None);
        assert_eq!(input.limite_efetivo(), 10);

        // Limite explícito dentro da faixa
        let json_com_limite = json!({ "q": "Silva", "limite": 15 });
        let input_15: BuscarInput = serde_json::from_value(json_com_limite).unwrap();
        assert_eq!(input_15.limite_efetivo(), 15);

        // Limite acima do teto é truncado para 25
        let json_acima_max = json!({ "q": "Silva", "limite": 100 });
        let input_max: BuscarInput = serde_json::from_value(json_acima_max).unwrap();
        assert_eq!(input_max.limite_efetivo(), 25);

        // Serialização round-trip
        let serialized = serde_json::to_value(&input_15).unwrap();
        assert_eq!(serialized["q"], "Silva");
        assert_eq!(serialized["limite"], 15);
    }

    #[test]
    fn test_parse_e_formatar_busca_resume_apenas_campos_esperados() {
        let payload_api = json!({
            "query": "alfa",
            "total": 3,
            "resultados": [
                {
                    "tipo": "POLITICO",
                    "id": 42,
                    "identificador": "280000123456",
                    "titulo": "JOÃO ALFA",
                    "subtitulo": "Deputado Federal",
                    "nome": "JOÃO ALFA",
                    "detalhe": "Deputado Federal",
                    "documento": null
                },
                {
                    "tipo": "FORNECEDOR",
                    "id": null,
                    "identificador": "12.345.678/0001-90",
                    "titulo": "ALFA ENGENHARIA LTDA",
                    "subtitulo": "Fornecedor CEAP",
                    "nome": "ALFA ENGENHARIA LTDA",
                    "documento": "12.345.678/0001-90",
                    "relevancia": 0.88
                }
            ]
        });

        let saida = parse_e_formatar_busca(&payload_api, "alfa", 10);
        assert_eq!(saida.query, "alfa");
        assert_eq!(saida.total_retornado, 2);
        assert_eq!(saida.resultados[0].id, "42");
        assert_eq!(saida.resultados[0].tipo, "POLITICO");
        assert_eq!(saida.resultados[0].nome, "JOÃO ALFA");
        assert_eq!(saida.resultados[0].relevancia, 1.0);

        assert_eq!(saida.resultados[1].id, "12.345.678/0001-90");
        assert_eq!(saida.resultados[1].tipo, "FORNECEDOR");
        assert_eq!(saida.resultados[1].nome, "ALFA ENGENHARIA LTDA");
        assert_eq!(saida.resultados[1].relevancia, 0.88);

        // Verifica que o JSON de cada item contém exclusivamente id, tipo, nome e relevancia
        let item_val = serde_json::to_value(&saida.resultados[0]).unwrap();
        let obj = item_val.as_object().unwrap();
        assert_eq!(obj.len(), 4);
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("tipo"));
        assert!(obj.contains_key("nome"));
        assert!(obj.contains_key("relevancia"));
    }
}
