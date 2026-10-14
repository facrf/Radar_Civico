use async_trait::async_trait;
use std::sync::Arc;
use tokio::task::spawn_blocking;

use crate::error::{IngestionError, Result};
use crate::importers::sink::BatchSink;
use crate::importers::traits::{ImportContext, ImportStage, SourceImporter};

// -------------------------------------------------------------------------------------------------
// 1. Receita Federal (QSA / CNPJ)
// -------------------------------------------------------------------------------------------------
#[derive(Clone, Default)]
pub struct ReceitaFederalImporter {
    pub mock_csv: Option<String>,
}

impl ReceitaFederalImporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_mock_csv(csv: impl Into<String>) -> Self {
        Self {
            mock_csv: Some(csv.into()),
        }
    }
}

#[async_trait]
impl SourceImporter for ReceitaFederalImporter {
    fn id(&self) -> &str {
        "receita_qsa"
    }

    fn name(&self) -> &str {
        "Receita Federal (QSA/CNPJ)"
    }

    fn description(&self) -> &str {
        "Ingestão de cadastro de empresas e quadro de sócios e administradores (QSA)"
    }

    async fn run(&self, ctx: Arc<ImportContext>, sink: Arc<BatchSink>) -> Result<()> {
        ctx.set_stage(
            ImportStage::Conectando,
            "Conectando à base da Receita Federal...",
        );

        let csv_data = self.mock_csv.clone().ok_or_else(|| IngestionError::Custom(
            "QSA exige arquivo oficial: utilize o upload RECEITA_QSA. Nenhum dado foi inserido.".into()
        ))?;

        if ctx.is_cancelled() {
            ctx.set_stage(ImportStage::Cancelado, "Cancelado pelo usuário");
            return Ok(());
        }

        ctx.set_stage(
            ImportStage::Processando,
            "Processando registros de sócios e CNPJs...",
        );

        let pool = sink.pool().clone();
        let inseridos = spawn_blocking(move || -> Result<usize> {
            let mut conn = pool.get().map_err(IngestionError::Storage)?;
            storage::aplicar_pragmas_ingestao(&conn).map_err(IngestionError::Storage)?;

            let tx = storage::transaction_immediate(&mut conn).map_err(IngestionError::Storage)?;
            let mut count = 0;
            {
                let mut stmt = tx.prepare_cached(
                    "INSERT OR REPLACE INTO empresas_qsa (
                        cnpj_basico, cnpj_ordem, cnpj_dv, razao_social, socio_cpf_cnpj_mascarado, socio_nome, qualificacao_socio
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
                ).map_err(IngestionError::Sqlite)?;

                let mut rdr = csv::ReaderBuilder::new()
                    .delimiter(b';')
                    .has_headers(true)
                    .flexible(true)
                    .from_reader(csv_data.as_bytes());

                for result in rdr.records() {
                    let record = result.map_err(|e| IngestionError::Parse(e.to_string()))?;
                    if record.len() >= 7 {
                        stmt.execute(storage::rusqlite::params![
                            record.get(0).unwrap_or(""),
                            record.get(1).unwrap_or(""),
                            record.get(2).unwrap_or(""),
                            record.get(3).unwrap_or(""),
                            record.get(4).unwrap_or(""),
                            record.get(5).unwrap_or(""),
                            record.get(6).unwrap_or(""),
                        ])
                        .map_err(IngestionError::Sqlite)?;
                        count += 1;
                    }
                }
            }
            tx.commit().map_err(IngestionError::Sqlite)?;
            Ok(count)
        })
        .await
        .map_err(|e| IngestionError::TokioJoin(e.to_string()))??;

        ctx.update_progress("qsa_receita.csv", 1, 1, inseridos as u64);
        ctx.finish(format!(
            "Receita Federal: {} vínculos QSA processados com sucesso",
            inseridos
        ));
        Ok(())
    }
}

// -------------------------------------------------------------------------------------------------
// 2. Câmara dos Deputados (CEAP)
// -------------------------------------------------------------------------------------------------
#[derive(Clone, Default)]
pub struct CamaraCeapImporter {
    pub ano: u32,
    pub mock_zip: Option<Vec<u8>>,
}

impl CamaraCeapImporter {
    pub fn new(ano: u32) -> Self {
        Self {
            ano,
            mock_zip: None,
        }
    }

