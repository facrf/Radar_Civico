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
        .trim_start_matches("\u{feff}")
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
        if let Some(index) = headers
            .iter()
            .position(|h| normalize_header(h) == normalize_header(candidate))
        {
            return Some(index);
        }
    }
    for candidate in candidates {
        if let Some(index) = headers
            .iter()
            .position(|h| normalize_header(h).contains(&normalize_header(candidate)))
        {
            return Some(index);
        }
    }
    None
}

pub async fn processar_stream_ceap_com_delimitador<R: AsyncRead + Unpin + Send>(
    reader: R,
    delimiter: u8,
) -> Result<Vec<CeapBulkRecord>> {
    let mut all = Vec::new();
    processar_ceap_em_lotes(reader, delimiter, 500, |batch| {
        all.extend_from_slice(batch);
        Ok(())
    })
    .await?;
    Ok(all)
}

async fn processar_ceap_em_lotes<R, F>(
    reader: R,
    delimiter: u8,
    batch_size: usize,
    mut on_batch: F,
) -> Result<usize>
where
    R: AsyncRead + Unpin + Send,
    F: FnMut(&[CeapBulkRecord]) -> Result<()>,
{
    if !(1..=5000).contains(&batch_size) {
        return Err(IngestionError::Parse(
            "Lote CEAP deve ter entre 1 e 5000 registros".into(),
        ));
    }
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

    for (name, column) in [
        ("parlamentar", col_nome),
        ("data", col_data),
        ("fornecedor", col_forn_doc),
        ("valor_liquido", col_valor),
    ] {
        if column.is_none() {
            return Err(IngestionError::Parse(format!(
                "CSV CEAP sem coluna obrigatória: {name}"
            )));
        }
    }
    let mut records = Vec::with_capacity(batch_size);
    let mut total = 0usize;
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
        if records.len() == batch_size {
            on_batch(&records)?;
            total += records.len();
            records.clear();
        }
    }
    if !records.is_empty() {
        on_batch(&records)?;
        total += records.len();
    }
    Ok(total)
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

pub const MAX_CEAP_DOWNLOAD_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_CEAP_CSV_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// Baixa em blocos para disco; nunca mantém o dump anual completo na RAM.
pub async fn baixar_ceap(client: &reqwest::Client, url: &str) -> Result<tempfile::NamedTempFile> {
    use tokio::io::AsyncWriteExt;
    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await?
        .error_for_status()?;
    if response
        .content_length()
        .is_some_and(|size| size > MAX_CEAP_DOWNLOAD_BYTES)
    {
        return Err(IngestionError::Parse(
            "Download CEAP excede limite de 1 GiB".into(),
        ));
    }
    let temporary = tempfile::NamedTempFile::new()?;
    let mut output = tokio::fs::File::from_std(temporary.reopen()?);
    let mut chunks = response.bytes_stream();
    let mut total = 0u64;
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk?;
        total += chunk.len() as u64;
        if total > MAX_CEAP_DOWNLOAD_BYTES {
            return Err(IngestionError::Parse(
                "Download CEAP excede limite de 1 GiB".into(),
            ));
        }
        output.write_all(&chunk).await?;
    }
    output.flush().await?;
    Ok(temporary)
}

// Este adaptador só é usado pelo parser dentro de spawn_blocking/Handle::block_on.
struct BlockingReader<R>(R);
impl<R: Read + Unpin> AsyncRead for BlockingReader<R> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.0.read(buf.initialize_unfilled()) {
            Ok(count) => {
                buf.advance(count);
                std::task::Poll::Ready(Ok(()))
            }
            Err(error) => std::task::Poll::Ready(Err(error)),
        }
    }
}

