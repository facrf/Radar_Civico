use chrono::{Datelike, NaiveDate};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::future::Future;
use storage::rusqlite::Connection;
use tokio_util::sync::CancellationToken;

use crate::error::{IngestionError, Result};
use crate::normalizer::limpar_cnpj;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PncpOrgao {
    #[serde(rename = "razaoSocial", default)]
    pub razao_social: String,
    #[serde(default)]
    pub cnpj: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PncpContratoItem {
    #[serde(rename = "numeroContratoEmpenho", default)]
    pub numero_contrato: Option<String>,
    #[serde(rename = "orgaoEntidade")]
    pub orgao_entidade: PncpOrgao,
    #[serde(rename = "niFornecedor", default)]
    pub ni_fornecedor: String,
    #[serde(rename = "nomeRazaoSocialFornecedor", default)]
    pub nome_razao_social_fornecedor: String,
    #[serde(rename = "valorGlobal", default)]
    pub valor_global: f64,
    #[serde(rename = "objetoContrato", default)]
    pub objeto_contrato: Option<String>,
    #[serde(rename = "dataAssinatura", default)]
    pub data_assinatura: Option<String>,
    #[serde(rename = "dataVigenciaFim", default)]
    pub data_vigencia_fim: Option<String>,
    #[serde(rename = "modalidadeNome", alias = "modalidade", default)]
    pub modalidade: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PncpConsultaResponse {
    pub data: Vec<PncpContratoItem>,
    #[serde(rename = "totalRegistros", default)]
    pub total_registros: Option<i64>,
    #[serde(rename = "totalPaginas", default)]
    pub total_paginas: Option<u32>,
    #[serde(rename = "numeroPagina", default)]
    pub numero_pagina: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelatorioPncp {
    pub registros_gravados: usize,
    pub paginas_processadas: usize,
    pub periodos_concluidos: usize,
    pub falhas: Vec<String>,
    pub cancelado: bool,
}
impl RelatorioPncp {
    pub fn status(&self) -> &'static str {
        if self.cancelado {
            "CANCELADO"
        } else if self.falhas.is_empty() {
            "CONCLUIDO"
        } else if self.paginas_processadas > 0 {
            "PARCIAL"
        } else {
            "ERRO"
        }
    }
    pub fn resumo(&self) -> String {
        format!("PNCP: {} registros gravados, {} páginas, {}/12 meses completos por data de publicação; {} falhas. {}",
            self.registros_gravados,self.paginas_processadas,self.periodos_concluidos,self.falhas.len(),self.falhas.join("; "))
    }
}

#[derive(Debug, Clone)]
pub struct ProgressoPncp {
    pub mes: u32,
    pub pagina: u32,
    pub registros_gravados: usize,
}

#[derive(Clone)]
pub struct PncpClient {
    client: Client,
    pub base_url: String,
}

impl PncpClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("RadarCivico/1.0")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            base_url: "https://pncp.gov.br/api/consulta/v1".to_string(),
        }
    }

    pub fn with_base_url(base_url: String) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RadarCivico/1.0")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            base_url,
        }
    }

    pub fn parse_contratos_json(json_str: &str) -> Result<Vec<PncpContratoItem>> {
        // Suporta tanto array direto quanto envelope { "data": [...] }
        if let Ok(resp) = serde_json::from_str::<PncpConsultaResponse>(json_str) {
            return Ok(resp.data);
        }

        let lista: Vec<PncpContratoItem> = serde_json::from_str(json_str)
            .map_err(|e| IngestionError::Parse(format!("Falha ao parsear contratos PNCP: {e}")))?;
        Ok(lista)
    }

    /// API de consulta filtra publicação, não fornecedor. Cada página ocupa no máximo 16 MiB.
    async fn pagina(
        &self,
        inicio: NaiveDate,
        fim: NaiveDate,
        pagina: u32,
        cancel: &CancellationToken,
    ) -> Result<PncpConsultaResponse> {
        let url = format!("{}/contratos", self.base_url);
        for attempt in 0..3 {
            let request = self.client.get(&url).query(&[
                ("dataInicial", inicio.format("%Y%m%d").to_string()),
                ("dataFinal", fim.format("%Y%m%d").to_string()),
                ("pagina", pagina.to_string()),
                ("tamanhoPagina", "500".into()),
            ]);
            let response = tokio::select! {
                response=request.send()=>response?,
                _=cancel.cancelled()=>return Err(IngestionError::Custom("PNCP cancelado".into())),
            };
            if response.status() == reqwest::StatusCode::NO_CONTENT {
                return Ok(PncpConsultaResponse {
                    data: Vec::new(),
                    total_registros: Some(0),
                    total_paginas: Some(0),
                    numero_pagina: Some(pagina),
                });
            }
            if (response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
                || response.status().is_server_error())
                && attempt < 2
            {
                let seconds = response
                    .headers()
                    .get("retry-after")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(1 << attempt)
                    .min(30);
                tokio::select! { _=tokio::time::sleep(std::time::Duration::from_secs(seconds))=>{}, _=cancel.cancelled()=>return Err(IngestionError::Custom("PNCP cancelado".into())) }
                continue;
            }
            let mut response = response.error_for_status()?;
            let mut bytes = Vec::new();
            while let Some(chunk) = tokio::select! { r=response.chunk()=>r?, _=cancel.cancelled()=>return Err(IngestionError::Custom("PNCP cancelado".into())) }
            {
                if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
                    return Err(IngestionError::Parse("Página PNCP excede 16 MiB".into()));
                }
                bytes.extend_from_slice(&chunk);
            }
            if let Ok(envelope) = serde_json::from_slice::<PncpConsultaResponse>(&bytes) {
                if envelope.numero_pagina.is_some_and(|n| n != pagina)
                    || (envelope.data.is_empty()
                        && envelope.total_paginas.is_some_and(|n| pagina < n))
                {
                    return Err(IngestionError::Parse(
                        "PNCP retornou metadados de paginação inconsistentes".into(),
                    ));
                }
                return Ok(envelope);
            }
            let data = serde_json::from_slice(&bytes)
                .map_err(|e| IngestionError::Parse(format!("Página PNCP inválida: {e}")))?;
            return Ok(PncpConsultaResponse {
                data,
                total_registros: None,
                total_paginas: None,
                numero_pagina: None,
            });
        }
        Err(IngestionError::Custom("PNCP excedeu retentativas".into()))
    }

    fn periodo(ano: i32, mes: u32) -> Result<(NaiveDate, NaiveDate)> {
        let inicio = NaiveDate::from_ymd_opt(ano, mes, 1)
            .ok_or_else(|| IngestionError::Parse("Ano/mês PNCP inválido".into()))?;
        let next = if mes == 12 {
            NaiveDate::from_ymd_opt(ano + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(ano, mes + 1, 1)
        };
        let fim = next
            .and_then(|d| d.pred_opt())
            .ok_or_else(|| IngestionError::Parse("Período PNCP inválido".into()))?;
        Ok((inicio, fim))
    }

    /// Grava por página, percorre os doze meses e preserva falhas parciais.
    pub async fn sincronizar_ano<F, Fut>(
        &self,
        pool: &storage::DbPool,
        ano: i32,
        fornecedor: Option<&str>,
        cancel: &CancellationToken,
        mut progresso: F,
    ) -> Result<RelatorioPncp>
    where
        F: FnMut(ProgressoPncp) -> Fut,
        Fut: Future<Output = ()>,
    {
        if !(2000..=chrono::Utc::now().year() + 1).contains(&ano) {
            return Err(IngestionError::Parse(
                "Ano PNCP fora do intervalo permitido".into(),
            ));
        }
        let filter = fornecedor.map(limpar_cnpj);
        if filter.as_ref().is_some_and(|s| s.len() != 14) {
            return Err(IngestionError::Parse(
                "CNPJ do fornecedor deve ter 14 dígitos".into(),
            ));
        }
        let mut report = RelatorioPncp::default();
        for mes in 1..=12 {
            let (inicio, fim) = Self::periodo(ano, mes)?;
            let mut pagina = 1;
            let mut previous_hash = None;
            loop {
                if cancel.is_cancelled() {
                    report.cancelado = true;
                    return Ok(report);
                }
                let response = match self.pagina(inicio, fim, pagina, cancel).await {
                    Ok(r) => r,
                    Err(error) => {
                        if cancel.is_cancelled() {
                            report.cancelado = true;
                            return Ok(report);
                        }
                        report
                            .falhas
                            .push(format!("{ano}-{mes:02} página {pagina}: {error}"));
                        break;
                    }
                };
                let page_hash = storage::calcular_sha256_bytes(
                    serde_json::to_string(&response.data)
                        .map_err(|e| IngestionError::Parse(e.to_string()))?
                        .as_bytes(),
                );
                if !response.data.is_empty() && previous_hash.as_ref() == Some(&page_hash) {
                    report.falhas.push(format!("{ano}-{mes:02}: API repetiu página; coleta interrompida sem declarar conclusão"));
                    break;
                }
                previous_hash = Some(page_hash);
                let empty = response.data.is_empty();
                let last = response.total_paginas.is_some_and(|last| pagina >= last);
                let contratos = response
                    .data
                    .into_iter()
                    .filter(|c| {
                        filter
                            .as_ref()
                            .is_none_or(|f| limpar_cnpj(&c.ni_fornecedor) == *f)
                    })
                    .collect::<Vec<_>>();
                let saved = pool
                    .run_blocking(move |conn| {
                        Self::salvar_contratos(conn, &contratos).map_err(|e| {
                            storage::StorageError::Io(std::io::Error::other(e.to_string()))
                        })
                    })
                    .await;
                match saved {
                    Ok(count) => report.registros_gravados += count,
                    Err(error) => {
                        report
                            .falhas
                            .push(format!("{ano}-{mes:02} gravação página {pagina}: {error}"));
                        break;
                    }
                }
                report.paginas_processadas += 1;
                progresso(ProgressoPncp {
                    mes,
                    pagina,
                    registros_gravados: report.registros_gravados,
                })
                .await;
                if empty || last {
                    report.periodos_concluidos += 1;
                    break;
                }
                if pagina >= 100_000 {
                    report
                        .falhas
                        .push(format!("{ano}-{mes:02}: limite de páginas excedido"));
                    break;
                }
                pagina += 1;
            }
        }
        Ok(report)
    }

    /// Compatibilidade: retorna somente o fornecedor após filtrar páginas oficiais localmente.
    pub async fn buscar_contratos_por_cnpj(
        &self,
        cnpj: &str,
        ano: i32,
    ) -> Result<Vec<PncpContratoItem>> {
        let filter = limpar_cnpj(cnpj);
        let mut data = Vec::new();
        let cancel = CancellationToken::new();
        for mes in 1..=12 {
            let (inicio, fim) = Self::periodo(ano, mes)?;
            for pagina in 1..=100_000 {
                let response = self.pagina(inicio, fim, pagina, &cancel).await?;
                let last =
                    response.data.is_empty() || response.total_paginas.is_some_and(|n| pagina >= n);
                data.extend(
                    response
                        .data
                        .into_iter()
                        .filter(|c| limpar_cnpj(&c.ni_fornecedor) == filter),
                );
                if data.len() > 100_000 {
                    return Err(IngestionError::Parse(
                        "Resultado excede limite; use sincronizar_ano".into(),
                    ));
                }
                if last {
                    break;
                }
                if pagina == 100_000 {
                    return Err(IngestionError::Parse(
                        "PNCP excede limite de páginas".into(),
                    ));
                }
            }
        }
        Ok(data)
    }

    /// Página explícita de janeiro, mantida para compatibilidade; coleta anual usa sincronizar_ano.
    pub async fn buscar_contratos_por_periodo(
        &self,
        ano: i32,
        pagina: u32,
    ) -> Result<Vec<PncpContratoItem>> {
        let (inicio, fim) = Self::periodo(ano, 1)?;
        Ok(self
            .pagina(inicio, fim, pagina, &CancellationToken::new())
            .await?
            .data)
    }

    pub fn salvar_contratos(
        conn: &mut Connection,
        contratos: &[PncpContratoItem],
    ) -> Result<usize> {
        if contratos.is_empty() {
            return Ok(0);
        }

        let tx = storage::transaction_immediate(conn)?;
        let mut inseridos = 0;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO contratos_publicos (
                    orgao_contratante, fornecedor_cnpj, valor_contratado,
                    objeto, data_assinatura, data_termino, modalidade
                 ) SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7
                 WHERE NOT EXISTS (
                     SELECT 1 FROM contratos_publicos
                     WHERE orgao_contratante = ?1
                       AND fornecedor_cnpj = ?2
                       AND (data_assinatura = ?5 OR (data_assinatura IS NULL AND ?5 IS NULL))
                       AND ABS(valor_contratado - ?3) < 0.001
                     LIMIT 1
                 )",
            )?;

            for c in contratos {
                let fornecedor_cnpj = limpar_cnpj(&c.ni_fornecedor);

                let affected = stmt.execute(storage::rusqlite::params![
                    c.orgao_entidade.razao_social,
                    fornecedor_cnpj,
                    storage::Money::from_reais(c.valor_global)?.reais(),
                    c.objeto_contrato,
                    c.data_assinatura,
                    c.data_vigencia_fim,
                    c.modalidade,
                ])?;
                inseridos += affected;
            }
        }
        tx.commit()?;

        Ok(inseridos)
    }
}