    pub fn with_mock_zip(ano: u32, zip: Vec<u8>) -> Self {
        Self {
            ano,
            mock_zip: Some(zip),
        }
    }
}

#[async_trait]
impl SourceImporter for CamaraCeapImporter {
    fn id(&self) -> &str {
        "camara_ceap"
    }

    fn name(&self) -> &str {
        "Câmara dos Deputados (CEAP)"
    }

    fn description(&self) -> &str {
        "Ingestão de notas fiscais da Cota para o Exercício da Atividade Parlamentar"
    }

    async fn run(&self, ctx: Arc<ImportContext>, sink: Arc<BatchSink>) -> Result<()> {
        let ano = if self.ano == 0 { 2024 } else { self.ano };
        ctx.set_stage(
            ImportStage::Baixando,
            format!("Baixando dump CEAP {ano} para disco..."),
        );
        let temporary = if let Some(bytes) = self.mock_zip.clone() {
            spawn_blocking(move || -> Result<tempfile::NamedTempFile> {
                use std::io::Write;
                let mut file = tempfile::NamedTempFile::new()?;
                file.write_all(&bytes)?;
                Ok(file)
            })
            .await
            .map_err(|e| IngestionError::TokioJoin(e.to_string()))??
        } else {
            let client = reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(15))
                .user_agent("RadarCivico/1.0")
                .build()?;
            let url = format!("https://www.camara.leg.br/cotas/Ano-{ano}.csv.zip");
            tokio::select! {
                result = crate::camara::baixar_ceap(&client, &url) => result?,
                _ = ctx.cancellation_token.cancelled() => {
                    ctx.set_stage(ImportStage::Cancelado, "Cancelado durante download CEAP");
                    return Ok(());
                }
            }
        };
        let pool = sink.pool().clone();
        let worker_ctx = ctx.clone();
        ctx.set_stage(ImportStage::Processando, "Processando CSV CEAP em lotes...");
        let inserted = spawn_blocking(move || {
            crate::camara::processar_ceap_zip_em_lotes(
                &pool,
                temporary.reopen()?,
                500,
                |_, total| {
                    if worker_ctx.is_cancelled() {
                        return Err(IngestionError::Custom(
                            "CEAP cancelada; lotes confirmados preservados".into(),
                        ));
                    }
                    worker_ctx.update_progress(format!("Ano-{ano}.csv.zip"), 0, 1, total as u64);
                    Ok(())
                },
            )
        })
        .await
        .map_err(|e| IngestionError::TokioJoin(e.to_string()))??;
        ctx.update_progress(format!("Ano-{ano}.csv.zip"), 1, 1, inserted as u64);
        ctx.finish(format!("CEAP: {inserted} novas despesas gravadas"));
        Ok(())
    }
}

// -------------------------------------------------------------------------------------------------
// 3. Portal Nacional de Contratações Públicas (PNCP)
// -------------------------------------------------------------------------------------------------
#[derive(Clone, Default)]
pub struct PncpImporter {
    pub ano: u32,
    pub cnpj_filtro: Option<String>,
    pub mock_json: Option<String>,
}

impl PncpImporter {
    pub fn new(ano: u32) -> Self {
        Self {
            ano,
            cnpj_filtro: None,
            mock_json: None,
        }
    }

    pub fn with_filtro(ano: u32, cnpj: impl Into<String>) -> Self {
        Self {
            ano,
            cnpj_filtro: Some(cnpj.into()),
            mock_json: None,
        }
    }

    pub fn with_mock_json(ano: u32, json: impl Into<String>) -> Self {
        Self {
            ano,
            cnpj_filtro: None,
            mock_json: Some(json.into()),
        }
    }
}

#[async_trait]
impl SourceImporter for PncpImporter {
    fn id(&self) -> &str {
        "pncp"
    }

    fn name(&self) -> &str {
        "Portal Nacional de Contratações Públicas (PNCP)"
    }

    fn description(&self) -> &str {
        "Ingestão e sincronização de contratos e atas de registro de preços do PNCP"
    }

