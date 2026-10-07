use reqwest::Client;
use serde::{Deserialize, Serialize};
use storage::rusqlite::Connection;

use crate::error::{IngestionError, Result};
use crate::normalizer::{limpar_cnpj, mascarar_cpf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CeapItemApi {
    #[serde(default)]
    pub ano: Option<i32>,
    #[serde(rename = "nomeParlamentar", default)]
    pub nome_parlamentar: Option<String>,
    #[serde(rename = "cpfParlamentar", default)]
    pub cpf_parlamentar: Option<String>,
    #[serde(rename = "dataDocumento")]
    pub data_documento: Option<String>,
    #[serde(rename = "tipoDespesa")]
    pub tipo_despesa: String,
    #[serde(rename = "nomeFornecedor")]
    pub nome_fornecedor: String,
    #[serde(rename = "cnpjCpfFornecedor")]
    pub cnpj_cpf_fornecedor: String,
    #[serde(rename = "valorLiquido")]
    pub valor_liquido: f64,
    #[serde(rename = "numDocumento", default)]
    pub num_documento: Option<String>,
    #[serde(rename = "urlDocumento", default)]
    pub url_documento: Option<String>,
    #[serde(default)]
    pub detalhes_litros: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CeapApiResponse {
    pub dados: Vec<CeapItemApi>,
}

#[derive(Clone)]
pub struct CeapClient {
    client: Client,
    pub base_url: String,
}

impl CeapClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("RadarCivico/1.0")
                .build()
                .unwrap_or_default(),
            base_url: "https://dadosabertos.camara.leg.br/api/v2".to_string(),
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

    pub fn parse_despesas_json(json_str: &str) -> Result<Vec<CeapItemApi>> {
        let resp: CeapApiResponse = serde_json::from_str(json_str)
            .map_err(|e| IngestionError::Parse(format!("Falha ao parsear JSON CEAP: {e}")))?;
        Ok(resp.dados)
    }

    pub async fn buscar_despesas_deputado(
        &self,
        id_deputado: u32,
        ano: i32,
    ) -> Result<Vec<CeapItemApi>> {
        let url = format!("{}/deputados/{}/despesas?ano={}", self.base_url, id_deputado, ano);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(IngestionError::Reqwest)?;

        let api_resp: CeapApiResponse = resp
            .json()
            .await
            .map_err(IngestionError::Reqwest)?;

        Ok(api_resp.dados)
    }

    pub fn salvar_despesas_ceap(
        conn: &mut Connection,
        despesas: &[CeapItemApi],
        fallback_nome: Option<&str>,
        fallback_cpf: Option<&str>,
    ) -> Result<usize> {
        if despesas.is_empty() {
            return Ok(0);
        }

        let tx = storage::transaction_immediate(conn)?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO despesas_parlamentares (
                    casa_legislativa, parlamentar_nome, parlamentar_cpf_mascarado,
                    data_emissao, categoria_despesa, fornecedor_nome, fornecedor_cnpj_cpf,
                    valor_liquido, numero_documento, url_nota_fiscal, detalhes_litros
                 ) VALUES ('CAMARA', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?;

            for item in despesas {
                let nome = item
                    .nome_parlamentar
                    .as_deref()
                    .or(fallback_nome)
                    .unwrap_or("PARLAMENTAR DESCONHECIDO");

                let raw_cpf = item.cpf_parlamentar.as_deref().or(fallback_cpf).unwrap_or("");
                let cpf_masc = if !raw_cpf.is_empty() {
                    Some(mascarar_cpf(raw_cpf))
                } else {
                    None
                };

                let data_emissao = item
                    .data_documento
                    .clone()
                    .unwrap_or_else(|| "1970-01-01".to_string());

                let doc_forn = limpar_cnpj(&item.cnpj_cpf_fornecedor);

                stmt.execute(storage::rusqlite::params![
                    nome,
                    cpf_masc,
                    data_emissao,
                    item.tipo_despesa,
                    item.nome_fornecedor,
                    doc_forn,
                    item.valor_liquido,
                    item.num_documento,
                    item.url_documento,
                    item.detalhes_litros,
                ])?;
            }
        }
        tx.commit()?;

        Ok(despesas.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::{run_migrations, DbPool};

    #[test]
    fn test_ceap_parse_json_and_save() {
        let json_data = r#"{
            "dados": [
                {
                    "ano": 2024,
                    "nomeParlamentar": "DEPUTADO EXEMPLO",
                    "cpfParlamentar": "12345678901",
                    "dataDocumento": "2024-04-15",
                    "tipoDespesa": "COMBUSTÍVEIS E LUBRIFICANTES.",
                    "nomeFornecedor": "POSTO BANDEIRANTES LTDA",
                    "cnpjCpfFornecedor": "00.123.456/0001-99",
                    "valorLiquido": 350.00,
                    "numDocumento": "NF-9988",
                    "urlDocumento": "http://nfe.fazenda.gov.br/doc9988",
                    "detalhes_litros": 60.5
                },
                {
                    "ano": 2024,
                    "nomeParlamentar": "DEPUTADO EXEMPLO",
                    "cpfParlamentar": "12345678901",
                    "dataDocumento": "2024-04-20",
                    "tipoDespesa": "SERVIÇOS POSTAIS",
                    "nomeFornecedor": "CORREIOS",
                    "cnpjCpfFornecedor": "34.028.316/0001-03",
                    "valorLiquido": 45.20,
                    "numDocumento": "REC-123"
                }
            ]
        }"#;

        let despesas = CeapClient::parse_despesas_json(json_data).unwrap();
        assert_eq!(despesas.len(), 2);
        assert_eq!(despesas[0].valor_liquido, 350.00);
        assert_eq!(despesas[0].detalhes_litros, Some(60.5));

        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        let inseridos = CeapClient::salvar_despesas_ceap(&mut conn, &despesas, None, None).unwrap();
        assert_eq!(inseridos, 2);

        let count: i64 = conn
            .query_row("SELECT count(*) FROM despesas_parlamentares", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let cpf: String = conn
            .query_row(
                "SELECT parlamentar_cpf_mascarado FROM despesas_parlamentares WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cpf, "***.456.789-**");
    }
}
