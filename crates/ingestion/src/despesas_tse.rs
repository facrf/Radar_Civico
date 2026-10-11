use csv_async::AsyncReaderBuilder;
use futures::StreamExt;
use storage::rusqlite::Connection;
use storage::{batch_insert_despesas, NovaDespesa};
use tokio::io::AsyncRead;

use crate::error::{IngestionError, Result};
use crate::normalizer::limpar_cnpj;

#[derive(Debug, Clone, PartialEq)]
pub struct DespesaTseCsvRecord {
    pub sq_candidato: Option<String>,
    pub fornecedor_cpf_cnpj: String,
    pub fornecedor_nome: String,
    pub valor: f64,
    pub data_despesa: Option<String>,
    pub tipo_despesa: Option<String>,
    pub descricao: Option<String>,
}

pub async fn processar_stream_despesas_tse<R: AsyncRead + Unpin + Send>(
    reader: R,
) -> Result<Vec<DespesaTseCsvRecord>> {
    let mut csv_reader = AsyncReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .flexible(true)
        .create_reader(reader);

    let headers = csv_reader
        .headers()
        .await
        .map_err(|e| IngestionError::Csv(e))?
        .clone();

    let col_sq = headers.iter().position(|h| h.trim() == "SQ_CANDIDATO");
    let col_forn_doc = headers.iter().position(|h| {
        let clean = h.trim();
        clean == "NR_CPF_CNPJ_FORNECEDOR"
            || clean == "NR_CPF_FORNECEDOR"
            || clean == "CD_CPF_CNPJ_FORNECEDOR"
    });
    let col_forn_nome = headers.iter().position(|h| {
        let clean = h.trim();
        clean == "NM_FORNECEDOR" || clean == "NM_RAZAO_SOCIAL_FORNECEDOR"
    });
    let col_valor = headers.iter().position(|h| {
        let clean = h.trim();
        clean == "VR_DESPESA" || clean == "VR_PAGTO_DESPESA" || clean == "VR_DOCUMENTO"
    });
    let col_data = headers.iter().position(|h| {
        let clean = h.trim();
        clean == "DT_DESPESA" || clean == "DT_PAGTO" || clean == "DT_DOCUMENTO"
    });
    let col_tipo = headers.iter().position(|h| {
        let clean = h.trim();
        clean == "DS_TIPO_DOCUMENTO" || clean == "DS_ORIGEM_DESPESA" || clean == "DS_TIPO_DESPESA"
    });
    let col_desc = headers.iter().position(|h| {
        let clean = h.trim();
        clean == "DS_DESPESA" || clean == "DS_HISTORICO"
    });

    let mut records = Vec::new();
    let mut records_stream = csv_reader.into_records();

    while let Some(record_res) = records_stream.next().await {
        let record = record_res.map_err(|e| IngestionError::Csv(e))?;

        let fornecedor_nome = col_forn_nome
            .and_then(|idx| record.get(idx))
            .unwrap_or("")
            .trim()
            .to_string();

        let raw_doc = col_forn_doc.and_then(|idx| record.get(idx)).unwrap_or("");
        let fornecedor_cpf_cnpj = limpar_cnpj(raw_doc);

        let valor_str = col_valor.and_then(|idx| record.get(idx)).unwrap_or("");
        let valor = crate::validation::money_field(
            Some(valor_str),
            record.position().map(|p| p.line()).unwrap_or(0),
            "valor",
        )?;

        let sq_candidato = col_sq
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let data_despesa = col_data
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let tipo_despesa = col_tipo
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let descricao = col_desc
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        records.push(DespesaTseCsvRecord {
            sq_candidato,
            fornecedor_cpf_cnpj,
            fornecedor_nome,
            valor,
            data_despesa,
            tipo_despesa,
            descricao,
        });
    }

    Ok(records)
}

pub async fn ingerir_despesas_tse_em_lotes<R: AsyncRead + Unpin + Send>(
    conn: &mut Connection,
    reader: R,
    candidatura_id: Option<i64>,
    batch_size: usize,
) -> Result<usize> {
    let despesas = processar_stream_despesas_tse(reader).await?;
    let mut total_inserido = 0;

    for chunk in despesas.chunks(batch_size) {
        let batch: Vec<NovaDespesa> = chunk
            .iter()
            .map(|d| NovaDespesa {
                candidatura_id,
                fornecedor_cpf_cnpj: d.fornecedor_cpf_cnpj.clone(),
                fornecedor_nome: d.fornecedor_nome.clone(),
                valor: d.valor,
                data_despesa: d.data_despesa.clone(),
                tipo_despesa: d.tipo_despesa.clone(),
                descricao: d.descricao.clone(),
            })
            .collect();

        let inseridos = batch_insert_despesas(conn, &batch)?;
        total_inserido += inseridos;
    }

    Ok(total_inserido)
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::{run_migrations, DbPool};

    #[tokio::test]
    async fn test_processar_stream_despesas_tse() {
        let csv_data = "\
SQ_CANDIDATO;NR_CPF_CNPJ_FORNECEDOR;NM_FORNECEDOR;VR_DESPESA;DT_DESPESA;DS_TIPO_DESPESA;DS_DESPESA\n\
250001234567;12.345.678/0001-90;GRAFICA IMPRESSAO RAPIDA LTDA;12.450,75;2024-09-05;Publicidade Impressa;Santinhos e adesivos\n\
250001234567;98.765.432/0001-10;AGENCIA MARKETING DIGITAL;35.000,00;2024-09-12;Criacao de Website;Gestao de redes sociais\n";

        let records = processar_stream_despesas_tse(csv_data.as_bytes())
            .await
            .unwrap();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].fornecedor_cpf_cnpj, "12345678000190");
        assert_eq!(records[0].fornecedor_nome, "GRAFICA IMPRESSAO RAPIDA LTDA");
        assert!((records[0].valor - 12450.75).abs() < 0.01);
        assert_eq!(
            records[0].tipo_despesa.as_deref(),
            Some("Publicidade Impressa")
        );

        assert_eq!(records[1].fornecedor_cpf_cnpj, "98765432000110");
        assert!((records[1].valor - 35000.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_ingerir_despesas_tse_em_lotes() {
        let csv_data = "\
SQ_CANDIDATO;NR_CPF_CNPJ_FORNECEDOR;NM_FORNECEDOR;VR_DESPESA;DT_DESPESA;DS_TIPO_DESPESA;DS_DESPESA\n\
250001234567;12345678000190;FORNECEDOR A;1000,00;2024-09-01;Servicos;Consultoria\n\
250001234567;98765432000110;FORNECEDOR B;2000,00;2024-09-02;Servicos;Transporte\n";

        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        let total = ingerir_despesas_tse_em_lotes(&mut conn, csv_data.as_bytes(), None, 100)
            .await
            .unwrap();

        assert_eq!(total, 2);

        let count: i64 = conn
            .query_row("SELECT count(*) FROM despesas_campanha", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let soma: f64 = conn
            .query_row("SELECT sum(valor) FROM despesas_campanha", [], |r| r.get(0))
            .unwrap();
        assert_eq!(soma, 3000.0);
    }
}
