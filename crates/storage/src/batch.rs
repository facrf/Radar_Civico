use rusqlite::Connection;
use serde::{Deserialize, Serialize};

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

    let tx = conn.transaction()?;
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

    let tx = conn.transaction()?;
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

    let tx = conn.transaction()?;
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

    let tx = conn.transaction()?;
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
}
