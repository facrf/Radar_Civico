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

use crate::tools::{format_http_error_json, format_validation_error_json, truncar_texto, McpTool};
use crate::utils::compact::{ordenar_e_truncar_lista, serializar_json_compacto};
use crate::RadarHttpClient;

pub const MAX_SOCIOS_CONTEXTO: usize = 10;
pub const MAX_ALERTAS_CONTEXTO: usize = 10;
pub const MAX_CONTRATOS_CONTEXTO: usize = 10;
pub const MAX_DESCRICAO_ALERTA_CHARS: usize = 220;

/// Parâmetros de entrada da ferramenta MCP `dossie_cnpj`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DossieCnpjInput {
    /// CNPJ da pessoa jurídica (com ou sem máscara).
    pub cnpj: String,
}

impl DossieCnpjInput {
    /// Retorna o CNPJ sanitizado sem pontos, traços, barras ou espaços.
    pub fn cnpj_sanitizado(&self) -> String {
        sanitizar_cnpj(&self.cnpj)
    }
}

/// Remove pontos (`.`), traços (`-`), barras (`/`) e espaços de um CNPJ.
pub fn sanitizar_cnpj(raw: &str) -> String {
    raw.chars()
        .filter(|c| !matches!(c, '.' | '-' | '/' | ' ' | '\t' | '\n' | '\r'))
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_uppercase()
}

/// Resumo cadastral e financeiro consolidado da pessoa jurídica.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResumoCadastralCnpj {
    pub cnpj: String,
    pub cnpj_formatado: String,
    pub razao_social: String,
    pub situacao_cadastral: String,
    pub data_abertura: Option<String>,
    pub score_risco: String,
    pub total_faturado_ceap: f64,
    pub total_notas_ceap: usize,
    pub total_contratado_pncp: f64,
    pub total_contratos_pncp: usize,
    pub total_doacoes_socios: f64,
}

/// Dados resumidos de um sócio do quadro societário (QSA).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocioResumidoCnpj {
    pub nome: String,
    pub documento_mascarado: String,
    pub qualificacao: Option<String>,
    pub tipo: String,
    pub total_outras_empresas: usize,
}

/// Alerta heurístico resumido e truncado para contexto de LLM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaResumidoCnpj {
    pub tipo: String,
    pub severidade: String,
    pub titulo: String,
    pub descricao: String,
    pub valor_envolvido: Option<f64>,
    pub fonte: String,
}

/// Contrato público resumido para contexto de LLM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContratoResumidoCnpj {
    pub numero_controle_pncp: Option<String>,
    pub orgao_nome: Option<String>,
    pub objeto: Option<String>,
    pub valor: f64,
    pub data_assinatura: Option<String>,
    pub ano: Option<i32>,
}

/// Dossiê CNPJ limpo e truncado para consumo por modelos de linguagem (LLM).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DossieCnpjFormatado {
    pub nota_metodologica: String,
    pub resumo_cadastral: ResumoCadastralCnpj,
    pub mais_registros: bool,
    pub total_socios: usize,
    pub socios_truncados: bool,
    pub socios: Vec<SocioResumidoCnpj>,
    pub total_alertas: usize,
    pub alertas_truncados: bool,
    pub alertas: Vec<AlertaResumidoCnpj>,
    pub contratos: Vec<ContratoResumidoCnpj>,
}

