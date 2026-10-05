use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

pub const JANELA_DIAS_POSSE: i64 = 180;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoadorCampanha {
    pub doador_cpf_cnpj: String,
    pub doador_nome: String,
    pub politico_id: i64,
    pub politico_nome: String,
    pub data_posse: NaiveDate,
    pub valor_doado: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocioEmpresa {
    pub socio_cpf_cnpj: String,
    pub socio_nome: String,
    pub empresa_cnpj: String,
    pub empresa_razao_social: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContratoPublico {
    pub id: i64,
    pub orgao_contratante: String,
    pub fornecedor_cnpj: String,
    pub valor_contratado: f64,
    pub objeto: Option<String>,
    pub data_assinatura: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaTriangulacao {
    pub contrato_id: i64,
    pub politico_nome: String,
    pub doador_nome: String,
    pub doador_cpf: String,
    pub empresa_cnpj: String,
    pub empresa_razao_social: String,
    pub valor_contrato: f64,
    pub valor_doacao: f64,
    pub dias_apos_posse: i64,
    pub motivo: String,
    pub score_risco: u8,
}

fn limpar_documento(doc: &str) -> String {
    doc.chars().filter(|c| c.is_ascii_digit()).collect()
}

pub fn auditar_triangulacao(
    doadores: &[DoadorCampanha],
    socios: &[SocioEmpresa],
    contratos: &[ContratoPublico],
) -> Vec<AlertaTriangulacao> {
    let mut alertas = Vec::new();

    for doador in doadores {
        let doador_doc_clean = limpar_documento(&doador.doador_cpf_cnpj);

        // Encontrar empresas onde o doador é sócio
        let empresas_do_doador: Vec<&SocioEmpresa> = socios
            .iter()
            .filter(|s| {
                let socio_doc_clean = limpar_documento(&s.socio_cpf_cnpj);
                socio_doc_clean == doador_doc_clean
                    || (!doador.doador_nome.is_empty()
                        && s.socio_nome.trim().eq_ignore_ascii_case(doador.doador_nome.trim()))
            })
            .collect();

        for socio in empresas_do_doador {
            let empresa_cnpj_clean = limpar_documento(&socio.empresa_cnpj);

            // Encontrar contratos firmados por esta empresa
            for contrato in contratos {
                let forn_cnpj_clean = limpar_documento(&contrato.fornecedor_cnpj);
                if forn_cnpj_clean != empresa_cnpj_clean {
                    continue;
                }

                let dias = (contrato.data_assinatura - doador.data_posse).num_days();

                // Regra: Contrato assinado após a posse em janela <= 180 dias
                if dias >= 0 && dias <= JANELA_DIAS_POSSE {
                    let score_risco = if dias <= 60 {
                        95
                    } else if dias <= 120 {
                        85
                    } else {
                        70
                    };

                    alertas.push(AlertaTriangulacao {
                        contrato_id: contrato.id,
                        politico_nome: doador.politico_nome.clone(),
                        doador_nome: doador.doador_nome.clone(),
                        doador_cpf: doador.doador_cpf_cnpj.clone(),
                        empresa_cnpj: socio.empresa_cnpj.clone(),
                        empresa_razao_social: socio.empresa_razao_social.clone(),
                        valor_contrato: contrato.valor_contratado,
                        valor_doacao: doador.valor_doado,
                        dias_apos_posse: dias,
                        motivo: format!(
                            "Contrato público de R$ {:.2} assinado {} dias após a posse com empresa ({}) vinculada ao doador de campanha ({}) que doou R$ {:.2}.",
                            contrato.valor_contratado, dias, socio.empresa_razao_social, doador.doador_nome, doador.valor_doado
                        ),
                        score_risco,
                    });
                }
            }
        }
    }

    alertas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triangulacao_detecta_contrato_em_menos_de_180_dias() {
        let posse = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let data_contrato = NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(); // 59 dias após a posse

        let doadores = vec![DoadorCampanha {
            doador_cpf_cnpj: "123.456.789-00".to_string(),
            doador_nome: "EMPRESARIO DOADOR".to_string(),
            politico_id: 1,
            politico_nome: "PREFEITO ELEITO".to_string(),
            data_posse: posse,
            valor_doado: 50000.0,
        }];

        let socios = vec![SocioEmpresa {
            socio_cpf_cnpj: "12345678900".to_string(),
            socio_nome: "EMPRESARIO DOADOR".to_string(),
            empresa_cnpj: "11.222.333/0001-44".to_string(),
            empresa_razao_social: "CONSTRUTORA AMIGA LTDA".to_string(),
        }];

        let contratos = vec![ContratoPublico {
            id: 501,
            orgao_contratante: "PREFEITURA MUNICIPAL".to_string(),
            fornecedor_cnpj: "11222333000144".to_string(),
            valor_contratado: 2500000.0,
            objeto: Some("PAVIMENTACAO ASFALTICA".to_string()),
            data_assinatura: data_contrato,
        }];

        let alertas = auditar_triangulacao(&doadores, &socios, &contratos);
        assert_eq!(alertas.len(), 1);
        let a = &alertas[0];
        assert_eq!(a.contrato_id, 501);
        assert_eq!(a.dias_apos_posse, 59);
        assert_eq!(a.score_risco, 95);
        assert!(a.motivo.contains("CONSTRUTORA AMIGA LTDA"));
    }

    #[test]
    fn test_triangulacao_caso_controle_contrato_apos_180_dias() {
        let posse = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let data_contrato = NaiveDate::from_ymd_opt(2025, 9, 1).unwrap(); // 243 dias (> 180 dias)

        let doadores = vec![DoadorCampanha {
            doador_cpf_cnpj: "123.456.789-00".to_string(),
            doador_nome: "EMPRESARIO DOADOR".to_string(),
            politico_id: 1,
            politico_nome: "PREFEITO ELEITO".to_string(),
            data_posse: posse,
            valor_doado: 10000.0,
        }];

        let socios = vec![SocioEmpresa {
            socio_cpf_cnpj: "12345678900".to_string(),
            socio_nome: "EMPRESARIO DOADOR".to_string(),
            empresa_cnpj: "11.222.333/0001-44".to_string(),
            empresa_razao_social: "EMPRESA REGULAR".to_string(),
        }];

        let contratos = vec![ContratoPublico {
            id: 502,
            orgao_contratante: "PREFEITURA MUNICIPAL".to_string(),
            fornecedor_cnpj: "11222333000144".to_string(),
            valor_contratado: 100000.0,
            objeto: Some("SERVICOS".to_string()),
            data_assinatura: data_contrato,
        }];

        let alertas = auditar_triangulacao(&doadores, &socios, &contratos);
        assert!(alertas.is_empty());
    }

    #[test]
    fn test_triangulacao_caso_controle_sem_vinculo_societario() {
        let posse = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let data_contrato = NaiveDate::from_ymd_opt(2025, 2, 1).unwrap();

        let doadores = vec![DoadorCampanha {
            doador_cpf_cnpj: "999.888.777-66".to_string(),
            doador_nome: "OUTRO DOADOR".to_string(),
            politico_id: 1,
            politico_nome: "PREFEITO ELEITO".to_string(),
            data_posse: posse,
            valor_doado: 5000.0,
        }];

        let socios = vec![SocioEmpresa {
            socio_cpf_cnpj: "11122233344".to_string(), // CPF diferente
            socio_nome: "SOCIO DESCONHECIDO".to_string(),
            empresa_cnpj: "55.666.777/0001-88".to_string(),
            empresa_razao_social: "OUTRA EMPRESA".to_string(),
        }];

        let contratos = vec![ContratoPublico {
            id: 503,
            orgao_contratante: "PREFEITURA MUNICIPAL".to_string(),
            fornecedor_cnpj: "55666777000188".to_string(),
            valor_contratado: 300000.0,
            objeto: Some("MATERIAL ESCOLAR".to_string()),
            data_assinatura: data_contrato,
        }];

        let alertas = auditar_triangulacao(&doadores, &socios, &contratos);
        assert!(alertas.is_empty());
    }
}
