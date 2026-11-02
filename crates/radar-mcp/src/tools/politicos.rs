use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use rmcp::{
    model::{CallToolResult, ContentBlock, JsonObject, Tool},
    ErrorData as McpError,
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::tools::{
    format_deserialization_error_json, format_validation_error_json, sanitizar_parametro_log,
    tratar_erro_http_tool, truncar_texto, McpTool,
};
use crate::utils::compact::{ordenar_e_truncar_lista, serializar_json_compacto};
use crate::RadarHttpClient;

pub const DEFAULT_PAGINA_POLITICOS: usize = 1;
pub const DEFAULT_LIMITE_POLITICOS: usize = 20;
pub const MAX_LIMITE_POLITICOS: usize = 50;

pub const MAX_CANDIDATURAS_RESUMO: usize = 10;
pub const MAX_DOADORES_RESUMO: usize = 10;
pub const MAX_EMENDAS_RESUMO: usize = 10;
pub const MAX_EMPRESAS_CPF_RESUMO: usize = 10;
pub const MAX_DOACOES_CPF_RESUMO: usize = 10;
pub const MAX_ALERTAS_CPF_RESUMO: usize = 10;
pub const MAX_DESCRICAO_CHARS: usize = 220;

// ============================================================================
// Utilitários de Sanitização e Mascaramento Estrito de CPF (LGPD)
// ============================================================================

/// Sanitiza um CPF removendo pontos, traços, barras, espaços e caracteres não numéricos/asteriscos.
pub fn sanitizar_cpf(raw: &str) -> String {
    raw.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Aplica mascaramento estrito no padrão `***.XXX.XXX-**` para qualquer CPF sem máscara
/// (11 dígitos, formatado `XXX.XXX.XXX-XX` ou miolo de 6 dígitos).
/// Preserva CNPJs (14 dígitos) inalterados.
pub fn mascarar_cpf_estrito(doc: &str) -> String {
    let trimmed = doc.trim();
    if trimmed.is_empty() {
        return "***.000.000-**".to_string();
    }

    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();

    // Se for CNPJ completo (14 dígitos) e não tiver asteriscos de CPF, preserva como CNPJ
    if digits.len() == 14 && !trimmed.contains('*') {
        return trimmed.to_string();
    }

    // CPF completo de 11 dígitos -> extrai os 6 dígitos centrais (posições 3..9)
    if digits.len() == 11 {
        return format!("***.{}.{}-**", &digits[3..6], &digits[6..9]);
    }

    // Miolo de 6 dígitos (ex.: "***.123.456-**" ou "123456")
    if digits.len() == 6 {
        return format!("***.{}.{}-**", &digits[0..3], &digits[3..6]);
    }

    // Caso tenha mais de 6 dígitos mas não 11 nem 14, usa os 6 dígitos centrais disponíveis
    if digits.len() > 6 {
        let start = (digits.len() - 6) / 2;
        let mid = &digits[start..start + 6];
        return format!("***.{}.{}-**", &mid[0..3], &mid[3..6]);
    }

    // Fallback seguro se houver menos de 6 dígitos: preenche com zeros no formato estrito
    let padded = format!("{digits:0>6}");
    format!("***.{}.{}-**", &padded[0..3], &padded[3..6])
}

// ============================================================================
// 1. Ferramenta `listar_politicos`
// ============================================================================

/// Parâmetros de entrada da ferramenta `listar_politicos`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListarPoliticosInput {
    /// Sigla da Unidade Federativa com 2 letras (ex.: "SP", "RJ", "DF").
    #[serde(default)]
    pub uf: Option<String>,
    /// Sigla do partido político (ex.: "PT", "PL", "PSDB", "MDB").
    #[serde(default)]
    pub partido: Option<String>,
    /// Número da página (default: 1).
    #[serde(default)]
    pub pagina: Option<usize>,
    /// Quantidade de registros por página (default: 20, máximo: 50).
    #[serde(default)]
    pub limite: Option<usize>,
}

impl ListarPoliticosInput {
    pub fn pagina_efetiva(&self) -> usize {
        self.pagina.unwrap_or(DEFAULT_PAGINA_POLITICOS).max(1)
    }

    pub fn limite_efetivo(&self) -> usize {
        match self.limite {
            Some(0) | None => DEFAULT_LIMITE_POLITICOS,
            Some(v) => v.min(MAX_LIMITE_POLITICOS),
        }
    }

    pub fn uf_normalizada(&self) -> Option<String> {
        self.uf
            .as_ref()
            .map(|u| u.trim().to_uppercase())
            .filter(|u| !u.is_empty())
    }

    pub fn partido_normalizado(&self) -> Option<String> {
        self.partido
            .as_ref()
            .map(|p| p.trim().to_uppercase())
            .filter(|p| !p.is_empty())
    }
}

/// Item compacto de político retornado por `listar_politicos`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemPoliticoCompacto {
    pub id: String,
    pub nome_eleitoral: String,
    pub cargo: String,
    pub partido: String,
    pub uf: String,
}

