use reqwest::Client;
use serde::{Deserialize, Serialize};
use storage::rusqlite::Connection;

use crate::error::{IngestionError, Result};
use crate::normalizer::limpar_cnpj;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PncpOrgao {
    #[serde(rename = "razaoSocial")]
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
    #[serde(rename = "niFornecedor")]
    pub ni_fornecedor: String,
    #[serde(rename = "nomeRazaoSocialFornecedor")]
    pub nome_razao_social_fornecedor: String,
    #[serde(rename = "valorGlobal")]
    pub valor_global: f64,
    #[serde(rename = "objetoContrato", default)]
    pub objeto_contrato: Option<String>,
    #[serde(rename = "dataAssinatura")]
    pub data_assinatura: String,
    #[serde(rename = "dataVigenciaFim", default)]
    pub data_vigencia_fim: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PncpConsultaResponse {
    pub data: Vec<PncpContratoItem>,
    #[serde(rename = "totalRegistros", default)]
    pub total_registros: Option<i64>,
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
                .build()
                .unwrap_or_default(),
            base_url: "https://pncp.gov.br/api/consulta/v1".to_string(),
        }
    }

    pub fn with_base_url(base_url: String) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RadarCivico/1.0")
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

    pub async fn buscar_contratos_por_cnpj(
        &self,
        cnpj: &str,
        ano: i32,
    ) -> Result<Vec<PncpContratoItem>> {
        let doc_clean = limpar_cnpj(cnpj);
        let url = format!("{}/contratos?cnpj={}&ano={}", self.base_url, doc_clean, ano);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(IngestionError::Reqwest)?;

        let texto = resp.text().await.map_err(IngestionError::Reqwest)?;
        Self::parse_contratos_json(&texto)
    }

    pub fn salvar_contratos(
        conn: &mut Connection,
        contratos: &[PncpContratoItem],
    ) -> Result<usize> {
        if contratos.is_empty() {
            return Ok(0);
        }

        let tx = storage::transaction_immediate(conn)?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO contratos_publicos (
                    orgao_contratante, fornecedor_cnpj, valor_contratado,
                    objeto, data_assinatura, data_termino
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;

            for c in contratos {
                let fornecedor_cnpj = limpar_cnpj(&c.ni_fornecedor);

                stmt.execute(storage::rusqlite::params![
                    c.orgao_entidade.razao_social,
                    fornecedor_cnpj,
                    c.valor_global,
                    c.objeto_contrato,
                    c.data_assinatura,
                    c.data_vigencia_fim,
                ])?;
            }
        }
        tx.commit()?;

        Ok(contratos.len())
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
    }
}
