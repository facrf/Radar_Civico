use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VinculoCeapDoacao {
    pub politico_id: i64,
    pub politico_nome: String,
    pub fornecedor_cnpj_cpf: String,
    pub fornecedor_nome: String,
    pub doador_cpf_cnpj: String,
    pub doador_nome: String,
    pub valor_ceap_total: f64,
    pub valor_doacao_total: f64,
    pub ano_eleicao: i32,
    pub data_despesa: String,
    pub data_doacao: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaDoacaoCircular {
    pub politico_id: i64,
    pub politico_nome: String,
    pub fornecedor_cnpj_cpf: String,
    pub fornecedor_nome: String,
    pub doador_cpf_cnpj: String,
    pub doador_nome: String,
    pub valor_ceap_total: f64,
    pub valor_doacao_total: f64,
    pub ano_eleicao: i32,
    pub gravidade: String,
    pub motivo: String,
}

pub fn auditar_doacoes_circulares(
    vinculos: &[VinculoCeapDoacao],
    piso_ceap: f64,
    piso_doacao: f64,
) -> Vec<AlertaDoacaoCircular> {
    let mut alertas = Vec::new();

    for v in vinculos {
        if v.valor_ceap_total < piso_ceap || v.valor_doacao_total < piso_doacao {
            continue;
        }

        let gravidade = if v.valor_doacao_total >= 10_000.0 || v.valor_ceap_total >= 50_000.0 {
            "CRITICA"
        } else {
            "ALTA"
        };

        let motivo = format!(
            "Fluxo circular de recursos detectado: o fornecedor {} ({}) recebeu R$ {:.2} em cota parlamentar de {} e doou R$ {:.2} (via {}) para a campanha eleitoral de {}.",
            v.fornecedor_nome, v.fornecedor_cnpj_cpf, v.valor_ceap_total, v.politico_nome,
            v.valor_doacao_total, v.doador_nome, v.ano_eleicao
        );

        alertas.push(AlertaDoacaoCircular {
            politico_id: v.politico_id,
            politico_nome: v.politico_nome.clone(),
            fornecedor_cnpj_cpf: v.fornecedor_cnpj_cpf.clone(),
            fornecedor_nome: v.fornecedor_nome.clone(),
            doador_cpf_cnpj: v.doador_cpf_cnpj.clone(),
            doador_nome: v.doador_nome.clone(),
            valor_ceap_total: (v.valor_ceap_total * 100.0).round() / 100.0,
            valor_doacao_total: (v.valor_doacao_total * 100.0).round() / 100.0,
            ano_eleicao: v.ano_eleicao,
            gravidade: gravidade.to_string(),
            motivo,
        });
    }

    alertas.sort_by(|a, b| {
        (b.valor_ceap_total + b.valor_doacao_total)
            .partial_cmp(&(a.valor_ceap_total + a.valor_doacao_total))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    alertas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doacao_circular_detectada() {
        let vinculos = vec![VinculoCeapDoacao {
            politico_id: 1,
            politico_nome: "DEPUTADO W".into(),
            fornecedor_cnpj_cpf: "12345678000199".into(),
            fornecedor_nome: "CONSULTORIA POLITICA LTDA".into(),
            doador_cpf_cnpj: "12345678000199".into(),
            doador_nome: "CONSULTORIA POLITICA LTDA".into(),
            valor_ceap_total: 60_000.0,
            valor_doacao_total: 15_000.0,
            ano_eleicao: 2024,
            data_despesa: "2023-08-15".into(),
            data_doacao: "2024-09-01".into(),
        }];

        let alertas = auditar_doacoes_circulares(&vinculos, 10_000.0, 1_000.0);
        assert_eq!(alertas.len(), 1);
        let a = &alertas[0];
        assert_eq!(a.gravidade, "CRITICA");
        assert!(a.motivo.contains("Fluxo circular de recursos detectado"));
    }
}
