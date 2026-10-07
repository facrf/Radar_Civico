use serde::{Deserialize, Serialize};

pub const LIMITE_TANQUE_VEICULO_LEVE: f64 = 80.0;
pub const PRECO_PADRAO_GASOLINA_ANP: f64 = 5.80;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Abastecimento {
    pub id: i64,
    pub parlamentar_nome: String,
    pub data_emissao: String,
    pub valor: f64,
    pub litros_declarados: Option<f64>,
    pub preco_combustivel_anp: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaCombustivel {
    pub abastecimento_id: i64,
    pub parlamentar_nome: String,
    pub litros_calculados: f64,
    pub valor: f64,
    pub motivo: String,
    pub gravidade: String,
}

pub fn calcular_litros(abastecimento: &Abastecimento, preco_referencia_anp: Option<f64>) -> f64 {
    if let Some(litros) = abastecimento.litros_declarados {
        if litros > 0.0 {
            return litros;
        }
    }

    let preco = abastecimento
        .preco_combustivel_anp
        .or(preco_referencia_anp)
        .unwrap_or(PRECO_PADRAO_GASOLINA_ANP);

    if preco <= 0.0 {
        return 0.0;
    }

    abastecimento.valor / preco
}

pub fn auditar_abastecimento(
    abastecimento: &Abastecimento,
    preco_referencia_anp: Option<f64>,
) -> Option<AlertaCombustivel> {
    auditar_abastecimento_com_limite(abastecimento, preco_referencia_anp, LIMITE_TANQUE_VEICULO_LEVE)
}

pub fn auditar_abastecimento_com_limite(
    abastecimento: &Abastecimento,
    preco_referencia_anp: Option<f64>,
    limite_litros: f64,
) -> Option<AlertaCombustivel> {
    let litros = calcular_litros(abastecimento, preco_referencia_anp);

    if litros > limite_litros {
        let gravidade = if litros > (limite_litros * 1.5) {
            "CRITICA"
        } else {
            "ALTA"
        };

        Some(AlertaCombustivel {
            abastecimento_id: abastecimento.id,
            parlamentar_nome: abastecimento.parlamentar_nome.clone(),
            litros_calculados: (litros * 100.0).round() / 100.0,
            valor: abastecimento.valor,
            motivo: format!(
                "Volume abastecido ({:.2} L) excede a capacidade física configurada ({:.0} L)",
                litros, limite_litros
            ),
            gravidade: gravidade.to_string(),
        })
    } else {
        None
    }
}

pub fn auditar_lote_abastecimentos(
    abastecimentos: &[Abastecimento],
    preco_referencia_anp: Option<f64>,
) -> Vec<AlertaCombustivel> {
    abastecimentos
        .iter()
        .filter_map(|a| auditar_abastecimento(a, preco_referencia_anp))
        .collect()
}

pub const LIMITE_SOBREPRECO_PERCENTUAL: f64 = 1.50; // 150% da referência ANP

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaSobreprecoCombustivel {
    pub abastecimento_id: i64,
    pub parlamentar_nome: String,
    pub preco_unitario_pago: f64,
    pub preco_referencia_anp: f64,
    pub sobrepreco_percentual: f64,
    pub valor_total: f64,
    pub litros_declarados: f64,
    pub gravidade: String,
    pub motivo: String,
}

pub fn auditar_sobrepreco_combustivel(
    abastecimento: &Abastecimento,
    preco_referencia_anp: Option<f64>,
) -> Option<AlertaSobreprecoCombustivel> {
    let litros = abastecimento.litros_declarados?;
    if litros <= 0.0 || abastecimento.valor <= 0.0 {
        return None;
    }

    let preco_ref = abastecimento
        .preco_combustivel_anp
        .or(preco_referencia_anp)
        .unwrap_or(PRECO_PADRAO_GASOLINA_ANP);

    if preco_ref <= 0.0 {
        return None;
    }

    let preco_pago = abastecimento.valor / litros;
    if preco_pago > preco_ref * LIMITE_SOBREPRECO_PERCENTUAL {
        let sobrepreco_pct = ((preco_pago - preco_ref) / preco_ref) * 100.0;
        let gravidade = if sobrepreco_pct > 100.0 { "CRITICA" } else { "ALTA" };
        let motivo = format!(
            "Preço unitário pago (R$ {:.2}/L) excede em {:.1}% o valor médio de referência da ANP (R$ {:.2}/L).",
            preco_pago, sobrepreco_pct, preco_ref
        );

        Some(AlertaSobreprecoCombustivel {
            abastecimento_id: abastecimento.id,
            parlamentar_nome: abastecimento.parlamentar_nome.clone(),
            preco_unitario_pago: (preco_pago * 100.0).round() / 100.0,
            preco_referencia_anp: preco_ref,
            sobrepreco_percentual: (sobrepreco_pct * 100.0).round() / 100.0,
            valor_total: abastecimento.valor,
            litros_declarados: litros,
            gravidade: gravidade.to_string(),
            motivo,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combustivel_anomalia_volume_declarado_fiat_uno() {
        let abastecimento = Abastecimento {
            id: 1,
            parlamentar_nome: "DEPUTADO UNO".to_string(),
            data_emissao: "2024-03-10".to_string(),
            valor: 600.0,
            litros_declarados: Some(120.0), // Volume impossível em veículo leve
            preco_combustivel_anp: None,
        };

        let alerta = auditar_abastecimento(&abastecimento, None);
        assert!(alerta.is_some());
        let alerta = alerta.unwrap();
        assert_eq!(alerta.abastecimento_id, 1);
        assert_eq!(alerta.litros_calculados, 120.0);
        assert_eq!(alerta.gravidade, "ALTA");
        assert!(alerta.motivo.contains("excede a capacidade física"));
    }

    #[test]
    fn test_combustivel_anomalia_calculo_inverso_anp() {
        let abastecimento = Abastecimento {
            id: 2,
            parlamentar_nome: "DEPUTADO TANQUE GIGANTE".to_string(),
            data_emissao: "2024-03-11".to_string(),
            valor: 1500.0,
            litros_declarados: None, // Cálculo via ANP
            preco_combustivel_anp: Some(5.00),
        };

        // 1500 / 5.00 = 300 Litros!
        let alerta = auditar_abastecimento(&abastecimento, None);
        assert!(alerta.is_some());
        let alerta = alerta.unwrap();
        assert_eq!(alerta.litros_calculados, 300.0);
        assert_eq!(alerta.gravidade, "CRITICA");
    }

    #[test]
    fn test_combustivel_caso_controle_sem_falso_positivo() {
        // Abastecimento normal: 45 litros (tanque comum de passeio)
        let abastecimento_normal = Abastecimento {
            id: 3,
            parlamentar_nome: "DEPUTADO REGULAR".to_string(),
            data_emissao: "2024-03-12".to_string(),
            valor: 261.0,
            litros_declarados: Some(45.0),
            preco_combustivel_anp: Some(5.80),
        };

        let alerta = auditar_abastecimento(&abastecimento_normal, None);
        assert!(alerta.is_none());

        // Limite de 80.0 litros exatos: sem alerta
        let abastecimento_limite = Abastecimento {
            id: 4,
            parlamentar_nome: "DEPUTADO LIMITE".to_string(),
            data_emissao: "2024-03-12".to_string(),
            valor: 464.0,
            litros_declarados: Some(80.0),
            preco_combustivel_anp: Some(5.80),
        };
        let alerta_limite = auditar_abastecimento(&abastecimento_limite, None);
        assert!(alerta_limite.is_none());

        // 80.1 litros: alerta gerado
        let abastecimento_acima = Abastecimento {
            id: 5,
            parlamentar_nome: "DEPUTADO POUCO ACIMA".to_string(),
            data_emissao: "2024-03-12".to_string(),
            valor: 464.58,
            litros_declarados: Some(80.1),
            preco_combustivel_anp: Some(5.80),
        };
        let alerta_acima = auditar_abastecimento(&abastecimento_acima, None);
        assert!(alerta_acima.is_some());
    }

    #[test]
    fn test_combustivel_auditar_lote() {
        let lista = vec![
            Abastecimento {
                id: 10,
                parlamentar_nome: "DEP A".to_string(),
                data_emissao: "2024-01-01".to_string(),
                valor: 200.0,
                litros_declarados: Some(40.0),
                preco_combustivel_anp: None,
            },
            Abastecimento {
                id: 11,
                parlamentar_nome: "DEP B".to_string(),
                data_emissao: "2024-01-02".to_string(),
                valor: 900.0,
                litros_declarados: Some(150.0),
                preco_combustivel_anp: None,
            },
        ];

        let alertas = auditar_lote_abastecimentos(&lista, None);
        assert_eq!(alertas.len(), 1);
        assert_eq!(alertas[0].abastecimento_id, 11);
    }

    #[test]
    fn test_combustivel_sobrepreco_detectado() {
        let abastecimento = Abastecimento {
            id: 20,
            parlamentar_nome: "DEP CARO".to_string(),
            data_emissao: "2024-03-01".to_string(),
            valor: 500.0,
            litros_declarados: Some(40.0), // R$ 12,50 por litro! (vs R$ 5,80 ANP)
            preco_combustivel_anp: Some(5.80),
        };

        let alerta = auditar_sobrepreco_combustivel(&abastecimento, None);
        assert!(alerta.is_some());
        let a = alerta.unwrap();
        assert_eq!(a.preco_unitario_pago, 12.50);
        assert_eq!(a.gravidade, "CRITICA");
    }
}