/// Saída formatada da ferramenta `listar_politicos`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SaidaListarPoliticos {
    pub pagina: usize,
    pub limite: usize,
    pub total: usize,
    pub politicos: Vec<ItemPoliticoCompacto>,
}

/// Extrai e resume a resposta de `/api/v1/politicos` contendo `id`, `nome_eleitoral`, `cargo`, `partido`, `uf`.
pub fn parse_e_formatar_lista_politicos(
    raw: &Value,
    pagina_default: usize,
    limite: usize,
) -> SaidaListarPoliticos {
    let limite_clamped = if limite == 0 {
        DEFAULT_LIMITE_POLITICOS
    } else {
        limite.min(MAX_LIMITE_POLITICOS)
    };

    let pagina = raw
        .get("page")
        .or_else(|| raw.get("pagina"))
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(pagina_default);

    let raw_list = raw
        .get("politicos")
        .and_then(Value::as_array)
        .or_else(|| raw.get("resultados").and_then(Value::as_array))
        .or_else(|| raw.as_array());

    let mut politicos = Vec::new();
    if let Some(items) = raw_list {
        for item in items.iter().take(limite_clamped) {
            let id = extrair_id_valor(item.get("id"))
                .or_else(|| {
                    item.get("sq_candidato")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
                .unwrap_or_default();

            let nome_eleitoral = item
                .get("nome_eleitoral")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .or_else(|| {
                    item.get("nome_urna")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                })
                .or_else(|| {
                    item.get("nome_completo")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                })
                .unwrap_or("NÃO INFORMADO")
                .to_string();

            let cargo = item
                .get("cargo")
                .and_then(Value::as_str)
                .unwrap_or("NÃO INFORMADO")
                .to_string();

            let partido = item
                .get("partido")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .or_else(|| {
                    item.get("sigla_partido")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                })
                .unwrap_or("S/P")
                .to_string();

            let uf = item
                .get("uf")
                .and_then(Value::as_str)
                .unwrap_or("--")
                .to_string();

            politicos.push(ItemPoliticoCompacto {
                id,
                nome_eleitoral,
                cargo,
                partido,
                uf,
            });
        }
    }

    let total = raw
        .get("total")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(politicos.len());

    SaidaListarPoliticos {
        pagina,
        limite: limite_clamped,
        total,
        politicos,
    }
}

pub struct ListarPoliticosTool;

#[async_trait]
impl McpTool for ListarPoliticosTool {
    fn definition(&self) -> Tool {
        let schema_val = json!({
            "type": "object",
            "properties": {
                "uf": {
                    "type": "string",
                    "description": "Sigla do estado (UF) com 2 letras (opcional, ex.: SP, RJ, MG)."
                },
                "partido": {
                    "type": "string",
                    "description": "Sigla do partido político (opcional, ex.: PT, PL, MDB)."
                },
                "pagina": {
                    "type": "integer",
                    "description": "Número da página (default: 1).",
                    "default": 1,
                    "minimum": 1
                },
                "limite": {
                    "type": "integer",
                    "description": "Quantidade máxima de itens por página (default: 20, máx: 50).",
                    "default": 20,
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
            "listar_politicos",
            "Lista políticos cadastrados com suporte a filtros por estado (UF) e partido. Retorna identificadores e nomes para cruzamento.",
            Arc::new(schema_obj),
        )
    }

    #[tracing::instrument(
        name = "tool.listar_politicos",
        skip(self, client, arguments),
        fields(
            ferramenta = "listar_politicos",
            uf = tracing::field::Empty,
            partido = tracing::field::Empty,
            pagina = tracing::field::Empty,
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
        let input: ListarPoliticosInput = match serde_json::from_value(Value::Object(arguments)) {
            Ok(val) => val,
            Err(err) => {
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                warn!(elapsed_ms, erro = %err, "Parâmetros de entrada inválidos para 'listar_politicos'");
                let msg = format_deserialization_error_json("listar_politicos", &err);
                return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]));
            }
        };

        if let Some(ref uf) = input.uf_normalizada() {
            if uf.chars().count() != 2 || !uf.chars().all(|c| c.is_ascii_alphabetic()) {
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                warn!(uf = %uf, elapsed_ms, "Parâmetro 'uf' inválido em 'listar_politicos'");
                let msg = format_validation_error_json(
                    "listar_politicos",
                    "O parâmetro 'uf' deve conter exatamente 2 letras (ex.: 'SP', 'DF').",
                );
                return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]));
            }
        }

        let pagina = input.pagina_efetiva();
        let limite = input.limite_efetivo();
        let uf_norm = input.uf_normalizada();
        let partido_norm = input.partido_normalizado();

        tracing::Span::current().record("pagina", pagina);
        tracing::Span::current().record("limite", limite);
        if let Some(ref u) = uf_norm {
            tracing::Span::current().record("uf", u.as_str());
        }
        if let Some(ref p) = partido_norm {
            tracing::Span::current().record("partido", p.as_str());
        }

        info!(
            uf = uf_norm.as_deref().unwrap_or("*"),
            partido = partido_norm.as_deref().unwrap_or("*"),
            pagina,
            limite,
            "Executando ferramenta MCP 'listar_politicos'"
        );

        let mut query_params: Vec<(&str, String)> =
            vec![("page", pagina.to_string()), ("limit", limite.to_string())];
        if let Some(uf) = uf_norm {
            query_params.push(("uf", uf));
        }
        if let Some(partido) = partido_norm {
            query_params.push(("partido", partido));
        }

        match client.get_json("/api/v1/politicos", &query_params).await {
            Ok(raw_json) => {
                let saida = parse_e_formatar_lista_politicos(&raw_json, pagina, limite);
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                info!(
                    retornados = saida.politicos.len(),
                    total = saida.total,
                    elapsed_ms,
                    "Listagem de políticos concluída com sucesso"
                );
                let texto = serializar_json_compacto(&saida);
                Ok(CallToolResult::success(vec![ContentBlock::text(texto)]))
            }
            Err(err) => {
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                warn!(elapsed_ms, erro = %err, "Falha na chamada à API durante 'listar_politicos'");
                Ok(tratar_erro_http_tool(
                    "listar_politicos",
                    client.base_url(),
                    &err,
                ))
            }
        }
    }
}

