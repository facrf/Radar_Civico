use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::connection::transaction_immediate;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovaReceita {
    pub candidatura_id: Option<i64>,
    pub doador_cpf_cnpj: String,
    pub doador_nome: String,
    pub valor: f64,
    pub data_receita: Option<String>,
    pub tipo_origem: Option<String>,
    pub descricao: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovaDespesa {
    pub candidatura_id: Option<i64>,
    pub fornecedor_cpf_cnpj: String,
    pub fornecedor_nome: String,
    pub valor: f64,
    pub data_despesa: Option<String>,
    pub tipo_despesa: Option<String>,
    pub descricao: Option<String>,
}

pub fn batch_insert_receitas(conn: &mut Connection, receitas: &[NovaReceita]) -> Result<usize> {
    if receitas.is_empty() {
        return Ok(0);
    }

    let tx = transaction_immediate(conn)?;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO receitas_campanha (
                candidatura_id, doador_cpf_cnpj, doador_nome, valor, data_receita, tipo_origem, descricao
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;

        for r in receitas {
            stmt.execute(rusqlite::params![
                r.candidatura_id,
                r.doador_cpf_cnpj,
                r.doador_nome,
                r.valor,
                r.data_receita,
                r.tipo_origem,
                r.descricao,
            ])?;
        }
    }
    tx.commit()?;

    Ok(receitas.len())
}

pub fn batch_insert_despesas(conn: &mut Connection, despesas: &[NovaDespesa]) -> Result<usize> {
    if despesas.is_empty() {
        return Ok(0);
    }

    let tx = transaction_immediate(conn)?;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO despesas_campanha (
                candidatura_id, fornecedor_cpf_cnpj, fornecedor_nome, valor, data_despesa, tipo_despesa, descricao
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;

        for d in despesas {
            stmt.execute(rusqlite::params![
                d.candidatura_id,
                d.fornecedor_cpf_cnpj,
                d.fornecedor_nome,
                d.valor,
                d.data_despesa,
                d.tipo_despesa,
                d.descricao,
            ])?;
        }
    }
    tx.commit()?;

    Ok(despesas.len())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovoBeneficio {
    pub cpf_mascarado: String,
    pub nome_beneficiario: String,
    pub municipio: Option<String>,
    pub uf: Option<String>,
    pub mes_disponibilizacao: String,
    pub parcela: Option<String>,
    pub valor: f64,
    pub enquadramento: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovoAlertaBeneficioIndevido {
    pub politico_id: i64,
    pub beneficio_id: Option<i64>,
    pub motivo: String,
    pub detalhes: Option<String>,
    pub valor_recebido: f64,
    pub total_bens: Option<f64>,
    pub cargo_ou_mandato: Option<String>,
    pub ano_exercicio: Option<i32>,
    pub status_analise: Option<String>,
}

pub fn batch_insert_beneficios(conn: &mut Connection, beneficios: &[NovoBeneficio]) -> Result<usize> {
    if beneficios.is_empty() {
        return Ok(0);
    }

    let tx = transaction_immediate(conn)?;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO beneficios_emergenciais (
                cpf_mascarado, nome_beneficiario, municipio, uf, mes_disponibilizacao, parcela, valor, enquadramento
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;

        for b in beneficios {
            stmt.execute(rusqlite::params![
                b.cpf_mascarado,
                b.nome_beneficiario,
                b.municipio,
                b.uf,
                b.mes_disponibilizacao,
                b.parcela,
                b.valor,
                b.enquadramento,
            ])?;
        }
    }
    tx.commit()?;

    Ok(beneficios.len())
}

pub fn batch_insert_alertas_beneficio(
    conn: &mut Connection,
    alertas: &[NovoAlertaBeneficioIndevido],
) -> Result<usize> {
    if alertas.is_empty() {
        return Ok(0);
    }

    let tx = transaction_immediate(conn)?;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO alertas_beneficio_indevido (
                politico_id, beneficio_id, motivo, detalhes, valor_recebido, total_bens, cargo_ou_mandato, ano_exercicio, status_analise
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;

        for a in alertas {
            stmt.execute(rusqlite::params![
                a.politico_id,
                a.beneficio_id,
                a.motivo,
                a.detalhes,
                a.valor_recebido,
                a.total_bens,
                a.cargo_ou_mandato,
                a.ano_exercicio,
                a.status_analise.as_deref().unwrap_or("PENDENTE"),
            ])?;
        }
    }
    tx.commit()?;

    Ok(alertas.len())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovaDespesaParlamentar {
    pub casa_legislativa: String,
    pub parlamentar_nome: String,
    pub parlamentar_cpf_mascarado: Option<String>,
    pub data_emissao: String,
    pub categoria_despesa: String,
    pub fornecedor_nome: String,
    pub fornecedor_cnpj_cpf: String,
    pub valor_liquido: f64,
    pub numero_documento: Option<String>,
    pub url_nota_fiscal: Option<String>,
    pub detalhes_litros: Option<f64>,
}

pub fn batch_insert_despesas_parlamentares(
    conn: &mut Connection,
    despesas: &[NovaDespesaParlamentar],
) -> Result<usize> {
    if despesas.is_empty() {
        return Ok(0);
    }

    let tx = transaction_immediate(conn)?;
    let mut inseridos = 0;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT OR IGNORE INTO despesas_parlamentares (
                casa_legislativa, parlamentar_nome, parlamentar_cpf_mascarado,
                data_emissao, categoria_despesa, fornecedor_nome, fornecedor_cnpj_cpf,
                valor_liquido, numero_documento, url_nota_fiscal, detalhes_litros
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?;

        for d in despesas {
            let changes = stmt.execute(rusqlite::params![
                d.casa_legislativa,
                d.parlamentar_nome,
                d.parlamentar_cpf_mascarado,
                d.data_emissao,
                d.categoria_despesa,
                d.fornecedor_nome,
                d.fornecedor_cnpj_cpf,
                d.valor_liquido,
                d.numero_documento,
                d.url_nota_fiscal,
                d.detalhes_litros,
            ])?;
            if changes > 0 {
                inseridos += 1;
            }
        }
    }
    tx.commit()?;

    Ok(inseridos)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovoBemCandidato {
    pub candidatura_id: Option<i64>,
    pub tipo_bem: String,
    pub descricao: Option<String>,
    pub valor_declarado: f64,
}

pub fn batch_insert_bens_candidato(
    conn: &mut Connection,
    bens: &[NovoBemCandidato],
) -> Result<usize> {
    if bens.is_empty() {
        return Ok(0);
    }

    let tx = transaction_immediate(conn)?;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO bens_candidato (
                candidatura_id, tipo_bem, descricao, valor_declarado
             ) VALUES (?1, ?2, ?3, ?4)",
        )?;

        for b in bens {
            stmt.execute(rusqlite::params![
                b.candidatura_id,
                b.tipo_bem,
                b.descricao,
                b.valor_declarado,
            ])?;
        }
    }
    tx.commit()?;

    Ok(bens.len())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovoCandidatoTse {
    pub sq_candidato: String,
    pub cpf_mascarado: String,
    pub nome_completo: String,
    pub nome_urna: String,
    pub data_nascimento: Option<String>,
    pub grau_instrucao: Option<String>,
    pub ocupacao: Option<String>,
    pub ano_eleicao: i32,
    pub cargo: String,
    pub numero_urna: Option<i32>,
    pub sigla_partido: String,
    pub uf: String,
    pub municipio: Option<String>,
    pub situacao_totalizacao: Option<String>,
}

pub fn batch_insert_candidatos_tse(
    conn: &mut Connection,
    candidatos: &[NovoCandidatoTse],
) -> Result<usize> {
    if candidatos.is_empty() {
        return Ok(0);
    }

    let tx = transaction_immediate(conn)?;
    {
        // 1. Tabela temporária de staging
        tx.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS staging_candidatos (
                sq_candidato TEXT,
                cpf_mascarado TEXT,
                nome_completo TEXT,
                nome_urna TEXT,
                data_nascimento TEXT,
                grau_instrucao TEXT,
                ocupacao TEXT,
                ano_eleicao INTEGER,
                cargo TEXT,
                numero_urna INTEGER,
                sigla_partido TEXT,
                uf TEXT,
                municipio TEXT,
                situacao_totalizacao TEXT
            );",
        )?;

        let mut stmt_staging = tx.prepare_cached(
            "INSERT INTO staging_candidatos (
                sq_candidato, cpf_mascarado, nome_completo, nome_urna,
                data_nascimento, grau_instrucao, ocupacao, ano_eleicao,
                cargo, numero_urna, sigla_partido, uf, municipio, situacao_totalizacao
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )?;

        for c in candidatos {
            stmt_staging.execute(rusqlite::params![
                c.sq_candidato,
                c.cpf_mascarado,
                c.nome_completo,
                c.nome_urna,
                c.data_nascimento,
                c.grau_instrucao,
                c.ocupacao,
                c.ano_eleicao,
                c.cargo,
                c.numero_urna,
                c.sigla_partido,
                c.uf,
                c.municipio,
                c.situacao_totalizacao,
            ])?;
        }

        // 2. Carga em massa relacional com ON CONFLICT (deduplicando dentro do lote via ROW_NUMBER)
        tx.execute_batch(
            "INSERT INTO politicos (
                sq_candidato, cpf_mascarado, nome_completo, nome_urna,
                data_nascimento, grau_instrucao, ocupacao
             )
             SELECT sq_candidato, cpf_mascarado, nome_completo, nome_urna,
                    data_nascimento, grau_instrucao, ocupacao
             FROM (
                 SELECT sq_candidato, cpf_mascarado, nome_completo, nome_urna,
                        data_nascimento, grau_instrucao, ocupacao,
                        ROW_NUMBER() OVER(PARTITION BY sq_candidato ORDER BY rowid DESC) AS rn
                 FROM staging_candidatos
             )
             WHERE rn = 1
             ON CONFLICT(sq_candidato) DO UPDATE SET
                cpf_mascarado = excluded.cpf_mascarado,
                nome_completo = excluded.nome_completo,
                nome_urna = excluded.nome_urna;

             INSERT INTO candidaturas (
                politico_id, ano_eleicao, cargo, numero_urna, sigla_partido, uf, municipio, situacao_totalizacao
             )
             SELECT p.id, s.ano_eleicao, s.cargo, s.numero_urna, s.sigla_partido, s.uf, s.municipio, s.situacao_totalizacao
             FROM (
                 SELECT sq_candidato, ano_eleicao, cargo, numero_urna, sigla_partido, uf, municipio, situacao_totalizacao,
                        ROW_NUMBER() OVER(PARTITION BY sq_candidato, ano_eleicao, cargo ORDER BY rowid DESC) AS rn
                 FROM staging_candidatos
             ) s
             JOIN politicos p ON p.sq_candidato = s.sq_candidato
             WHERE s.rn = 1
             ON CONFLICT(politico_id, ano_eleicao, cargo) DO UPDATE SET
                numero_urna = excluded.numero_urna,
                sigla_partido = excluded.sigla_partido,
                uf = excluded.uf,
                municipio = excluded.municipio,
                situacao_totalizacao = excluded.situacao_totalizacao;

             DELETE FROM staging_candidatos;",
        )?;
    }
    tx.commit()?;

    Ok(candidatos.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::DbPool;
    use crate::migrations::run_migrations;

    #[test]
    fn test_batch_insert_receitas_and_despesas() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;
        run_migrations(&mut conn)?;

        let receitas: Vec<NovaReceita> = (0..100)
            .map(|i| NovaReceita {
                candidatura_id: None,
                doador_cpf_cnpj: format!("111222333{:02}", i % 100),
                doador_nome: format!("DOADOR TESTE {}", i),
                valor: 100.0 + (i as f64),
                data_receita: Some("2024-08-15".to_string()),
                tipo_origem: Some("Doação PF".to_string()),
                descricao: Some("Doação via PIX".to_string()),
            })
            .collect();

        let inserted_rec = batch_insert_receitas(&mut conn, &receitas)?;
        assert_eq!(inserted_rec, 100);

        let rec_count: i64 = conn.query_row("SELECT count(*) FROM receitas_campanha", [], |r| r.get(0))?;
        assert_eq!(rec_count, 100);

        let despesas: Vec<NovaDespesa> = (0..150)
            .map(|i| NovaDespesa {
                candidatura_id: None,
                fornecedor_cpf_cnpj: format!("998877660001{:02}", i % 100),
                fornecedor_nome: format!("FORNECEDOR TESTE {}", i),
                valor: 50.0 + (i as f64),
                data_despesa: Some("2024-09-01".to_string()),
                tipo_despesa: Some("Publicidade".to_string()),
                descricao: Some("Serviços gráficos".to_string()),
            })
            .collect();

        let inserted_desp = batch_insert_despesas(&mut conn, &despesas)?;
        assert_eq!(inserted_desp, 150);

        let desp_count: i64 = conn.query_row("SELECT count(*) FROM despesas_campanha", [], |r| r.get(0))?;
        assert_eq!(desp_count, 150);

        // Verify triggers populated fornecedores_fts
        let fts_count: i64 = conn.query_row("SELECT count(*) FROM fornecedores_fts", [], |r| r.get(0))?;
        assert_eq!(fts_count, 150);

        Ok(())
    }

    #[test]
    fn test_batch_insert_beneficios_and_alertas() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;
        run_migrations(&mut conn)?;

        conn.execute(
            "INSERT INTO politicos (nome_completo, nome_urna, cpf_mascarado)
             VALUES ('DEPUTADO TESTE', 'TESTE', '***.000.111-**')",
            [],
        )?;
        let politico_id = conn.last_insert_rowid();

        let beneficios: Vec<NovoBeneficio> = (0..50)
            .map(|i| NovoBeneficio {
                cpf_mascarado: format!("***.{:03}.456-**", i),
                nome_beneficiario: format!("BENEFICIARIO {}", i),
                municipio: Some("SAO PAULO".to_string()),
                uf: Some("SP".to_string()),
                mes_disponibilizacao: "202004".to_string(),
                parcela: Some("1".to_string()),
                valor: 600.0,
                enquadramento: Some("EXTRA CAD".to_string()),
            })
            .collect();

        let inserted = batch_insert_beneficios(&mut conn, &beneficios)?;
        assert_eq!(inserted, 50);

        let count: i64 = conn.query_row("SELECT count(*) FROM beneficios_emergenciais", [], |r| r.get(0))?;
        assert_eq!(count, 50);

        let alertas = vec![NovoAlertaBeneficioIndevido {
            politico_id,
            beneficio_id: Some(1),
            motivo: "PATRIMONIO_SUPERIOR_300K".to_string(),
            detalhes: Some("Patrimonio declarado de R$ 450.000,00".to_string()),
            valor_recebido: 600.0,
            total_bens: Some(450000.0),
            cargo_ou_mandato: Some("VEREADOR".to_string()),
            ano_exercicio: Some(2020),
            status_analise: Some("PENDENTE".to_string()),
        }];

        let inserted_alertas = batch_insert_alertas_beneficio(&mut conn, &alertas)?;
        assert_eq!(inserted_alertas, 1);

        let count_alertas: i64 = conn.query_row("SELECT count(*) FROM alertas_beneficio_indevido", [], |r| r.get(0))?;
        assert_eq!(count_alertas, 1);

        Ok(())
    }

    #[test]
    fn test_batch_insert_despesas_parlamentares() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;
        run_migrations(&mut conn)?;

        let despesas = vec![
            NovaDespesaParlamentar {
                casa_legislativa: "CAMARA".to_string(),
                parlamentar_nome: "DEPUTADO JOAO".to_string(),
                parlamentar_cpf_mascarado: Some("***.123.456-**".to_string()),
                data_emissao: "2024-04-10".to_string(),
                categoria_despesa: "COMBUSTIVEIS".to_string(),
                fornecedor_nome: "POSTO CENTRAL".to_string(),
                fornecedor_cnpj_cpf: "00111222000133".to_string(),
                valor_liquido: 300.0,
                numero_documento: Some("NF-100".to_string()),
                url_nota_fiscal: Some("https://nf.exemplo.com/100".to_string()),
                detalhes_litros: Some(50.0),
            },
            NovaDespesaParlamentar {
                casa_legislativa: "CAMARA".to_string(),
                parlamentar_nome: "DEPUTADA MARIA".to_string(),
                parlamentar_cpf_mascarado: Some("***.987.654-**".to_string()),
                data_emissao: "2024-04-12".to_string(),
                categoria_despesa: "PASSAGEM AEREA".to_string(),
                fornecedor_nome: "GOL LINHAS AEREAS".to_string(),
                fornecedor_cnpj_cpf: "07575651000159".to_string(),
                valor_liquido: 1250.80,
                numero_documento: Some("ETK-5544".to_string()),
                url_nota_fiscal: None,
                detalhes_litros: None,
            },
        ];

        let inseridos = batch_insert_despesas_parlamentares(&mut conn, &despesas)?;
        assert_eq!(inseridos, 2);

        let total: i64 = conn.query_row("SELECT count(*) FROM despesas_parlamentares", [], |r| r.get(0))?;
        assert_eq!(total, 2);

        // Testa idempotência: re-inserir o mesmo lote não duplica e retorna 0 novos inseridos
        let re_inseridos = batch_insert_despesas_parlamentares(&mut conn, &despesas)?;
        assert_eq!(re_inseridos, 0);

        let total_pos: i64 = conn.query_row("SELECT count(*) FROM despesas_parlamentares", [], |r| r.get(0))?;
        assert_eq!(total_pos, 2);

        Ok(())
    }

    #[test]
    fn test_batch_insert_candidatos_tse_staging() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;
        run_migrations(&mut conn)?;

        let candidatos = vec![
            NovoCandidatoTse {
                sq_candidato: "1001".to_string(),
                cpf_mascarado: "***.111.222-**".to_string(),
                nome_completo: "CANDIDATO UM".to_string(),
                nome_urna: "UM".to_string(),
                data_nascimento: Some("1980-01-01".to_string()),
                grau_instrucao: Some("SUPERIOR".to_string()),
                ocupacao: Some("ADVOGADO".to_string()),
                ano_eleicao: 2024,
                cargo: "PREFEITO".to_string(),
                numero_urna: Some(10),
                sigla_partido: "PART1".to_string(),
                uf: "SP".to_string(),
                municipio: Some("SAO PAULO".to_string()),
                situacao_totalizacao: Some("ELEITO".to_string()),
            },
            NovoCandidatoTse {
                sq_candidato: "1002".to_string(),
                cpf_mascarado: "***.333.444-**".to_string(),
                nome_completo: "CANDIDATO DOIS".to_string(),
                nome_urna: "DOIS".to_string(),
                data_nascimento: Some("1985-05-05".to_string()),
                grau_instrucao: Some("MEDIO".to_string()),
                ocupacao: Some("EMPRESARIO".to_string()),
                ano_eleicao: 2024,
                cargo: "VEREADOR".to_string(),
                numero_urna: Some(10001),
                sigla_partido: "PART2".to_string(),
                uf: "SP".to_string(),
                municipio: Some("SAO PAULO".to_string()),
                situacao_totalizacao: Some("SUPLENTE".to_string()),
            },
            // Registro duplicado no mesmo lote para testar deduplicação/conflito
            NovoCandidatoTse {
                sq_candidato: "1001".to_string(),
                cpf_mascarado: "***.111.222-**".to_string(),
                nome_completo: "CANDIDATO UM ATUALIZADO".to_string(),
                nome_urna: "UM NOVO".to_string(),
                data_nascimento: Some("1980-01-01".to_string()),
                grau_instrucao: Some("SUPERIOR".to_string()),
                ocupacao: Some("ADVOGADO".to_string()),
                ano_eleicao: 2024,
                cargo: "PREFEITO".to_string(),
                numero_urna: Some(10),
                sigla_partido: "PART1".to_string(),
                uf: "SP".to_string(),
                municipio: Some("SAO PAULO".to_string()),
                situacao_totalizacao: Some("ELEITO".to_string()),
            },
        ];

        let n = batch_insert_candidatos_tse(&mut conn, &candidatos)?;
        assert_eq!(n, 3);

        let total_politicos: i64 = conn.query_row("SELECT count(*) FROM politicos", [], |r| r.get(0))?;
        assert_eq!(total_politicos, 2);

        let total_candidaturas: i64 = conn.query_row("SELECT count(*) FROM candidaturas", [], |r| r.get(0))?;
        assert_eq!(total_candidaturas, 2);

        let nome: String = conn.query_row("SELECT nome_completo FROM politicos WHERE sq_candidato = '1001'", [], |r| r.get(0))?;
        assert_eq!(nome, "CANDIDATO UM ATUALIZADO");

        Ok(())
    }
}
