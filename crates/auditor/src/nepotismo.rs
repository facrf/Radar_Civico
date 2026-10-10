use serde::{Deserialize, Serialize};

/// Stop-list de sobrenomes brasileiros de alta dispersão demográfica
const SOBRENOMES_COMUNS_STOPLIST: &[&str] = &[
    "SILVA", "SANTOS", "OLIVEIRA", "SOUZA", "SOUSA", "RODRIGUES", "FERREIRA",
    "ALVES", "PEREIRA", "LIMA", "GOMES", "COSTA", "RIBEIRO", "MARTINS",
    "CARVALHO", "ALMEIDA", "LOPES", "SOARES", "FERNANDES", "VIEIRA", "BARBOSA",
    "ROCHA", "DIAS", "NASCIMENTO", "ANDRADE", "MOREIRA", "NUNES", "MARQUES",
    "MACHADO", "MENDES", "RAMOS", "SANTANA", "TEIXEIRA", "CARDOSO", "PINTO",
    "CASTRO", "AZEVEDO", "FREITAS", "CAMPOS", "COELHO", "BARROS", "MORAES",
    "REIS", "BATISTA", "CORREIA", "CORREA", "TAVARES", "MIRANDA",
];

/// Extrai sobrenomes raros e incomuns (mínimo de 5 caracteres, fora da stop-list)
pub fn extrair_sobrenomes_raros(nome_completo: &str) -> Vec<String> {
    let limpo = nome_completo.to_uppercase();
    let tokens: Vec<&str> = limpo
        .split_whitespace()
        .filter(|t| t.len() >= 5)
        .collect();

    // Descarta o primeiro token (primeiro nome) e avalia apenas os sobrenomes
    tokens
        .into_iter()
        .skip(1)
        .filter(|t| !SOBRENOMES_COMUNS_STOPLIST.contains(t))
        .map(|s| s.to_string())
        .collect()
}

/// Alerta emitido ao identificar compartilhamento de sobrenome raro
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaPossivelParentesco {
    pub politico_nome: String,
    pub alvo_nome: String,
    pub sobrenome_compartilhado: String,
    pub tipo_vinculo: String,
    pub cnpj_cpf_alvo: String,
    pub uf: String,
    pub explicacao: String,
}

/// Informações do alvo (sócio de empresa contratada ou doador)
#[derive(Debug, Clone)]
pub struct AlvoAuditoriaParentesco {
    pub nome: String,
    pub cnpj_cpf: String,
    pub uf: String,
    pub tipo_vinculo: String, // "SOCIO_FORNECEDOR_CEAP" ou "DOADOR_CAMPANHA"
}

/// Cruza o nome de um político/autoridade com alvos para detectar compartilhamento de sobrenomes raros
pub fn auditar_possivel_parentesco(
    politico_nome: &str,
    politico_uf: &str,
    alvos: &[AlvoAuditoriaParentesco],
) -> Vec<AlertaPossivelParentesco> {
    let sobrenomes_pol = extrair_sobrenomes_raros(politico_nome);
    if sobrenomes_pol.is_empty() {
        return Vec::new();
    }

    let mut alertas = Vec::new();

    for alvo in alvos {
        // Exige compatibilidade geográfica da UF (ou BR nacional)
        if !politico_uf.is_empty() && !alvo.uf.is_empty() && politico_uf != "BR" && alvo.uf != "BR" && politico_uf != alvo.uf {
            continue;
        }

        let sobrenomes_alvo = extrair_sobrenomes_raros(&alvo.nome);

        for sob in &sobrenomes_pol {
            if sobrenomes_alvo.contains(sob) {
                alertas.push(AlertaPossivelParentesco {
                    politico_nome: politico_nome.to_string(),
                    alvo_nome: alvo.nome.clone(),
                    sobrenome_compartilhado: sob.clone(),
                    tipo_vinculo: alvo.tipo_vinculo.clone(),
                    cnpj_cpf_alvo: alvo.cnpj_cpf.clone(),
                    uf: if !politico_uf.is_empty() { politico_uf.to_string() } else { alvo.uf.clone() },
                    explicacao: format!(
                        "Coincidência do sobrenome raro '{}' entre o agente público e '{}' ({}) na UF '{}'.",
                        sob, alvo.nome, alvo.tipo_vinculo, politico_uf
                    ),
                });
                break;
            }
        }
    }

    alertas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ignora_sobrenomes_comuns() {
        let sobrenomes = extrair_sobrenomes_raros("JOSÉ DA SILVA SANTOS OLIVEIRA");
        assert!(sobrenomes.is_empty(), "Sobrenomes comuns da stop-list devem ser ignorados");
    }

    #[test]
    fn test_extrai_sobrenome_raro() {
        let sobrenomes = extrair_sobrenomes_raros("ANTONIO AUGUSTO ANASTASIA");
        assert_eq!(sobrenomes, vec!["AUGUSTO", "ANASTASIA"]);
    }

    #[test]
    fn test_auditar_possivel_parentesco_detecta_caso_real() {
        let alvos = vec![
            AlvoAuditoriaParentesco {
                nome: "MARCELO ANASTASIA EMPREENDIMENTOS".to_string(),
                cnpj_cpf: "12345678000199".to_string(),
                uf: "MG".to_string(),
                tipo_vinculo: "SOCIO_FORNECEDOR_CEAP".to_string(),
            },
            AlvoAuditoriaParentesco {
                nome: "JOAO CARLOS DA SILVA".to_string(),
                cnpj_cpf: "99988877766".to_string(),
                uf: "MG".to_string(),
                tipo_vinculo: "DOADOR_CAMPANHA".to_string(),
            },
        ];

        let alertas = auditar_possivel_parentesco("ANTONIO AUGUSTO ANASTASIA", "MG", &alvos);
        assert_eq!(alertas.len(), 1);
        assert_eq!(alertas[0].sobrenome_compartilhado, "ANASTASIA");
        assert_eq!(alertas[0].alvo_nome, "MARCELO ANASTASIA EMPREENDIMENTOS");
    }

    #[test]
    fn test_auditar_ignora_uf_divergente() {
        let alvos = vec![
            AlvoAuditoriaParentesco {
                nome: "MARCELO ANASTASIA".to_string(),
                cnpj_cpf: "12345678000199".to_string(),
                uf: "RS".to_string(), // UF divergente
                tipo_vinculo: "SOCIO_FORNECEDOR_CEAP".to_string(),
            },
        ];

        let alertas = auditar_possivel_parentesco("ANTONIO AUGUSTO ANASTASIA", "MG", &alvos);
        assert!(alertas.is_empty(), "Deve ignorar alvos com UF divergente");
    }
}
