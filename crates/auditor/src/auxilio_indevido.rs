use serde::{Deserialize, Serialize};
use storage::rusqlite::Connection;
use storage::{batch_insert_alertas_beneficio, NovoAlertaBeneficioIndevido};

use crate::error::Result;

pub const LIMITE_BENS_AUXILIO: f64 = 300_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MotivoAuxilioIndevido {
    MandatoVigente,
    PatrimonioSuperior300k,
    Ambos,
}

impl MotivoAuxilioIndevido {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MandatoVigente => "MANDATO_VIGENTE",
            Self::PatrimonioSuperior300k => "PATRIMONIO_SUPERIOR_300K",
            Self::Ambos => "MANDATO_E_PATRIMONIO_SUPERIOR_300K",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoliticoPerfilAuxilio {
    pub politico_id: i64,
    pub nome: String,
    pub cpf_mascarado: String,
    pub total_bens: f64,
    pub tem_mandato_vigente: bool,
    pub cargo_mandato: Option<String>,
    pub ano_mandato: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistroAuxilio {
    pub beneficio_id: Option<i64>,
    pub cpf_mascarado: String,
    pub nome_beneficiario: String,
    pub valor: f64,
    pub mes_disponibilizacao: String,
    pub parcela: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaAuxilioIndevido {
    pub politico_id: i64,
    pub beneficio_id: Option<i64>,
    pub politico_nome: String,
    pub cpf_mascarado: String,
    pub motivo: MotivoAuxilioIndevido,
    pub detalhes: String,
    pub valor_beneficio: f64,
    pub total_bens_declarados: f64,
    pub cargo_ou_mandato: Option<String>,
    pub ano_exercicio: Option<i32>,
    pub severidade: String,
}

pub fn auditar_recebimento_auxilio(
    politico: &PoliticoPerfilAuxilio,
    beneficio: &RegistroAuxilio,
) -> Option<AlertaAuxilioIndevido> {
    // Normalização de CPF
    let cpf_pol_clean: String = politico.cpf_mascarado.chars().filter(|c| c.is_ascii_digit()).collect();
    let cpf_ben_clean: String = beneficio.cpf_mascarado.chars().filter(|c| c.is_ascii_digit()).collect();

    if !cpf_pol_clean.is_empty() && !cpf_ben_clean.is_empty() && cpf_pol_clean != cpf_ben_clean {
        return None;
    }

    let tem_mandato = politico.tem_mandato_vigente;
    let bens_acima_limite = politico.total_bens > LIMITE_BENS_AUXILIO;

    if !tem_mandato && !bens_acima_limite {
        return None;
    }

    let motivo = if tem_mandato && bens_acima_limite {
        MotivoAuxilioIndevido::Ambos
    } else if tem_mandato {
        MotivoAuxilioIndevido::MandatoVigente
    } else {
        MotivoAuxilioIndevido::PatrimonioSuperior300k
    };

    let severidade = match motivo {
        MotivoAuxilioIndevido::MandatoVigente | MotivoAuxilioIndevido::Ambos => "ALTA".to_string(),
        MotivoAuxilioIndevido::PatrimonioSuperior300k => "MEDIA".to_string(),
    };

    let detalhes = match motivo {
        MotivoAuxilioIndevido::Ambos => format!(
            "Recebimento de R$ {:.2} de Auxílio Emergencial em {} com mandato ativo de {} e patrimônio declarado de R$ {:.2} (acima do teto legal de R$ 300.000,00).",
            beneficio.valor,
            beneficio.mes_disponibilizacao,
            politico.cargo_mandato.as_deref().unwrap_or("MANDATO ELETIVO"),
            politico.total_bens
        ),
        MotivoAuxilioIndevido::MandatoVigente => format!(
            "Recebimento indevido de R$ {:.2} de Auxílio Emergencial em {} por agente público ocupante do mandato de {}.",
            beneficio.valor,
            beneficio.mes_disponibilizacao,
            politico.cargo_mandato.as_deref().unwrap_or("MANDATO ELETIVO")
        ),
        MotivoAuxilioIndevido::PatrimonioSuperior300k => format!(
            "Recebimento de R$ {:.2} de Auxílio Emergencial com patrimônio declarado de R$ {:.2}, excedendo o teto legal de R$ 300.000,00 (Lei 13.982/2020).",
            beneficio.valor,
            politico.total_bens
        ),
    };

    Some(AlertaAuxilioIndevido {
        politico_id: politico.politico_id,
        beneficio_id: beneficio.beneficio_id,
        politico_nome: politico.nome.clone(),
        cpf_mascarado: politico.cpf_mascarado.clone(),
        motivo,
        detalhes,
        valor_beneficio: beneficio.valor,
        total_bens_declarados: politico.total_bens,
        cargo_ou_mandato: politico.cargo_mandato.clone(),
        ano_exercicio: politico.ano_mandato,
        severidade,
    })
}

pub fn auditar_lote_auxilio_indevido(
    politicos: &[PoliticoPerfilAuxilio],
    beneficios: &[RegistroAuxilio],
) -> Vec<AlertaAuxilioIndevido> {
    let mut alertas = Vec::new();

    for p in politicos {
        for b in beneficios {
            if let Some(alerta) = auditar_recebimento_auxilio(p, b) {
                alertas.push(alerta);
            }
        }
    }

    alertas
}

pub fn executar_auditoria_auxilio_sqlite(conn: &mut Connection) -> Result<Vec<AlertaAuxilioIndevido>> {
    // 1. Coletar políticos com seus bens e mandatos vigentes
    let sql_politicos = "
        SELECT 
            p.id, 
            p.nome_completo, 
            p.cpf_mascarado,
            COALESCE(MAX(c.total_bens_declarados), 0.0) as total_bens,
            CASE 
                WHEN MAX(
                    CASE 
                        WHEN UPPER(c.situacao_totalizacao) LIKE '%ELEITO%' 
                             AND UPPER(c.situacao_totalizacao) NOT LIKE '%NÃO ELEITO%' 
                             AND UPPER(c.situacao_totalizacao) NOT LIKE '%NAO ELEITO%' 
                        THEN 1 
                        ELSE 0 
                    END
                ) = 1 THEN 1 
                ELSE 0 
            END as tem_mandato,
            MAX(
                CASE 
                    WHEN UPPER(c.situacao_totalizacao) LIKE '%ELEITO%' 
                         AND UPPER(c.situacao_totalizacao) NOT LIKE '%NÃO ELEITO%' 
                         AND UPPER(c.situacao_totalizacao) NOT LIKE '%NAO ELEITO%' 
                    THEN c.cargo 
                    ELSE NULL 
                END
            ) as cargo,
            MAX(c.ano_eleicao) as ano_eleicao
        FROM politicos p
        LEFT JOIN candidaturas c ON c.politico_id = p.id
        WHERE p.cpf_mascarado IS NOT NULL AND p.cpf_mascarado != ''
        GROUP BY p.id, p.nome_completo, p.cpf_mascarado
    ";

    let mut stmt_pol = conn.prepare(sql_politicos)?;
    let mut politicos_map = Vec::new();

    let rows_pol = stmt_pol.query_map([], |row| {
        let politico_id: i64 = row.get(0)?;
        let nome: String = row.get(1)?;
        let cpf_mascarado: String = row.get(2)?;
        let total_bens: f64 = row.get(3)?;
        let tem_mandato_int: i32 = row.get(4)?;
        let cargo_mandato: Option<String> = row.get(5)?;
        let ano_mandato: Option<i32> = row.get(6)?;

        Ok(PoliticoPerfilAuxilio {
            politico_id,
            nome,
            cpf_mascarado,
            total_bens,
            tem_mandato_vigente: tem_mandato_int == 1,
            cargo_mandato,
            ano_mandato,
        })
    })?;

    for p in rows_pol {
        politicos_map.push(p?);
    }
    drop(stmt_pol);

    // 2. Coletar benefícios pagos
    let sql_beneficios = "
        SELECT id, cpf_mascarado, nome_beneficiario, valor, mes_disponibilizacao, parcela
        FROM beneficios_emergenciais
    ";

    let mut stmt_ben = conn.prepare(sql_beneficios)?;
    let mut beneficios = Vec::new();

    let rows_ben = stmt_ben.query_map([], |row| {
        let beneficio_id: i64 = row.get(0)?;
        let cpf_mascarado: String = row.get(1)?;
        let nome_beneficiario: String = row.get(2)?;
        let valor: f64 = row.get(3)?;
        let mes_disponibilizacao: String = row.get(4)?;
        let parcela: Option<String> = row.get(5)?;

        Ok(RegistroAuxilio {
            beneficio_id: Some(beneficio_id),
            cpf_mascarado,
            nome_beneficiario,
            valor,
            mes_disponibilizacao,
            parcela,
        })
    })?;

    for b in rows_ben {
        beneficios.push(b?);
    }
    drop(stmt_ben);

    // 3. Executar auditoria
    let alertas = auditar_lote_auxilio_indevido(&politicos_map, &beneficios);

    // 4. Salvar na tabela alertas_beneficio_indevido
    let novos_alertas: Vec<NovoAlertaBeneficioIndevido> = alertas
        .iter()
        .map(|a| NovoAlertaBeneficioIndevido {
            politico_id: a.politico_id,
            beneficio_id: a.beneficio_id,
            motivo: a.motivo.as_str().to_string(),
            detalhes: Some(a.detalhes.clone()),
            valor_recebido: a.valor_beneficio,
            total_bens: Some(a.total_bens_declarados),
            cargo_ou_mandato: a.cargo_ou_mandato.clone(),
            ano_exercicio: a.ano_exercicio,
            status_analise: Some("PENDENTE".to_string()),
        })
        .collect();

    batch_insert_alertas_beneficio(conn, &novos_alertas)?;

    Ok(alertas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::{run_migrations, DbPool};

    #[test]
    fn test_auxilio_indevido_mandato_vigente() {
        let politico = PoliticoPerfilAuxilio {
            politico_id: 1,
            nome: "VEREADOR CARLOS".to_string(),
            cpf_mascarado: "***.123.456-**".to_string(),
            total_bens: 150_000.0, // Abaixo do limite de 300k
            tem_mandato_vigente: true,
            cargo_mandato: Some("VEREADOR".to_string()),
            ano_mandato: Some(2020),
        };

        let beneficio = RegistroAuxilio {
            beneficio_id: Some(10),
            cpf_mascarado: "***.123.456-**".to_string(),
            nome_beneficiario: "CARLOS DA SILVA".to_string(),
            valor: 600.0,
            mes_disponibilizacao: "202004".to_string(),
            parcela: Some("1".to_string()),
        };

        let alerta = auditar_recebimento_auxilio(&politico, &beneficio);
        assert!(alerta.is_some());
        let alerta = alerta.unwrap();

        assert_eq!(alerta.motivo, MotivoAuxilioIndevido::MandatoVigente);
        assert_eq!(alerta.severidade, "ALTA");
        assert_eq!(alerta.valor_beneficio, 600.0);
        assert!(alerta.detalhes.contains("mandato de VEREADOR"));
    }

    #[test]
    fn test_auxilio_indevido_patrimonio_superior_300k() {
        let politico = PoliticoPerfilAuxilio {
            politico_id: 2,
            nome: "CANDIDATO RICO".to_string(),
            cpf_mascarado: "***.777.888-**".to_string(),
            total_bens: 850_000.0, // Superior a 300k
            tem_mandato_vigente: false,
            cargo_mandato: Some("VEREADOR".to_string()),
            ano_mandato: Some(2020),
        };

        let beneficio = RegistroAuxilio {
            beneficio_id: Some(20),
            cpf_mascarado: "***.777.888-**".to_string(),
            nome_beneficiario: "CANDIDATO RICO".to_string(),
            valor: 1200.0,
            mes_disponibilizacao: "202005".to_string(),
            parcela: Some("2".to_string()),
        };

        let alerta = auditar_recebimento_auxilio(&politico, &beneficio);
        assert!(alerta.is_some());
        let alerta = alerta.unwrap();

        assert_eq!(alerta.motivo, MotivoAuxilioIndevido::PatrimonioSuperior300k);
        assert_eq!(alerta.severidade, "MEDIA");
        assert_eq!(alerta.total_bens_declarados, 850_000.0);
        assert!(alerta.detalhes.contains("300.000,00"));
    }

    #[test]
    fn test_auxilio_indevido_ambos_mandato_e_patrimonio() {
        let politico = PoliticoPerfilAuxilio {
            politico_id: 3,
            nome: "DEPUTADO MILIONARIO".to_string(),
            cpf_mascarado: "***.999.000-**".to_string(),
            total_bens: 1_500_000.0,
            tem_mandato_vigente: true,
            cargo_mandato: Some("DEPUTADO ESTADUAL".to_string()),
            ano_mandato: Some(2020),
        };

        let beneficio = RegistroAuxilio {
            beneficio_id: Some(30),
            cpf_mascarado: "***.999.000-**".to_string(),
            nome_beneficiario: "DEPUTADO MILIONARIO".to_string(),
            valor: 600.0,
            mes_disponibilizacao: "202006".to_string(),
            parcela: Some("3".to_string()),
        };

        let alerta = auditar_recebimento_auxilio(&politico, &beneficio);
        assert!(alerta.is_some());
        let alerta = alerta.unwrap();

        assert_eq!(alerta.motivo, MotivoAuxilioIndevido::Ambos);
        assert_eq!(alerta.severidade, "ALTA");
        assert!(alerta.detalhes.contains("DEPUTADO ESTADUAL"));
        assert!(alerta.detalhes.contains("1500000.00"));
    }

    #[test]
    fn test_auxilio_indevido_caso_controle_sem_infracao() {
        let politico = PoliticoPerfilAuxilio {
            politico_id: 4,
            nome: "CANDIDATO MODESTO SUPLENTE".to_string(),
            cpf_mascarado: "***.555.666-**".to_string(),
            total_bens: 45_000.0, // Bem abaixo de 300k
            tem_mandato_vigente: false, // Sem mandato
            cargo_mandato: None,
            ano_mandato: Some(2020),
        };

        let beneficio = RegistroAuxilio {
            beneficio_id: Some(40),
            cpf_mascarado: "***.555.666-**".to_string(),
            nome_beneficiario: "CANDIDATO MODESTO SUPLENTE".to_string(),
            valor: 600.0,
            mes_disponibilizacao: "202004".to_string(),
            parcela: Some("1".to_string()),
        };

        let alerta = auditar_recebimento_auxilio(&politico, &beneficio);
        assert!(alerta.is_none(), "Não deve gerar alerta para candidato sem mandato e bens baixos");
    }

    #[test]
    fn test_auxilio_indevido_executar_auditoria_sqlite() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // 1. Inserir político com mandato eleito
        conn.execute(
            "INSERT INTO politicos (nome_completo, nome_urna, cpf_mascarado)
             VALUES ('DEPUTADO INFRATOR', 'INFRATOR', '***.111.222-**')",
            [],
        ).unwrap();
        let pol1_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf, situacao_totalizacao, total_bens_declarados)
             VALUES (?1, 2020, 'PREFEITO', 'PARTIDO A', 'SP', 'ELEITO', 120000.0)",
            [pol1_id],
        ).unwrap();

        // 2. Inserir candidato com bens > 300k (não eleito)
        conn.execute(
            "INSERT INTO politicos (nome_completo, nome_urna, cpf_mascarado)
             VALUES ('CANDIDATO PATRIMONIO ALTO', 'RICO', '***.333.444-**')",
            [],
        ).unwrap();
        let pol2_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf, situacao_totalizacao, total_bens_declarados)
             VALUES (?1, 2020, 'VEREADOR', 'PARTIDO B', 'SP', 'SUPLENTE', 750000.0)",
            [pol2_id],
        ).unwrap();

