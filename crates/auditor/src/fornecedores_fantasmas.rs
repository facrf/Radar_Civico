use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

pub const DIAS_LIMITE_RECEM_CRIADA: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FornecedorReceita {
    pub cnpj: String,
    pub razao_social: String,
    pub data_abertura: NaiveDate,
    pub situacao_cadastral: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagamentoFornecedor {
    pub id: i64,
    pub fornecedor_cnpj: String,
    pub fornecedor_nome: String,
    pub valor: f64,
    pub data_pagamento: NaiveDate,
    pub origem: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TipoIrregularidadeFornecedor {
    SituacaoInapta,
    RecemCriada,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaFornecedorFantasma {
    pub pagamento_id: i64,
    pub fornecedor_cnpj: String,
    pub fornecedor_nome: String,
    pub tipo_irregularidade: TipoIrregularidadeFornecedor,
    pub motivo: String,
    pub gravidade: String,
}

fn limpar_cnpj(cnpj: &str) -> String {
    cnpj.chars().filter(|c| c.is_ascii_digit()).collect()
}

pub fn auditar_fornecedores_fantasmas(
    pagamentos: &[PagamentoFornecedor],
    cadastro_receita: &[FornecedorReceita],
) -> Vec<AlertaFornecedorFantasma> {
    let mut alertas = Vec::new();

    for pag in pagamentos {
        let pag_cnpj_clean = limpar_cnpj(&pag.fornecedor_cnpj);

        if let Some(forn) = cadastro_receita
            .iter()
            .find(|f| limpar_cnpj(&f.cnpj) == pag_cnpj_clean)
        {
            let situacao = forn.situacao_cadastral.trim().to_uppercase();

            // 1. Situação cadastral irregular na Receita Federal
            if situacao == "INAPTA" || situacao == "BAIXADA" || situacao == "SUSPENSA" || situacao == "NULA" {
                alertas.push(AlertaFornecedorFantasma {
                    pagamento_id: pag.id,
                    fornecedor_cnpj: forn.cnpj.clone(),
                    fornecedor_nome: forn.razao_social.clone(),
                    tipo_irregularidade: TipoIrregularidadeFornecedor::SituacaoInapta,
                    motivo: format!(
                        "Pagamento público/eleitoral de R$ {:.2} destinado a pessoa jurídica em situação '{}' perante a Receita Federal.",
                        pag.valor, forn.situacao_cadastral
                    ),
                    gravidade: "CRITICA".to_string(),
                });
                continue;
            }

            // 2. Fornecedor recém-criado (< 30 dias de existência)
            let dias_existencia = (pag.data_pagamento - forn.data_abertura).num_days();

            if dias_existencia < DIAS_LIMITE_RECEM_CRIADA {
                alertas.push(AlertaFornecedorFantasma {
                    pagamento_id: pag.id,
                    fornecedor_cnpj: forn.cnpj.clone(),
                    fornecedor_nome: forn.razao_social.clone(),
                    tipo_irregularidade: TipoIrregularidadeFornecedor::RecemCriada,
                    motivo: format!(
                        "Fornecedor constituído há apenas {} dias (abertura em {}) ao receber pagamento de R$ {:.2}.",
                        dias_existencia, forn.data_abertura, pag.valor
                    ),
                    gravidade: if dias_existencia < 15 {
                        "CRITICA".to_string()
                    } else {
                        "ALTA".to_string()
                    },
                });
            }
        }
    }

    alertas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fantasmas_detecta_empresa_inapta() {
        let pagamentos = vec![PagamentoFornecedor {
            id: 1,
            fornecedor_cnpj: "12.345.678/0001-90".to_string(),
            fornecedor_nome: "CONSULTORIA FANTASMA".to_string(),
            valor: 120000.0,
            data_pagamento: NaiveDate::from_ymd_opt(2024, 8, 20).unwrap(),
            origem: "CAMPANHA".to_string(),
        }];

        let receita = vec![FornecedorReceita {
            cnpj: "12345678000190".to_string(),
            razao_social: "CONSULTORIA FANTASMA LTDA".to_string(),
            data_abertura: NaiveDate::from_ymd_opt(2015, 1, 1).unwrap(),
            situacao_cadastral: "INAPTA".to_string(),
        }];

        let alertas = auditar_fornecedores_fantasmas(&pagamentos, &receita);
        assert_eq!(alertas.len(), 1);
        assert_eq!(alertas[0].tipo_irregularidade, TipoIrregularidadeFornecedor::SituacaoInapta);
        assert_eq!(alertas[0].gravidade, "CRITICA");
    }

    #[test]
    fn test_fantasmas_detecta_empresa_recem_criada_menos_30_dias() {
        let abertura = NaiveDate::from_ymd_opt(2024, 8, 1).unwrap();
        let pagamento = NaiveDate::from_ymd_opt(2024, 8, 20).unwrap(); // 19 dias de vida!

        let pagamentos = vec![PagamentoFornecedor {
            id: 2,
            fornecedor_cnpj: "99.888.777/0001-11".to_string(),
            fornecedor_nome: "GRAFICA EXPRESS".to_string(),
            valor: 500000.0,
            data_pagamento: pagamento,
            origem: "CAMPANHA".to_string(),
        }];

        let receita = vec![FornecedorReceita {
            cnpj: "99888777000111".to_string(),
            razao_social: "GRAFICA EXPRESS EIRELI".to_string(),
            data_abertura: abertura,
            situacao_cadastral: "ATIVA".to_string(),
        }];

        let alertas = auditar_fornecedores_fantasmas(&pagamentos, &receita);
        assert_eq!(alertas.len(), 1);
        assert_eq!(alertas[0].tipo_irregularidade, TipoIrregularidadeFornecedor::RecemCriada);
        assert!(alertas[0].motivo.contains("19 dias"));
    }

    #[test]
    fn test_fantasmas_caso_controle_empresa_antiga_e_ativa() {
        let abertura = NaiveDate::from_ymd_opt(2018, 5, 10).unwrap();
        let pagamento = NaiveDate::from_ymd_opt(2024, 8, 20).unwrap(); // Anos de existência

        let pagamentos = vec![PagamentoFornecedor {
            id: 3,
            fornecedor_cnpj: "11.222.333/0001-44".to_string(),
            fornecedor_nome: "EMPRESA CONSOLIDADA".to_string(),
            valor: 35000.0,
            data_pagamento: pagamento,
            origem: "CEAP".to_string(),
        }];

        let receita = vec![FornecedorReceita {
            cnpj: "11222333000144".to_string(),
            razao_social: "EMPRESA CONSOLIDADA LTDA".to_string(),
            data_abertura: abertura,
            situacao_cadastral: "ATIVA".to_string(),
        }];

        let alertas = auditar_fornecedores_fantasmas(&pagamentos, &receita);
        assert!(alertas.is_empty());
    }
}
