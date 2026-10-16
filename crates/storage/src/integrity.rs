use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

use crate::error::Result;

/// Calcula o hash SHA-256 em hexadecimal a partir de uma fatia de bytes na memória.
pub fn calcular_sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Calcula o hash SHA-256 e a contagem total de bytes a partir de qualquer leitor em streaming (buffer de 64 KB).
pub fn calcular_sha256_reader<R: Read>(mut reader: R) -> std::io::Result<(String, u64)> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut total_bytes = 0u64;

    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        total_bytes += n as u64;
    }

    Ok((hex::encode(hasher.finalize()), total_bytes))
}

/// Calcula o hash SHA-256 e o tamanho de um arquivo no disco sem carregá-lo por completo na memória.
pub fn calcular_sha256_file<P: AsRef<Path>>(path: P) -> std::io::Result<(String, u64)> {
    let file = std::fs::File::open(path)?;
    calcular_sha256_reader(file)
}

/// Representação estruturada de um arquivo rastreado na tabela de integridade.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistroArquivoIngerido {
    pub id: i64,
    pub fonte: String,
    pub nome_arquivo: String,
    pub hash_sha256: String,
    pub tamanho_bytes: i64,
    pub status: String,
    pub registros_afetados: i64,
    pub detalhes: Option<String>,
    pub criado_em: String,
}

/// Registra ou atualiza metadados e hash SHA-256 de um arquivo ingerido no banco de dados.
#[derive(Debug, Clone, Copy)]
pub struct NovoArquivoIngerido<'a> {
    pub fonte: &'a str,
    pub nome_arquivo: &'a str,
    pub hash_sha256: &'a str,
    pub tamanho_bytes: i64,
    pub status: &'a str,
    pub registros_afetados: i64,
    pub detalhes: Option<&'a str>,
}

pub fn registrar_arquivo_ingerido(
    conn: &Connection,
    registro: &NovoArquivoIngerido<'_>,
) -> Result<i64> {
    let NovoArquivoIngerido {
        fonte,
        nome_arquivo,
        hash_sha256,
        tamanho_bytes,
        status,
        registros_afetados,
        detalhes,
    } = *registro;
    let id = conn.query_row(
        "INSERT INTO arquivos_ingeridos (
            fonte, nome_arquivo, hash_sha256, tamanho_bytes, status, registros_afetados, detalhes
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(fonte, hash_sha256) DO UPDATE SET
            status = excluded.status,
            registros_afetados = excluded.registros_afetados,
            detalhes = excluded.detalhes,
            criado_em = CURRENT_TIMESTAMP RETURNING id",
        params![
            fonte,
            nome_arquivo,
            hash_sha256,
            tamanho_bytes,
            status,
            registros_afetados,
            detalhes
        ],
        |r| r.get(0),
    )?;
    Ok(id)
}

/// Busca um registro de arquivo ingerido pelo hash SHA-256 e pela fonte de origem.
pub fn buscar_arquivo_por_hash(
    conn: &Connection,
    fonte: &str,
    hash_sha256: &str,
) -> Result<Option<RegistroArquivoIngerido>> {
    let mut stmt = conn.prepare(
        "SELECT id, fonte, nome_arquivo, hash_sha256, tamanho_bytes, status, registros_afetados, detalhes, criado_em
         FROM arquivos_ingeridos
         WHERE fonte = ?1 AND hash_sha256 = ?2
         LIMIT 1;",
    )?;

    let mut rows = stmt.query(params![fonte, hash_sha256])?;

    if let Some(row) = rows.next()? {
        Ok(Some(RegistroArquivoIngerido {
            id: row.get(0)?,
            fonte: row.get(1)?,
            nome_arquivo: row.get(2)?,
            hash_sha256: row.get(3)?,
            tamanho_bytes: row.get(4)?,
            status: row.get(5)?,
            registros_afetados: row.get(6)?,
            detalhes: row.get(7)?,
            criado_em: row.get(8)?,
        }))
    } else {
        Ok(None)
    }
}

