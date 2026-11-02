use serde::Serialize;
use serde_json::{Map, Value};

/// Limite padrão de itens por sub-lista (ex.: contratos, emendas, sócios) para proteção de contexto.
pub const DEFAULT_MAX_ITENS_SUBLISTA: usize = 10;

/// Chaves de auditoria e controle interno de ETL/banco irrelevantes para o raciocínio da IA.
pub const CHAVES_AUDITORIA_INTERNA: &[&str] = &[
    "created_at",
    "updated_at",
    "hash_linha",
    "versao_parser",
    "criado_em",
    "atualizado_em",
    "deleted_at",
    "hash_arquivo",
    "id_interno_etl",
    "foto_base64",
    "cor_risco_hex",
];

/// Chaves de data/ano utilizadas para priorizar os registros mais recentes antes do truncamento.
const CHAVES_ORDENACAO_RECENTE: &[&str] = &[
    "data_assinatura",
    "data_publicacao",
    "data_emissao",
    "data_documento",
    "data",
    "ano_eleicao",
    "ano_exercicio",
    "ano",
];

/// Verifica se o nome de uma propriedade corresponde a metadados de auditoria interna.
pub fn eh_chave_auditoria_interna(chave: &str) -> bool {
    let normalizada = chave.trim().to_ascii_lowercase();
    CHAVES_AUDITORIA_INTERNA.contains(&normalizada.as_str())
}

/// Remove recursivamente:
/// - chaves de auditoria interna (`created_at`, `updated_at`, `hash_linha`, `versao_parser`, etc.);
/// - campos com valor `null`;
/// - strings vazias `""`;
/// - arrays vazios `[]`;
/// - objetos que fiquem vazios `{}` após a poda recursiva.
pub fn podar_valor_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let keys_to_remove: Vec<String> = map
                .iter()
                .filter_map(|(k, _)| {
                    if eh_chave_auditoria_interna(k) {
                        Some(k.clone())
                    } else {
                        None
                    }
                })
                .collect();

            for k in keys_to_remove {
                map.remove(&k);
            }

            for val in map.values_mut() {
                podar_valor_json(val);
            }

            map.retain(|_, v| !valor_deve_ser_podado(v));
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                podar_valor_json(item);
            }
            arr.retain(|item| !valor_deve_ser_podado(item));
        }
        _ => {}
    }
}

fn valor_deve_ser_podado(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(arr) => arr.is_empty(),
        Value::Object(map) => map.is_empty(),
        _ => false,
    }
}

/// Extrai uma chave textual comparável (ISO-8601 ou ano) de um item de lista para ordenação decrescente
/// (mais recentes primeiro), caso o item seja um objeto com campos de data/ano.
fn extrair_chave_temporal(item: &Value) -> Option<String> {
    let obj = item.as_object()?;
    for campo in CHAVES_ORDENACAO_RECENTE {
        if let Some(val) = obj.get(*campo) {
            if let Some(s) = val.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(normalizar_data_para_ordenacao(trimmed));
                }
            }
            if let Some(n) = val.as_i64() {
                return Some(format!("{n:08}"));
            }
            if let Some(u) = val.as_u64() {
                return Some(format!("{u:08}"));
            }
        }
    }
    None
}

/// Converte datas `DD/MM/AAAA` para `AAAA-MM-DD` para comparação lexicográfica consistente.
pub(crate) fn normalizar_data_para_ordenacao(raw: &str) -> String {
    let bytes = raw.as_bytes();
    if bytes.len() == 10 && bytes[2] == b'/' && bytes[5] == b'/' {
        let dia = &raw[0..2];
        let mes = &raw[3..5];
        let ano = &raw[6..10];
        if dia.chars().all(|c| c.is_ascii_digit())
            && mes.chars().all(|c| c.is_ascii_digit())
            && ano.chars().all(|c| c.is_ascii_digit())
        {
            return format!("{ano}-{mes}-{dia}");
        }
    }
    raw.to_string()
}

