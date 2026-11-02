use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use rmcp::{
    ErrorData as McpError,
    model::{CallToolResult, ContentBlock, JsonObject, Tool},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::tools::politicos::mascarar_cpf_estrito;
use crate::tools::{McpTool, format_http_error_json, format_validation_error_json, truncar_texto};
use crate::{RadarHttpClient, RadarHttpError};

pub const DEFAULT_LIMITE_ARESTAS_SUBGRAFO: usize = 30;
pub const MAX_LIMITE_ARESTAS_SUBGRAFO: usize = 50;

pub const DEFAULT_LIMITE_ALERTAS: usize = 15;
pub const MAX_LIMITE_ALERTAS: usize = 50;
pub const MAX_RESUMO_FATO_CHARS: usize = 240;

pub const DESCRICAO_SUBGRAFO: &str = "Obtém as conexões imediatas (raio 1) de uma entidade no grafo de relacionamentos. Retorna nós conectados e tipos de aresta.";

pub const DESCRICAO_LISTAR_ALERTAS: &str = "Lista indícios e incongruências detectadas por rotinas automáticas de auditoria. ATENÇÃO: Os alertas são apontamentos heurísticos para triagem investigativa, não constituem comprovação de irregularidade ou condenação.";

// ============================================================================
// 1. Ferramenta `subgrafo`
// ============================================================================

/// Parâmetros de entrada da ferramenta MCP `subgrafo`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubgrafoInput {
    /// Identificador do nó raiz (ex: 'cnpj:12345678000195' ou 'cpf:12345678900').
    pub id: String,
    /// Quantidade máxima de nós e arestas retornados (default: 30, máx: 50).
    #[serde(default)]
    pub limite_arestas: Option<usize>,
}

impl SubgrafoInput {
    /// Retorna o limite efetivo respeitando o default (30) e o teto máximo (50).
    pub fn limite_efetivo(&self) -> usize {
        match self.limite_arestas {
            Some(0) | None => DEFAULT_LIMITE_ARESTAS_SUBGRAFO,
            Some(v) => v.min(MAX_LIMITE_ARESTAS_SUBGRAFO),
        }
    }
}

/// Nó resumido do subgrafo de relacionamentos.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NoSubgrafoFormatado {
    pub id: String,
    pub nome: String,
    pub tipo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documento: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uf: Option<String>,
}

/// Aresta resumida do subgrafo de relacionamentos.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArestaSubgrafoFormatada {
    pub origem: String,
    pub destino: String,
    pub tipo_aresta: String,
    pub valor: f64,
    pub ano: i32,
    pub fonte: String,
}

/// Estrutura de saída protegida contra explosão de contexto da ferramenta `subgrafo`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SaidaSubgrafoFormatada {
    pub raiz_id: String,
    pub truncado: bool,
    pub limite_arestas: usize,
    pub total_nos_original: usize,
    pub total_arestas_original: usize,
    pub nos: Vec<NoSubgrafoFormatado>,
    pub arestas: Vec<ArestaSubgrafoFormatada>,
}

