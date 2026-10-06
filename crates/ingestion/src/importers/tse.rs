use std::sync::Arc;
use async_trait::async_trait;
use tokio::task::spawn_blocking;

use crate::error::{IngestionError, Result};
use crate::importers::sink::BatchSink;
use crate::importers::traits::{ImportContext, ImportStage, SourceImporter};
use crate::tse_ckan::{descobrir_urls_tse, processar_zip_tse_bytes};

pub struct TseImporter {
    pub ano: u32,
    pub mock_data: Option<Vec<u8>>,
    pub datasets: Vec<String>,
}

impl TseImporter {
    pub fn new(ano: u32) -> Self {
        Self {
            ano,
            mock_data: None,
            datasets: vec![
                "candidatos".to_string(),
                "bens-candidatos".to_string(),
                "prestacao-contas-eleitorais-candidatos".to_string(),
            ],
        }
    }

    pub fn with_mock_data(ano: u32, mock_data: Vec<u8>) -> Self {
        Self {
            ano,
            mock_data: Some(mock_data),
            datasets: Vec::new(),
        }
    }

    pub fn with_datasets(mut self, datasets: Vec<String>) -> Self {
        self.datasets = datasets;
        self
    }
}

impl Default for TseImporter {
    fn default() -> Self {
        Self::new(2024)
    }
}

#[async_trait]
impl SourceImporter for TseImporter {
    fn id(&self) -> &str {
        "tse"
    }

    fn name(&self) -> &str {
        "Tribunal Superior Eleitoral"
    }

    fn description(&self) -> &str {
        "Importação de dados eleitorais, candidatos, bens e receitas do repositório oficial do TSE"
    }

    async fn run(&self, ctx: Arc<ImportContext>, sink: Arc<BatchSink>) -> Result<()> {
        ctx.set_stage(
            ImportStage::Conectando,
            format!("Iniciando preparação da importação do TSE para o ano {}", self.ano),
        );

        if let Some(ref mock_bytes) = self.mock_data {
            if ctx.is_cancelled() {
                ctx.set_stage(ImportStage::Cancelado, "Operação cancelada");
                return Ok(());
            }

            ctx.set_stage(
                ImportStage::Descompactando,
                "Descompactando e gravando registros em lote no banco",
            );

            let bytes = mock_bytes.clone();
            let pool = sink.pool().clone();

            let total_inseridos = spawn_blocking(move || -> Result<usize> {
                let mut conn = pool.get().map_err(IngestionError::Storage)?;
                storage::aplicar_pragmas_ingestao(&conn).map_err(IngestionError::Storage)?;
                processar_zip_tse_bytes(&mut conn, &bytes)
            })
            .await
            .map_err(|e| IngestionError::TokioJoin(e.to_string()))??;

            ctx.update_progress("mock_tse.zip", 1, 1, total_inseridos as u64);
            ctx.finish(format!(
                "Importação TSE concluída: {} registros persistidos",
                total_inseridos
            ));
            return Ok(());
        }

        // Modo Online via CKAN
        ctx.set_stage(
            ImportStage::Conectando,
            format!("Descobrindo pacotes do TSE para o ano {} via API CKAN...", self.ano),
        );

        let dataset_slices: Vec<&str> = self.datasets.iter().map(|s| s.as_str()).collect();
        let urls = descobrir_urls_tse(self.ano, &dataset_slices).await?;

        if urls.is_empty() {
            ctx.finish(format!(
                "Nenhum pacote encontrado para o ano eleitoral {} no CKAN",
                self.ano
            ));
            return Ok(());
        }

        let total_urls = urls.len();
        let mut total_records = 0u64;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(IngestionError::Reqwest)?;

        for (index, url) in urls.into_iter().enumerate() {
            if ctx.is_cancelled() {
                ctx.set_stage(ImportStage::Cancelado, "Importação cancelada pelo usuário");
                return Ok(());
            }

            let nome_arquivo = url.rsplit('/').next().unwrap_or("pacote.zip").to_string();
            ctx.set_stage(
                ImportStage::Baixando,
                format!("Baixando pacote [{}/{}]: {}", index + 1, total_urls, nome_arquivo),
            );

            let response = client.get(&url).send().await.map_err(IngestionError::Reqwest)?;
            if !response.status().is_success() {
                continue;
            }

            let bytes = response.bytes().await.map_err(IngestionError::Reqwest)?;

            if ctx.is_cancelled() {
                ctx.set_stage(ImportStage::Cancelado, "Importação cancelada pelo usuário");
                return Ok(());
            }

            ctx.set_stage(
                ImportStage::Processando,
                format!("Processando e gravando [{}/{}]: {}", index + 1, total_urls, nome_arquivo),
            );

            let pool = sink.pool().clone();
            let bytes_vec = bytes.to_vec();

            let inseridos = spawn_blocking(move || -> Result<usize> {
                let mut conn = pool.get().map_err(IngestionError::Storage)?;
                storage::aplicar_pragmas_ingestao(&conn).map_err(IngestionError::Storage)?;
                processar_zip_tse_bytes(&mut conn, &bytes_vec)
            })
            .await
            .map_err(|e| IngestionError::TokioJoin(e.to_string()))??;

            total_records += inseridos as u64;
            ctx.update_progress(&nome_arquivo, index + 1, total_urls, total_records);
        }

        ctx.finish(format!(
            "Processamento do TSE finalizado: {} pacotes lidos, {} registros persistidos",
            total_urls, total_records
        ));

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::io::Write;
    use storage::{run_migrations, DbPool};

    #[tokio::test]
    async fn test_tse_importer_com_mock_zip_e_batch_sink() {
        let pool = DbPool::open_in_memory().expect("pool");
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }

        let sink = Arc::new(BatchSink::new(pool.clone()));

        // Cria CSV ISO-8859-1 com candidatos
        let csv_utf8 = "\
ANO_ELEICAO;SG_UF;DS_CARGO;SQ_CANDIDATO;NR_CANDIDATO;NM_CANDIDATO;NM_URNA_CANDIDATO;NR_CPF_CANDIDATO;SG_PARTIDO;NM_MUNICIPIO;DS_SIT_TOT_TURNO;DS_OCUPACAO;DS_GRAU_INSTRUCAO;DT_NASCIMENTO\n\
2024;SP;PREFEITO;10001;15;CARLOS SILVA;CARLOS;12345678901;MDB;CAMPINAS;ELEITO;ADVOGADO;SUPERIOR COMPLETO;12/03/1980\n";
        let (bytes_latin1, _, _) = encoding_rs::WINDOWS_1252.encode(csv_utf8);

        let mut zip_buffer = Vec::new();
        {
            let mut zip_writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buffer));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);

            zip_writer
                .start_file("consulta_cand_2024_BRASIL.csv", options)
                .unwrap();
            zip_writer.write_all(&bytes_latin1).unwrap();
            zip_writer.finish().unwrap();
        }

        let importer = TseImporter::with_mock_data(2024, zip_buffer);
        assert_eq!(importer.id(), "tse");
        assert_eq!(importer.name(), "Tribunal Superior Eleitoral");

        let ctx = Arc::new(ImportContext::new("tse"));
        importer.run(Arc::clone(&ctx), sink).await.expect("execucao run");

        let prog = ctx.get_progress();
        assert_eq!(prog.stage, ImportStage::Concluido);
        assert_eq!(prog.records_processed, 1);

        let conn = pool.get().unwrap();
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM politicos", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 1);
    }
}