    async fn run(&self, ctx: Arc<ImportContext>, sink: Arc<BatchSink>) -> Result<()> {
        let ano = if self.ano == 0 { 2024 } else { self.ano as i32 };
        ctx.set_stage(
            ImportStage::Conectando,
            format!("Conectando ao PNCP para o exercício {ano}..."),
        );

        let pool = sink.pool().clone();

        // Se houver mock de dados (para testes ou injeção direta)
        if let Some(ref json_str) = self.mock_json {
            ctx.set_stage(ImportStage::Processando, "Processando contratos PNCP...");
            let contratos = crate::pncp::PncpClient::parse_contratos_json(json_str)?;
            let pool_worker = pool.clone();
            let inseridos = spawn_blocking(move || -> Result<usize> {
                let mut conn = pool_worker.get().map_err(IngestionError::Storage)?;
                crate::pncp::PncpClient::salvar_contratos(&mut conn, &contratos)
            })
            .await
            .map_err(|e| IngestionError::TokioJoin(e.to_string()))??;

            ctx.update_progress("pncp_contratos.json", 1, 1, inseridos as u64);
            ctx.finish(format!("PNCP: {} contratos salvos com sucesso", inseridos));
            return Ok(());
        }

        // Obtém a lista de CNPJs prioritários para sincronização (da CEAP e empresas_qsa)
        let cnpjs_alvo = if let Some(ref filtro) = self.cnpj_filtro {
            vec![filtro.clone()]
        } else {
            let conn = pool.get().map_err(IngestionError::Storage)?;
            let mut stmt = conn.prepare(
                "SELECT DISTINCT fornecedor_cnpj_cpf FROM despesas_parlamentares 
                 WHERE length(REPLACE(REPLACE(REPLACE(fornecedor_cnpj_cpf, '.', ''), '/', ''), '-', '')) = 14 
                 LIMIT 25"
            ).map_err(IngestionError::Sqlite)?;

            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(IngestionError::Sqlite)?;
            let mut list = Vec::new();
            for r in rows.flatten() {
                let clean = r.trim().to_string();
                if !clean.is_empty() && !list.contains(&clean) {
                    list.push(clean);
                }
            }
            list
        };

        if cnpjs_alvo.is_empty() {
            return Err(IngestionError::Custom(
                "Nenhum fornecedor catalogado para consulta no PNCP. Importe primeiro dados da CEAP/Câmara ou empresas QSA, ou informe um CNPJ.".into()
            ));
        }

        ctx.set_stage(
            ImportStage::Processando,
            format!("Consultando contratos PNCP para {} fornecedores...", cnpjs_alvo.len()),
        );

        let client = crate::pncp::PncpClient::new();
        let total_cnpjs = cnpjs_alvo.len();
        let mut total_inseridos = 0;

        for (idx, cnpj) in cnpjs_alvo.iter().enumerate() {
            if ctx.is_cancelled() {
                ctx.set_stage(ImportStage::Cancelado, "Sincronização PNCP cancelada");
                return Ok(());
            }

            ctx.update_progress(format!("CNPJ: {cnpj}"), idx + 1, total_cnpjs, total_inseridos as u64);

            match client.buscar_contratos_por_cnpj(cnpj, ano).await {
                Ok(contratos) if !contratos.is_empty() => {
                    let pool_worker = pool.clone();
                    let inseridos = spawn_blocking(move || -> Result<usize> {
                        let mut conn = pool_worker.get().map_err(IngestionError::Storage)?;
                        crate::pncp::PncpClient::salvar_contratos(&mut conn, &contratos)
                    })
                    .await
                    .map_err(|e| IngestionError::TokioJoin(e.to_string()))??;
                    total_inseridos += inseridos;
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("Consulta CNPJ {} no PNCP: {}", cnpj, e);
                }
            }
        }

        ctx.update_progress("pncp_contratos", total_cnpjs, total_cnpjs, total_inseridos as u64);
        ctx.finish(format!(
            "PNCP: {} novos contratos salvos a partir de {} fornecedores consultados",
            total_inseridos, total_cnpjs
        ));
        Ok(())
    }
}

// -------------------------------------------------------------------------------------------------
// 4. Querido Diário
// -------------------------------------------------------------------------------------------------
#[derive(Clone, Default)]
pub struct QueridoDiarioImporter {
    pub termo: String,
    pub municipio_uf: String,
}

impl QueridoDiarioImporter {
    pub fn new(termo: impl Into<String>, municipio_uf: impl Into<String>) -> Self {
        Self {
            termo: termo.into(),
            municipio_uf: municipio_uf.into(),
        }
    }
}

