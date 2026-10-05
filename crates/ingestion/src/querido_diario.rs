use reqwest::Client;
use serde::{Deserialize, Serialize};
use storage::rusqlite::Connection;

use crate::error::{IngestionError, Result};
use crate::normalizer::limpar_apenas_digitos;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExcerptDiario {
    pub date: Option<String>,
    pub territory_name: Option<String>,
    pub territory_id: Option<String>,
    pub url: Option<String>,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueridoDiarioApiResponse {
    pub total_gazettes: i64,
    pub gazettes: Vec<ExcerptDiario>,
}

#[derive(Clone)]
pub struct QueridoDiarioClient {
    client: Client,
    pub base_url: String,
}

impl QueridoDiarioClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("RadarCivico/1.0")
                .build()
                .unwrap_or_default(),
            base_url: "https://queridodiario.ok.org.br/api".to_string(),
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

    pub fn construir_query(doador_nome: &str, termo_acao: Option<&str>) -> String {
        let acao = termo_acao.unwrap_or("nomear OR portaria OR \"cargo em comissão\" OR lotação");
        format!("\"{}\" AND ({})", doador_nome.trim(), acao)
    }

    pub fn parse_response(json_str: &str) -> Result<QueridoDiarioApiResponse> {
        serde_json::from_str(json_str)
            .map_err(|e| IngestionError::Parse(format!("Falha ao parsear Querido Diário: {e}")))
    }

    pub async fn buscar_nomeacoes(
        &self,
        doador_nome: &str,
        municipio: Option<&str>,
        data_inicio: Option<&str>,
    ) -> Result<QueridoDiarioApiResponse> {
        let query = Self::construir_query(doador_nome, None);
        let mut url = format!("{}/gazettes?querystring={}", self.base_url, urlencoding::encode(&query));

        if let Some(muni) = municipio {
            url.push_str(&format!("&territory_name={}", urlencoding::encode(muni)));
        }
        if let Some(dt) = data_inicio {
            url.push_str(&format!("&since={}", dt));
        }

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(IngestionError::Reqwest)?;

        let texto = resp.text().await.map_err(IngestionError::Reqwest)?;
        Self::parse_response(&texto)
    }

    pub fn buscar_cache(
        conn: &Connection,
        doador_cpf_cnpj: &str,
        municipio_uf: &str,
    ) -> Result<Option<QueridoDiarioApiResponse>> {
        let doc_clean = limpar_apenas_digitos(doador_cpf_cnpj);
        let mut stmt = conn.prepare_cached(
            "SELECT payload_json FROM cache_consultas_diario
             WHERE (doador_cpf_cnpj = ?1 OR doador_cpf_cnpj = ?2) AND municipio_uf = ?3
             ORDER BY data_consulta DESC LIMIT 1",
        )?;

        let res = stmt
            .query_row(
                storage::rusqlite::params![doador_cpf_cnpj, doc_clean, municipio_uf],
                |row| {
                    let payload: Option<String> = row.get(0)?;
                    Ok(payload)
                },
            )
            .ok()
            .flatten();

        if let Some(json_str) = res {
            let parsed = Self::parse_response(&json_str)?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    pub fn salvar_cache(
        conn: &mut Connection,
        doador_cpf_cnpj: &str,
        termo: &str,
        municipio_uf: &str,
        ocorrencias: i64,
        payload_json: &str,
    ) -> Result<()> {
        let doc_clean = limpar_apenas_digitos(doador_cpf_cnpj);
        conn.execute(
            "INSERT INTO cache_consultas_diario (
                doador_cpf_cnpj, termo_pesquisado, municipio_uf, ocorrencias_encontradas, payload_json
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            storage::rusqlite::params![doc_clean, termo, municipio_uf, ocorrencias, payload_json],
        )?;
        Ok(())
    }
}

// Simple urlencoding helper to avoid extra external crate
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::with_capacity(s.len() * 3);
        for byte in s.bytes() {
            match byte {
                b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'-' | b'_' | b'.' | b'~' => {
                    result.push(byte as char);
                }
                b' ' => result.push('+'),
                _ => {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::{run_migrations, DbPool};

    #[test]
    fn test_querido_diario_construir_query() {
        let q = QueridoDiarioClient::construir_query("MARIA DA SILVA", None);
        assert_eq!(
            q,
            "\"MARIA DA SILVA\" AND (nomear OR portaria OR \"cargo em comissão\" OR lotação)"
        );
    }

    #[test]
    fn test_querido_diario_cache_flow() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        let mock_json = r#"{
            "total_gazettes": 1,
            "gazettes": [
                {
                    "date": "2024-02-01",
                    "territory_name": "Campinas",
                    "territory_id": "3509502",
                    "url": "https://diario.campinas.sp.gov.br/edicao123.pdf",
                    "excerpt": "Portaria 12/2024: Resolve nomear MARIA DA SILVA para o cargo em comissão de assessora..."
                }
            ]
        }"#;

        let cpf = "123.456.789-00";
        let muni = "CAMPINAS-SP";

        // Cache deve ser None inicialmente
        let inicial = QueridoDiarioClient::buscar_cache(&conn, cpf, muni).unwrap();
        assert!(inicial.is_none());

        // Salva resposta no cache
        QueridoDiarioClient::salvar_cache(&mut conn, cpf, "MARIA DA SILVA", muni, 1, mock_json).unwrap();

        // Agora busca no cache
        let em_cache = QueridoDiarioClient::buscar_cache(&conn, cpf, muni)
            .unwrap()
            .expect("Deve encontrar no cache");

        assert_eq!(em_cache.total_gazettes, 1);
        assert_eq!(em_cache.gazettes[0].territory_name.as_deref(), Some("Campinas"));
        assert!(em_cache.gazettes[0].excerpt.contains("Resolve nomear MARIA DA SILVA"));
    }
}
