use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use storage::rusqlite::Connection;
use storage::{batch_insert_despesas_parlamentares, NovaDespesaParlamentar};

use crate::error::{IngestionError, Result};
use crate::normalizer::{limpar_cnpj, mascarar_cpf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiLink {
    pub rel: String,
    pub href: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CamaraDespesaItem {
    #[serde(default)]
    pub ano: Option<i32>,
    #[serde(default)]
    pub mes: Option<i32>,
    #[serde(rename = "tipoDespesa", default)]
    pub tipo_despesa: String,
    #[serde(rename = "codDocumento", default)]
    pub cod_documento: Option<i64>,
    #[serde(rename = "tipoDocumento", default)]
    pub tipo_documento: Option<String>,
    #[serde(rename = "codTipoDocumento", default)]
    pub cod_tipo_documento: Option<i64>,
    #[serde(rename = "dataDocumento", default)]
    pub data_documento: Option<String>,
    #[serde(rename = "numDocumento", default)]
    pub num_documento: Option<String>,
    #[serde(rename = "valorDocumento", default)]
    pub valor_documento: Option<f64>,
    #[serde(rename = "urlDocumento", default)]
    pub url_documento: Option<String>,
    #[serde(rename = "nomeFornecedor", default)]
    pub nome_fornecedor: String,
    #[serde(rename = "cnpjCpfFornecedor", default)]
    pub cnpj_cpf_fornecedor: String,
    #[serde(rename = "valorLiquido", default)]
    pub valor_liquido: f64,
    #[serde(rename = "valorGlosa", default)]
    pub valor_glosa: Option<f64>,
    #[serde(rename = "numRessarcimento", default)]
    pub num_ressarcimento: Option<String>,
    #[serde(rename = "codLote", default)]
    pub cod_lote: Option<i64>,
    #[serde(default)]
    pub parcela: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CamaraDeputadoItem {
    pub id: u32,
    #[serde(rename = "uri", default)]
    pub uri: Option<String>,
    #[serde(rename = "nome", default)]
    pub nome: String,
    #[serde(rename = "siglaPartido", default)]
    pub sigla_partido: Option<String>,
    #[serde(rename = "uriPartido", default)]
    pub uri_partido: Option<String>,
    #[serde(rename = "siglaUf", default)]
    pub sigla_uf: Option<String>,
    #[serde(rename = "idLegislatura", default)]
    pub id_legislatura: Option<i32>,
    #[serde(rename = "urlFoto", default)]
    pub url_foto: Option<String>,
    #[serde(rename = "email", default)]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CamaraListResponse<T> {
    pub dados: Vec<T>,
    #[serde(default)]
    pub links: Vec<ApiLink>,
}

#[derive(Clone)]
pub struct CamaraApiClient {
    client: Client,
    pub base_url: String,
    pub rate_limit_delay: Duration,
}

impl Default for CamaraApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl CamaraApiClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("RadarCivico/1.0 (Auditoria Civica)")
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            base_url: "https://dadosabertos.camara.leg.br/api/v2".to_string(),
            rate_limit_delay: Duration::from_millis(50),
        }
    }

    pub fn with_base_url(base_url: String) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RadarCivico/1.0 (Auditoria Civica)")
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            base_url,
            rate_limit_delay: Duration::ZERO,
        }
    }

    pub fn with_rate_limit(mut self, delay: Duration) -> Self {
        self.rate_limit_delay = delay;
        self
    }

    pub fn extrair_link_next(links: &[ApiLink]) -> Option<String> {
        links
            .iter()
            .find(|l| l.rel.trim().eq_ignore_ascii_case("next"))
            .map(|l| l.href.clone())
    }

    pub async fn buscar_pagina_despesas(&self, url: &str) -> Result<CamaraListResponse<CamaraDespesaItem>> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(IngestionError::Reqwest)?;

        let list_resp: CamaraListResponse<CamaraDespesaItem> = resp
            .json()
            .await
            .map_err(IngestionError::Reqwest)?;

        Ok(list_resp)
    }

    pub async fn buscar_despesas_deputado_hateoas(
        &self,
        id_deputado: u32,
        ano: i32,
        max_paginas: Option<usize>,
    ) -> Result<Vec<CamaraDespesaItem>> {
        let mut url_atual = format!(
            "{}/deputados/{}/despesas?ano={}&itens=100&ordem=ASC&ordenarPor=ano",
            self.base_url, id_deputado, ano
        );

        let mut todas_despesas = Vec::new();
        let mut paginas_processadas = 0;

        loop {
            let pagina = self.buscar_pagina_despesas(&url_atual).await?;
            todas_despesas.extend(pagina.dados);
            paginas_processadas += 1;

            if let Some(limite) = max_paginas {
                if paginas_processadas >= limite {
                    break;
                }
            }

            if let Some(proxima_url) = Self::extrair_link_next(&pagina.links) {
                if self.rate_limit_delay > Duration::ZERO {
                    tokio::time::sleep(self.rate_limit_delay).await;
                }
                url_atual = proxima_url;
            } else {
                break;
            }
        }

        Ok(todas_despesas)
    }

    pub async fn listar_deputados(&self, max_paginas: Option<usize>) -> Result<Vec<CamaraDeputadoItem>> {
        let mut url_atual = format!(
            "{}/deputados?ordem=ASC&ordenarPor=nome",
            self.base_url
        );

        let mut todos_deputados = Vec::new();
        let mut paginas_processadas = 0;

        loop {
            let resp = self
                .client
                .get(&url_atual)
                .send()
                .await
                .map_err(IngestionError::Reqwest)?;

            let list_resp: CamaraListResponse<CamaraDeputadoItem> = resp
                .json()
                .await
                .map_err(IngestionError::Reqwest)?;

            todos_deputados.extend(list_resp.dados);
            paginas_processadas += 1;

            if let Some(limite) = max_paginas {
                if paginas_processadas >= limite {
                    break;
                }
            }

            if let Some(proxima_url) = Self::extrair_link_next(&list_resp.links) {
                if self.rate_limit_delay > Duration::ZERO {
                    tokio::time::sleep(self.rate_limit_delay).await;
                }
                url_atual = proxima_url;
            } else {
                break;
            }
        }

        Ok(todos_deputados)
    }

    pub fn converter_para_despesas_parlamentares(
        deputado_nome: &str,
        deputado_cpf: Option<&str>,
        itens: &[CamaraDespesaItem],
    ) -> Vec<NovaDespesaParlamentar> {
        let cpf_mascarado = deputado_cpf.map(mascarar_cpf);

        itens
            .iter()
            .map(|item| {
                let data_emissao = item
                    .data_documento
                    .clone()
                    .unwrap_or_else(|| "1970-01-01".to_string());
                let fornecedor_doc = limpar_cnpj(&item.cnpj_cpf_fornecedor);

                NovaDespesaParlamentar {
                    casa_legislativa: "CAMARA".to_string(),
                    parlamentar_nome: deputado_nome.to_string(),
                    parlamentar_cpf_mascarado: cpf_mascarado.clone(),
                    data_emissao,
                    categoria_despesa: if item.tipo_despesa.is_empty() {
                        "DESPESA DIVERSA".to_string()
                    } else {
                        item.tipo_despesa.clone()
                    },
                    fornecedor_nome: if item.nome_fornecedor.is_empty() {
                        "FORNECEDOR".to_string()
                    } else {
                        item.nome_fornecedor.clone()
                    },
                    fornecedor_cnpj_cpf: fornecedor_doc,
                    valor_liquido: item.valor_liquido,
                    numero_documento: item.num_documento.clone(),
                    url_nota_fiscal: item.url_documento.clone(),
                    detalhes_litros: None,
                }
            })
            .collect()
    }

    pub fn salvar_despesas_api(
        conn: &mut Connection,
        deputado_nome: &str,
        deputado_cpf: Option<&str>,
        itens: &[CamaraDespesaItem],
    ) -> Result<usize> {
        if itens.is_empty() {
            return Ok(0);
        }

        let despesas = Self::converter_para_despesas_parlamentares(deputado_nome, deputado_cpf, itens);
        batch_insert_despesas_parlamentares(conn, &despesas).map_err(IngestionError::Storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use storage::{run_migrations, DbPool};

    #[test]
    fn test_camara_api_extrair_link_next() {
        let links = vec![
            ApiLink {
                rel: "self".to_string(),
                href: "https://api.camara.leg.br/deputados/1/despesas?pagina=1".to_string(),
            },
            ApiLink {
                rel: "next".to_string(),
                href: "https://api.camara.leg.br/deputados/1/despesas?pagina=2".to_string(),
            },
            ApiLink {
                rel: "last".to_string(),
                href: "https://api.camara.leg.br/deputados/1/despesas?pagina=5".to_string(),
            },
        ];

        let next = CamaraApiClient::extrair_link_next(&links);
        assert_eq!(
            next,
            Some("https://api.camara.leg.br/deputados/1/despesas?pagina=2".to_string())
        );

        let links_sem_next = vec![ApiLink {
            rel: "self".to_string(),
            href: "https://api.camara.leg.br/deputados/1/despesas?pagina=5".to_string(),
        }];
        assert_eq!(CamaraApiClient::extrair_link_next(&links_sem_next), None);
    }

    #[tokio::test]
    async fn test_camara_api_hateoas_pagination_and_persistence() {
        let request_counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = request_counter.clone();

        // Sobe servidor HTTP de teste em porta aleatória
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();
        let port = local_addr.port();

        tokio::spawn(async move {
            loop {
                if let Ok((mut socket, _)) = listener.accept().await {
                    let c = counter_clone.fetch_add(1, Ordering::SeqCst);
                    tokio::spawn(async move {
                        use tokio::io::{AsyncReadExt, AsyncWriteExt};
                        let mut buf = [0u8; 2048];
                        let _ = socket.read(&mut buf).await;

                        let body = if c == 0 {
                            // Página 1: 2 itens com link next
                            serde_json::json!({
                                "dados": [
                                    {
                                        "ano": 2024,
                                        "tipoDespesa": "COMBUSTÍVEIS E LUBRIFICANTES.",
                                        "dataDocumento": "2024-04-01",
                                        "numDocumento": "NF-01",
                                        "nomeFornecedor": "POSTO ALPHA",
                                        "cnpjCpfFornecedor": "11.222.333/0001-44",
                                        "valorLiquido": 200.00
                                    },
                                    {
                                        "ano": 2024,
                                        "tipoDespesa": "TELEFONIA",
                                        "dataDocumento": "2024-04-05",
                                        "numDocumento": "TEL-99",
                                        "nomeFornecedor": "OPERADORA X",
                                        "cnpjCpfFornecedor": "22.333.444/0001-55",
                                        "valorLiquido": 150.00
                                    }
                                ],
                                "links": [
                                    {
                                        "rel": "self",
                                        "href": format!("http://127.0.0.1:{}/deputados/50/despesas?pagina=1", port)
                                    },
                                    {
                                        "rel": "next",
                                        "href": format!("http://127.0.0.1:{}/deputados/50/despesas?pagina=2", port)
                                    }
                                ]
                            })
                        } else {
                            // Página 2: 1 item sem link next
                            serde_json::json!({
                                "dados": [
                                    {
                                        "ano": 2024,
                                        "tipoDespesa": "DIVULGAÇÃO",
                                        "dataDocumento": "2024-04-10",
                                        "numDocumento": "PUB-88",
                                        "nomeFornecedor": "AGENCIA Y",
                                        "cnpjCpfFornecedor": "33.444.555/0001-66",
                                        "valorLiquido": 5000.00
                                    }
                                ],
                                "links": [
                                    {
                                        "rel": "self",
                                        "href": format!("http://127.0.0.1:{}/deputados/50/despesas?pagina=2", port)
                                    }
                                ]
                            })
                        };

                        let body_str = serde_json::to_string(&body).unwrap();
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body_str.len(),
                            body_str
                        );
                        let _ = socket.write_all(response.as_bytes()).await;
                    });
                }
            }
        });

        let client = CamaraApiClient::with_base_url(format!("http://127.0.0.1:{}", port));
        let despesas = client
            .buscar_despesas_deputado_hateoas(50, 2024, None)
            .await
            .expect("falha ao buscar despesas hateoas");

        assert_eq!(despesas.len(), 3);
        assert_eq!(despesas[0].valor_liquido, 200.00);
        assert_eq!(despesas[1].valor_liquido, 150.00);
        assert_eq!(despesas[2].valor_liquido, 5000.00);

        // Valida persistência no SQLite
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        let inseridos = CamaraApiClient::salvar_despesas_api(
            &mut conn,
            "DEPUTADO TESTE",
            Some("12345678900"),
            &despesas,
        )
        .unwrap();
        assert_eq!(inseridos, 3);

        let count: i64 = conn
            .query_row("SELECT count(*) FROM despesas_parlamentares", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_camara_api_rate_limit_delay() {
        let client = CamaraApiClient::new().with_rate_limit(Duration::from_millis(25));
        assert_eq!(client.rate_limit_delay, Duration::from_millis(25));
    }
}
