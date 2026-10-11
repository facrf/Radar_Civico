use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContratoEmpresaSocio {
    pub contrato_id: i64,
    pub orgao_contratante: String,
    pub empresa_cnpj: String,
    pub empresa_razao_social: String,
    pub valor_contratado: f64,
    pub data_assinatura: String,
    pub socio_cpf_mascarado: String,
    pub socio_nome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaConluioLicitacao {
    pub orgao_contratante: String,
    pub socio_nome: String,
    pub socio_cpf_mascarado: String,
    pub cnpjs_envolvidos: Vec<String>,
    pub empresas_envolvidas: Vec<String>,
    pub valor_total_contratado: f64,
    pub quantidade_contratos: usize,
    pub gravidade: String,
    pub motivo: String,
}

pub fn auditar_socios_comuns_contratos(
    contratos: &[ContratoEmpresaSocio],
) -> crate::Result<Vec<AlertaConluioLicitacao>> {
    // Agrupa por (órgão contratante, chave do sócio)
    let mut grupos: HashMap<(String, String), Vec<&ContratoEmpresaSocio>> = HashMap::new();

    for c in contratos {
        let orgao = c.orgao_contratante.trim().to_uppercase();
        let socio_key = if !c.socio_cpf_mascarado.trim().is_empty() {
            c.socio_cpf_mascarado.trim().to_string()
        } else {
            c.socio_nome.trim().to_uppercase()
        };

        if orgao.is_empty() || socio_key.is_empty() {
            continue;
        }

        grupos.entry((orgao, socio_key)).or_default().push(c);
    }

    let mut alertas = Vec::new();

    for ((orgao, _), itens) in grupos {
        let mut cnpjs: HashSet<String> = HashSet::new();
        let mut empresas: HashSet<String> = HashSet::new();
        let mut total_valor = storage::Money::ZERO;
        let mut socio_nome = String::new();
        let mut socio_cpf = String::new();

        for item in &itens {
            cnpjs.insert(item.empresa_cnpj.clone());
            empresas.insert(item.empresa_razao_social.clone());
            total_valor =
                total_valor.checked_add(storage::Money::from_reais(item.valor_contratado)?)?;
            if socio_nome.is_empty() {
                socio_nome = item.socio_nome.clone();
                socio_cpf = item.socio_cpf_mascarado.clone();
            }
        }

        let total_valor = total_valor.reais();
        // Alerta apenas quando o sócio atua em pelo menos 2 CNPJs distintos contratados pelo mesmo órgão
        if cnpjs.len() >= 2 {
            let gravidade = if total_valor >= 1_000_000.0 || cnpjs.len() >= 3 {
                "CRITICA"
            } else {
                "ALTA"
            };

            let mut cnpjs_vec: Vec<String> = cnpjs.into_iter().collect();
            cnpjs_vec.sort();

            let mut empresas_vec: Vec<String> = empresas.into_iter().collect();
            empresas_vec.sort();

            let motivo = format!(
                "O sócio/administrador '{}' figura no QSA de {} empresas distintas ({}) contratadas pelo mesmo órgão ('{}'), totalizando R$ {:.2} em contratos (indício de grupo econômico velado ou concentração licitatória).",
                socio_nome,
                cnpjs_vec.len(),
                empresas_vec.join(", "),
                orgao,
                total_valor
            );

            alertas.push(AlertaConluioLicitacao {
                orgao_contratante: orgao,
                socio_nome,
                socio_cpf_mascarado: socio_cpf,
                cnpjs_envolvidos: cnpjs_vec,
                empresas_envolvidas: empresas_vec,
                valor_total_contratado: (total_valor * 100.0).round() / 100.0,
                quantidade_contratos: itens.len(),
                gravidade: gravidade.to_string(),
                motivo,
            });
        }
    }

    alertas.sort_by(|a, b| {
        b.valor_total_contratado
            .partial_cmp(&a.valor_total_contratado)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(alertas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cartel_detecta_socios_comuns_mesmo_orgao() {
        let contratos = vec![
            ContratoEmpresaSocio {
                contrato_id: 1,
                orgao_contratante: "PREFEITURA DE CAMPINAS".to_string(),
                empresa_cnpj: "11111111000101".to_string(),
                empresa_razao_social: "ALFA ENGENHARIA LTDA".to_string(),
                valor_contratado: 600_000.0,
                data_assinatura: "2024-03-01".to_string(),
                socio_cpf_mascarado: "***.777.888-**".to_string(),
                socio_nome: "CARLOS ANDRADE".to_string(),
            },
            ContratoEmpresaSocio {
                contrato_id: 2,
                orgao_contratante: "PREFEITURA DE CAMPINAS".to_string(),
                empresa_cnpj: "22222222000102".to_string(), // CNPJ Diferente
                empresa_razao_social: "BETA CONSTRUCOES LTDA".to_string(),
                valor_contratado: 500_000.0,
                data_assinatura: "2024-04-10".to_string(),
                socio_cpf_mascarado: "***.777.888-**".to_string(), // Mesmo Sócio
                socio_nome: "CARLOS ANDRADE".to_string(),
            },
        ];

        let alertas = auditar_socios_comuns_contratos(&contratos).unwrap();
        assert_eq!(alertas.len(), 1);
        let a = &alertas[0];
        assert_eq!(a.orgao_contratante, "PREFEITURA DE CAMPINAS");
        assert_eq!(a.cnpjs_envolvidos.len(), 2);
        assert_eq!(a.valor_total_contratado, 1_100_000.0);
        assert_eq!(a.gravidade, "CRITICA");
    }

    #[test]
    fn test_cartel_caso_controle_empresas_com_socios_distintos() {
        let contratos = vec![
            ContratoEmpresaSocio {
                contrato_id: 1,
                orgao_contratante: "PREFEITURA DE CAMPINAS".to_string(),
                empresa_cnpj: "11111111000101".to_string(),
                empresa_razao_social: "ALFA ENGENHARIA LTDA".to_string(),
                valor_contratado: 600_000.0,
                data_assinatura: "2024-03-01".to_string(),
                socio_cpf_mascarado: "***.111.222-**".to_string(),
                socio_nome: "SOCIO UM".to_string(),
            },
            ContratoEmpresaSocio {
                contrato_id: 2,
                orgao_contratante: "PREFEITURA DE CAMPINAS".to_string(),
                empresa_cnpj: "22222222000102".to_string(),
                empresa_razao_social: "BETA CONSTRUCOES LTDA".to_string(),
                valor_contratado: 500_000.0,
                data_assinatura: "2024-04-10".to_string(),
                socio_cpf_mascarado: "***.333.444-**".to_string(),
                socio_nome: "SOCIO DOIS".to_string(),
            },
        ];

        let alertas = auditar_socios_comuns_contratos(&contratos).unwrap();
        assert!(alertas.is_empty());
    }
}