/// Ordena (priorizando itens mais recentes quando possuem campo de data/ano) e trunca um slice
/// de `Value` ao limite máximo informado, retornando `(itens_truncados, houve_excedente)`.
pub fn ordenar_e_truncar_lista(lista: &[Value], max_itens: usize) -> (Vec<Value>, bool) {
    let limite = if max_itens == 0 {
        DEFAULT_MAX_ITENS_SUBLISTA
    } else {
        max_itens
    };

    let mut copia = lista.to_vec();
    let possui_datas = copia
        .iter()
        .any(|item| extrair_chave_temporal(item).is_some());
    if possui_datas {
        copia.sort_by(|a, b| {
            let ka = extrair_chave_temporal(a).unwrap_or_default();
            let kb = extrair_chave_temporal(b).unwrap_or_default();
            kb.cmp(&ka)
        });
    }

    let houve_excedente = copia.len() > limite;
    if houve_excedente {
        copia.truncate(limite);
    }
    (copia, houve_excedente)
}

/// Trunca recursivamente todas as sub-listas (`Value::Array`) dentro de objetos JSON que excedam
/// `max_itens`, priorizando os registros mais recentes e injetando `"mais_registros": true`
/// no objeto pai sempre que houver corte de elementos excedentes.
pub fn truncar_sublistas_json(value: &mut Value, max_itens: usize) -> bool {
    let limite = if max_itens == 0 {
        DEFAULT_MAX_ITENS_SUBLISTA
    } else {
        max_itens
    };

    match value {
        Value::Object(map) => {
            let mut truncou_neste_objeto = false;

            for val in map.values_mut() {
                match val {
                    Value::Array(arr) => {
                        if arr.len() > limite {
                            let (truncados, _) = ordenar_e_truncar_lista(arr, limite);
                            *arr = truncados;
                            truncou_neste_objeto = true;
                        }
                        for elem in arr.iter_mut() {
                            if truncar_sublistas_json(elem, limite) {
                                truncou_neste_objeto = true;
                            }
                        }
                    }
                    Value::Object(_) => {
                        truncou_neste_objeto |= truncar_sublistas_json(val, limite);
                    }
                    _ => {}
                }
            }

            if truncou_neste_objeto {
                map.insert("mais_registros".to_string(), Value::Bool(true));
            }

            truncou_neste_objeto
        }
        Value::Array(arr) => {
            let mut truncou = false;
            if arr.len() > limite {
                let (truncados, _) = ordenar_e_truncar_lista(arr, limite);
                *arr = truncados;
                truncou = true;
            }
            for elem in arr.iter_mut() {
                if truncar_sublistas_json(elem, limite) {
                    truncou = true;
                }
            }
            truncou
        }
        _ => false,
    }
}

/// Pipeline completo de sanitização e compactação de um `serde_json::Value` para consumo da LLM:
/// 1. Remove recursivamente chaves de auditoria interna (`created_at`, `updated_at`, `hash_linha`, `versao_parser`),
///    campos `null`, strings vazias `""` e arrays vazios `[]`.
/// 2. Trunca sub-listas extensas (ex.: contratos, emendas, sócios) ao limite `max_itens_sublista`,
///    mantendo os itens mais recentes e sinalizando `"mais_registros": true`.
pub fn compactar_json_para_llm(mut value: Value, max_itens_sublista: usize) -> Value {
    podar_valor_json(&mut value);
    truncar_sublistas_json(&mut value, max_itens_sublista);
    // Caso o Value raiz seja um Object vazio após poda total, garante um objeto JSON válido `{}`
    if value.is_null() {
        Value::Object(Map::new())
    } else {
        value
    }
}

