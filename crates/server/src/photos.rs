//! Retratos conhecidos por nome completo, com cópia local e atribuição pública.
use serde::Deserialize;
use storage::{rusqlite::params, DbPool};
#[derive(Deserialize)]
struct Portrait {
    nome: String,
    arquivo: String,
    url: String,
}
pub fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() > 5 * 1024 * 1024 {
        return None;
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) && bytes.ends_with(&[0xff, 0xd9]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.windows(4).any(|w| w == b"IEND") {
        Some("image/png")
    } else if bytes.len() > 20 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}
pub async fn download(url: &str) -> anyhow::Result<(Vec<u8>, String)> {
    let client = crate::security::public_client(url).await?;
    let mut response = client.get(url).send().await?.error_for_status()?;
    anyhow::ensure!(
        response.status().is_success(),
        "Fonte redirecionou; informe a URL final da imagem"
    );
    anyhow::ensure!(
        response.content_length().unwrap_or(0) <= 5 * 1024 * 1024,
        "Imagem excede 5 MB"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(
            bytes.len() + chunk.len() <= 5 * 1024 * 1024,
            "Imagem excede 5 MB"
        );
        bytes.extend_from_slice(&chunk);
    }
    let mime = image_mime(&bytes).ok_or_else(|| {
        anyhow::anyhow!("A fonte não retornou uma imagem JPEG, PNG ou WebP válida")
    })?;
    Ok((bytes, mime.into()))
}
pub fn restore_catalog(pool: &DbPool, web_dir: &std::path::Path) -> anyhow::Result<usize> {
    let path = web_dir.join("autoridades/fontes.json");
    if !path.exists() {
        return Ok(0);
    }
    let records: Vec<Portrait> = serde_json::from_slice(&std::fs::read(path)?)?;
    let mut conn = pool.get()?;
    let tx = storage::transaction_immediate(&mut conn)?;
    let mut count = 0;
    for record in records {
        anyhow::ensure!(
            !record.arquivo.contains('/') && !record.arquivo.contains(".."),
            "Arquivo inválido no catálogo"
        );
        let bytes = std::fs::read(web_dir.join("autoridades").join(&record.arquivo))?;
        let mime = image_mime(&bytes)
            .ok_or_else(|| anyhow::anyhow!("Retrato inválido de {}", record.nome))?;
        let mut stmt = tx.prepare("SELECT id, foto_blob FROM politicos WHERE UPPER(TRIM(nome_completo)) = UPPER(TRIM(?1))")?;
        let entries: Vec<(i64, Option<Vec<u8>>)> = stmt
            .query_map([&record.nome], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (id, existing) in entries {
            if existing.as_deref().and_then(image_mime).is_none() {
                count += tx.execute(
                    "UPDATE politicos SET foto_blob=?1, foto_mime=?2, foto_url=?3 WHERE id=?4",
                    params![bytes, mime, record.url, id],
                )?;
            }
        }
    }
    tx.commit()?;
    Ok(count)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_error_pages_and_truncated_images() {
        assert!(image_mime(b"<html>Access denied</html>").is_none());
        assert!(image_mime(&[0xff, 0xd8, 0xff, 0x00]).is_none());
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    use base64::Engine;
    #[test]
    fn exact_identity_preserves_manual_portraits_and_unknown_people() {
        let pool = DbPool::open_in_memory().unwrap();
        { let mut conn=pool.get().unwrap(); storage::run_migrations(&mut conn).unwrap();
          conn.execute("INSERT INTO politicos (nome_completo, nome_urna) VALUES ('PESSOA CERTA', 'CERTA'), ('PESSOA CERTA JUNIOR', 'JUNIOR')",[]).unwrap(); }
        let dir=tempfile::tempdir().unwrap(); std::fs::create_dir(dir.path().join("autoridades")).unwrap();
        let png=base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==").unwrap();
        std::fs::write(dir.path().join("autoridades/test.png"),&png).unwrap();
        std::fs::write(dir.path().join("autoridades/fontes.json"),r#"[{"nome":"PESSOA CERTA","arquivo":"test.png","url":"/autoridades/test.png"}]"#).unwrap();
        assert_eq!(restore_catalog(&pool,dir.path()).unwrap(),1);
        assert_eq!(restore_catalog(&pool,dir.path()).unwrap(),0);
        let conn=pool.get().unwrap(); let n:i64=conn.query_row("SELECT count(*) FROM politicos WHERE foto_blob IS NOT NULL",[],|r|r.get(0)).unwrap();assert_eq!(n,1);
    }
}
