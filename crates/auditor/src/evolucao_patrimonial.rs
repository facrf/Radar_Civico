use serde::{Deserialize, Serialize};

pub const LIMITE_VARIACAO_PERCENTUAL_PADRAO: f64 = 300.0;
pub const LIMITE_INCREMENTO_ABSOLUTO_PADRAO: f64 = 200_000.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeclaracaoPatrimonioAno {
    pub ano: i32,
    pub cargo: String,
    pub valor_total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaEvolucaoPatrimonial {
    pub politico_id: i64,
    pub politico_nome: String,
    pub ano_anterior: i32,
    pub valor_anterior: f64,
    pub ano_recente: i32,
    pub valor_recente: f64,
    pub variacao_percentual: f64,
    pub incremento_absoluto: f64,
    pub gravidade: String,
    pub motivo: String,
}

pub fn auditar_evolucao_patrimonial(
    politico_id: i64,
    politico_nome: &str,
    declaracoes: &[DeclaracaoPatrimonioAno],
    limite_percentual: f64,
    limite_absoluto: f64,
) -> Vec<AlertaEvolucaoPatrimonial> {
    if declaracoes.len() < 2 {
        return Vec::new();
    }

    let mut ordenadas = declaracoes.to_vec();
    ordenadas.sort_by_key(|d| d.ano);

    let mut alertas = Vec::new();

    for janela in ordenadas.windows(2) {
        let anterior = &janela[0];
        let recente = &janela[1];

        let incremento = recente.valor_total - anterior.valor_total;
        if incremento < limite_absoluto {
            continue;
        }

        let variacao_percentual = if anterior.valor_total <= 0.0 {
            // Salto de patrimônio nulo ou zerado para valor substancial
            1000.0
        } else {
            (incremento / anterior.valor_total) * 100.0
        };

        if variacao_percentual >= limite_percentual {
            let gravidade = if variacao_percentual >= 500.0 || incremento >= 1_000_000.0 {
                "CRITICA"
            } else {
                "ALTA"
            };

            let motivo = format!(
                "Patrimônio declarado saltou de R$ {:.2} ({}) para R$ {:.2} ({}), representando crescimento de {:.1}% (+R$ {:.2}).",
                anterior.valor_total,
                anterior.ano,
                recente.valor_total,
                recente.ano,
                variacao_percentual,
                incremento
            );

            alertas.push(AlertaEvolucaoPatrimonial {
                politico_id,
                politico_nome: politico_nome.to_string(),
                ano_anterior: anterior.ano,
                valor_anterior: anterior.valor_total,
                ano_recente: recente.ano,
                valor_recente: recente.valor_total,
                variacao_percentual: (variacao_percentual * 100.0).round() / 100.0,
                incremento_absoluto: (incremento * 100.0).round() / 100.0,
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
    fn test_evolucao_patrimonial_detecta_salto_abrupto() {
        let declaracoes = vec![
            DeclaracaoPatrimonioAno {
                ano: 2020,
                cargo: "VEREADOR".to_string(),
                valor_total: 100_000.0,
            },
            DeclaracaoPatrimonioAno {
                ano: 2024,
                cargo: "PREFEITO".to_string(),
                valor_total: 800_000.0, // +700k (+700%)
            },
        ];

        let alertas = auditar_evolucao_patrimonial(
            1,
            "POLITICO EXEMPLO",
            &declaracoes,
            LIMITE_VARIACAO_PERCENTUAL_PADRAO,
            LIMITE_INCREMENTO_ABSOLUTO_PADRAO,
        );

        assert_eq!(alertas.len(), 1);
        let a = &alertas[0];
        assert_eq!(a.ano_anterior, 2020);
        assert_eq!(a.ano_recente, 2024);
        assert_eq!(a.variacao_percentual, 700.0);
        assert_eq!(a.gravidade, "CRITICA");
    }

    #[test]
    fn test_evolucao_patrimonial_caso_controle_sem_alerta() {
        let declaracoes = vec![
            DeclaracaoPatrimonioAno {
                ano: 2020,
                cargo: "VEREADOR".to_string(),
                valor_total: 200_000.0,
            },
            DeclaracaoPatrimonioAno {
                ano: 2024,
                cargo: "VEREADOR".to_string(),
                valor_total: 240_000.0, // +40k (+20%)
            },
        ];

        let alertas = auditar_evolucao_patrimonial(
            1,
            "POLITICO EXEMPLO",
            &declaracoes,
            LIMITE_VARIACAO_PERCENTUAL_PADRAO,
            LIMITE_INCREMENTO_ABSOLUTO_PADRAO,
        );

        assert!(alertas.is_empty());
    }

    #[test]
    fn test_evolucao_patrimonial_de_zero_com_valor_expressivo() {
        let declaracoes = vec![
            DeclaracaoPatrimonioAno {
                ano: 2020,
                cargo: "VEREADOR".to_string(),
                valor_total: 0.0,
            },
            DeclaracaoPatrimonioAno {
                ano: 2024,
                cargo: "DEPUTADO".to_string(),
                valor_total: 500_000.0,
            },
        ];

        let alertas = auditar_evolucao_patrimonial(
            1,
            "NOVO POLITICO",
            &declaracoes,
            LIMITE_VARIACAO_PERCENTUAL_PADRAO,
            LIMITE_INCREMENTO_ABSOLUTO_PADRAO,
        );

        assert_eq!(alertas.len(), 1);
        assert_eq!(alertas[0].incremento_absoluto, 500_000.0);
    }
}