// ============================================================================
// 2. Ferramenta `detalhe_politico`
// ============================================================================

fn deserialize_id_flexivel<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let val = Value::deserialize(deserializer)?;
    match val {
        Value::String(s) => Ok(s.trim().to_string()),
        Value::Number(n) => Ok(n.to_string()),
        _ => Err(serde::de::Error::custom(
            "O campo 'id' deve ser um número inteiro ou uma string",
        )),
    }
}

/// Parâmetros de entrada da ferramenta `detalhe_politico`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DetalhePoliticoInput {
    /// Identificador do político (inteiro ou string conforme o padrão da API).
    #[serde(deserialize_with = "deserialize_id_flexivel")]
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidaturaResumoPolitico {
    pub ano_eleicao: i32,
    pub cargo: String,
    pub partido: String,
    pub uf: String,
    pub municipio: Option<String>,
    pub situacao_totalizacao: Option<String>,
    pub total_bens_declarados: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DoadorResumoPolitico {
    pub doador_nome: String,
    pub doador_documento: String,
    pub valor: f64,
    pub ano_eleicao: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CargoVinculoPolitico {
    pub orgao: String,
    pub cargo: String,
    pub esfera: String,
    pub uf: Option<String>,
    pub data_posse: Option<String>,
    pub data_exoneracao: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmendaResumoPolitico {
    pub codigo: Option<String>,
    pub ano: Option<i32>,
    pub tipo: Option<String>,
    pub localidade: Option<String>,
    pub valor_empenhado: f64,
    pub valor_pago: f64,
}

/// Estrutura resumida do perfil completo de um político, descartando metadados internos de UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetalhePoliticoFormatado {
    pub id: String,
    pub sq_candidato: Option<String>,
    pub cpf_mascarado: Option<String>,
    pub nome_completo: String,
    pub nome_eleitoral: String,
    pub partido: String,
    pub uf: String,
    pub cargo: String,
    pub municipio: Option<String>,
    pub ocupacao: Option<String>,
    pub grau_instrucao: Option<String>,
    pub score_integridade: Option<u32>,
    pub nivel_risco: String,
    pub total_gasto_ceap: f64,
    pub total_notas_ceap: i64,
    pub total_bens_declarados: f64,
    pub total_doacoes_campanha: f64,
    pub mais_registros: bool,
    pub candidaturas: Vec<CandidaturaResumoPolitico>,
    pub cargos_vinculados: Vec<CargoVinculoPolitico>,
    pub principais_doadores: Vec<DoadorResumoPolitico>,
    pub emendas: Vec<EmendaResumoPolitico>,
}

/// Converte a resposta de `/api/v1/politicos/{id}` descartando metadados de UI (fotos base64, cores hex, etc.).
pub fn parse_e_formatar_detalhe_politico(
    raw: &Value,
    fallback_id: &str,
) -> DetalhePoliticoFormatado {
    let id = extrair_id_valor(raw.get("id")).unwrap_or_else(|| fallback_id.to_string());
    let sq_candidato = raw
        .get("sq_candidato")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let cpf_mascarado = raw
        .get("cpf_mascarado")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(mascarar_cpf_estrito);

    let nome_completo = raw
        .get("nome_completo")
        .and_then(Value::as_str)
        .unwrap_or("NÃO INFORMADO")
        .to_string();

    let nome_eleitoral = raw
        .get("nome_urna")
        .or_else(|| raw.get("nome_eleitoral"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&nome_completo)
        .to_string();

    let partido = raw
        .get("partido")
        .or_else(|| raw.get("sigla_partido"))
        .and_then(Value::as_str)
        .unwrap_or("S/P")
        .to_string();

    let uf = raw
        .get("uf")
        .and_then(Value::as_str)
        .unwrap_or("--")
        .to_string();

    let cargo = raw
        .get("cargo")
        .and_then(Value::as_str)
        .unwrap_or("NÃO INFORMADO")
        .to_string();

    let municipio = raw
        .get("municipio")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let ocupacao = raw
        .get("ocupacao")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let grau_instrucao = raw
        .get("grau_instrucao")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let score_integridade = raw
        .get("score_integridade")
        .and_then(Value::as_u64)
        .map(|n| n as u32);

    let nivel_risco = raw
        .get("nivel_risco")
        .and_then(Value::as_str)
        .unwrap_or("BAIXO")
        .to_string();

    let resumo_fin = raw.get("resumo_financeiro");
    let total_gasto_ceap = resumo_fin
        .and_then(|r| r.get("total_gasto_ceap"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let total_notas_ceap = resumo_fin
        .and_then(|r| r.get("total_notas_ceap"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let total_bens_declarados = resumo_fin
        .and_then(|r| r.get("total_bens_declarados"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let total_doacoes_campanha = resumo_fin
        .and_then(|r| r.get("total_doacoes_campanha"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);

    let raw_candidaturas = raw.get("candidaturas").and_then(Value::as_array);
    let (candidaturas, cand_truncado) = if let Some(arr) = raw_candidaturas {
        let (recentes, truncou) = ordenar_e_truncar_lista(arr, MAX_CANDIDATURAS_RESUMO);
        let itens = recentes
            .into_iter()
            .map(|c| CandidaturaResumoPolitico {
                ano_eleicao: c.get("ano_eleicao").and_then(Value::as_i64).unwrap_or(0) as i32,
                cargo: c
                    .get("cargo")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                partido: c
                    .get("sigla_partido")
                    .or_else(|| c.get("partido"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                uf: c
                    .get("uf")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                municipio: c
                    .get("municipio")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                situacao_totalizacao: c
                    .get("situacao_totalizacao")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                total_bens_declarados: c
                    .get("total_bens_declarados")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0),
            })
            .collect();
        (itens, truncou)
    } else {
        (Vec::new(), false)
    };

    let raw_cargos = raw.get("cargos_autoridades").and_then(Value::as_array);
    let (cargos_vinculados, cargos_truncado) = if let Some(arr) = raw_cargos {
        let (recentes, truncou) = ordenar_e_truncar_lista(arr, MAX_CANDIDATURAS_RESUMO);
        let itens = recentes
            .into_iter()
            .map(|ca| CargoVinculoPolitico {
                orgao: ca
                    .get("orgao")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                cargo: ca
                    .get("cargo")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                esfera: ca
                    .get("esfera")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                uf: ca
                    .get("uf")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                data_posse: ca
                    .get("data_posse")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                data_exoneracao: ca
                    .get("data_exoneracao")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            })
            .collect();
        (itens, truncou)
    } else {
        (Vec::new(), false)
    };

    let raw_doadores = raw.get("doadores").and_then(Value::as_array);
    let (principais_doadores, doadores_truncado) = if let Some(arr) = raw_doadores {
        let truncou = arr.len() > MAX_DOADORES_RESUMO;
        let itens = arr
            .iter()
            .take(MAX_DOADORES_RESUMO)
            .map(|d| {
                let doc_bruto = d
                    .get("doador_cpf_cnpj")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                DoadorResumoPolitico {
                    doador_nome: d
                        .get("doador_nome")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    doador_documento: mascarar_cpf_estrito(doc_bruto),
                    valor: d.get("valor").and_then(Value::as_f64).unwrap_or(0.0),
                    ano_eleicao: d.get("ano_eleicao").and_then(Value::as_i64).unwrap_or(0) as i32,
                }
            })
            .collect();
        (itens, truncou)
    } else {
        (Vec::new(), false)
    };

    let raw_emendas = raw.get("emendas").and_then(Value::as_array);
    let (emendas, emendas_truncado) = if let Some(arr) = raw_emendas {
        let (recentes, truncou) = ordenar_e_truncar_lista(arr, MAX_EMENDAS_RESUMO);
        let itens = recentes
            .into_iter()
            .map(|e| EmendaResumoPolitico {
                codigo: e
                    .get("codigo")
                    .or_else(|| e.get("id"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                ano: e.get("ano").and_then(Value::as_i64).map(|n| n as i32),
                tipo: e
                    .get("tipo")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                localidade: e
                    .get("localidade")
                    .or_else(|| e.get("municipio"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                valor_empenhado: e
                    .get("valor_empenhado")
                    .or_else(|| e.get("valor"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0),
                valor_pago: e.get("valor_pago").and_then(Value::as_f64).unwrap_or(0.0),
            })
            .collect();
        (itens, truncou)
    } else {
        (Vec::new(), false)
    };

    let mais_registros = cand_truncado || cargos_truncado || doadores_truncado || emendas_truncado;

    DetalhePoliticoFormatado {
        id,
        sq_candidato,
        cpf_mascarado,
        nome_completo,
        nome_eleitoral,
        partido,
        uf,
        cargo,
        municipio,
        ocupacao,
        grau_instrucao,
        score_integridade,
        nivel_risco,
        total_gasto_ceap,
        total_notas_ceap,
        total_bens_declarados,
        total_doacoes_campanha,
        mais_registros,
        candidaturas,
        cargos_vinculados,
        principais_doadores,
        emendas,
    }
}

pub struct DetalhePoliticoTool;

#[async_trait]
impl McpTool for DetalhePoliticoTool {
    fn definition(&self) -> Tool {
        let schema_val = json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": ["integer", "string"],
                    "description": "Identificador do político (ID numérico interno ou sequencial de candidato como string)."
                }
            },
            "required": ["id"],
            "additionalProperties": false
        });

        let schema_obj = match schema_val {
            Value::Object(map) => map,
            _ => JsonObject::new(),
        };

        Tool::new(
            "detalhe_politico",
            "Consulta o perfil completo de um político por ID, incluindo histórico de candidaturas e vínculos conhecidos.",
            Arc::new(schema_obj),
        )
    }

    #[tracing::instrument(
        name = "tool.detalhe_politico",
        skip(self, client, arguments),
        fields(
            ferramenta = "detalhe_politico",
            id = tracing::field::Empty,
            elapsed_ms = tracing::field::Empty
        )
    )]
    async fn call(
        &self,
        client: &RadarHttpClient,
        arguments: JsonObject,
    ) -> Result<CallToolResult, McpError> {
        let inicio = Instant::now();
        let input: DetalhePoliticoInput = match serde_json::from_value(Value::Object(arguments)) {
            Ok(val) => val,
            Err(err) => {
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                warn!(elapsed_ms, erro = %err, "Parâmetros de entrada inválidos para 'detalhe_politico'");
                let msg = format_deserialization_error_json("detalhe_politico", &err);
                return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]));
            }
        };

        let id_clean = input.id.trim();
        if id_clean.is_empty() {
            let elapsed_ms = inicio.elapsed().as_millis() as u64;
            tracing::Span::current().record("elapsed_ms", elapsed_ms);
            warn!(elapsed_ms, "Parâmetro 'id' vazio em 'detalhe_politico'");
            let msg = format_validation_error_json(
                "detalhe_politico",
                "O parâmetro obrigatório 'id' não pode ser vazio.",
            );
            return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]));
        }

        let id_seguro = sanitizar_parametro_log(id_clean);
        tracing::Span::current().record("id", id_seguro.as_str());
        info!(id = %id_seguro, "Consultando detalhe do político na API local");

        let encoded_id = urlencoding::encode(id_clean);
        let path = format!("/api/v1/politicos/{encoded_id}");
        match client.get_json(&path, &[]).await {
            Ok(raw_json) => {
                let formatado = parse_e_formatar_detalhe_politico(&raw_json, id_clean);
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                info!(
                    id = %id_seguro,
                    candidaturas = formatado.candidaturas.len(),
                    elapsed_ms,
                    "Detalhe do político formatado com sucesso"
                );
                let texto = serializar_json_compacto(&formatado);
                Ok(CallToolResult::success(vec![ContentBlock::text(texto)]))
            }
            Err(err) => {
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                warn!(
                    id = %id_seguro,
                    elapsed_ms,
                    erro = %err,
                    "Falha na chamada à API durante 'detalhe_politico'"
                );
                Ok(tratar_erro_http_tool(
                    "detalhe_politico",
                    client.base_url(),
                    &err,
                ))
            }
        }
    }
}

// ============================================================================
// 3. Ferramenta `dossie_cpf`
// ============================================================================

/// Parâmetros de entrada da ferramenta `dossie_cpf`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DossieCpfInput {
    /// CPF da pessoa física (com ou sem pontuação).
    pub cpf: String,
}

impl DossieCpfInput {
    /// Retorna os dígitos numéricos do CPF sem pontuação.
    pub fn cpf_sanitizado(&self) -> String {
        sanitizar_cpf(&self.cpf)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SociedadeCpfResumo {
    pub cnpj: String,
    pub razao_social: String,
    pub qualificacao: Option<String>,
    pub confianca_vinculo: String,
    pub total_ceap: f64,
    pub total_pncp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DoacaoCpfResumo {
    pub politico_nome: String,
    pub partido: String,
    pub cargo: String,
    pub ano_eleicao: i32,
    pub valor: f64,
    pub confianca_vinculo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaCpfResumo {
    pub tipo: String,
    pub severidade: String,
    pub titulo: String,
    pub descricao: String,
    pub valor_envolvido: Option<f64>,
    pub fonte: String,
}

/// Dossiê consolidado de pessoa física por CPF com garantia estrita de mascaramento `***.XXX.XXX-**`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DossieCpfFormatado {
    pub cpf_mascarado: String,
    pub nome: String,
    pub score_risco: String,
    pub total_faturado_empresas_ceap: f64,
    pub total_contratado_empresas_pncp: f64,
    pub mais_registros: bool,
    pub total_empresas_socio: usize,
    pub sociedades_empresariais: Vec<SociedadeCpfResumo>,
    pub total_doacoes_eleitorais: usize,
    pub doacoes_eleitorais: Vec<DoacaoCpfResumo>,
    pub total_alertas: usize,
    pub alertas: Vec<AlertaCpfResumo>,
}

/// Converte o payload de `/api/v1/dossie/cpf/{cpf}` garantindo que o CPF seja sempre mascarado
/// no padrão `***.XXX.XXX-**` mesmo se o endpoint retornar o documento sem máscara.
pub fn parse_e_formatar_dossie_cpf(raw: &Value, fallback_cpf: &str) -> DossieCpfFormatado {
    let raw_cpf = raw
        .get("cpf_mascarado")
        .or_else(|| raw.get("cpf"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback_cpf);

    let cpf_mascarado = mascarar_cpf_estrito(raw_cpf);

    let nome = raw
        .get("nome")
        .and_then(Value::as_str)
        .unwrap_or("NÃO IDENTIFICADO")
        .to_string();

    let score_risco = raw
        .get("score_risco")
        .and_then(Value::as_str)
        .unwrap_or("BAIXO")
        .to_string();

    let total_faturado_empresas_ceap = raw
        .get("total_faturado_empresas_ceap")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);

    let total_contratado_empresas_pncp = raw
        .get("total_contratado_empresas_pncp")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);

    let raw_empresas = raw.get("empresas_socio").and_then(Value::as_array);
    let total_empresas_socio = raw_empresas.map(|a| a.len()).unwrap_or(0);
    let sociedades_empresariais = raw_empresas
        .map(|arr| {
            arr.iter()
                .take(MAX_EMPRESAS_CPF_RESUMO)
                .map(|e| SociedadeCpfResumo {
                    cnpj: e
                        .get("cnpj_formatado")
                        .or_else(|| e.get("cnpj"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    razao_social: e
                        .get("razao_social")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    qualificacao: e
                        .get("qualificacao")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    confianca_vinculo: e
                        .get("confianca_vinculo")
                        .and_then(Value::as_str)
                        .unwrap_or("MEDIA")
                        .to_string(),
                    total_ceap: e.get("total_ceap").and_then(Value::as_f64).unwrap_or(0.0),
                    total_pncp: e.get("total_pncp").and_then(Value::as_f64).unwrap_or(0.0),
                })
                .collect()
        })
        .unwrap_or_default();

    let raw_doacoes = raw.get("doacoes_eleitorais").and_then(Value::as_array);
    let total_doacoes_eleitorais = raw_doacoes.map(|a| a.len()).unwrap_or(0);
    let doacoes_eleitorais = raw_doacoes
        .map(|arr| {
            arr.iter()
                .take(MAX_DOACOES_CPF_RESUMO)
                .map(|d| DoacaoCpfResumo {
                    politico_nome: d
                        .get("politico_nome")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    partido: d
                        .get("partido")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    cargo: d
                        .get("cargo")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    ano_eleicao: d.get("ano_eleicao").and_then(Value::as_i64).unwrap_or(0) as i32,
                    valor: d.get("valor").and_then(Value::as_f64).unwrap_or(0.0),
                    confianca_vinculo: d
                        .get("confianca_vinculo")
                        .and_then(Value::as_str)
                        .unwrap_or("MEDIA")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    let raw_alertas = raw.get("alertas").and_then(Value::as_array);
    let total_alertas = raw
        .get("total_alertas")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or_else(|| raw_alertas.map(|a| a.len()).unwrap_or(0));

    let alertas = raw_alertas
        .map(|arr| {
            arr.iter()
                .take(MAX_ALERTAS_CPF_RESUMO)
                .map(|a| {
                    let desc_bruta = a.get("descricao").and_then(Value::as_str).unwrap_or("");
                    AlertaCpfResumo {
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
                        descricao: truncar_texto(desc_bruta, MAX_DESCRICAO_CHARS),
                        valor_envolvido: a.get("valor_envolvido").and_then(Value::as_f64),
                        fonte: a
                            .get("fonte")
                            .and_then(Value::as_str)
                            .unwrap_or("Radar Cívico")
                            .to_string(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let mais_registros = total_empresas_socio > MAX_EMPRESAS_CPF_RESUMO
        || total_doacoes_eleitorais > MAX_DOACOES_CPF_RESUMO
        || total_alertas > MAX_ALERTAS_CPF_RESUMO;

    DossieCpfFormatado {
        cpf_mascarado,
        nome,
        score_risco,
        total_faturado_empresas_ceap,
        total_contratado_empresas_pncp,
        mais_registros,
        total_empresas_socio,
        sociedades_empresariais,
        total_doacoes_eleitorais,
        doacoes_eleitorais,
        total_alertas,
        alertas,
    }
}

pub struct DossieCpfTool;

#[async_trait]
impl McpTool for DossieCpfTool {
    fn definition(&self) -> Tool {
        let schema_val = json!({
            "type": "object",
            "properties": {
                "cpf": {
                    "type": "string",
                    "description": "CPF da pessoa física (com ou sem pontuação; pontos e traços serão removidos automaticamente)."
                }
            },
            "required": ["cpf"],
            "additionalProperties": false
        });

        let schema_obj = match schema_val {
            Value::Object(map) => map,
            _ => JsonObject::new(),
        };

        Tool::new(
            "dossie_cpf",
            "Consulta o dossiê consolidado de pessoa física por CPF. Retorna sociedades empresariais, doações e alertas vinculados.",
            Arc::new(schema_obj),
        )
    }

    #[tracing::instrument(
        name = "tool.dossie_cpf",
        skip(self, client, arguments),
        fields(
            ferramenta = "dossie_cpf",
            cpf = tracing::field::Empty,
            elapsed_ms = tracing::field::Empty
        )
    )]
    async fn call(
        &self,
        client: &RadarHttpClient,
        arguments: JsonObject,
    ) -> Result<CallToolResult, McpError> {
        let inicio = Instant::now();
        let input: DossieCpfInput = match serde_json::from_value(Value::Object(arguments)) {
            Ok(val) => val,
            Err(err) => {
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                warn!(elapsed_ms, erro = %err, "Parâmetros de entrada inválidos para 'dossie_cpf'");
                let msg = format_deserialization_error_json("dossie_cpf", &err);
                return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]));
            }
        };

        let cpf_limpo = input.cpf_sanitizado();
        if cpf_limpo.is_empty() {
            let elapsed_ms = inicio.elapsed().as_millis() as u64;
            tracing::Span::current().record("elapsed_ms", elapsed_ms);
            warn!(elapsed_ms, "Parâmetro 'cpf' vazio em 'dossie_cpf'");
            let msg = format_validation_error_json(
                "dossie_cpf",
                "O parâmetro obrigatório 'cpf' não contém dígitos válidos.",
            );
            return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]));
        }

        // Garante que nos logs/spans o CPF nunca apareça íntegro, apenas mascarado (***.XXX.XXX-**)
        let cpf_log = mascarar_cpf_estrito(&cpf_limpo);
        tracing::Span::current().record("cpf", cpf_log.as_str());
        info!(cpf = %cpf_log, "Consultando dossiê CPF na API local");

        let path = format!("/api/v1/dossie/cpf/{cpf_limpo}");
        match client.get_json(&path, &[]).await {
            Ok(raw_json) => {
                let formatado = parse_e_formatar_dossie_cpf(&raw_json, &cpf_limpo);
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                info!(
                    cpf = %cpf_log,
                    sociedades = formatado.total_empresas_socio,
                    alertas = formatado.total_alertas,
                    elapsed_ms,
                    "Dossiê CPF formatado com sucesso"
                );
                let texto = serializar_json_compacto(&formatado);
                Ok(CallToolResult::success(vec![ContentBlock::text(texto)]))
            }
            Err(err) => {
                let elapsed_ms = inicio.elapsed().as_millis() as u64;
                tracing::Span::current().record("elapsed_ms", elapsed_ms);
                warn!(
                    cpf = %cpf_log,
                    elapsed_ms,
                    erro = %err,
                    "Falha na chamada à API durante 'dossie_cpf'"
                );
                Ok(tratar_erro_http_tool("dossie_cpf", client.base_url(), &err))
            }
        }
    }
}

// ============================================================================
// Auxiliares internos
// ============================================================================

fn extrair_id_valor(val: Option<&Value>) -> Option<String> {
    let v = val?;
    if let Some(n) = v.as_i64() {
        return Some(n.to_string());
    }
    if let Some(u) = v.as_u64() {
        return Some(u.to_string());
    }
    if let Some(s) = v.as_str() {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mascaramento_estrito_cpf_em_todos_os_formatos() {
        // CPF completo sem máscara (11 dígitos) -> ***.XXX.XXX-**
        assert_eq!(mascarar_cpf_estrito("12345678901"), "***.456.789-**");
        // CPF completo com pontuação -> ***.XXX.XXX-**
        assert_eq!(mascarar_cpf_estrito("123.456.789-01"), "***.456.789-**");
        // CPF já mascarado -> mantém ***.XXX.XXX-**
        assert_eq!(mascarar_cpf_estrito("***.456.789-**"), "***.456.789-**");
        // Miolo de 6 dígitos -> ***.XXX.XXX-**
        assert_eq!(mascarar_cpf_estrito("456789"), "***.456.789-**");
        // CNPJ (14 dígitos) permanece preservado
        assert_eq!(mascarar_cpf_estrito("12345678000190"), "12345678000190");
    }

    #[test]
    fn test_listar_politicos_input_e_parsing_saida_compacta() {
        let input_json = json!({
            "uf": "sp",
            "partido": "pt",
            "pagina": 2,
            "limite": 80
        });
        let input: ListarPoliticosInput = serde_json::from_value(input_json).unwrap();
        assert_eq!(input.uf_normalizada().as_deref(), Some("SP"));
        assert_eq!(input.partido_normalizado().as_deref(), Some("PT"));
        assert_eq!(input.pagina_efetiva(), 2);
        assert_eq!(input.limite_efetivo(), 50); // truncado ao máximo 50

        let mock_api = json!({
            "total": 1,
            "page": 2,
            "limit": 50,
            "politicos": [
                {
                    "id": 101,
                    "nome_completo": "MARIA DA SILVA",
                    "nome_urna": "MARIA SILVA",
                    "sigla_partido": "PT",
                    "uf": "SP",
                    "cargo": "DEPUTADO FEDERAL",
                    "foto_base64": "AAAA_METADADO_PESADO_DE_UI"
                }
            ]
        });

        let saida = parse_e_formatar_lista_politicos(&mock_api, 2, 50);
        assert_eq!(saida.pagina, 2);
        assert_eq!(saida.politicos.len(), 1);
        assert_eq!(
            saida.politicos[0],
            ItemPoliticoCompacto {
                id: "101".to_string(),
                nome_eleitoral: "MARIA SILVA".to_string(),
                cargo: "DEPUTADO FEDERAL".to_string(),
                partido: "PT".to_string(),
                uf: "SP".to_string(),
            }
        );
    }

    #[test]
    fn test_detalhe_politico_aceita_id_int_ou_string_e_descarta_ui() {
        let input_int: DetalhePoliticoInput = serde_json::from_value(json!({ "id": 77 })).unwrap();
        assert_eq!(input_int.id, "77");

        let input_str: DetalhePoliticoInput =
            serde_json::from_value(json!({ "id": "28000123" })).unwrap();
        assert_eq!(input_str.id, "28000123");

        let mock_detalhe = json!({
            "id": 77,
            "sq_candidato": "28000123",
            "cpf_mascarado": "11122233344",
            "nome_completo": "CARLOS EXEMPLO",
            "nome_urna": "CARLOS",
            "partido": "MDB",
            "uf": "RJ",
            "cargo": "SENADOR",
            "foto_base64": "BASE64_GIGANTE_UI",
            "cor_risco_hex": "#ff0000",
            "resumo_financeiro": {
                "total_gasto_ceap": 12000.0,
                "total_notas_ceap": 4,
                "total_bens_declarados": 500000.0,
                "total_doacoes_campanha": 80000.0
            },
            "candidaturas": [
                {
                    "ano_eleicao": 2022,
                    "cargo": "SENADOR",
                    "sigla_partido": "MDB",
                    "uf": "RJ",
                    "municipio": null,
                    "situacao_totalizacao": "ELEITO",
                    "total_bens_declarados": 500000.0
                }
            ],
            "cargos_autoridades": [],
            "doadores": [
                {
                    "doador_nome": "JOSE DOADOR",
                    "doador_cpf_cnpj": "999.888.777-66",
                    "valor": 5000.0,
                    "ano_eleicao": 2022
                }
            ]
        });

        let detalhe = parse_e_formatar_detalhe_politico(&mock_detalhe, "77");
        assert_eq!(detalhe.id, "77");
        assert_eq!(detalhe.cpf_mascarado.as_deref(), Some("***.222.333-**"));
        assert_eq!(
            detalhe.principais_doadores[0].doador_documento,
            "***.888.777-**"
        );

        let json_out = serde_json::to_value(&detalhe).unwrap();
        assert!(json_out.get("foto_base64").is_none());
        assert!(json_out.get("cor_risco_hex").is_none());
    }

    #[test]
    fn test_dossie_cpf_aplica_mascaramento_estrito_quando_endpoint_retorna_sem_mascara() {
        let input: DossieCpfInput =
            serde_json::from_value(json!({ "cpf": "123.456.789-01" })).unwrap();
        assert_eq!(input.cpf_sanitizado(), "12345678901");

        // Simula endpoint devolvendo o CPF sem máscara ("12345678901")
        let mock_dossie_cpf = json!({
            "cpf_mascarado": "12345678901",
            "nome": "FULANO DE TAL",
            "score_risco": "MEDIO",
            "total_faturado_empresas_ceap": 10000.0,
            "total_contratado_empresas_pncp": 0.0,
            "empresas_socio": [
                {
                    "cnpj": "12345678000190",
                    "cnpj_formatado": "12.345.678/0001-90",
                    "razao_social": "EMPRESA TAL LTDA",
                    "qualificacao": "Sócio",
                    "confianca_vinculo": "ALTA",
                    "total_ceap": 10000.0,
                    "total_pncp": 0.0
                }
            ],
            "doacoes_eleitorais": [],
            "total_alertas": 0,
            "alertas": []
        });

        let dossie = parse_e_formatar_dossie_cpf(&mock_dossie_cpf, "12345678901");
        assert_eq!(dossie.cpf_mascarado, "***.456.789-**");
        assert_eq!(dossie.nome, "FULANO DE TAL");
        assert_eq!(dossie.sociedades_empresariais.len(), 1);
    }
}