/// Leitura do ZIP e parsing ficam na thread bloqueante. A conexão é emprestada
/// apenas durante cada lote, permitindo consultas e snapshots entre transações.
pub fn processar_ceap_zip_em_lotes<R, F>(
    pool: &storage::DbPool,
    input: R,
    batch_size: usize,
    mut progress: F,
) -> Result<usize>
where
    R: Read + std::io::Seek + Send + Unpin,
    F: FnMut(usize, usize) -> Result<()>,
{
    let mut archive =
        zip::ZipArchive::new(input).map_err(|e| IngestionError::Zip(e.to_string()))?;
    let index = (0..archive.len())
        .find(|&index| {
            archive
                .by_index(index)
                .is_ok_and(|file| file.name().to_lowercase().ends_with(".csv"))
        })
        .ok_or_else(|| IngestionError::Parse("Nenhum CSV no ZIP CEAP".into()))?;
    let file = archive
        .by_index(index)
        .map_err(|e| IngestionError::Zip(e.to_string()))?;
    if file.size() > MAX_CEAP_CSV_BYTES {
        return Err(IngestionError::Parse(
            "CSV CEAP excede limite de 4 GiB".into(),
        ));
    }
    let mut input = crate::tse_ckan::LimitedRead::new(file, MAX_CEAP_CSV_BYTES);
    let mut csv_file = tempfile::NamedTempFile::new()?;
    std::io::copy(&mut input, &mut csv_file)?;
    let mut input = std::io::BufReader::new(csv_file.reopen()?);
    use std::io::BufRead;
    let header = input.fill_buf()?;
    let delimiter = if header
        .iter()
        .take_while(|&&b| b != b'\n')
        .filter(|&&b| b == b';')
        .count()
        > header
            .iter()
            .take_while(|&&b| b != b'\n')
            .filter(|&&b| b == b',')
            .count()
    {
        b';'
    } else {
        b','
    };
    let mut inserted = 0usize;
    tokio::runtime::Handle::current().block_on(processar_ceap_em_lotes(
        BlockingReader(RecordLimit {
            inner: input,
            bytes: 0,
            quoted: false,
        }),
        delimiter,
        batch_size,
        |batch| {
            let n = {
                let mut conn = pool.get()?;
                ingerir_ceap_bulk_em_lotes(&mut conn, batch, batch.len())?
            };
            inserted += n;
            progress(batch.len(), inserted)?;
            Ok(())
        },
    ))?;
    Ok(inserted)
}

// Impede que uma linha ou um campo entre aspas sem fechamento cresça sem limite.
struct RecordLimit<R> {
    inner: R,
    bytes: usize,
    quoted: bool,
}
impl<R: Read> Read for RecordLimit<R> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let count = self.inner.read(output)?;
        for &byte in &output[..count] {
            self.bytes += 1;
            if byte == b'"' {
                self.quoted = !self.quoted;
            }
            if byte == b'\n' && !self.quoted {
                self.bytes = 0;
            }
            if self.bytes > 1024 * 1024 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Registro CEAP excede 1 MiB",
                ));
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod streaming_regressions {
    use super::*;
    use std::io::Write;
    fn zip(csv: &str) -> Vec<u8> {
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        archive
            .start_file("Ano-2024.csv", zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(csv.as_bytes()).unwrap();
        archive.finish().unwrap().into_inner()
    }
    #[tokio::test]
    async fn bounded_batches_release_single_connection_and_deduplicate() {
        let directory = tempfile::tempdir().unwrap();
        let pool = storage::DbPool::open_with_size(directory.path().join("db.sqlite"), 1).unwrap();
        storage::run_migrations(&mut pool.get().unwrap()).unwrap();
        let mut csv =
            "\u{feff}txNomeParlamentar;datEmissao;txtCNPJCPF;vlrDocumento;vlrLiquido;txtNumero\n"
                .to_string();
        for index in 0..1201 {
            csv.push_str(&format!(
                "DEPUTADO ÁGUA;2024-01-01;11222333000144;999,00;10,00;NF-{index}\n"
            ));
        }
        let bytes = zip(&csv);
        let worker_pool = pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut sizes = Vec::new();
            let count = processar_ceap_zip_em_lotes(
                &worker_pool,
                std::io::Cursor::new(&bytes),
                500,
                |size, _| {
                    sizes.push(size);
                    // Pool com uma conexão: consulta só funciona se o lote a devolveu.
                    let conn = worker_pool.get_timeout(std::time::Duration::from_millis(100))?;
                    let amount: f64 = conn.query_row(
                        "SELECT valor_liquido FROM despesas_parlamentares LIMIT 1",
                        [],
                        |r| r.get(0),
                    )?;
                    assert_eq!(amount, 10.0);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(sizes, [500, 500, 201]);
            assert_eq!(count, 1201);
            assert_eq!(
                processar_ceap_zip_em_lotes(
                    &worker_pool,
                    std::io::Cursor::new(bytes),
                    500,
                    |_, _| Ok(())
                )
                .unwrap(),
                0
            );
        })
        .await
        .unwrap();
        let count: i64 = pool
            .get()
            .unwrap()
            .query_row("SELECT count(*) FROM despesas_parlamentares", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1201);
    }
    #[test]
    fn oversized_quoted_record_is_rejected_before_unbounded_allocation() {
        let input = format!("\"{}\n{}", "a".repeat(600000), "b".repeat(600000));
        let mut reader = RecordLimit {
            inner: input.as_bytes(),
            bytes: 0,
            quoted: false,
        };
        let mut buffer = [0; 8192];
        let error = loop {
            match reader.read(&mut buffer) {
                Ok(0) => panic!("Registro excessivo aceito"),
                Ok(_) => (),
                Err(error) => break error,
            }
        };
        assert!(error.to_string().contains("1 MiB"));
    }
}