        // 3. Inserir caso de controle (candidato humilde não eleito)
        conn.execute(
            "INSERT INTO politicos (nome_completo, nome_urna, cpf_mascarado)
             VALUES ('CANDIDATO HUMILDE', 'HUMILDE', '***.555.666-**')",
            [],
        ).unwrap();
        let pol3_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf, situacao_totalizacao, total_bens_declarados)
             VALUES (?1, 2020, 'VEREADOR', 'PARTIDO C', 'SP', 'NAO ELEITO', 15000.0)",
            [pol3_id],
        ).unwrap();

        // 4. Inserir benefícios correspondentes aos 3
        conn.execute(
            "INSERT INTO beneficios_emergenciais (cpf_mascarado, nome_beneficiario, municipio, uf, mes_disponibilizacao, parcela, valor)
             VALUES ('***.111.222-**', 'DEPUTADO INFRATOR', 'CAMPINAS', 'SP', '202004', '1', 600.0)",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO beneficios_emergenciais (cpf_mascarado, nome_beneficiario, municipio, uf, mes_disponibilizacao, parcela, valor)
             VALUES ('***.333.444-**', 'CANDIDATO PATRIMONIO ALTO', 'CAMPINAS', 'SP', '202004', '1', 600.0)",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO beneficios_emergenciais (cpf_mascarado, nome_beneficiario, municipio, uf, mes_disponibilizacao, parcela, valor)
             VALUES ('***.555.666-**', 'CANDIDATO HUMILDE', 'CAMPINAS', 'SP', '202004', '1', 600.0)",
            [],
        ).unwrap();

        // 5. Executar auditoria
        let alertas = executar_auditoria_auxilio_sqlite(&mut conn).unwrap();
        assert_eq!(alertas.len(), 2, "Devem ser gerados exatamente 2 alertas (mandato e bens > 300k)");

        // 6. Verificar persistência em alertas_beneficio_indevido
        let count_salvo: i64 = conn.query_row(
            "SELECT count(*) FROM alertas_beneficio_indevido",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count_salvo, 2);
    }
}
