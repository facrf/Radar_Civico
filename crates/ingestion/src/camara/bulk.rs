use csv_async::AsyncReaderBuilder;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::io::Read;
use storage::rusqlite::Connection;
use storage::{batch_insert_despesas_parlamentares, NovaDespesaParlamentar};
use tokio::io::{AsyncBufReadExt, AsyncRead};

use crate::error::{IngestionError, Result};
use crate::normalizer::{limpar_cnpj, mascarar_cpf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CeapBulkRecord {
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

fn normalizar_data_ceap(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        return "1970-01-01".to_string();
    }
    if let Some((date_part, _)) = s.split_once('T') {
        return date_part.to_string();
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
        .replace([' ', '-', '_', '"', '\''], "")
}

fn find_col(headers: &csv_async::StringRecord, candidates: &[&str]) -> Option<usize> {
    for (i, h) in headers.iter().enumerate() {
        let h_norm = normalize_header(h);
        for &c in candidates {
            let c_norm = normalize_header(c);
            if h_norm == c_norm || h_norm.contains(&c_norm) {
                return Some(i);
            }
        }
    }
    None
}

pub async fn processar_stream_ceap_com_delimitador<R: AsyncRead + Unpin + Send>(
    reader: R,
    delimiter: u8,
) -> Result<Vec<CeapBulkRecord>> {
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

    let col_nome = find_col(
        &headers,
        &[
            "TXNOMEPARLAMENTAR",
            "NOMEPARLAMENTAR",
            "PARLAMENTARNOME",
            "NOME",
        ],
    );

    let col_cpf = find_col(
        &headers,
        &[
            "CPF",
            "CPFLIMPO",
            "CPFPARLAMENTAR",
            "PARLAMENTARCPFMASCARADO",
            "NRCPF",
        ],
    );

    let col_data = find_col(
        &headers,
        &[
            "DATEMISSAO",
            "DATADOCUMENTO",
            "DATAEMISSAO",
            "DATANF",
            "DATA",
        ],
    );

    let col_tipo = find_col(
        &headers,
        &[
            "TXTDESCRICAO",
            "TIPODESPESA",
            "CATEGORIADESPESA",
            "DESPESA",
            "DESCRICAO",
        ],
    );

    let col_forn_nome = find_col(
        &headers,
        &[
            "TXTFORNECEDOR",
            "NOMEFORNECEDOR",
            "FORNECEDORNOME",
            "FORNECEDOR",
            "RAZAOSOCIAL",
        ],
    );

    let col_forn_doc = find_col(
        &headers,
        &[
            "TXTCNPJCPF",
            "CNPJCPFFORNECEDOR",
            "FORNECEDORCNPJCPF",
            "CNPJFORNECEDOR",
            "CNPJCPF",
            "CPFCNPJ",
        ],
    );

    let col_valor = find_col(
        &headers,
        &[
            "VLRLIQUIDO",
            "VALORLIQUIDO",
            "VALOR",
            "VLRDOCUMENTO",
            "VALORDOCUMENTO",
        ],
    );

    let col_doc = find_col(
        &headers,
        &[
            "TXTNUMERO",
            "NUMDOCUMENTO",
            "NUMERODOCUMENTO",
            "NUMDOC",
            "IDEDOCUMENTO",
        ],
    );

    let col_url = find_col(
        &headers,
        &[
            "URLDOCUMENTO",
            "URLNOTAFISCAL",
            "URLNOTA",
            "URL",
            "LINKDOCUMENTO",
        ],
    );

    let col_litros = find_col(
        &headers,
        &["DETALHESLITROS", "LITROS", "QTDLITROS", "VOLUMELITROS"],
    );

    let mut records = Vec::new();
    let mut records_stream = csv_reader.into_records();

    while let Some(record_res) = records_stream.next().await {
        let record = record_res.map_err(IngestionError::Csv)?;

        let nome = col_nome
            .and_then(|i| record.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "PARLAMENTAR DESCONHECIDO".to_string());

        let raw_cpf = col_cpf.and_then(|i| record.get(i)).unwrap_or("").trim();
        let cpf_mascarado = if !raw_cpf.is_empty() {
            Some(mascarar_cpf(raw_cpf))
        } else {
            None
        };

        let raw_data = col_data.and_then(|i| record.get(i)).unwrap_or("").trim();
        let data_emissao = normalizar_data_ceap(raw_data);

        let categoria_despesa = col_tipo
            .and_then(|i| record.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "DESPESA PARLAMENTAR".to_string());

        let fornecedor_nome = col_forn_nome
            .and_then(|i| record.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "FORNECEDOR DIVERSO".to_string());

        let raw_forn_doc = col_forn_doc
            .and_then(|i| record.get(i))
            .unwrap_or("")
            .trim();
        let fornecedor_cnpj_cpf = limpar_cnpj(raw_forn_doc);

        let valor_liquido = crate::validation::money_field(
            col_valor.and_then(|i| record.get(i)),
            record.position().map(|p| p.line()).unwrap_or(0),
            "valor_liquido",
        )?;

        let numero_documento = col_doc
            .and_then(|i| record.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let url_nota_fiscal = col_url
            .and_then(|i| record.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let detalhes_litros = col_litros
            .and_then(|i| record.get(i))
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                crate::validation::number_field(
                    s,
                    record.position().map(|p| p.line()).unwrap_or(0),
                    "litros",
                )
            })
            .transpose()?
            .filter(|&v| v > 0.0);

        records.push(CeapBulkRecord {
            casa_legislativa: "CAMARA".to_string(),
            parlamentar_nome: nome,
            parlamentar_cpf_mascarado: cpf_mascarado,
            data_emissao,
            categoria_despesa,
            fornecedor_nome,
            fornecedor_cnpj_cpf,
            valor_liquido,
            numero_documento,
            url_nota_fiscal,
            detalhes_litros,
        });
    }

    Ok(records)
}

pub async fn processar_stream_ceap_csv<R: AsyncRead + Unpin + Send>(
    reader: R,
) -> Result<Vec<CeapBulkRecord>> {
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
    let chained = tokio::io::AsyncReadExt::chain(cursor, buf_reader);

    processar_stream_ceap_com_delimitador(chained, delimiter).await
}

pub async fn extrair_e_processar_ceap_zip(zip_bytes: &[u8]) -> Result<Vec<CeapBulkRecord>> {
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| IngestionError::Parse(format!("Falha ao descompactar ZIP CEAP: {e}")))?;

    let mut csv_conteudo: Option<Vec<u8>> = None;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| IngestionError::Parse(format!("Erro ao acessar arquivo no ZIP: {e}")))?;

        let name = file.name().to_lowercase();
        if name.ends_with(".csv") {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).map_err(IngestionError::Io)?;
            csv_conteudo = Some(buf);
            break;
        }
    }

    let buf = csv_conteudo.ok_or_else(|| {
        IngestionError::Parse("Nenhum arquivo CSV encontrado dentro do ZIP da CEAP".to_string())
    })?;

    processar_stream_ceap_csv(buf.as_slice()).await
}

