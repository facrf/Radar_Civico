use storage::rusqlite::Connection;
use storage::{
    aplicar_pragmas_ingestao, batch_insert_alertas_beneficio, batch_insert_beneficios,
    batch_insert_bens_candidato, batch_insert_candidatos_tse, batch_insert_despesas,
    batch_insert_despesas_parlamentares, batch_insert_receitas, DbPool, NovaDespesa,
    NovaDespesaParlamentar, NovaReceita, NovoAlertaBeneficioIndevido, NovoBemCandidato,
    NovoBeneficio, NovoCandidatoTse,
};
use tokio::task::spawn_blocking;

use crate::error::{IngestionError, Result};

pub const DEFAULT_SINK_BATCH_SIZE: usize = 25_000;

#[derive(Clone)]
pub struct BatchSink {
    pool: DbPool,
    batch_size: usize,
}

impl BatchSink {
    pub fn new(pool: DbPool) -> Self {
        Self {
            pool,
            batch_size: DEFAULT_SINK_BATCH_SIZE,
        }
    }

    pub fn with_batch_size(pool: DbPool, batch_size: usize) -> Self {
        Self { pool, batch_size }
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    pub async fn execute_in_transaction<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut Connection) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let pool = self.pool.clone();
        spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| IngestionError::Storage(e))?;
            aplicar_pragmas_ingestao(&conn).map_err(|e| IngestionError::Storage(e))?;
            f(&mut conn)
        })
        .await
        .map_err(|e| IngestionError::TokioJoin(e.to_string()))?
    }

    pub async fn insert_receitas(&self, records: Vec<NovaReceita>) -> Result<usize> {
        if records.is_empty() {
            return Ok(0);
        }
        let pool = self.pool.clone();
        spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| IngestionError::Storage(e))?;
            aplicar_pragmas_ingestao(&conn).map_err(|e| IngestionError::Storage(e))?;
            batch_insert_receitas(&mut conn, &records).map_err(|e| IngestionError::Storage(e))
        })
        .await
        .map_err(|e| IngestionError::TokioJoin(e.to_string()))?
    }

    pub async fn insert_despesas(&self, records: Vec<NovaDespesa>) -> Result<usize> {
        if records.is_empty() {
            return Ok(0);
        }
        let pool = self.pool.clone();
        spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| IngestionError::Storage(e))?;
            aplicar_pragmas_ingestao(&conn).map_err(|e| IngestionError::Storage(e))?;
            batch_insert_despesas(&mut conn, &records).map_err(|e| IngestionError::Storage(e))
        })
        .await
        .map_err(|e| IngestionError::TokioJoin(e.to_string()))?
    }

    pub async fn insert_candidatos(&self, records: Vec<NovoCandidatoTse>) -> Result<usize> {
        if records.is_empty() {
            return Ok(0);
        }
        let pool = self.pool.clone();
        spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| IngestionError::Storage(e))?;
            aplicar_pragmas_ingestao(&conn).map_err(|e| IngestionError::Storage(e))?;
            batch_insert_candidatos_tse(&mut conn, &records).map_err(|e| IngestionError::Storage(e))
        })
        .await
        .map_err(|e| IngestionError::TokioJoin(e.to_string()))?
    }

    pub async fn insert_bens(&self, records: Vec<NovoBemCandidato>) -> Result<usize> {
        if records.is_empty() {
            return Ok(0);
        }
        let pool = self.pool.clone();
        spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| IngestionError::Storage(e))?;
            aplicar_pragmas_ingestao(&conn).map_err(|e| IngestionError::Storage(e))?;
            batch_insert_bens_candidato(&mut conn, &records).map_err(|e| IngestionError::Storage(e))
        })
        .await
        .map_err(|e| IngestionError::TokioJoin(e.to_string()))?
    }

    pub async fn insert_despesas_parlamentares(
        &self,
        records: Vec<NovaDespesaParlamentar>,
    ) -> Result<usize> {
        if records.is_empty() {
            return Ok(0);
        }
        let pool = self.pool.clone();
        spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| IngestionError::Storage(e))?;
            aplicar_pragmas_ingestao(&conn).map_err(|e| IngestionError::Storage(e))?;
            batch_insert_despesas_parlamentares(&mut conn, &records)
                .map_err(|e| IngestionError::Storage(e))
        })
        .await
        .map_err(|e| IngestionError::TokioJoin(e.to_string()))?
    }

    pub async fn insert_beneficios(&self, records: Vec<NovoBeneficio>) -> Result<usize> {
        if records.is_empty() {
            return Ok(0);
        }
        let pool = self.pool.clone();
        spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| IngestionError::Storage(e))?;
            aplicar_pragmas_ingestao(&conn).map_err(|e| IngestionError::Storage(e))?;
            batch_insert_beneficios(&mut conn, &records).map_err(|e| IngestionError::Storage(e))
        })
        .await
        .map_err(|e| IngestionError::TokioJoin(e.to_string()))?
    }

    pub async fn insert_alertas_beneficio(
        &self,
        records: Vec<NovoAlertaBeneficioIndevido>,
    ) -> Result<usize> {
        if records.is_empty() {
            return Ok(0);
        }
        let pool = self.pool.clone();
        spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| IngestionError::Storage(e))?;
            aplicar_pragmas_ingestao(&conn).map_err(|e| IngestionError::Storage(e))?;
            batch_insert_alertas_beneficio(&mut conn, &records).map_err(|e| IngestionError::Storage(e))
        })
        .await
        .map_err(|e| IngestionError::TokioJoin(e.to_string()))?
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use storage::run_migrations;

    #[tokio::test]
    async fn test_batch_sink_receitas_com_pragmas_e_transacao() {
        let pool = DbPool::open_in_memory().expect("pool em memoria");
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }

        let sink = BatchSink::new(pool.clone());
        let receitas = vec![
            NovaReceita {
                candidatura_id: None,
                doador_cpf_cnpj: "11122233344".to_string(),
                doador_nome: "Doador A".to_string(),
                valor: 5000.0,
                data_receita: Some("2024-09-01".to_string()),
                tipo_origem: Some("Transferencia".to_string()),
                descricao: Some("Doacao de campanha".to_string()),
            },
            NovaReceita {
                candidatura_id: None,
                doador_cpf_cnpj: "55566677788".to_string(),
                doador_nome: "Doador B".to_string(),
                valor: 15000.0,
                data_receita: Some("2024-09-02".to_string()),
                tipo_origem: Some("PIX".to_string()),
                descricao: Some("Doacao direta".to_string()),
            },
        ];

        let inserted = sink.insert_receitas(receitas).await.expect("inserir receitas");
        assert_eq!(inserted, 2);

        let conn = pool.get().unwrap();
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM receitas_campanha", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 2);
    }

    #[tokio::test]
    async fn test_batch_sink_despesas_parlamentares() {
        let pool = DbPool::open_in_memory().expect("pool em memoria");
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }

        let sink = BatchSink::new(pool.clone());
        let despesas = vec![NovaDespesaParlamentar {
            casa_legislativa: "camara".to_string(),
            parlamentar_nome: "Deputado X".to_string(),
            parlamentar_cpf_mascarado: Some("***.123.456-**".to_string()),
            data_emissao: "2024-03-10".to_string(),
            categoria_despesa: "COMBUSTIVEL".to_string(),
            fornecedor_nome: "Posto Central".to_string(),
            fornecedor_cnpj_cpf: "12345678000199".to_string(),
            valor_liquido: 250.0,
            numero_documento: Some("NF-1234".to_string()),
            url_nota_fiscal: None,
            detalhes_litros: Some(45.0),
        }];

        let inserted = sink
            .insert_despesas_parlamentares(despesas)
            .await
            .expect("inserir despesas parlamentares");
        assert_eq!(inserted, 1);

        let conn = pool.get().unwrap();
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM despesas_parlamentares", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 1);
    }
}
