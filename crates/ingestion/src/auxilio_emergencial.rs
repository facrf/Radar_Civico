use csv_async::AsyncReaderBuilder;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use storage::rusqlite::Connection;
use storage::{batch_insert_beneficios, NovoBeneficio};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt};

use crate::error::{IngestionError, Result};
use crate::normalizer::mascarar_cpf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeneficioCsvRecord {
    pub mes_disponibilizacao: String,
    pub uf: Option<String>,
    pub municipio: Option<String>,
    pub cpf_mascarado: String,
    pub nome_beneficiario: String,
    pub enquadramento: Option<String>,
    pub parcela: Option<String>,
    pub valor: f64,
}

fn parse_float_br(val: &str) -> f64 {
    let clean = val.trim().replace("R$", "").replace(' ', "");
    if clean.contains(',') {
        clean.replace('.', "").replace(',', ".").parse::<f64>().unwrap_or(0.0)
    } else {
        clean.parse::<f64>().unwrap_or(0.0)
    }
}

fn normalize_header(s: &str) -> String {
    s.trim()
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
        .replace(' ', "_")
}

fn find_col(headers: &csv_async::StringRecord, predicate: impl Fn(&str) -> bool) -> Option<usize> {
    headers.iter().position(|h| predicate(&normalize_header(h)))
}