/// Processa e trunca o retorno de `/api/v1/grafo/{id}` no limite configurado,
/// sinalizando `truncado: true` caso a lista de nós ou de arestas tenha sofrido corte.
pub fn parse_e_formatar_subgrafo(
    raw: &Value,
    fallback_id: &str,
    limite: usize,
) -> SaidaSubgrafoFormatada {
    let limite_clamped = if limite == 0 {
        DEFAULT_LIMITE_ARESTAS_SUBGRAFO
    } else {
        limite.min(MAX_LIMITE_ARESTAS_SUBGRAFO)
    };

    let raiz_id = raw
        .get("raiz_id")
        .and_then(Value::as_str)
        .unwrap_or(fallback_id)
        .to_string();

    // Suporta tanto `nos`/`arestas` diretos quanto `cytoscape.nodes`/`cytoscape.edges`
    let raw_nos = raw
        .get("nos")
        .and_then(Value::as_array)
        .or_else(|| {
            raw.get("cytoscape")
                .and_then(|c| c.get("nodes"))
                .and_then(Value::as_array)
        })
        .or_else(|| raw.get("nodes").and_then(Value::as_array));

    let raw_arestas = raw
        .get("arestas")
        .and_then(Value::as_array)
        .or_else(|| {
            raw.get("cytoscape")
                .and_then(|c| c.get("edges"))
                .and_then(Value::as_array)
        })
        .or_else(|| raw.get("edges").and_then(Value::as_array));

    let mut db_id_to_uuid = HashMap::<i64, String>::new();
    if let Some(lista_nos) = raw_nos {
        for n in lista_nos {
            let node_obj = n.get("data").unwrap_or(n);
            if let (Some(db_id), Some(uuid)) = (
                node_obj.get("db_id").and_then(Value::as_i64),
                node_obj
                    .get("uuid")
                    .or_else(|| node_obj.get("id"))
                    .and_then(Value::as_str),
            ) {
                db_id_to_uuid.insert(db_id, uuid.to_string());
            }
        }
    }

    let len_nos = raw_nos.map(|a| a.len()).unwrap_or(0);
    let len_arestas = raw_arestas.map(|a| a.len()).unwrap_or(0);

    let total_nos_original = raw
        .get("total_nos")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(len_nos)
        .max(len_nos);

    let total_arestas_original = raw
        .get("total_arestas")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(len_arestas)
        .max(len_arestas);

    let truncado = len_nos > limite_clamped
        || len_arestas > limite_clamped
        || total_nos_original > limite_clamped
        || total_arestas_original > limite_clamped;

    let nos = raw_nos
        .map(|lista| {
            lista
                .iter()
                .take(limite_clamped)
                .map(|item| {
                    let obj = item.get("data").unwrap_or(item);
                    let id = obj
                        .get("uuid")
                        .or_else(|| obj.get("id"))
                        .and_then(|v| {
                            v.as_str()
                                .map(ToString::to_string)
                                .or_else(|| v.as_i64().map(|n| n.to_string()))
                        })
                        .unwrap_or_default();

                    let nome = obj
                        .get("nome")
                        .or_else(|| obj.get("label"))
                        .or_else(|| obj.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("NÃO IDENTIFICADO")
                        .to_string();

                    let tipo = obj
                        .get("tipo")
                        .or_else(|| obj.get("category"))
                        .and_then(Value::as_str)
                        .unwrap_or("DESCONHECIDO")
                        .to_string();

                    let documento = obj
                        .get("documento")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                        .map(mascarar_cpf_estrito);

                    let uf = obj
                        .get("uf")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                        .map(ToString::to_string);

                    NoSubgrafoFormatado {
                        id,
                        nome,
                        tipo,
                        documento,
                        uf,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let arestas = raw_arestas
        .map(|lista| {
            lista
                .iter()
                .take(limite_clamped)
                .map(|item| {
                    let obj = item.get("data").unwrap_or(item);
                    let origem = extrair_endpoint_aresta(obj, "source", "origem_id", &db_id_to_uuid);
                    let destino =
                        extrair_endpoint_aresta(obj, "target", "destino_id", &db_id_to_uuid);
                    let tipo_aresta = obj
                        .get("tipo_aresta")
                        .or_else(|| obj.get("tipo_relacao"))
                        .or_else(|| obj.get("label"))
                        .and_then(Value::as_str)
                        .unwrap_or("VINCULO")
                        .to_string();
                    let valor = obj.get("valor").and_then(Value::as_f64).unwrap_or(0.0);
                    let ano = obj.get("ano").and_then(Value::as_i64).unwrap_or(0) as i32;
                    let fonte = obj
                        .get("fonte_dado")
                        .or_else(|| obj.get("fonte"))
                        .and_then(Value::as_str)
                        .unwrap_or("Radar Cívico")
                        .to_string();

                    ArestaSubgrafoFormatada {
                        origem,
                        destino,
                        tipo_aresta,
                        valor,
                        ano,
                        fonte,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    SaidaSubgrafoFormatada {
        raiz_id,
        truncado,
        limite_arestas: limite_clamped,
        total_nos_original,
        total_arestas_original,
        nos,
        arestas,
    }
}

fn extrair_endpoint_aresta(
    obj: &Value,
    str_key: &str,
    id_key: &str,
    db_id_to_uuid: &HashMap<i64, String>,
) -> String {
    if let Some(s) = obj.get(str_key).and_then(Value::as_str) {
        if !s.trim().is_empty() {
            return s.to_string();
        }
    }
    if let Some(id_num) = obj
        .get(id_key)
        .or_else(|| obj.get(str_key))
        .and_then(Value::as_i64)
    {
        if let Some(uuid) = db_id_to_uuid.get(&id_num) {
            return uuid.clone();
        }
        return id_num.to_string();
    }
    String::new()
}

/// Ferramenta MCP `subgrafo`.
pub struct SubgrafoTool;

#[async_trait]
impl McpTool for SubgrafoTool {
    fn definition(&self) -> Tool {
        let schema_val = json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Identificador da entidade no grafo (ex: 'cnpj:12345678000195' ou 'cpf:12345678900')."
                },
                "limite_arestas": {
                    "type": "integer",
                    "description": "Limite máximo de nós e arestas retornados (default: 30, máx: 50).",
                    "default": 30,
                    "minimum": 1,
                    "maximum": 50
                }
            },
            "required": ["id"],
            "additionalProperties": false
        });

        let schema_obj = match schema_val {
            Value::Object(map) => map,
            _ => JsonObject::new(),
        };

        Tool::new("subgrafo", DESCRICAO_SUBGRAFO, Arc::new(schema_obj))
    }

    async fn call(
        &self,
        client: &RadarHttpClient,
        arguments: JsonObject,
    ) -> Result<CallToolResult, McpError> {
        let input: SubgrafoInput = match serde_json::from_value(Value::Object(arguments)) {
            Ok(val) => val,
            Err(err) => {
                let msg = format_validation_error_json(
                    "subgrafo",
                    &format!("Parâmetros inválidos para 'subgrafo': {err}"),
                );
                return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]));
            }
        };

        let id_clean = input.id.trim();
        if id_clean.is_empty() {
            let msg = format_validation_error_json(
                "subgrafo",
                "O parâmetro obrigatório 'id' não pode ser vazio.",
            );
            return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]));
        }

        let limite = input.limite_efetivo();
        let encoded_id = urlencoding::encode(id_clean);
        let path = format!("/api/v1/grafo/{encoded_id}");
        let query_params = [("grau", "1".to_string())];

        match client.get_json(&path, &query_params).await {
            Ok(raw_json) => {
                let formatado = parse_e_formatar_subgrafo(&raw_json, id_clean, limite);
                let texto = serde_json::to_string_pretty(&formatado)
                    .unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![ContentBlock::text(texto)]))
            }
            Err(err) => Ok(tratar_erro_auditoria("subgrafo", &err)),
        }
    }
}