#[async_trait]
impl SourceImporter for QueridoDiarioImporter {
    fn id(&self) -> &str {
        "querido_diario"
    }

    fn name(&self) -> &str {
        "Querido Diário - Diários Oficiais"
    }

    fn description(&self) -> &str {
        "Monitoramento de diários oficiais municipais para busca de nomeações e atos públicos"
    }

    async fn run(&self, _ctx: Arc<ImportContext>, _sink: Arc<BatchSink>) -> Result<()> {
        Err(IngestionError::Custom("Utilize a sincronização de diários nas configurações ou a investigação por nome. Nenhum dado foi inserido.".into()))
    }
}

// -------------------------------------------------------------------------------------------------
// 5. Cadastro Nacional dos Advogados (CNA / OAB)
// -------------------------------------------------------------------------------------------------
#[derive(Clone, Default)]
pub struct CnaOabImporter {
    pub mock_csv: Option<String>,
}

impl CnaOabImporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_mock_csv(csv: impl Into<String>) -> Self {
        Self {
            mock_csv: Some(csv.into()),
        }
    }
}

#[async_trait]
impl SourceImporter for CnaOabImporter {
    fn id(&self) -> &str {
        "cna_oab"
    }

    fn name(&self) -> &str {
        "Cadastro Nacional dos Advogados (CNA/OAB)"
    }

    fn description(&self) -> &str {
        "Validação de situação cadastral e seccionais perante a Ordem dos Advogados do Brasil"
    }

    async fn run(&self, ctx: Arc<ImportContext>, sink: Arc<BatchSink>) -> Result<()> {
        ctx.set_stage(
            ImportStage::Conectando,
            "Conectando ao sistema CNA da OAB...",
        );

        let csv_data = if let Some(ref mock) = self.mock_csv {
            mock.clone()
        } else if std::path::Path::new("data/oab.csv").exists() {
            std::fs::read_to_string("data/oab.csv")
                .map_err(|e| IngestionError::Custom(format!("Falha ao ler data/oab.csv: {e}")))?
        } else if std::path::Path::new("data/cna.csv").exists() {
            std::fs::read_to_string("data/cna.csv")
                .map_err(|e| IngestionError::Custom(format!("Falha ao ler data/cna.csv: {e}")))?
        } else {
            return Err(IngestionError::Custom(
                "OAB exige arquivo oficial: utilize o upload OAB ou coloque 'data/oab.csv'. O portal CNA requer validação de reCAPTCHA e bloqueia consultas automatizadas sem navegador.".into()
            ));
        };

        if ctx.is_cancelled() {
            ctx.set_stage(ImportStage::Cancelado, "Cancelado");
            return Ok(());
        }

        ctx.set_stage(
            ImportStage::Processando,
            "Atualizando cadastros profissionais da OAB...",
        );
        let pool = sink.pool().clone();

        let inseridos = spawn_blocking(move || -> Result<usize> {
            let mut conn = pool.get().map_err(IngestionError::Storage)?;
            storage::aplicar_pragmas_ingestao(&conn).map_err(IngestionError::Storage)?;

            let tx = storage::transaction_immediate(&mut conn).map_err(IngestionError::Storage)?;
            let mut count = 0;
            {
                let mut stmt = tx.prepare_cached(
                    "INSERT OR REPLACE INTO registros_profissionais (
                        pessoa_nome, cpf_mascarado, orgao_emissor, numero_registro, seccional_uf, situacao_registro, tipo_inscricao
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
                ).map_err(IngestionError::Sqlite)?;

                let mut rdr = csv::ReaderBuilder::new()
                    .delimiter(b';')
                    .has_headers(true)
                    .flexible(true)
                    .from_reader(csv_data.as_bytes());

                for result in rdr.records() {
                    let record = result.map_err(|e| IngestionError::Parse(e.to_string()))?;
                    if record.len() >= 6 {
                        let res = stmt.execute(storage::rusqlite::params![
                            record.get(0).unwrap_or(""),
                            record.get(1).unwrap_or(""),
                            record.get(2).unwrap_or("OAB"),
                            record.get(3).unwrap_or(""),
                            record.get(4).unwrap_or("SP"),
                            record.get(5).unwrap_or("REGULAR"),
                            record.get(6).unwrap_or("ADVOGADO"),
                        ]);
                        res.map_err(IngestionError::Sqlite)?;
                        count += 1;
                    }
                }
            }
            tx.commit().map_err(IngestionError::Sqlite)?;
            Ok(count)
        })
        .await
        .map_err(|e| IngestionError::TokioJoin(e.to_string()))??;

        ctx.update_progress("oab_cadastros.csv", 1, 1, inseridos as u64);
        ctx.finish(format!(
            "CNA/OAB: {} registros profissionais atualizados com sucesso",
            inseridos
        ));
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use storage::{run_migrations, DbPool};

    #[tokio::test]
    async fn test_receita_federal_importer() {
        let pool = DbPool::open_in_memory().unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }
        let sink = Arc::new(BatchSink::new(pool.clone()));
        let importer = ReceitaFederalImporter::default();
        let ctx = Arc::new(ImportContext::new("receita_qsa"));

        importer.run(Arc::clone(&ctx), sink).await.unwrap_err();

        let conn = pool.get().unwrap();
        let total: i64 = conn
            .query_row("SELECT count(*) FROM empresas_qsa", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 0);
    }

