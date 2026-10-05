use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcupanteCargo {
    pub id: i64,
    pub nome: String,
    pub cpf_mascarado: Option<String>,
    pub cargo: String,
    pub orgao: String,
    pub municipio: String,
    pub uf: String,
    pub data_nomeacao: String,
    pub ativo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistroOab {
    pub pessoa_nome: String,
    pub numero_registro: String,
    pub seccional_uf: String,
    pub situacao_registro: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaConflitoOab {
    pub gestor_id: i64,
    pub nome: String,
    pub cargo: String,
    pub orgao: String,
    pub oab_registro: String,
    pub oab_uf: String,
    pub situacao_oab: String,
    pub motivo: String,
    pub enquadramento_legal: String,
}

pub fn is_cargo_incompativel(cargo: &str) -> bool {
    let cargo_upper = cargo.to_uppercase();
    let words: Vec<&str> = cargo_upper
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();

    let has_word = |target: &str| words.iter().any(|&w| w == target);

    if has_word("SECRETARIO")
        || has_word("SECRETÁRIO")
        || has_word("SECRETARIA")
        || has_word("SECRETÁRIA")
        || has_word("DIRETOR")
        || has_word("DIRETORA")
        || has_word("PRESIDENTE")
        || has_word("SUPERINTENDENTE")
        || has_word("MINISTRO")
        || has_word("MINISTRA")
    {
        return true;
    }

    if cargo_upper.contains("PROCURADOR-GERAL")
        || cargo_upper.contains("PROCURADOR GERAL")
        || cargo_upper.contains("CHEFE DE GABINETE")
    {
        return true;
    }

    false
}

pub fn auditar_conflito_oab(
    ocupante: &OcupanteCargo,
    registro_oab: &RegistroOab,
) -> Option<AlertaConflitoOab> {
    if !ocupante.ativo {
        return None;
    }

    if !is_cargo_incompativel(&ocupante.cargo) {
        return None;
    }

    let situacao = registro_oab.situacao_registro.trim().to_uppercase();

    // Apenas a situação REGULAR gera conflito; LICENCIADO ou CANCELADO cumpre a lei
    if situacao == "REGULAR" || situacao == "ATIVA" {
        Some(AlertaConflitoOab {
            gestor_id: ocupante.id,
            nome: ocupante.nome.clone(),
            cargo: ocupante.cargo.clone(),
            orgao: ocupante.orgao.clone(),
            oab_registro: registro_oab.numero_registro.clone(),
            oab_uf: registro_oab.seccional_uf.clone(),
            situacao_oab: registro_oab.situacao_registro.clone(),
            motivo: format!(
                "Exercício de cargo de chefia/direção ({}) com inscrição da OAB ({}/{}) em situação '{}', configurando incompatibilidade absoluta.",
                ocupante.cargo, registro_oab.numero_registro, registro_oab.seccional_uf, registro_oab.situacao_registro
            ),
            enquadramento_legal: "Art. 28, inciso III, da Lei Federal nº 8.906/1994 (Estatuto da OAB)".to_string(),
        })
    } else {
        None
    }
}

pub fn auditar_lote_conflitos_oab(
    pares: &[(OcupanteCargo, RegistroOab)],
) -> Vec<AlertaConflitoOab> {
    pares
        .iter()
        .filter_map(|(ocupante, oab)| auditar_conflito_oab(ocupante, oab))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflito_oab_secretario_regular_gera_alerta() {
        let ocupante = OcupanteCargo {
            id: 101,
            nome: "ADVOGADO NOMEADO".to_string(),
            cpf_mascarado: Some("***.123.456-**".to_string()),
            cargo: "SECRETARIO MUNICIPAL DE GOVERNO".to_string(),
            orgao: "PREFEITURA MUNICIPAL".to_string(),
            municipio: "CAMPINAS".to_string(),
            uf: "SP".to_string(),
            data_nomeacao: "2024-01-05".to_string(),
            ativo: true,
        };

        let oab = RegistroOab {
            pessoa_nome: "ADVOGADO NOMEADO".to_string(),
            numero_registro: "999888".to_string(),
            seccional_uf: "SP".to_string(),
            situacao_registro: "REGULAR".to_string(),
        };

        let alerta = auditar_conflito_oab(&ocupante, &oab);
        assert!(alerta.is_some());
        let alerta = alerta.unwrap();
        assert_eq!(alerta.gestor_id, 101);
        assert_eq!(alerta.situacao_oab, "REGULAR");
        assert!(alerta.enquadramento_legal.contains("Art. 28"));
    }

    #[test]
    fn test_conflito_oab_caso_controle_licenciado_sem_alerta() {
        let ocupante = OcupanteCargo {
            id: 102,
            nome: "ADVOGADO CORRETO".to_string(),
            cpf_mascarado: Some("***.321.654-**".to_string()),
            cargo: "SECRETÁRIO DE SAÚDE".to_string(),
            orgao: "PREFEITURA MUNICIPAL".to_string(),
            municipio: "SANTOS".to_string(),
            uf: "SP".to_string(),
            data_nomeacao: "2024-01-10".to_string(),
            ativo: true,
        };

        let oab = RegistroOab {
            pessoa_nome: "ADVOGADO CORRETO".to_string(),
            numero_registro: "111222".to_string(),
            seccional_uf: "SP".to_string(),
            situacao_registro: "LICENCIADO".to_string(), // Devidamente licenciado
        };

        let alerta = auditar_conflito_oab(&ocupante, &oab);
        assert!(alerta.is_none());
    }

    #[test]
    fn test_conflito_oab_cargo_sem_poder_de_direcao() {
        let ocupante = OcupanteCargo {
            id: 103,
            nome: "ANALISTA PUBLICO".to_string(),
            cpf_mascarado: Some("***.777.888-**".to_string()),
            cargo: "TECNICO ADMINISTRATIVO".to_string(),
            orgao: "SECRETARIA DE FAZENDA".to_string(),
            municipio: "CAMPINAS".to_string(),
            uf: "SP".to_string(),
            data_nomeacao: "2023-05-15".to_string(),
            ativo: true,
        };

        let oab = RegistroOab {
            pessoa_nome: "ANALISTA PUBLICO".to_string(),
            numero_registro: "333444".to_string(),
            seccional_uf: "SP".to_string(),
            situacao_registro: "REGULAR".to_string(),
        };

        let alerta = auditar_conflito_oab(&ocupante, &oab);
        assert!(alerta.is_none());
    }

    #[test]
    fn test_conflito_oab_ocupante_inativo() {
        let ocupante = OcupanteCargo {
            id: 104,
            nome: "EX SECRETARIO".to_string(),
            cpf_mascarado: None,
            cargo: "SECRETARIO MUNICIPAL DE OBRAS".to_string(),
            orgao: "PREFEITURA MUNICIPAL".to_string(),
            municipio: "CAMPINAS".to_string(),
            uf: "SP".to_string(),
            data_nomeacao: "2020-01-01".to_string(),
            ativo: false, // Não está mais ativo no cargo
        };

        let oab = RegistroOab {
            pessoa_nome: "EX SECRETARIO".to_string(),
            numero_registro: "555666".to_string(),
            seccional_uf: "SP".to_string(),
            situacao_registro: "REGULAR".to_string(),
        };

        let alerta = auditar_conflito_oab(&ocupante, &oab);
        assert!(alerta.is_none());
    }
}