// ============================================================================
// 2. Ferramenta `listar_alertas`
// ============================================================================

/// Parâmetros de entrada da ferramenta MCP `listar_alertas`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListarAlertasInput {
    /// Filtro opcional por tipo de alerta (ex: 'doador_fornecedor', 'socio_fantasma').
    #[serde(default)]
    pub tipo: Option<String>,
    /// Filtro opcional por severidade ('baixa', 'media', 'alta').
    #[serde(default)]
    pub severidade: Option<String>,
    /// Quantidade máxima de alertas retornados (default: 15).
    #[serde(default)]
    pub limite: Option<usize>,
}

impl ListarAlertasInput {
    pub fn limite_efetivo(&self) -> usize {
        match self.limite {
            Some(0) | None => DEFAULT_LIMITE_ALERTAS,
            Some(v) => v.min(MAX_LIMITE_ALERTAS),
        }
    }

    pub fn tipo_normalizado(&self) -> Option<String> {
        self.tipo
            .as_ref()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
    }

    pub fn severidade_normalizada(&self) -> Option<String> {
        self.severidade
            .as_ref()
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
    }
}

/// Item individual estruturado retornado por `listar_alertas`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemAlertaFormatado {
    pub id: String,
    pub alerta_tipo: String,
    pub entidade_alvo: String,
    pub grau_severidade: String,
    pub resumo_fato: String,
}

