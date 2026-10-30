use serde::{Deserialize, Serialize};

pub const PISO_FATURAMENTO_EXCLUSIVIDADE: f64 = 40_000.0;
pub const PERCENTUAL_DEPENDENCIA_CRITICA: f64 = 85.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FornecedorParlamentarFaturamento {
    pub fornecedor_cnpj_cpf: String,
    pub fornecedor_nome: String,
    pub parlamentar_nome: String,
    pub faturamento_parlamentar: f64,
    pub faturamento_total_publico: f64,
    pub quantidade_despesas: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaFornecedorExclusivo {
    pub fornecedor_cnpj_cpf: String,
    pub fornecedor_nome: String,
    pub parlamentar_nome: String,
    pub faturamento_parlamentar: f64,
    pub faturamento_total: f64,
    pub percentual_dependencia: f64,
    pub quantidade_despesas: usize,
    pub gravidade: String,
    pub motivo: String,
}

/// Detecta fornecedores cuja receita pública provém predominantemente (> 85%) de um único parlamentar
pub fn auditar_fornecedores_exclusivos(
    dados: &[FornecedorParlamentarFaturamento],
    piso_faturamento: f64,
    percentual_minimo: f64,
) -> Vec<AlertaFornecedorExclusivo> {
    let mut alertas = Vec::new();

    for item in dados {
        if item.faturamento_parlamentar < piso_faturamento || item.faturamento_total_publico <= 0.0 {
            continue;
        }

        let percentual = (item.faturamento_parlamentar / item.faturamento_total_publico) * 100.0;
        if percentual >= percentual_minimo {
            let gravidade = if percentual >= 95.0 && item.faturamento_parlamentar >= 100_000.0 {
                "CRITICA"
            } else {
                "ALTA"
            };

            let motivo = format!(
                "O fornecedor {} ({}) faturou R$ {:.2} na cota parlamentar, sendo {:.1}% originado exclusivamente do parlamentar {} (indício de empresa satélite de gabinete).",
                item.fornecedor_nome, item.fornecedor_cnpj_cpf, item.faturamento_parlamentar, percentual, item.parlamentar_nome
            );

            alertas.push(AlertaFornecedorExclusivo {
                fornecedor_cnpj_cpf: item.fornecedor_cnpj_cpf.clone(),
                fornecedor_nome: item.fornecedor_nome.clone(),
                parlamentar_nome: item.parlamentar_nome.clone(),
                faturamento_parlamentar: (item.faturamento_parlamentar * 100.0).round() / 100.0,
                faturamento_total: (item.faturamento_total_publico * 100.0).round() / 100.0,
                percentual_dependencia: (percentual * 10.0).round() / 10.0,
                quantidade_despesas: item.quantidade_despesas,
                gravidade: gravidade.to_string(),
                motivo,
            });
        }
    }

    alertas.sort_by(|a, b| {
        b.faturamento_parlamentar
            .partial_cmp(&a.faturamento_parlamentar)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    alertas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fornecedor_exclusivo_detecta_empresa_satelite() {
        let dados = vec![FornecedorParlamentarFaturamento {
            fornecedor_cnpj_cpf: "11222333000144".into(),
            fornecedor_nome: "MARKETING DIGITAL EXCLUSIVO LTDA".into(),
            parlamentar_nome: "DEPUTADO Z".into(),
            faturamento_parlamentar: 95_000.0,
            faturamento_total_publico: 100_000.0,
            quantidade_despesas: 10,
        }];

        let alertas = auditar_fornecedores_exclusivos(
            &dados,
            PISO_FATURAMENTO_EXCLUSIVIDADE,
            PERCENTUAL_DEPENDENCIA_CRITICA,
        );
        assert_eq!(alertas.len(), 1);
        let a = &alertas[0];
        assert_eq!(a.percentual_dependencia, 95.0);
        assert_eq!(a.gravidade, "ALTA");
        assert!(a.motivo.contains("95.0% originado exclusivamente"));
    }

    #[test]
    fn test_fornecedor_diversificado_nao_emite_alerta() {
        let dados = vec![FornecedorParlamentarFaturamento {
            fornecedor_cnpj_cpf: "55667788000199".into(),
            fornecedor_nome: "POSTO COMBUSTIVEL GIGANTE".into(),
            parlamentar_nome: "DEPUTADO Z".into(),
            faturamento_parlamentar: 50_000.0,
            faturamento_total_publico: 500_000.0, // Apenas 10% de dependência
            quantidade_despesas: 15,
        }];

        let alertas = auditar_fornecedores_exclusivos(
            &dados,
            PISO_FATURAMENTO_EXCLUSIVIDADE,
            PERCENTUAL_DEPENDENCIA_CRITICA,
        );
        assert!(alertas.is_empty(), "Fornecedor com múltiplos clientes públicos não deve ser alertado");
    }
}