pub async fn processar_stream_auxilio_com_delimitador<R: AsyncRead + Unpin + Send>(
    reader: R,
    delimiter: u8,
) -> Result<Vec<BeneficioCsvRecord>> {
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

    let col_cpf = find_col(&headers, |h| h == "CPF_BENEFICIARIO" || h == "CPF" || h == "NR_CPF")
        .or_else(|| find_col(&headers, |h| h.contains("CPF")));

    let col_nome = find_col(&headers, |h| {
        h == "NOME_BENEFICIARIO" || h == "NM_BENEFICIARIO" || (h.contains("NOME") && h.contains("BENEFICIARIO"))
    })
    .or_else(|| find_col(&headers, |h| h == "NOME" || h.contains("NOME")));

    let col_mes = find_col(&headers, |h| h.contains("MES") || h.contains("REFERENCIA"));
    let col_uf = find_col(&headers, |h| h == "UF" || h == "SG_UF" || h == "ESTADO");
    let col_mun = find_col(&headers, |h| {
        h == "NOME_MUNICIPIO"
            || h == "MUNICIPIO"
            || (h.contains("MUNICIPIO") && !h.contains("CODIGO") && !h.contains("IBGE"))
    });
    let col_parcela = find_col(&headers, |h| h.contains("PARCELA"));
    let col_valor = find_col(&headers, |h| h.contains("VALOR") || h.contains("VR_"));
    let col_enquadramento = find_col(&headers, |h| h.contains("ENQUADRAMENTO") || h.contains("TIPO"));

    let mut records = Vec::new();
    let mut records_stream = csv_reader.into_records();

    while let Some(record_res) = records_stream.next().await {
        let record = record_res.map_err(IngestionError::Csv)?;

        let raw_cpf = col_cpf.and_then(|i| record.get(i)).unwrap_or("").trim();
        if raw_cpf.is_empty() {
            continue;
        }
        let cpf_mascarado = mascarar_cpf(raw_cpf);

        let nome_beneficiario = col_nome
            .and_then(|i| record.get(i))
            .unwrap_or("")
            .trim()
            .to_string();

        let mes_disponibilizacao = col_mes
            .and_then(|i| record.get(i))
            .unwrap_or("202004")
            .trim()
            .to_string();

        let uf = col_uf
            .and_then(|i| record.get(i))
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty());

        let municipio = col_mun
            .and_then(|i| record.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let parcela = col_parcela
            .and_then(|i| record.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let valor_str = col_valor.and_then(|i| record.get(i)).unwrap_or("0");
        let valor = parse_float_br(valor_str);

        let enquadramento = col_enquadramento
            .and_then(|i| record.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        records.push(BeneficioCsvRecord {
            mes_disponibilizacao,
            uf,
            municipio,
            cpf_mascarado,
            nome_beneficiario,
            enquadramento,
            parcela,
            valor,
        });
    }

    Ok(records)
}

pub async fn processar_stream_auxilio_emergencial<R: AsyncRead + Unpin + Send>(
    reader: R,
) -> Result<Vec<BeneficioCsvRecord>> {
    let mut buf_reader = tokio::io::BufReader::new(reader);
    let mut first_line = String::new();
    let n = buf_reader
        .read_line(&mut first_line)
        .await
        .map_err(IngestionError::Io)?;
    if n == 0 {
        return Ok(Vec::new());
    }

    let delimiter = if first_line.matches(';').count() >= first_line.matches(',').count() {
        b';'
    } else {
        b','
    };

    let cursor = std::io::Cursor::new(first_line.into_bytes());
    let chained = cursor.chain(buf_reader);

    processar_stream_auxilio_com_delimitador(chained, delimiter).await
}

pub async fn ingerir_auxilio_emergencial_em_lotes<R: AsyncRead + Unpin + Send>(
    conn: &mut Connection,
    reader: R,
    batch_size: usize,
) -> Result<usize> {
    let beneficios = processar_stream_auxilio_emergencial(reader).await?;
    let mut total_inserido = 0;

    for chunk in beneficios.chunks(batch_size) {
        let batch: Vec<NovoBeneficio> = chunk
            .iter()
            .map(|b| NovoBeneficio {
                cpf_mascarado: b.cpf_mascarado.clone(),
                nome_beneficiario: b.nome_beneficiario.clone(),
                municipio: b.municipio.clone(),
                uf: b.uf.clone(),
                mes_disponibilizacao: b.mes_disponibilizacao.clone(),
                parcela: b.parcela.clone(),
                valor: b.valor,
                enquadramento: b.enquadramento.clone(),
            })
            .collect();

        let inseridos = batch_insert_beneficios(conn, &batch)?;
        total_inserido += inseridos;
    }

    Ok(total_inserido)
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::{run_migrations, DbPool};

    #[tokio::test]
    async fn test_processar_stream_auxilio_emergencial_cgu_semicolon() {
        let csv_cgu = "\
MÊS DISPONIBILIZAÇÃO;UF;CÓDIGO MUNICÍPIO IBGE;NOME MUNICÍPIO;NIS BENEFICIÁRIO;CPF BENEFICIÁRIO;NOME BENEFICIÁRIO;NIS RESPONSÁVEL;CPF RESPONSÁVEL;NOME RESPONSÁVEL;ENQUADRAMENTO;PARCELA;OBSERVAÇÃO;VALOR BENEFÍCIO\n\
202004;SP;3550308;SAO PAULO;12345678901;***.123.456-**;JOAO DA SILVA;12345678901;***.123.456-**;JOAO DA SILVA;EXTRA CAD;1ª PARCELA;Não há;600,00\n\
202005;RJ;3304557;RIO DE JANEIRO;98765432100;***987654**;MARIA DE SOUZA;98765432100;***987654**;MARIA DE SOUZA;BOLSA FAMILIA;2ª PARCELA;Não há;1.200,00\n";

        let records = processar_stream_auxilio_emergencial(csv_cgu.as_bytes())
            .await
            .unwrap();

        assert_eq!(records.len(), 2);

        assert_eq!(records[0].mes_disponibilizacao, "202004");
        assert_eq!(records[0].uf.as_deref(), Some("SP"));
        assert_eq!(records[0].municipio.as_deref(), Some("SAO PAULO"));
        assert_eq!(records[0].cpf_mascarado, "***.123.456-**");
        assert_eq!(records[0].nome_beneficiario, "JOAO DA SILVA");
        assert_eq!(records[0].enquadramento.as_deref(), Some("EXTRA CAD"));
        assert_eq!(records[0].parcela.as_deref(), Some("1ª PARCELA"));
        assert!((records[0].valor - 600.0).abs() < 0.001);

        assert_eq!(records[1].mes_disponibilizacao, "202005");
        assert_eq!(records[1].uf.as_deref(), Some("RJ"));
        assert_eq!(records[1].municipio.as_deref(), Some("RIO DE JANEIRO"));
        assert_eq!(records[1].cpf_mascarado, "***.987.654-**");
        assert_eq!(records[1].nome_beneficiario, "MARIA DE SOUZA");
        assert_eq!(records[1].enquadramento.as_deref(), Some("BOLSA FAMILIA"));
        assert_eq!(records[1].parcela.as_deref(), Some("2ª PARCELA"));
        assert!((records[1].valor - 1200.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_processar_stream_auxilio_brasilio_comma() {
        let csv_brasilio = "\
mes_referencia,uf,municipio,cpf_beneficiario,nome_beneficiario,enquadramento,parcela,valor_beneficio\n\
2020-04,MG,BELO HORIZONTE,***.333.444-**,CARLOS PEREIRA,CADUNICO,1,600.00\n\
2020-05,BA,SALVADOR,11122233344,ANA OLIVEIRA,EXTRA CAD,2,600\n";

        let records = processar_stream_auxilio_emergencial(csv_brasilio.as_bytes())
            .await
            .unwrap();

        assert_eq!(records.len(), 2);

        assert_eq!(records[0].mes_disponibilizacao, "2020-04");
        assert_eq!(records[0].uf.as_deref(), Some("MG"));
        assert_eq!(records[0].municipio.as_deref(), Some("BELO HORIZONTE"));
        assert_eq!(records[0].cpf_mascarado, "***.333.444-**");
        assert_eq!(records[0].nome_beneficiario, "CARLOS PEREIRA");
        assert!((records[0].valor - 600.0).abs() < 0.001);

        assert_eq!(records[1].mes_disponibilizacao, "2020-05");
        assert_eq!(records[1].uf.as_deref(), Some("BA"));
        assert_eq!(records[1].municipio.as_deref(), Some("SALVADOR"));
        assert_eq!(records[1].cpf_mascarado, "***.222.333-**");
        assert_eq!(records[1].nome_beneficiario, "ANA OLIVEIRA");
        assert!((records[1].valor - 600.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_ingerir_auxilio_emergencial_em_lotes() {
        let csv_data = "\
MÊS DISPONIBILIZAÇÃO;UF;NOME MUNICÍPIO;CPF BENEFICIÁRIO;NOME BENEFICIÁRIO;ENQUADRAMENTO;PARCELA;VALOR BENEFÍCIO\n\
202004;SP;CAMPINAS;***.111.222-**;FULANO 1;EXTRA CAD;1;600,00\n\
202004;SP;CAMPINAS;***.222.333-**;FULANO 2;EXTRA CAD;1;600,00\n\
202004;SP;CAMPINAS;***.333.444-**;FULANO 3;EXTRA CAD;1;600,00\n";

        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        let total = ingerir_auxilio_emergencial_em_lotes(&mut conn, csv_data.as_bytes(), 2)
            .await
            .unwrap();

        assert_eq!(total, 3);

        let count: i64 = conn
            .query_row("SELECT count(*) FROM beneficios_emergenciais", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);

        let soma: f64 = conn
            .query_row("SELECT sum(valor) FROM beneficios_emergenciais", [], |r| r.get(0))
            .unwrap();
        assert_eq!(soma, 1800.0);
    }
}