pub async fn processar_ceap_buffer_ou_zip(buffer: &[u8]) -> Result<Vec<CeapBulkRecord>> {
    if buffer.starts_with(b"PK\x03\x04") || buffer.starts_with(b"PK\x05\x06") {
        extrair_e_processar_ceap_zip(buffer).await
    } else {
        processar_stream_ceap_csv(buffer).await
    }
}

pub fn ingerir_ceap_bulk_em_lotes(
    conn: &mut Connection,
    records: &[CeapBulkRecord],
    batch_size: usize,
) -> Result<usize> {
    if records.is_empty() {
        return Ok(0);
    }

    let mut total_inserido = 0;

    for chunk in records.chunks(batch_size) {
        let batch: Vec<NovaDespesaParlamentar> = chunk
            .iter()
            .map(|r| NovaDespesaParlamentar {
                casa_legislativa: r.casa_legislativa.clone(),
                parlamentar_nome: r.parlamentar_nome.clone(),
                parlamentar_cpf_mascarado: r.parlamentar_cpf_mascarado.clone(),
                data_emissao: r.data_emissao.clone(),
                categoria_despesa: r.categoria_despesa.clone(),
                fornecedor_nome: r.fornecedor_nome.clone(),
                fornecedor_cnpj_cpf: r.fornecedor_cnpj_cpf.clone(),
                valor_liquido: r.valor_liquido,
                numero_documento: r.numero_documento.clone(),
                url_nota_fiscal: r.url_nota_fiscal.clone(),
                detalhes_litros: r.detalhes_litros,
            })
            .collect();

        let inseridos = batch_insert_despesas_parlamentares(conn, &batch)?;
        total_inserido += inseridos;
    }

    Ok(total_inserido)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use storage::{run_migrations, DbPool};

    #[tokio::test]
    async fn test_camara_bulk_csv_streaming_semicolon() {
        let csv_data = "\
txNomeParlamentar;cpf;ideCadastro;nuCarteiraParlamentar;nuLegislatura;sgUF;sgPartido;codLegislatura;numSubCota;txtDescricao;numEspecificacaoSubCota;txtDescricaoEspecificacao;txtFornecedor;txtCNPJCPF;txtNumero;indTipoDocumento;datEmissao;vlrDocumento;vlrGlosa;vlrLiquido;numMes;numAno;numParcela;txtPassageiro;txtTrecho;numLote;numRessarcimento;vlrRestituicao;nuDeputadoId;ideDocumento;urlDocumento;detalhes_litros\n\
DEPUTADO SILVA;12345678901;1000;1;57;SP;PARTIDO;57;3;COMBUSTIVEIS E LUBRIFICANTES.;0;;POSTO ALVORADA LTDA;12.345.678/0001-99;NF-9900;0;2024-05-10T00:00:00;350,50;0,00;350,50;5;2024;0;;;100;;;1000;999;https://nf.sefaz.sp.gov.br/doc9900;65,5\n\
DEPUTADA SANTOS;98765432100;2000;2;57;RJ;PARTIDO;57;1;PASSAGEM AEREA;0;;LATAM AIRLINES;02.012.862/0001-60;TK-1234;0;2024-05-12T00:00:00;1.200,00;0,00;1.200,00;5;2024;0;;;101;;;2000;998;https://latam.com/rec;;\n";

        let records = processar_stream_ceap_csv(csv_data.as_bytes())
            .await
            .expect("falha ao processar csv ceap");

        assert_eq!(records.len(), 2);

        assert_eq!(records[0].parlamentar_nome, "DEPUTADO SILVA");
        assert_eq!(
            records[0].parlamentar_cpf_mascarado,
            Some("***.456.789-**".to_string())
        );
        assert_eq!(records[0].data_emissao, "2024-05-10");
        assert_eq!(
            records[0].categoria_despesa,
            "COMBUSTIVEIS E LUBRIFICANTES."
        );
        assert_eq!(records[0].fornecedor_nome, "POSTO ALVORADA LTDA");
        assert_eq!(records[0].fornecedor_cnpj_cpf, "12345678000199");
        assert_eq!(records[0].valor_liquido, 350.50);
        assert_eq!(records[0].numero_documento, Some("NF-9900".to_string()));
        assert_eq!(records[0].detalhes_litros, Some(65.5));

        assert_eq!(records[1].parlamentar_nome, "DEPUTADA SANTOS");
        assert_eq!(
            records[1].parlamentar_cpf_mascarado,
            Some("***.654.321-**".to_string())
        );
        assert_eq!(records[1].data_emissao, "2024-05-12");
        assert_eq!(records[1].valor_liquido, 1200.00);
        assert_eq!(records[1].detalhes_litros, None);
    }

    #[tokio::test]
    async fn test_camara_bulk_zip_extraction_and_sqlite_persistence() {
        let csv_data = "\
txNomeParlamentar;cpf;datEmissao;txtDescricao;txtFornecedor;txtCNPJCPF;vlrLiquido;txtNumero;urlDocumento\n\
CARLOS DRUMMOND;11122233344;2024-06-01;LOCAÇÃO DE VEÍCULOS;LOCALIZA RENT A CAR;16.670.085/0001-55;2500,00;LOC-4321;https://localiza.com/fat\n";

        // Cria ZIP em memória contendo Ano-2024.csv
        let mut zip_buffer = Vec::new();
        {
            let mut zip_writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buffer));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip_writer
                .start_file("Ano-2024.csv", options)
                .expect("erro start_file");
            zip_writer
                .write_all(csv_data.as_bytes())
                .expect("erro write_all");
            zip_writer.finish().expect("erro zip finish");
        }

        let records = processar_ceap_buffer_ou_zip(&zip_buffer)
            .await
            .expect("falha ao processar zip ceap");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].parlamentar_nome, "CARLOS DRUMMOND");
        assert_eq!(records[0].valor_liquido, 2500.0);
        assert_eq!(records[0].fornecedor_cnpj_cpf, "16670085000155");

        // Testa persistência no SQLite
        let pool = DbPool::open_in_memory().expect("falha pool");
        let mut conn = pool.get().expect("falha conn");
        run_migrations(&mut conn).expect("falha migrations");

        let inseridos = ingerir_ceap_bulk_em_lotes(&mut conn, &records, 100)
            .expect("falha ao inserir despesas ceap");
        assert_eq!(inseridos, 1);

        let count: i64 = conn
            .query_row("SELECT count(*) FROM despesas_parlamentares", [], |r| {
                r.get(0)
            })
            .expect("falha count");
        assert_eq!(count, 1);

        let (parlamentar, valor, doc): (String, f64, String) = conn
            .query_row(
                "SELECT parlamentar_nome, valor_liquido, fornecedor_cnpj_cpf FROM despesas_parlamentares WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("falha select");
        assert_eq!(parlamentar, "CARLOS DRUMMOND");
        assert_eq!(valor, 2500.0);
        assert_eq!(doc, "16670085000155");
    }
}
