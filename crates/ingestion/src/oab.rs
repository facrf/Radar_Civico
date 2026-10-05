use reqwest::Client;
use serde::{Deserialize, Serialize};
use storage::rusqlite::Connection;

use crate::error::{IngestionError, Result};
use crate::normalizer::mascarar_cpf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OabConsultaResult {
    pub nome: String,
    pub inscricao: String,
    pub uf: String,
    pub situacao: String,
    pub tipo: String,
    pub payload_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CnaItemPayload {
    #[serde(rename = "Nome")]
    pub nome: String,
    #[serde(rename = "Inscricao")]
    pub inscricao: String,
    #[serde(rename = "Uf")]
    pub uf: String,
    #[serde(rename = "Situacao")]
    pub situacao: String,
    #[serde(rename = "TipoInscricao", default)]
    pub tipo_inscricao: Option<String>,
}

#[derive(Clone)]
pub struct OabScraperClient {
    client: Client,
    pub base_url: String,
}

impl OabScraperClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (compatible; RadarCivico/1.0)")
                .build()
                .unwrap_or_default(),
            base_url: "https://cna.oab.org.br/api".to_string(),
        }
    }

    pub fn with_base_url(base_url: String) -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (compatible; RadarCivico/1.0)")
                .build()
                .unwrap_or_default(),
            base_url,
        }
    }

    pub fn parse_cna_response(json_str: &str) -> Result<Vec<OabConsultaResult>> {
        let items: Vec<CnaItemPayload> = serde_json::from_str(json_str)
            .map_err(|e| IngestionError::Parse(format!("Falha ao parsear payload CNA/OAB: {e}")))?;

        let results = items
            .into_iter()
            .map(|item| {
                let payload_json = serde_json::to_string(&item).unwrap_or_default();
                OabConsultaResult {
                    nome: item.nome.trim().to_uppercase(),
                    inscricao: item.inscricao.trim().to_string(),
                    uf: item.uf.trim().to_uppercase(),
                    situacao: item.situacao.trim().to_uppercase(),
                    tipo: item
                        .tipo_inscricao
                        .unwrap_or_else(|| "ADVOGADO".to_string())
                        .trim()
                        .to_uppercase(),
                    payload_json,
                }
            })
            .collect();

        Ok(results)
    }

    pub async fn consultar_cna(
        &self,
        nome: &str,
        uf: Option<&str>,
    ) -> Result<Vec<OabConsultaResult>> {
        let mut url = format!("{}/search?q={}", self.base_url, urlencoding::encode(nome));
        if let Some(uf_val) = uf {
            url.push_str(&format!("&uf={}", uf_val));
        }

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(IngestionError::Reqwest)?;

        let texto = resp.text().await.map_err(IngestionError::Reqwest)?;
        Self::parse_cna_response(&texto)
    }

    pub fn buscar_registro_banco(
        conn: &Connection,
        nome: &str,
        uf: Option<&str>,
    ) -> Result<Option<OabConsultaResult>> {
        let nome_upper = nome.trim().to_uppercase();

        let query = if uf.is_some() {
            format!(
                "SELECT pessoa_nome, numero_registro, seccional_uf, situacao_registro, tipo_inscricao, payload_json
                 FROM registros_profissionais
                 WHERE orgao_emissor = 'OAB' AND pessoa_nome = ?1 AND seccional_uf = ?2
                 LIMIT 1"
            )
        } else {
            format!(
                "SELECT pessoa_nome, numero_registro, seccional_uf, situacao_registro, tipo_inscricao, payload_json
                 FROM registros_profissionais
                 WHERE orgao_emissor = 'OAB' AND pessoa_nome = ?1
                 LIMIT 1"
            )
        };

        let mut stmt = conn.prepare_cached(&query)?;

        let res = if let Some(uf_val) = uf {
            stmt.query_row(
                storage::rusqlite::params![nome_upper, uf_val.trim().to_uppercase()],
                |row| {
                    Ok(OabConsultaResult {
                        nome: row.get(0)?,
                        inscricao: row.get(1)?,
                        uf: row.get(2)?,
                        situacao: row.get(3)?,
                        tipo: row.get(4)?,
                        payload_json: row.get(5)?,
                    })
                },
            )
            .ok()
        } else {
            stmt.query_row(storage::rusqlite::params![nome_upper], |row| {
                Ok(OabConsultaResult {
                    nome: row.get(0)?,
                    inscricao: row.get(1)?,
                    uf: row.get(2)?,
                    situacao: row.get(3)?,
                    tipo: row.get(4)?,
                    payload_json: row.get(5)?,
                })
            })
            .ok()
        };

        Ok(res)
    }

    pub fn salvar_registro_oab(
        conn: &mut Connection,
        registro: &OabConsultaResult,
        cpf_mascarado: Option<&str>,
    ) -> Result<i64> {
        let cpf_masc = cpf_mascarado.map(|c| mascarar_cpf(c));

        conn.execute(
            "INSERT INTO registros_profissionais (
                pessoa_nome, cpf_mascarado, orgao_emissor, numero_registro,
                seccional_uf, situacao_registro, tipo_inscricao, payload_json
             ) VALUES (?1, ?2, 'OAB', ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(orgao_emissor, seccional_uf, numero_registro) DO UPDATE SET
                situacao_registro = excluded.situacao_registro,
                tipo_inscricao = excluded.tipo_inscricao,
                data_consulta = CURRENT_TIMESTAMP,
                payload_json = excluded.payload_json",
            storage::rusqlite::params![
                registro.nome,
                cpf_masc,
                registro.inscricao,
                registro.uf,
                registro.situacao,
                registro.tipo,
                registro.payload_json,
            ],
        )?;

        let id = conn.last_insert_rowid();
        Ok(id)
    }
}

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
    fn test_oab_cna_parse_and_cache() {
        let mock_json = r#"[
            {
                "Nome": "DR ADVOGADO TESTE",
                "Inscricao": "123456",
                "Uf": "SP",
                "Situacao": "REGULAR",
                "TipoInscricao": "ADVOGADO"
            }
        ]"#;

        let results = OabScraperClient::parse_cna_response(mock_json).unwrap();
        assert_eq!(results.len(), 1);
        let res = &results[0];
        assert_eq!(res.nome, "DR ADVOGADO TESTE");
        assert_eq!(res.inscricao, "123456");
        assert_eq!(res.situacao, "REGULAR");

        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // Salva registro
        OabScraperClient::salvar_registro_oab(&mut conn, res, Some("123.456.789-00")).unwrap();

        // Consulta banco
        let salvo = OabScraperClient::buscar_registro_banco(&conn, "DR ADVOGADO TESTE", Some("SP"))
            .unwrap()
            .expect("Deve encontrar registro salvo");

        assert_eq!(salvo.inscricao, "123456");
        assert_eq!(salvo.situacao, "REGULAR");
        assert_eq!(salvo.uf, "SP");

        // Atualização de status para LICENCIADO
        let mut atualizado = res.clone();
        atualizado.situacao = "LICENCIADO".to_string();
        OabScraperClient::salvar_registro_oab(&mut conn, &atualizado, None).unwrap();

        let verificado = OabScraperClient::buscar_registro_banco(&conn, "DR ADVOGADO TESTE", Some("SP"))
            .unwrap()
            .unwrap();

        assert_eq!(verificado.situacao, "LICENCIADO");
    }
}