impl Default for PncpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::{run_migrations, DbPool};

    #[test]
    fn test_pncp_parse_json_and_save() {
        let json_data = r#"{
            "data": [
                {
                    "numeroContratoEmpenho": "10/2024",
                    "orgaoEntidade": {
                        "razaoSocial": "PREFEITURA MUNICIPAL DE CAMPINAS",
                        "cnpj": "44.620.082/0001-44"
                    },
                    "niFornecedor": "12.345.678/0001-90",
                    "nomeRazaoSocialFornecedor": "CONSTRUTORA RADAR LTDA",
                    "valorGlobal": 1500000.00,
                    "objetoContrato": "Reforma de escolas municipais",
                    "dataAssinatura": "2024-02-15",
                    "dataVigenciaFim": "2025-02-15"
                }
            ],
            "totalRegistros": 1
        }"#;

        let contratos = PncpClient::parse_contratos_json(json_data).unwrap();
        assert_eq!(contratos.len(), 1);
        assert_eq!(contratos[0].valor_global, 1500000.00);
        assert_eq!(
            contratos[0].orgao_entidade.razao_social,
            "PREFEITURA MUNICIPAL DE CAMPINAS"
        );

        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        let inseridos = PncpClient::salvar_contratos(&mut conn, &contratos).unwrap();
        assert_eq!(inseridos, 1);

        let count: i64 = conn
            .query_row("SELECT count(*) FROM contratos_publicos", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let cnpj: String = conn
            .query_row(
                "SELECT fornecedor_cnpj FROM contratos_publicos WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cnpj, "12345678000190");

        // Teste de idempotência: segunda execução não duplica
        let inseridos_segunda = PncpClient::salvar_contratos(&mut conn, &contratos).unwrap();
        assert_eq!(inseridos_segunda, 0);
        let count_apos: i64 = conn
            .query_row("SELECT count(*) FROM contratos_publicos", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count_apos, 1);
    }
}

#[cfg(test)]
#[path = "pncp_tests.rs"]
mod paging_tests;
