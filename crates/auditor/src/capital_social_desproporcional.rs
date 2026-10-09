use serde::{Deserialize, Serialize};

pub const LIMITE_CAPITAL_INVEROSIMIL_MAX: f64 = 5_000.0;
pub const LIMITE_FATURAMENTO_PUBLICO_MIN: f64 = 100_000.0;
pub const RAZAO_FATURAMENTO_CAPITAL_CRITICA: f64 = 50.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FornecedorCapitalFaturamento {
    pub cnpj: String,
    pub razao_social: String,
    pub capital_social: f64,
    pub total_faturado: f64,
    pub quantidade_operacoes: usize,
    pub parlamentar_ou_orgao: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaCapitalDesproporcional {
    pub cnpj: String,
    pub razao_social: String,
    pub capital_social: f64,
    pub total_faturado: f64,
    pub multiplicador: f64,
    pub gravidade: String,
    pub motivo: String,
}

pub fn auditar_capital_desproporcional(
    fornecedores: &[FornecedorCapitalFaturamento],
    teto_capital: f64,
    piso_faturamento: f64,
) -> Vec<AlertaCapitalDesproporcional> {
    let mut alertas = Vec::new();

    for f in fornecedores {
        if f.capital_social <= teto_capital && f.total_faturado >= piso_faturamento {
            let mult = if f.capital_social > 0.0 {
                f.total_faturado / f.capital_social
            } else {
                1000.0
            };

            let gravidade = if mult >= RAZAO_FATURAMENTO_CAPITAL_CRITICA || f.total_faturado >= 300_000.0 {
                "CRITICA"
            } else {
                "ALTA"
            };

            let motivo = format!(
                "Empresa {} faturou R$ {:.2} em recursos públicos com capital social declarado de apenas R$ {:.2} ({:.0}x superior ao patrimônio societário).",
                f.razao_social, f.total_faturado, f.capital_social, mult
            );

            alertas.push(AlertaCapitalDesproporcional {
                cnpj: f.cnpj.clone(),
                razao_social: f.razao_social.clone(),
                capital_social: f.capital_social,
                total_faturado: f.total_faturado,
                multiplicador: (mult * 10.0).round() / 10.0,
                gravidade: gravidade.to_string(),
                motivo,
            });
        }
    }

    alertas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capital_desproporcional_detecta_empresa_fachada() {
        let dados = vec![FornecedorCapitalFaturamento {
            cnpj: "12345678000199".to_string(),
            razao_social: "SERVICOS RAPIDOS LTDA".to_string(),
            capital_social: 1_000.0,
            total_faturado: 150_000.0,
            quantidade_operacoes: 12,
            parlamentar_ou_orgao: "DEPUTADO X".to_string(),
        }];

        let alertas = auditar_capital_desproporcional(
            &dados,
            LIMITE_CAPITAL_INVEROSIMIL_MAX,
            LIMITE_FATURAMENTO_PUBLICO_MIN,
        );

        assert_eq!(alertas.len(), 1);
        let a = &alertas[0];
        assert_eq!(a.gravidade, "CRITICA");
        assert_eq!(a.multiplicador, 150.0);
        assert!(a.motivo.contains("150x superior"));
    }

    #[test]
    fn test_capital_desproporcional_ignora_empresa_com_capital_robusto() {
        let dados = vec![FornecedorCapitalFaturamento {
            cnpj: "99887766000100".to_string(),
            razao_social: "CONSTRUTORA FORTE S/A".to_string(),
            capital_social: 2_000_000.0,
            total_faturado: 500_000.0,
            quantidade_operacoes: 3,
            parlamentar_ou_orgao: "DEPUTADO Y".to_string(),
        }];

        let alertas = auditar_capital_desproporcional(
            &dados,
            LIMITE_CAPITAL_INVEROSIMIL_MAX,
            LIMITE_FATURAMENTO_PUBLICO_MIN,
        );

        assert!(alertas.is_empty(), "Não deve emitir alerta para empresa com capital social compatível");
    }
}
