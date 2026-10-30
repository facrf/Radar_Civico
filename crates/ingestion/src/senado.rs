use csv_async::AsyncReaderBuilder;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use storage::rusqlite::Connection;
use storage::{batch_insert_despesas_parlamentares, NovaDespesaParlamentar};
use tokio::io::AsyncRead;

use crate::error::{IngestionError, Result};
use crate::normalizer::limpar_cnpj;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SenadoDespesaRecord {
    pub ano: i32,
    pub mes: Option<i32>,
    pub senador_nome: String,
    pub tipo_despesa: String,
    pub fornecedor_nome: String,
    pub fornecedor_cnpj_cpf: String,
    pub data_emissao: String,
    pub valor_reembolsado: f64,
    pub numero_documento: Option<String>,
    pub detalhamento: Option<String>,
}

fn normalizar_header(s: &str) -> String {
    s.trim()
        .trim_start_matches('\u{feff}')
        .to_uppercase()
        .chars()
        .map(|c| match c {
            'Á' | 'À' | 'Ã' | 'Â' => 'A',
            'É' | 'È' | 'Ê' => 'E',
            'Í' | 'Ì' | 'Î' => 'I',
            'Ó' | 'Ò' | 'Õ' | 'Ô' => 'O',
            'Ú' | 'Ù' | 'Û' => 'U',
            'Ç' => 'C',
            other => other,
        })
        .collect::<String>()
        .replace([' ', '-', '_', '"', '\''], "")
}

fn find_col(headers: &csv_async::StringRecord, candidates: &[&str]) -> Option<usize> {
    for candidate in candidates {
        let cand_norm = normalizar_header(candidate);
        if let Some(index) = headers.iter().position(|h| normalizar_header(h) == cand_norm) {
            return Some(index);
        }
    }
    for candidate in candidates {
        let cand_norm = normalizar_header(candidate);
        if let Some(index) = headers.iter().position(|h| normalizar_header(h).contains(&cand_norm)) {
            return Some(index);
        }
    }
    None
}

fn normalizar_data(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        return "1970-01-01".to_string();
    }
    if s.contains('/') {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() == 3 && parts[2].len() == 4 {
            return format!("{}-{:0>2}-{:0>2}", parts[2], parts[1], parts[0]);
        }
    }
    if s.len() >= 10 && &s[4..5] == "-" && &s[7..8] == "-" {
        return s[..10].to_string();
    }
    s.to_string()
}

pub async fn processar_stream_senado_ceaps<R: AsyncRead + Unpin + Send>(
    reader: R,
    delimiter: u8,
) -> Result<Vec<SenadoDespesaRecord>> {
    let mut csv_reader = AsyncReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(true)
        .flexible(true)
        .create_reader(reader);

    let headers = csv_reader
        .headers()
        .await
        .map_err(IngestionError::Csv)?
        .clone();

    let col_ano = find_col(&headers, &["ANO"]);
    let col_mes = find_col(&headers, &["MES"]);
    let col_senador = find_col(&headers, &["SENADOR", "NOMESENADOR", "PARLAMENTAR"]);
    let col_tipo = find_col(&headers, &["TIPODESPESA", "DESPESA", "CATEGORIA"]);
    let col_forn_nome = find_col(&headers, &["FORNECEDOR", "NOMEFORNECEDOR", "RAZAOSOCIAL"]);
    let col_forn_doc = find_col(&headers, &["CNPJCPF", "CPFCNPJ", "FORNECEDORCNPJ", "CNPJ"]);
    let col_data = find_col(&headers, &["DATA", "DATAEMISSAO", "DATADOCUMENTO"]);
    let col_valor = find_col(&headers, &["VALORREEMBOLSADO", "VALOR", "VALORLIQUIDO", "VALORDOCUMENTO"]);
    let col_doc = find_col(&headers, &["DOCUMENTO", "NUMERODOCUMENTO", "NUMDOC"]);
    let col_detalhe = find_col(&headers, &["DETALHAMENTO", "DESCRICAO", "OBSERVACAO"]);

    if col_senador.is_none() || col_forn_doc.is_none() || col_valor.is_none() {
        return Err(IngestionError::Parse(
            "CSV do Senado sem colunas obrigatórias (SENADOR, CNPJ/CPF, VALOR)".into(),
        ));
    }

    let mut records = Vec::new();
    let mut records_stream = csv_reader.into_records();

    while let Some(record_res) = records_stream.next().await {
        let record = record_res.map_err(IngestionError::Csv)?;

        let senador = col_senador
            .and_then(|idx| record.get(idx))
            .unwrap_or("")
            .trim()
            .to_string();
        if senador.is_empty() {
            continue;
        }

        let raw_doc = col_forn_doc.and_then(|idx| record.get(idx)).unwrap_or("");
        let fornecedor_cnpj_cpf = limpar_cnpj(raw_doc);
        if fornecedor_cnpj_cpf.is_empty() {
            continue;
        }

        let valor_str = col_valor.and_then(|idx| record.get(idx)).unwrap_or("");
        let valor_reembolsado = crate::validation::money_field(
            Some(valor_str),
            record.position().map(|p| p.line()).unwrap_or(0),
            "valor_reembolsado",
        )?;

        let ano = col_ano
            .and_then(|idx| record.get(idx))
            .and_then(|s| s.trim().parse::<i32>().ok())
            .unwrap_or(0);

        let mes = col_mes
            .and_then(|idx| record.get(idx))
            .and_then(|s| s.trim().parse::<i32>().ok());

        let tipo_despesa = col_tipo
            .and_then(|idx| record.get(idx))
            .unwrap_or("DESPESA DIVERSA")
            .trim()
            .to_string();

        let fornecedor_nome = col_forn_nome
            .and_then(|idx| record.get(idx))
            .unwrap_or("FORNECEDOR")
            .trim()
            .to_string();

        let raw_data = col_data.and_then(|idx| record.get(idx)).unwrap_or("");
        let data_emissao = normalizar_data(raw_data);

        let numero_documento = col_doc
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let detalhamento = col_detalhe
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        records.push(SenadoDespesaRecord {
            ano,
            mes,
            senador_nome: senador,
            tipo_despesa,
            fornecedor_nome,
            fornecedor_cnpj_cpf,
            data_emissao,
            valor_reembolsado,
            numero_documento,
            detalhamento,
        });
    }

    Ok(records)
}