/// Verifica se um arquivo com o mesmo nome já havia sido ingerido anteriormente com um hash diferente.
/// Se houver, retorna `Some(hash_anterior)`, sinalizando uma retificação pelo órgão emissor (ex: TSE/Câmara).
pub fn verificar_retificacao(
    conn: &Connection,
    fonte: &str,
    nome_arquivo: &str,
    novo_hash: &str,
) -> Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT hash_sha256
         FROM arquivos_ingeridos
         WHERE fonte = ?1 AND nome_arquivo = ?2 AND hash_sha256 != ?3
         ORDER BY id DESC
         LIMIT 1;",
    )?;

    let mut rows = stmt.query(params![fonte, nome_arquivo, novo_hash])?;

    if let Some(row) = rows.next()? {
        let hash_antigo: String = row.get(0)?;
        Ok(Some(hash_antigo))
    } else {
        Ok(None)
    }
}

/// Lista os arquivos ingeridos mais recentes.
pub fn listar_arquivos_ingeridos(
    conn: &Connection,
    limit: Option<usize>,
) -> Result<Vec<RegistroArquivoIngerido>> {
    let limite = limit.unwrap_or(100) as i64;
    let mut stmt = conn.prepare(
        "SELECT id, fonte, nome_arquivo, hash_sha256, tamanho_bytes, status, registros_afetados, detalhes, criado_em
         FROM arquivos_ingeridos
         ORDER BY id DESC
         LIMIT ?1;",
    )?;

    let rows = stmt.query_map(params![limite], |row| {
        Ok(RegistroArquivoIngerido {
            id: row.get(0)?,
            fonte: row.get(1)?,
            nome_arquivo: row.get(2)?,
            hash_sha256: row.get(3)?,
            tamanho_bytes: row.get(4)?,
            status: row.get(5)?,
            registros_afetados: row.get(6)?,
            detalhes: row.get(7)?,
            criado_em: row.get(8)?,
        })
    })?;

    let mut lista = Vec::new();
    for row in rows {
        lista.push(row?);
    }
    Ok(lista)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::run_migrations;

    #[test]
    fn test_calculo_sha256_bytes_e_reader() {
        let payload = b"Radar Civico - Integridade e Transparencia";
        let hash_bytes = calcular_sha256_bytes(payload);
        assert_eq!(hash_bytes.len(), 64);

        let (hash_reader, total_bytes) = calcular_sha256_reader(&payload[..]).unwrap();
        assert_eq!(hash_bytes, hash_reader);
        assert_eq!(total_bytes, payload.len() as u64);
    }

    #[test]
    fn test_registro_e_busca_arquivo_ingerido() -> Result<()> {
        let mut conn = Connection::open_in_memory()?;
        run_migrations(&mut conn)?;

        let hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let id = registrar_arquivo_ingerido(
            &conn,
            &NovoArquivoIngerido {
                fonte: "UPLOAD_MANUAL",
                nome_arquivo: "teste_ceap.csv",
                hash_sha256: hash,
                tamanho_bytes: 1024,
                status: "CONCLUIDO",
                registros_afetados: 50,
                detalhes: Some("Tipo: CEAP"),
            },
        )?;
        assert!(id > 0);

        let achado =
            buscar_arquivo_por_hash(&conn, "UPLOAD_MANUAL", hash)?.expect("Deve encontrar");
        assert_eq!(achado.nome_arquivo, "teste_ceap.csv");
        assert_eq!(achado.registros_afetados, 50);

        let lista = listar_arquivos_ingeridos(&conn, Some(10))?;
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].hash_sha256, hash);

        Ok(())
    }

    #[test]
    fn test_deteccao_retificacao() -> Result<()> {
        let mut conn = Connection::open_in_memory()?;
        run_migrations(&mut conn)?;

        let hash_original = "1111111111111111111111111111111111111111111111111111111111111111";
        let hash_retificado = "2222222222222222222222222222222222222222222222222222222222222222";

        registrar_arquivo_ingerido(
            &conn,
            &NovoArquivoIngerido {
                fonte: "TSE",
                nome_arquivo: "consulta_cand_2024_ES.csv",
                hash_sha256: hash_original,
                tamanho_bytes: 5000,
                status: "CONCLUIDO",
                registros_afetados: 100,
                detalhes: None,
            },
        )?;

        // Mesma consulta com o mesmo hash não é retificação
        let retificacao =
            verificar_retificacao(&conn, "TSE", "consulta_cand_2024_ES.csv", hash_original)?;
        assert!(retificacao.is_none());

        // Mesma consulta com novo hash deve apontar retificação do hash anterior
        let retificacao =
            verificar_retificacao(&conn, "TSE", "consulta_cand_2024_ES.csv", hash_retificado)?;
        assert_eq!(retificacao, Some(hash_original.to_string()));

        Ok(())
    }
}