/// Serializa qualquer estrutura `Serialize` em JSON minificado (`serde_json::to_string`) após
/// aplicar a poda recursiva de campos `null`, strings vazias `""`, arrays vazios `[]` e chaves de auditoria interna.
pub fn serializar_json_compacto<T: Serialize>(valor: &T) -> String {
    match serde_json::to_value(valor) {
        Ok(mut val) => {
            podar_valor_json(&mut val);
            serde_json::to_string(&val).unwrap_or_else(|_| "{}".to_string())
        }
        Err(_) => "{}".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_poda_recursiva_de_nulos_strings_vazias_arrays_vazios_e_chaves_auditoria() {
        let mut payload = json!({
            "id": 10,
            "nome": "EMPRESA ALFA LTDA",
            "fantasia": "",
            "observacao": null,
            "tags_vazias": [],
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-02-01T00:00:00Z",
            "hash_linha": "abc123def456",
            "versao_parser": "v2.1.0",
            "detalhes": {
                "uf": "SP",
                "complemento": "",
                "inativo_desde": null,
                "historico_vazio": [],
                "created_at": "2024-10-10",
                "objeto_ apenas_nulo": {
                    "a": null,
                    "b": "",
                    "c": []
                }
            },
            "socios": [
                {
                    "nome": "MARIA SOCIA",
                    "qualificacao": "",
                    "email": null,
                    "hash_linha": "deadbeef",
                    "versao_parser": "1.0"
                },
                null,
                ""
            ]
        });

        podar_valor_json(&mut payload);

        let obj = payload
            .as_object()
            .expect("raiz deve continuar sendo objeto");
        assert_eq!(obj.get("id"), Some(&json!(10)));
        assert_eq!(obj.get("nome"), Some(&json!("EMPRESA ALFA LTDA")));

        // Campos nulos, vazios e de auditoria interna devem ter sido removidos
        assert!(!obj.contains_key("fantasia"));
        assert!(!obj.contains_key("observacao"));
        assert!(!obj.contains_key("tags_vazias"));
        assert!(!obj.contains_key("created_at"));
        assert!(!obj.contains_key("updated_at"));
        assert!(!obj.contains_key("hash_linha"));
        assert!(!obj.contains_key("versao_parser"));

        let detalhes = obj.get("detalhes").unwrap().as_object().unwrap();
        assert_eq!(detalhes.get("uf"), Some(&json!("SP")));
        assert!(!detalhes.contains_key("complemento"));
        assert!(!detalhes.contains_key("inativo_desde"));
        assert!(!detalhes.contains_key("historico_vazio"));
        assert!(!detalhes.contains_key("created_at"));
        assert!(!detalhes.contains_key("objeto_ apenas_nulo"));

        let socios = obj.get("socios").unwrap().as_array().unwrap();
        assert_eq!(socios.len(), 1);
        let socio = socios[0].as_object().unwrap();
        assert_eq!(socio.get("nome"), Some(&json!("MARIA SOCIA")));
        assert!(!socio.contains_key("qualificacao"));
        assert!(!socio.contains_key("email"));
        assert!(!socio.contains_key("hash_linha"));
        assert!(!socio.contains_key("versao_parser"));
    }

    #[test]
    fn test_truncamento_de_sublistas_mantem_10_mais_recentes_e_sinaliza_mais_registros() {
        let mut contratos = Vec::new();
        for dia in 1..=15 {
            contratos.push(json!({
                "numero": format!("CTR-{dia:02}"),
                "data_assinatura": format!("2024-05-{dia:02}"),
                "ano": 2024,
                "valor": 10000.0 * (dia as f64),
                "created_at": "2025-01-01T12:00:00Z",
                "observacao": ""
            }));
        }

        let raw = json!({
            "cnpj": "00000000000191",
            "contratos": contratos
        });

        let compactado = compactar_json_para_llm(raw, 10);
        assert_eq!(compactado["mais_registros"], json!(true));

        let lista_contratos = compactado["contratos"].as_array().unwrap();
        assert_eq!(lista_contratos.len(), 10);

        // Verifica que os contratos mais recentes foram priorizados (mes=15 -> ano 2035 primeiro)
        assert_eq!(lista_contratos[0]["numero"], "CTR-15");
        assert_eq!(lista_contratos[9]["numero"], "CTR-06");

        // Chaves internas e vazias dentro dos itens truncados também foram podadas
        assert!(lista_contratos[0].get("created_at").is_none());
        assert!(lista_contratos[0].get("observacao").is_none());

        // Verifica serialização minificada sem quebras de linha ou espaços desnecessários
        let minificado = serializar_json_compacto(&compactado);
        assert!(!minificado.contains('\n'));
        assert!(minificado.contains("\"mais_registros\":true"));
    }
}