/// Saída estruturada da ferramenta `listar_alertas`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SaidaListarAlertas {
    pub aviso_heuristico: String,
    pub total_retornado: usize,
    pub alertas: Vec<ItemAlertaFormatado>,
}

/// Converte o retorno de `/api/v1/auditoria/alertas` na lista estruturada contendo
/// `id`, `alerta_tipo`, `entidade_alvo`, `grau_severidade` e `resumo_fato`.
pub fn parse_e_formatar_alertas(raw: &Value, limite: usize) -> SaidaListarAlertas {
    let limite_clamped = if limite == 0 {
        DEFAULT_LIMITE_ALERTAS
    } else {
        limite.min(MAX_LIMITE_ALERTAS)
    };

    let raw_list = raw
        .get("alertas")
        .and_then(Value::as_array)
        .or_else(|| raw.get("resultados").and_then(Value::as_array))
        .or_else(|| raw.as_array());

    let mut alertas = Vec::new();
    if let Some(items) = raw_list {
        for item in items.iter().take(limite_clamped) {
            let id = item
                .get("id")
                .and_then(|v| {
                    v.as_i64()
                        .map(|n| n.to_string())
                        .or_else(|| v.as_u64().map(|n| n.to_string()))
                        .or_else(|| v.as_str().map(ToString::to_string))
                })
                .unwrap_or_default();

            let alerta_tipo = item
                .get("alerta_tipo")
                .or_else(|| item.get("tipo"))
                .and_then(Value::as_str)
                .unwrap_or("INDEFINIDO")
                .to_string();

            let entidade_alvo = item
                .get("entidade_alvo")
                .or_else(|| item.get("alvo_nome"))
                .and_then(Value::as_str)
                .unwrap_or("NÃO IDENTIFICADO")
                .to_string();

            let grau_severidade = item
                .get("grau_severidade")
                .or_else(|| item.get("severidade"))
                .and_then(Value::as_str)
                .unwrap_or("INFO")
                .to_string();

            let resumo_bruto = item
                .get("resumo_fato")
                .or_else(|| item.get("descricao"))
                .or_else(|| item.get("titulo"))
                .and_then(Value::as_str)
                .unwrap_or("");

            alertas.push(ItemAlertaFormatado {
                id,
                alerta_tipo,
                entidade_alvo,
                grau_severidade,
                resumo_fato: truncar_texto(resumo_bruto, MAX_RESUMO_FATO_CHARS),
            });
        }
    }

    SaidaListarAlertas {
        aviso_heuristico:
            "ATENÇÃO: Os alertas são apontamentos heurísticos para triagem investigativa, não constituem comprovação de irregularidade ou condenação."
                .to_string(),
        total_retornado: alertas.len(),
        alertas,
    }
}

/// Ferramenta MCP `listar_alertas`.
pub struct ListarAlertasTool;