/// Converte o payload bruto de `/api/v1/dossie/cnpj/{cnpj}` em uma estrutura enxuta e truncada.
pub fn parse_e_formatar_dossie_cnpj(raw: &Value, fallback_cnpj: &str) -> DossieCnpjFormatado {
    let cnpj = raw
        .get("cnpj")
        .and_then(Value::as_str)
        .unwrap_or(fallback_cnpj)
        .to_string();

    let cnpj_formatado = raw
        .get("cnpj_formatado")
        .and_then(Value::as_str)
        .unwrap_or(&cnpj)
        .to_string();

    let razao_social = raw
        .get("razao_social")
        .and_then(Value::as_str)
        .unwrap_or("NÃO INFORMADO")
        .to_string();

    let situacao_cadastral = raw
        .get("situacao_cadastral")
        .and_then(Value::as_str)
        .unwrap_or("DESCONHECIDA")
        .to_string();

    let data_abertura = raw
        .get("data_abertura")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let score_risco = raw
        .get("score_risco")
        .and_then(Value::as_str)
        .unwrap_or("BAIXO")
        .to_string();

    let ceap = raw.get("ceap");
    let total_faturado_ceap = ceap
        .and_then(|c| c.get("total_faturado"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let total_notas_ceap = ceap
        .and_then(|c| c.get("total_notas"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;

    let pncp = raw.get("pncp");
    let total_contratado_pncp = pncp
        .and_then(|p| p.get("total_contratado"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let total_contratos_pncp = pncp
        .and_then(|p| p.get("total_contratos"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;

    let total_doacoes_socios = raw
        .get("tse")
        .and_then(|t| t.get("total_doacoes_socios"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);

    let resumo_cadastral = ResumoCadastralCnpj {
        cnpj,
        cnpj_formatado,
        razao_social,
        situacao_cadastral,
        data_abertura,
        score_risco,
        total_faturado_ceap,
        total_notas_ceap,
        total_contratado_pncp,
        total_contratos_pncp,
        total_doacoes_socios,
    };

    // Extração e truncamento do quadro societário (QSA)
    let raw_socios = raw
        .get("qsa")
        .and_then(|q| q.get("socios"))
        .and_then(Value::as_array)
        .or_else(|| raw.get("socios").and_then(Value::as_array));

    let total_socios_declarado = raw
        .get("qsa")
        .and_then(|q| q.get("total_socios"))
        .and_then(Value::as_u64)
        .map(|n| n as usize);

    let (socios, total_socios, socios_truncados) = if let Some(lista) = raw_socios {
        let total = total_socios_declarado.unwrap_or(lista.len());
        let truncado = lista.len() > MAX_SOCIOS_CONTEXTO || total > MAX_SOCIOS_CONTEXTO;
        let itens = lista
            .iter()
            .take(MAX_SOCIOS_CONTEXTO)
            .map(|s| {
                let outras_count = s
                    .get("outras_empresas")
                    .and_then(Value::as_array)
                    .map(|a| a.len())
                    .unwrap_or(0);
                SocioResumidoCnpj {
                    nome: s
                        .get("nome")
                        .and_then(Value::as_str)
                        .unwrap_or("SÓCIO NÃO IDENTIFICADO")
                        .to_string(),
                    documento_mascarado: s
                        .get("documento_mascarado")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    qualificacao: s
                        .get("qualificacao")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    tipo: s
                        .get("tipo")
                        .and_then(Value::as_str)
                        .unwrap_or("PF")
                        .to_string(),
                    total_outras_empresas: outras_count,
                }
            })
            .collect();
        (itens, total, truncado)
    } else {
        (Vec::new(), total_socios_declarado.unwrap_or(0), false)
    };

    // Extração e truncamento dos alertas heurísticos
    let raw_alertas = raw.get("alertas").and_then(Value::as_array);
    let total_alertas_declarado = raw
        .get("total_alertas")
        .and_then(Value::as_u64)
        .map(|n| n as usize);

    let (alertas, total_alertas, alertas_truncados) = if let Some(lista) = raw_alertas {
        let total = total_alertas_declarado.unwrap_or(lista.len());
        let truncado = lista.len() > MAX_ALERTAS_CONTEXTO || total > MAX_ALERTAS_CONTEXTO;
        let itens = lista
            .iter()
            .take(MAX_ALERTAS_CONTEXTO)
            .map(|a| {
                let desc_bruta = a.get("descricao").and_then(Value::as_str).unwrap_or("");
                AlertaResumidoCnpj {
                    tipo: a
                        .get("tipo")
                        .and_then(Value::as_str)
                        .unwrap_or("INDEFINIDO")
                        .to_string(),
                    severidade: a
                        .get("severidade")
                        .and_then(Value::as_str)
                        .unwrap_or("INFO")
                        .to_string(),
                    titulo: a
                        .get("titulo")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    descricao: truncar_texto(desc_bruta, MAX_DESCRICAO_ALERTA_CHARS),
                    valor_envolvido: a.get("valor_envolvido").and_then(Value::as_f64),
                    fonte: a
                        .get("fonte")
                        .and_then(Value::as_str)
                        .unwrap_or("Radar Cívico")
                        .to_string(),
                }
            })
            .collect();
        (itens, total, truncado)
    } else {
        (Vec::new(), total_alertas_declarado.unwrap_or(0), false)
    };

    // Extração e truncamento dos 10 contratos mais recentes do PNCP
    let raw_contratos = pncp
        .and_then(|p| p.get("contratos"))
        .and_then(Value::as_array)
        .or_else(|| raw.get("contratos").and_then(Value::as_array));

    let (contratos, contratos_truncados) = if let Some(lista) = raw_contratos {
        let (recentes, truncou) = ordenar_e_truncar_lista(lista, MAX_CONTRATOS_CONTEXTO);
        let truncado = truncou || total_contratos_pncp > MAX_CONTRATOS_CONTEXTO;
        let itens = recentes
            .into_iter()
            .map(|c| {
                let obj_bruto = c
                    .get("objeto")
                    .or_else(|| c.get("descricao"))
                    .and_then(Value::as_str)
                    .map(|s| truncar_texto(s, MAX_DESCRICAO_ALERTA_CHARS));
                ContratoResumidoCnpj {
                    numero_controle_pncp: c
                        .get("numero_controle_pncp")
                        .or_else(|| c.get("id"))
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    orgao_nome: c
                        .get("orgao_nome")
                        .or_else(|| c.get("orgao"))
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    objeto: obj_bruto,
                    valor: c
                        .get("valor")
                        .or_else(|| c.get("valor_global"))
                        .or_else(|| c.get("valor_inicial"))
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0),
                    data_assinatura: c
                        .get("data_assinatura")
                        .or_else(|| c.get("data_publicacao"))
                        .or_else(|| c.get("data"))
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    ano: c
                        .get("ano")
                        .or_else(|| c.get("ano_contrato"))
                        .and_then(Value::as_i64)
                        .map(|n| n as i32),
                }
            })
            .collect();
        (itens, truncado)
    } else {
        (Vec::new(), total_contratos_pncp > MAX_CONTRATOS_CONTEXTO)
    };

    let mais_registros = socios_truncados || alertas_truncados || contratos_truncados;

    DossieCnpjFormatado {
        nota_metodologica: "Alertas detectados por heurística sobre dados públicos; não constituem acusações formais.".to_string(),
        resumo_cadastral,
        mais_registros,
        total_socios,
        socios_truncados,
        socios,
        total_alertas,
        alertas_truncados,
        alertas,
        contratos,
    }
}

/// Ferramenta MCP `dossie_cnpj`.
pub struct DossieCnpjTool;

#[async_trait]
impl McpTool for DossieCnpjTool {
    fn definition(&self) -> Tool {
        let schema_val = json!({
            "type": "object",
            "properties": {
                "cnpj": {
                    "type": "string",
                    "description": "CNPJ da pessoa jurídica (com ou sem pontuação; pontos, traços e barras serão removidos automaticamente)."
                }
            },
            "required": ["cnpj"],
            "additionalProperties": false
        });

        let schema_obj = match schema_val {
            Value::Object(map) => map,
            _ => JsonObject::new(),
        };

        Tool::new(
            "dossie_cnpj",
            "Consulta o dossiê consolidado de uma pessoa jurídica pelo CNPJ. Retorna resumo cadastral, sócios e alertas detectados por heurística (não são acusações formais).",
            Arc::new(schema_obj),
        )
    }

    #[tracing::instrument(
        name = "tool.dossie_cnpj",
        skip(self, client, arguments),
        fields(
            ferramenta = "dossie_cnpj",
            cnpj = tracing::field::Empty,
            elapsed_ms = tracing::field::Empty
        )
    )]
    async fn call(
        &self,
        client: &RadarHttpClient,
        arguments: JsonObject,
    ) -> Result<CallToolResult, McpError> {
        let inicio = Instant::now();
        let input: DossieCnpjInput = match serde_json::from_value(Value::Object(arguments)) {
            Ok(val) => val,
            Err(err) => {
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                warn!(elapsed_ms, erro = %err, "Parâmetros de entrada inválidos para 'dossie_cnpj'");
                let msg = format_validation_error_json(
                    "dossie_cnpj",
                    &format!("Parâmetros inválidos para 'dossie_cnpj': {err}"),
                );
                return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]));
            }
        };

        let cnpj_limpo = input.cnpj_sanitizado();
        if cnpj_limpo.is_empty() {
            let elapsed_ms = inicio.elapsed().as_millis() as u64;
            tracing::Span::current().record("elapsed_ms", elapsed_ms);
            warn!(elapsed_ms, "Parâmetro 'cnpj' vazio em 'dossie_cnpj'");
            let msg = format_validation_error_json(
                "dossie_cnpj",
                "O parâmetro obrigatório 'cnpj' não contém caracteres válidos.",
            );
            return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]));
        }

        tracing::Span::current().record("cnpj", cnpj_limpo.as_str());
        info!(cnpj = %cnpj_limpo, "Consultando dossiê CNPJ na API local");

        let path = format!("/api/v1/dossie/cnpj/{cnpj_limpo}");
        match client.get_json(&path, &[]).await {
            Ok(raw_json) => {
                let formatado = parse_e_formatar_dossie_cnpj(&raw_json, &cnpj_limpo);
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                info!(
                    cnpj = %cnpj_limpo,
                    total_socios = formatado.total_socios,
                    total_alertas = formatado.total_alertas,
                    elapsed_ms,
                    "Dossiê CNPJ formatado com sucesso"
                );
                let texto = serializar_json_compacto(&formatado);
                Ok(CallToolResult::success(vec![ContentBlock::text(texto)]))
            }
            Err(err) => {
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                warn!(
                    cnpj = %cnpj_limpo,
                    elapsed_ms,
                    erro = %err,
                    "Falha na chamada à API durante 'dossie_cnpj'"
                );
                let erro_json = format_http_error_json("dossie_cnpj", &err);
                Ok(CallToolResult::error(vec![ContentBlock::text(erro_json)]))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitizacao_e_serializacao_dossie_cnpj_input() {
        let raw_json = json!({ "cnpj": "12.345.678/0001-90" });
        let input: DossieCnpjInput = serde_json::from_value(raw_json).unwrap();
        assert_eq!(input.cnpj, "12.345.678/0001-90");
        assert_eq!(input.cnpj_sanitizado(), "12345678000190");

        assert_eq!(sanitizar_cnpj(" 98.765.432/0001-10 "), "98765432000110");
        assert_eq!(sanitizar_cnpj("12345678000190"), "12345678000190");

        let serialized = serde_json::to_value(&input).unwrap();
        assert_eq!(serialized["cnpj"], "12.345.678/0001-90");
    }

    #[test]
    fn test_parse_e_formatar_dossie_cnpj_limpa_e_trunca_payload() {
        let descricao_longa = "A".repeat(400);
        let mut socios_mock = Vec::new();
        for i in 0..15 {
            socios_mock.push(json!({
                "nome": format!("SOCIO {i}"),
                "documento_mascarado": "***.123.456-**",
                "qualificacao": "Sócio-Administrador",
                "tipo": "PF",
                "outras_empresas": [
                    { "cnpj": "11111111000111", "cnpj_formatado": "11.111.111/0001-11", "razao_social": "OUTRA LTDA", "vinculo": "SOCIO" }
                ]
            }));
        }

        let payload_api = json!({
            "avisos_identidade": [],
            "cnpj": "12345678000190",
            "cnpj_formatado": "12.345.678/0001-90",
            "razao_social": "CONSTRUTORA EXEMPLO S.A.",
            "situacao_cadastral": "ATIVA",
            "data_abertura": "2020-05-10",
            "score_risco": "ALTO",
            "total_alertas": 1,
            "alertas": [
                {
                    "tipo": "FORNECEDOR_FANTASMA",
                    "severidade": "ALTA",
                    "titulo": "Empresa recém-aberta contratada",
                    "descricao": descricao_longa,
                    "valor_envolvido": 150000.50,
                    "fonte": "CEAP / Receita Federal"
                }
            ],
            "qsa": {
                "total_socios": 15,
                "socios": socios_mock,
                "empresas_interligadas": []
            },
            "ceap": {
                "total_faturado": 250000.0,
                "total_notas": 12,
                "compradores": [],
                "notas_fiscais": []
            },
            "pncp": {
                "total_contratado": 500000.0,
                "total_contratos": 3,
                "orgaos_contratantes": [],
                "contratos": []
            },
            "tse": {
                "total_doacoes_socios": 15000.0,
                "doacoes": [],
                "candidaturas_socios": []
            },
            "diarios": []
        });

        let dossie = parse_e_formatar_dossie_cnpj(&payload_api, "12345678000190");
        assert_eq!(dossie.resumo_cadastral.cnpj, "12345678000190");
        assert_eq!(
            dossie.resumo_cadastral.razao_social,
            "CONSTRUTORA EXEMPLO S.A."
        );
        assert_eq!(dossie.resumo_cadastral.situacao_cadastral, "ATIVA");
        assert_eq!(dossie.resumo_cadastral.score_risco, "ALTO");
        assert_eq!(dossie.resumo_cadastral.total_faturado_ceap, 250000.0);
        assert_eq!(dossie.resumo_cadastral.total_contratado_pncp, 500000.0);

        // Valida truncamento da lista de sócios (15 -> MAX_SOCIOS_CONTEXTO = 10)
        assert_eq!(dossie.total_socios, 15);
        assert!(dossie.socios_truncados);
        assert_eq!(dossie.socios.len(), MAX_SOCIOS_CONTEXTO);
        assert_eq!(dossie.socios[0].total_outras_empresas, 1);

        // Valida truncamento da descrição do alerta
        assert_eq!(dossie.total_alertas, 1);
        assert!(!dossie.alertas_truncados);
        assert_eq!(dossie.alertas.len(), 1);
        assert!(dossie.alertas[0].descricao.ends_with("..."));
        assert!(dossie.alertas[0].descricao.chars().count() <= MAX_DESCRICAO_ALERTA_CHARS + 3);
        assert!(dossie.mais_registros);

        // Valida que contratos excedentes (> 10) trazem apenas os 10 mais recentes e sinalizam mais_registros
        let mut contratos_mock = Vec::new();
        for i in 1..=14 {
            contratos_mock.push(json!({
                "numero_controle_pncp": format!("PNCP-{i:02}"),
                "orgao_nome": "PREFEITURA MUNICIPAL",
                "objeto": format!("Contrato {i}"),
                "valor": (i as f64) * 10000.0,
                "data_assinatura": format!("2024-03-{i:02}")
            }));
        }
        let mut payload_com_contratos = payload_api.clone();
        payload_com_contratos["qsa"]["socios"] = json!([]);
        payload_com_contratos["qsa"]["total_socios"] = json!(0);
        payload_com_contratos["pncp"]["total_contratos"] = json!(14);
        payload_com_contratos["pncp"]["contratos"] = Value::Array(contratos_mock);

        let dossie_contratos =
            parse_e_formatar_dossie_cnpj(&payload_com_contratos, "12345678000190");
        assert_eq!(dossie_contratos.contratos.len(), MAX_CONTRATOS_CONTEXTO);
        assert!(dossie_contratos.mais_registros);
        assert_eq!(
            dossie_contratos.contratos[0].data_assinatura.as_deref(),
            Some("2024-03-14")
        );
        assert_eq!(
            dossie_contratos.contratos[9].data_assinatura.as_deref(),
            Some("2024-03-05")
        );
    }
}
