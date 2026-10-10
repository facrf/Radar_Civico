use serde::{Deserialize, Serialize};

/// Classificação do nível de risco cívico
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NivelRiscoCivico {
    Minimo,
    Baixo,
    Moderado,
    Critico,
}

impl NivelRiscoCivico {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Minimo => "MÍNIMO",
            Self::Baixo => "BAIXO",
            Self::Moderado => "MODERADO",
            Self::Critico => "CRÍTICO",
        }
    }

    pub fn cor_hex(&self) -> &'static str {
        match self {
            Self::Minimo => "#10b981", // Emerald 500
            Self::Baixo => "#06b6d4",  // Cyan 500
            Self::Moderado => "#f59e0b", // Amber 500
            Self::Critico => "#ef4444",  // Rose 500
        }
    }
}

/// Item descritivo de penalidade aplicada ao Score
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemPenalidadeScore {
    pub categoria: String,
    pub motivo: String,
    pub pontos_deduzidos: u32,
}

/// Resultado consolidado da auditoria de Score de Integridade
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResumoScoreIntegridade {
    pub score: u32,
    pub nivel_risco: String,
    pub cor_hex: String,
    pub total_penalidades: u32,
    pub itens_penalidades: Vec<ItemPenalidadeScore>,
}

/// Calcula o Score de Integridade (0 a 100) com base nos alertas e anomalias acumuladas
pub fn calcular_score_integridade(
    qtd_criticos: usize,
    qtd_graves: usize,
    qtd_medios: usize,
    qtd_leves: usize,
    motivos_detalhados: &[String],
) -> ResumoScoreIntegridade {
    let mut penalidades = Vec::new();
    let mut total_deducao: u32 = 0;

    // 1. Alertas Críticos (-20 pts cada, máx -40)
    if qtd_criticos > 0 {
        let deducao = ((qtd_criticos as u32) * 20).min(40);
        total_deducao += deducao;
        penalidades.push(ItemPenalidadeScore {
            categoria: "CRÍTICA".to_string(),
            motivo: format!(
                "{} anomalia(s) crítica(s) identificada(s) (ex.: empresa recém-criada ou auxílio indevido)",
                qtd_criticos
            ),
            pontos_deduzidos: deducao,
        });
    }

    // 2. Alertas Graves (-15 pts cada, máx -30)
    if qtd_graves > 0 {
        let deducao = ((qtd_graves as u32) * 15).min(30);
        total_deducao += deducao;
        penalidades.push(ItemPenalidadeScore {
            categoria: "GRAVE".to_string(),
            motivo: format!(
                "{} ocorrência(s) grave(s) (ex.: conflito OAB, doador com inexigibilidade ou fornecedor inapto)",
                qtd_graves
            ),
            pontos_deduzidos: deducao,
        });
    }

    // 3. Alertas Médios (-10 pts cada, máx -20)
    if qtd_medios > 0 {
        let deducao = ((qtd_medios as u32) * 10).min(20);
        total_deducao += deducao;
        penalidades.push(ItemPenalidadeScore {
            categoria: "MÉDIA".to_string(),
            motivo: format!(
                "{} anomalia(s) de média relevância (ex.: combustível excessivo, evolução patrimonial atípica ou fornecedor hub)",
                qtd_medios
            ),
            pontos_deduzidos: deducao,
        });
    }

    // 4. Alertas Leves (-5 pts cada, máx -10)
    if qtd_leves > 0 {
        let deducao = ((qtd_leves as u32) * 5).min(10);
        total_deducao += deducao;
        penalidades.push(ItemPenalidadeScore {
            categoria: "LEVE".to_string(),
            motivo: format!(
                "{} inconsistência(s) leve(s) (ex.: deslocamentos sem justificativa geográfica)",
                qtd_leves
            ),
            pontos_deduzidos: deducao,
        });
    }

    // Inclui motivos específicos caso fornecidos
    for m in motivos_detalhados.iter().take(3) {
        if !penalidades.iter().any(|p| p.motivo.contains(m)) {
            penalidades.push(ItemPenalidadeScore {
                categoria: "DETALHE".to_string(),
                motivo: m.clone(),
                pontos_deduzidos: 0,
            });
        }
    }

    let score = 100u32.saturating_sub(total_deducao);

    let nivel = if score >= 90 {
        NivelRiscoCivico::Minimo
    } else if score >= 70 {
        NivelRiscoCivico::Baixo
    } else if score >= 50 {
        NivelRiscoCivico::Moderado
    } else {
        NivelRiscoCivico::Critico
    };

    ResumoScoreIntegridade {
        score,
        nivel_risco: nivel.as_str().to_string(),
        cor_hex: nivel.cor_hex().to_string(),
        total_penalidades: total_deducao,
        itens_penalidades: penalidades,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_integridade_sem_alertas() {
        let res = calcular_score_integridade(0, 0, 0, 0, &[]);
        assert_eq!(res.score, 100);
        assert_eq!(res.nivel_risco, "MÍNIMO");
        assert_eq!(res.cor_hex, "#10b981");
        assert_eq!(res.total_penalidades, 0);
    }

    #[test]
    fn test_score_integridade_risco_moderado() {
        let res = calcular_score_integridade(1, 1, 1, 0, &[]);
        // Crítico: 20, Grave: 15, Médio: 10 = total 45. Score: 55
        assert_eq!(res.score, 55);
        assert_eq!(res.nivel_risco, "MODERADO");
        assert_eq!(res.cor_hex, "#f59e0b");
    }

    #[test]
    fn test_score_integridade_risco_critico() {
        let res = calcular_score_integridade(2, 2, 2, 2, &[]);
        // Max deductions: 40 + 30 + 20 + 10 = 100 deduzidos. Score: 0
        assert_eq!(res.score, 0);
        assert_eq!(res.nivel_risco, "CRÍTICO");
        assert_eq!(res.cor_hex, "#ef4444");
    }
}