    #[tokio::test]
    async fn test_pncp_importer() {
        let pool = DbPool::open_in_memory().unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }
        let sink = Arc::new(BatchSink::new(pool.clone()));
        let importer = PncpImporter::new(2024);
        let ctx = Arc::new(ImportContext::new("pncp"));

        importer.run(Arc::clone(&ctx), sink).await.unwrap_err();

        let conn = pool.get().unwrap();
        let total: i64 = conn
            .query_row("SELECT count(*) FROM contratos_publicos", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 0);
    }

    #[tokio::test]
    async fn test_pncp_importer_com_mock_json() {
        let pool = DbPool::open_in_memory().unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }
        let sink = Arc::new(BatchSink::new(pool.clone()));
        let mock_json = r#"{
            "data": [
                {
                    "numeroContratoEmpenho": "2024NE0001",
                    "orgaoEntidade": { "razaoSocial": "MINISTERIO DA EDUCACAO" },
                    "niFornecedor": "11222333000199",
                    "nomeRazaoSocialFornecedor": "FORNECEDOR TESTE LTDA",
                    "valorGlobal": 75000.0,
                    "objetoContrato": "Servicos de TI",
                    "dataAssinatura": "2024-03-01"
                }
            ]
        }"#;
        let importer = PncpImporter::with_mock_json(2024, mock_json);
        let ctx = Arc::new(ImportContext::new("pncp"));

        importer.run(Arc::clone(&ctx), sink).await.unwrap();

        let conn = pool.get().unwrap();
        let total: i64 = conn
            .query_row("SELECT count(*) FROM contratos_publicos", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(ctx.get_progress().stage, ImportStage::Concluido);
    }

    #[tokio::test]
    async fn test_querido_diario_importer() {
        let pool = DbPool::open_in_memory().unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }
        let sink = Arc::new(BatchSink::new(pool.clone()));
        let importer = QueridoDiarioImporter::default();
        let ctx = Arc::new(ImportContext::new("querido_diario"));

        importer.run(Arc::clone(&ctx), sink).await.unwrap_err();

        let conn = pool.get().unwrap();
        let total: i64 = conn
            .query_row("SELECT count(*) FROM cache_consultas_diario", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(total, 0);
    }

    #[tokio::test]
    async fn test_cna_oab_importer() {
        let pool = DbPool::open_in_memory().unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }
        let sink = Arc::new(BatchSink::new(pool.clone()));
        let importer = CnaOabImporter::default();
        let ctx = Arc::new(ImportContext::new("cna_oab"));

        importer.run(Arc::clone(&ctx), sink).await.unwrap_err();

        let conn = pool.get().unwrap();
        let total: i64 = conn
            .query_row("SELECT count(*) FROM registros_profissionais", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(total, 0);
    }

    #[tokio::test]
    async fn test_camara_ceap_importer() {
        let pool = DbPool::open_in_memory().unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }
        let sink = Arc::new(BatchSink::new(pool.clone()));
        let importer = CamaraCeapImporter::new(2024);
        let ctx = Arc::new(ImportContext::new("camara_ceap"));

        importer.run(Arc::clone(&ctx), sink).await.unwrap();
        assert_eq!(ctx.get_progress().stage, ImportStage::Concluido);
    }
}