#[async_trait]
impl McpTool for ListarAlertasTool {
    fn definition(&self) -> Tool {
        let schema_val = json!({
            "type": "object",
            "description": DESCRICAO_LISTAR_ALERTAS,
            "properties": {
                "tipo": {
                    "type": "string",
                    "description": "Tipo de alerta heurístico (opcional, ex: 'doador_fornecedor', 'socio_fantasma')."
                },
                "severidade": {
                    "type": "string",
                    "enum": ["baixa", "media", "alta", "BAIXA", "MEDIA", "ALTA", "CRITICA"],
                    "description": "Grau de severidade do apontamento heurístico (opcional: 'baixa', 'media', 'alta')."
                },
                "limite": {
                    "type": "integer",
                    "description": "Quantidade máxima de alertas retornados (default: 15).",
                    "default": 15,
                    "minimum": 1,
                    "maximum": 50
                }
            },
            "additionalProperties": false
        });

        let schema_obj = match schema_val {
            Value::Object(map) => map,
            _ => JsonObject::new(),
        };

        Tool::new(
            "listar_alertas",
            DESCRICAO_LISTAR_ALERTAS,
            Arc::new(schema_obj),
        )
    }

    async fn call(
        &self,
        client: &RadarHttpClient,
        arguments: JsonObject,
    ) -> Result<CallToolResult, McpError> {
        let input: ListarAlertasInput = match serde_json::from_value(Value::Object(arguments)) {
            Ok(val) => val,
            Err(err) => {
                let msg = format_validation_error_json(
                    "listar_alertas",
                    &format!("Parâmetros inválidos para 'listar_alertas': {err}"),
                );
                return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]));
            }
        };

        let limite = input.limite_efetivo();
        let mut query_params: Vec<(&str, String)> = vec![("limit", limite.to_string())];

        if let Some(tipo) = input.tipo_normalizado() {
            query_params.push(("tipo", tipo));
        }
        if let Some(sev) = input.severidade_normalizada() {
            query_params.push(("severidade", sev));
        }

        match client
            .get_json("/api/v1/auditoria/alertas", &query_params)
            .await
        {
            Ok(raw_json) => {
                let saida = parse_e_formatar_alertas(&raw_json, limite);
                let texto = serde_json::to_string_pretty(&saida)
                    .unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![ContentBlock::text(texto)]))
            }
            Err(err) => Ok(tratar_erro_auditoria("listar_alertas", &err)),
        }
    }
}

