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
}
