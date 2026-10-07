use serde::{Deserialize, Serialize};

pub const LIMITE_DOACAO_SUSPEITA_BENEFICIARIO: f64 = 1_000.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DoadorCampanhaAnalise {
    pub candidatura_id: Option<i64>,
    pub doador_cpf_cnpj: String,
    pub doador_nome: String,
    pub valor: f64,
    pub data_receita: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BeneficiarioSocialAnalise {
    pub cpf_mascarado: String,
    pub nome_beneficiario: String,
    pub valor_total_beneficio: f64,
    pub parcelas: usize,
    pub municipio: Option<String>,
    pub uf: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaDoadorIncompativel {
    pub doador_cpf_mascarado: String,
    pub doador_nome: String,
    pub valor_doado: f64,
    pub valor_beneficio: f64,
    pub gravidade: String,
    pub motivo: String,
}

fn normalizar_texto(s: &str) -> String {
    s.to_uppercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn extrair_seis_digitos_centrais(cpf_ou_mascara: &str) -> Option<String> {
    let digitos: String = cpf_ou_mascara.chars().filter(|c| c.is_ascii_digit()).collect();
    if digitos.len() == 11 {
        Some(digitos[3..9].to_string())
    } else if digitos.len() == 6 {
        Some(digitos)
    } else {
        None
    }
}

fn nomes_compativeis(nome_doador: &str, nome_beneficiario: &str) -> bool {
    let d = normalizar_texto(nome_doador);
    let b = normalizar_texto(nome_beneficiario);

    if d == b {
        return true;
    }

    let tokens_d: Vec<&str> = d.split_whitespace().collect();
    let tokens_b: Vec<&str> = b.split_whitespace().collect();

    if tokens_d.len() >= 2 && tokens_b.len() >= 2 {
        let primeiro_igual = tokens_d[0] == tokens_b[0];
        let ultimo_igual = tokens_d.last() == tokens_b.last();
        if primeiro_igual && ultimo_igual {
            return true;
        }
    }

    false
}

pub fn auditar_doador_incompativel(
    doador: &DoadorCampanhaAnalise,
    beneficiario: &BeneficiarioSocialAnalise,
    limite_minimo_doacao: f64,
) -> Option<AlertaDoadorIncompativel> {
    if doador.valor < limite_minimo_doacao {
        return None;
    }

    let digitos_doador = extrair_seis_digitos_centrais(&doador.doador_cpf_cnpj)?;
    let digitos_benef = extrair_seis_digitos_centrais(&beneficiario.cpf_mascarado)?;

    if digitos_doador != digitos_benef {
        return None;
    }

    if !nomes_compativeis(&doador.doador_nome, &beneficiario.nome_beneficiario) {
        return None;
    }

    let gravidade = if doador.valor >= 5_000.0 || doador.valor >= (beneficiario.valor_total_beneficio * 3.0) {
        "CRITICA"
    } else {
        "ALTA"
    };

    let motivo = format!(
        "Doador de campanha ({}) aportou R$ {:.2}, mas consta como beneficiário de auxílio governamental (R$ {:.2} recebidos), indicando potencial incompatibilidade de capacidade econômico-financeira (suspeita de doador interposto/laranja).",
        doador.doador_nome,
        doador.valor,
        beneficiario.valor_total_beneficio
    );

    let masc = if doador.doador_cpf_cnpj.contains('*') {
        doador.doador_cpf_cnpj.clone()
    } else {
        format!("***.{}.-**", digitos_doador)
    };

    Some(AlertaDoadorIncompativel {
        doador_cpf_mascarado: masc,
        doador_nome: doador.doador_nome.clone(),
        valor_doado: doador.valor,
        valor_beneficio: beneficiario.valor_total_beneficio,
        gravidade: gravidade.to_string(),
        motivo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doador_incompativel_detecta_beneficiario_social_doador_alto() {
        let doador = DoadorCampanhaAnalise {
            candidatura_id: Some(10),
            doador_cpf_cnpj: "12345678900".to_string(), // central: 456789
            doador_nome: "MARIA SILVA SANTOS".to_string(),
            valor: 6_000.0,
            data_receita: Some("2024-08-20".to_string()),
        };

        let benef = BeneficiarioSocialAnalise {
            cpf_mascarado: "***.456.789-**".to_string(),
            nome_beneficiario: "MARIA SILVA SANTOS".to_string(),
            valor_total_beneficio: 1_800.0,
            parcelas: 3,
            municipio: Some("SAO PAULO".to_string()),
            uf: Some("SP".to_string()),
        };

        let alerta = auditar_doador_incompativel(&doador, &benef, LIMITE_DOACAO_SUSPEITA_BENEFICIARIO);
        assert!(alerta.is_some());
        let a = alerta.unwrap();
        assert_eq!(a.gravidade, "CRITICA");
        assert_eq!(a.valor_doado, 6_000.0);
    }

    #[test]
    fn test_doador_incompativel_caso_controle_doacao_pequena() {
        let doador = DoadorCampanhaAnalise {
            candidatura_id: Some(10),
            doador_cpf_cnpj: "12345678900".to_string(),
            doador_nome: "MARIA SILVA SANTOS".to_string(),
            valor: 50.0, // Doação modesta não deve disparar alerta de laranja
            data_receita: Some("2024-08-20".to_string()),
        };

        let benef = BeneficiarioSocialAnalise {
            cpf_mascarado: "***.456.789-**".to_string(),
            nome_beneficiario: "MARIA SILVA SANTOS".to_string(),
            valor_total_beneficio: 1_800.0,
            parcelas: 3,
            municipio: Some("SAO PAULO".to_string()),
            uf: Some("SP".to_string()),
        };

        let alerta = auditar_doador_incompativel(&doador, &benef, LIMITE_DOACAO_SUSPEITA_BENEFICIARIO);
        assert!(alerta.is_none());
    }

    #[test]
    fn test_doador_incompativel_nao_colide_homonimos_distintos() {
        let doador = DoadorCampanhaAnalise {
            candidatura_id: Some(10),
            doador_cpf_cnpj: "12345678900".to_string(),
            doador_nome: "JOSE CARLOS LIMA".to_string(),
            valor: 3_000.0,
            data_receita: Some("2024-08-20".to_string()),
        };

        let benef = BeneficiarioSocialAnalise {
            cpf_mascarado: "***.456.789-**".to_string(),
            nome_beneficiario: "MARIA APARECIDA SANTOS".to_string(), // CPF bate mas pessoa diferente
            valor_total_beneficio: 1_800.0,
            parcelas: 3,
            municipio: Some("SAO PAULO".to_string()),
            uf: Some("SP".to_string()),
        };

        let alerta = auditar_doador_incompativel(&doador, &benef, LIMITE_DOACAO_SUSPEITA_BENEFICIARIO);
        assert!(alerta.is_none());
    }
}