fn tratar_erro_auditoria(ferramenta: &str, err: &RadarHttpError) -> CallToolResult {
    if let RadarHttpError::HttpStatus {
        status: 404,
        url,
        body,
    } = err
    {
        let detalhe = serde_json::from_str::<Value>(body)
            .ok()
            .and_then(|v| {
                v.get("mensagem")
                    .or_else(|| v.get("error"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| {
                "Entidade ou subgrafo não localizado na base de dados do Radar Cívico.".to_string()
            });

        let payload = json!({
            "erro": false,
            "encontrado": false,
            "status_http": 404,
            "ferramenta": ferramenta,
            "url": url,
            "mensagem": format!("Recurso não encontrado (404) ao consultar '{ferramenta}'."),
            "detalhe": detalhe
        });
        let texto = serde_json::to_string_pretty(&payload)
            .unwrap_or_else(|_| r#"{"encontrado":false,"status_http":404}"#.to_string());
        return CallToolResult::error(vec![ContentBlock::text(texto)]);
    }

    let erro_json = format_http_error_json(ferramenta, err);
    CallToolResult::error(vec![ContentBlock::text(erro_json)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subgrafo_input_e_truncamento_com_flag_truncado() {
        let input_default: SubgrafoInput =
            serde_json::from_value(json!({ "id": "cnpj:12345678000195" })).unwrap();
        assert_eq!(input_default.id, "cnpj:12345678000195");
        assert_eq!(input_default.limite_efetivo(), 30);

        let input_max: SubgrafoInput = serde_json::from_value(json!({
            "id": "cpf:12345678900",
            "limite_arestas": 99
        }))
        .unwrap();
        assert_eq!(input_max.limite_efetivo(), 50);

        // Grafo maior que o limite configurado (5 nós e 5 arestas, limite = 3)
        let mut nos_mock = Vec::new();
        let mut arestas_mock = Vec::new();
        for i in 1..=5 {
            nos_mock.push(json!({
                "db_id": i,
                "uuid": format!("node_{i}"),
                "tipo": "EMPRESA",
                "documento": "12345678901",
                "nome": format!("ENTIDADE {i}"),
                "uf": "SP"
            }));
            arestas_mock.push(json!({
                "db_id": i * 10,
                "origem_id": 1,
                "destino_id": i,
                "tipo_relacao": "SOCIO_DE",
                "valor": 1000.0 * (i as f64),
                "ano": 2024,
                "fonte_dado": "QSA"
            }));
        }

        let payload_api = json!({
            "raiz_id": "node_1",
            "grau": 1,
            "total_nos": 5,
            "total_arestas": 5,
            "nos": nos_mock,
            "arestas": arestas_mock
        });

        let saida_truncada = parse_e_formatar_subgrafo(&payload_api, "node_1", 3);
        assert!(saida_truncada.truncado);
        assert_eq!(saida_truncada.nos.len(), 3);
        assert_eq!(saida_truncada.arestas.len(), 3);
        assert_eq!(saida_truncada.arestas[0].origem, "node_1");
        assert_eq!(saida_truncada.arestas[0].tipo_aresta, "SOCIO_DE");
        assert_eq!(
            saida_truncada.nos[0].documento.as_deref(),
            Some("***.456.789-**")
        );

        // Quando o limite comporta todos os nós e arestas -> truncado = false
        let saida_completa = parse_e_formatar_subgrafo(&payload_api, "node_1", 10);
        assert!(!saida_completa.truncado);
        assert_eq!(saida_completa.nos.len(), 5);
        assert_eq!(saida_completa.arestas.len(), 5);
    }

    #[test]
    fn test_listar_alertas_schema_carrega_aviso_heuristico_e_formata_campos() {
        let tool = ListarAlertasTool;
        let def = tool.definition();
        let desc = def.description.as_deref().unwrap_or("");
        assert!(
            desc.contains("ATENÇÃO: Os alertas são apontamentos heurísticos para triagem investigativa, não constituem comprovação de irregularidade ou condenação."),
            "Descrição deve conter aviso explícito de natureza heurística: {desc}"
        );

        let mock_api = json!({
            "total": 1,
            "limit": 15,
            "offset": 0,
            "alertas": [
                {
                    "id": 901,
                    "tipo": "DOADOR_FORNECEDOR",
                    "severidade": "ALTA",
                    "titulo": "Triangulação detectada",
                    "descricao": "Sócio de empresa contratada realizou doação de campanha em janela inferior a 180 dias.",
                    "alvo_nome": "CONSTRUTORA HORIZONTE LTDA",
                    "valor_envolvido": 320000.0,
                    "fonte_dado": "TSE / PNCP"
                }
            ]
        });

        let saida = parse_e_formatar_alertas(&mock_api, 15);
        assert_eq!(saida.total_retornado, 1);
        assert_eq!(
            saida.alertas[0],
            ItemAlertaFormatado {
                id: "901".to_string(),
                alerta_tipo: "DOADOR_FORNECEDOR".to_string(),
                entidade_alvo: "CONSTRUTORA HORIZONTE LTDA".to_string(),
                grau_severidade: "ALTA".to_string(),
                resumo_fato: "Sócio de empresa contratada realizou doação de campanha em janela inferior a 180 dias.".to_string(),
            }
        );

        let item_val = serde_json::to_value(&saida.alertas[0]).unwrap();
        let obj = item_val.as_object().unwrap();
        assert_eq!(obj.len(), 5);
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("alerta_tipo"));
        assert!(obj.contains_key("entidade_alvo"));
        assert!(obj.contains_key("grau_severidade"));
        assert!(obj.contains_key("resumo_fato"));
    }
}