pub fn converter_senado_para_despesas(
    itens: &[SenadoDespesaRecord],
) -> Vec<NovaDespesaParlamentar> {
    itens
        .iter()
        .map(|item| {
            let doc_key = item.numero_documento.as_deref().unwrap_or("0");
            let id_origem = format!("senado_{}_{}_{}_{}", item.ano, item.mes.unwrap_or(0), item.fornecedor_cnpj_cpf, doc_key);

            NovaDespesaParlamentar {
                id_origem: Some(id_origem),
                casa_legislativa: "SENADO".to_string(),
                parlamentar_nome: item.senador_nome.clone(),
                parlamentar_cpf_mascarado: None,
                data_emissao: item.data_emissao.clone(),
                categoria_despesa: item.tipo_despesa.clone(),
                fornecedor_nome: item.fornecedor_nome.clone(),
                fornecedor_cnpj_cpf: item.fornecedor_cnpj_cpf.clone(),
                valor_liquido: item.valor_reembolsado,
                numero_documento: item.numero_documento.clone(),
                url_nota_fiscal: None,
                detalhes_litros: None,
            }
        })
        .collect()
}

pub fn ingerir_senado_ceaps_em_lotes(
    conn: &mut Connection,
    itens: &[SenadoDespesaRecord],
) -> Result<usize> {
    if itens.is_empty() {
        return Ok(0);
    }
    let despesas = converter_senado_para_despesas(itens);
    batch_insert_despesas_parlamentares(conn, &despesas).map_err(IngestionError::Storage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::{run_migrations, DbPool};

    #[tokio::test]
    async fn test_processar_stream_senado_ceaps() {
        let csv_data = "\
ANO;MES;SENADOR;TIPO_DESPESA;CNPJ_CPF;FORNECEDOR;DOCUMENTO;DATA;DETALHAMENTO;VALOR_REEMBOLSADO\n\
2024;3;SENADOR ALFA;Passagens aéreas;00.000.000/0001-91;COMPANHIA AEREA;TKT-99;15/03/2024;Brasilia para SP;1.850,50\n\
2024;3;SENADOR BETA;Combustíveis e lubrificantes;12.345.678/0001-90;POSTO BRASIL;NF-123;16/03/2024;Gasolina comum;320,00\n";

        let records = processar_stream_senado_ceaps(csv_data.as_bytes(), b';')
            .await
            .unwrap();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].senador_nome, "SENADOR ALFA");
        assert_eq!(records[0].fornecedor_cnpj_cpf, "00000000000191");
        assert_eq!(records[0].data_emissao, "2024-03-15");
        assert!((records[0].valor_reembolsado - 1850.50).abs() < 0.01);

        assert_eq!(records[1].senador_nome, "SENADOR BETA");
        assert_eq!(records[1].fornecedor_cnpj_cpf, "12345678000190");
        assert_eq!(records[1].data_emissao, "2024-03-16");
        assert!((records[1].valor_reembolsado - 320.00).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_ingerir_senado_ceaps_sqlite() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        let itens = vec![SenadoDespesaRecord {
            ano: 2024,
            mes: Some(4),
            senador_nome: "SENADOR TESTE".into(),
            tipo_despesa: "Consultoria Técnica".into(),
            fornecedor_nome: "EMPRESA DE CONSULTORIA".into(),
            fornecedor_cnpj_cpf: "11222333000188".into(),
            data_emissao: "2024-04-10".into(),
            valor_reembolsado: 12500.0,
            numero_documento: Some("NF-501".into()),
            detalhamento: Some("Parecer jurídico".into()),
        }];

        let inseridos = ingerir_senado_ceaps_em_lotes(&mut conn, &itens).unwrap();
        assert_eq!(inseridos, 1);

        let casa: String = conn
            .query_row(
                "SELECT casa_legislativa FROM despesas_parlamentares WHERE parlamentar_nome = 'SENADOR TESTE'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(casa, "SENADO");
    }
}
