//! Snapshots duráveis de tarefas, independentes da representação da aplicação.
use crate::Result;
use rusqlite::{Connection, OptionalExtension};
pub fn save(
    conn: &Connection,
    namespace: &str,
    id: &str,
    running: bool,
    payload: &str,
) -> Result<()> {
    conn.execute("INSERT INTO tarefas_importacao(namespace,id,em_execucao,payload_json) VALUES (?1,?2,?3,?4)
        ON CONFLICT(namespace,id) DO UPDATE SET em_execucao=excluded.em_execucao,payload_json=excluded.payload_json,atualizado_em=CURRENT_TIMESTAMP", rusqlite::params![namespace,id,running,payload])?;
    Ok(())
}
pub fn load(conn: &Connection, namespace: &str, id: &str) -> Result<Option<String>> {
    let started = std::time::Instant::now();
    loop {
        let result = conn
            .query_row(
                "SELECT payload_json FROM tarefas_importacao WHERE namespace=?1 AND id=?2",
                [namespace, id],
                |row| row.get(0),
            )
            .optional();
        match result {
            Ok(value) => return Ok(value),
            Err(error) if matches!(&error, rusqlite::Error::SqliteFailure(code, _) if matches!(code.code, rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)) =>
            {
                if started.elapsed() >= std::time::Duration::from_secs(60) {
                    return Err(error.into());
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

pub fn list(conn: &Connection, namespace: &str) -> Result<Vec<(String, bool, String)>> {
    let mut stmt=conn.prepare("SELECT id,em_execucao,payload_json FROM tarefas_importacao WHERE namespace=?1 ORDER BY atualizado_em DESC")?;
    let rows = stmt.query_map([namespace], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}
/// Executar uma vez no arranque: tarefas antigas não possuem workers neste processo.
pub fn recover_interrupted(conn: &Connection) -> Result<usize> {
    Ok(conn.execute("UPDATE tarefas_importacao SET em_execucao=0, atualizado_em=CURRENT_TIMESTAMP,
        payload_json=CASE namespace
          WHEN 'config' THEN json_set(payload_json,'$.status','INTERROMPIDO','$.mensagem','Interrompido pelo reinício do serviço; inicie uma nova importação.','$.concluido_em',strftime('%Y-%m-%dT%H:%M:%SZ','now'))
          WHEN 'importer' THEN json_set(payload_json,'$.stage','INTERROMPIDO','$.is_running',json('false'),'$.message','Interrompido pelo reinício do serviço; inicie uma nova importação.','$.last_error','Reinício do serviço','$.finished_at',strftime('%Y-%m-%dT%H:%M:%SZ','now'))
          WHEN 'legacy-progress' THEN json_set(payload_json,'$.is_running',json('false'),'$.last_error','Interrompido pelo reinício do serviço; inicie uma nova importação.')
          ELSE payload_json END WHERE em_execucao=1",[])?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recuperacao_preserva_progresso_legado() {
        let pool = crate::DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        crate::run_migrations(&mut conn).unwrap();
        save(
            &conn,
            "legacy-progress",
            "global",
            true,
            r#"{"is_running":true,"records_processed":123,"percentage":40}"#,
        )
        .unwrap();
        assert_eq!(recover_interrupted(&conn).unwrap(), 1);
        let value: serde_json::Value =
            serde_json::from_str(&load(&conn, "legacy-progress", "global").unwrap().unwrap())
                .unwrap();
        assert_eq!(value["is_running"], false);
        assert_eq!(value["records_processed"], 123);
        assert!(value["last_error"].as_str().unwrap().contains("reinício"));
    }
    #[test]
    fn worker_antigo_nao_sobrescreve_novo() {
        let pool = crate::DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        crate::run_migrations(&mut conn).unwrap();
        save(
            &conn,
            "importer",
            "tse",
            true,
            r#"{"started_at":"novo","records_processed":0}"#,
        )
        .unwrap();
        assert!(!save_current(
            &conn,
            "importer",
            "tse",
            false,
            r#"{"started_at":"antigo","records_processed":100}"#
        )
        .unwrap());
        assert!(load(&conn, "importer", "tse")
            .unwrap()
            .unwrap()
            .contains("novo"));
        assert!(save_current(
            &conn,
            "importer",
            "tse",
            false,
            r#"{"started_at":"novo","records_processed":200}"#
        )
        .unwrap());
    }
    #[test]
    fn snapshots_persistem_e_atualizam() {
        let pool = crate::DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        crate::run_migrations(&mut conn).unwrap();
        save(&conn, "jobs", "a", true, "{\"n\":1}").unwrap();
        save(&conn, "jobs", "a", false, "{\"n\":2}").unwrap();
        assert_eq!(list(&conn, "jobs").unwrap().len(), 1);
        assert_eq!(load(&conn, "jobs", "a").unwrap().unwrap(), "{\"n\":2}");
        assert!(load(&conn, "other", "a").unwrap().is_none());
    }
}

/// Um snapshot de worker antigo não pode substituir uma execução recém-iniciada.
pub fn save_current(
    conn: &Connection,
    namespace: &str,
    id: &str,
    running: bool,
    payload: &str,
) -> Result<bool> {
    Ok(conn.execute("UPDATE tarefas_importacao SET em_execucao=?3,payload_json=?4,atualizado_em=CURRENT_TIMESTAMP WHERE namespace=?1 AND id=?2 AND json_extract(payload_json,'$.started_at') IS json_extract(?4,'$.started_at')",rusqlite::params![namespace,id,running,payload])? > 0)
}

/// Reserva atômica. Configuração e importadores compartilham as mesmas famílias.
/// Uma carga anual desconhecida (importador) conflita com todos os anos da família.
pub fn save_exclusive(
    conn: &mut Connection,
    namespace: &str,
    id: &str,
    payload: &str,
) -> Result<()> {
    let requested = resources(namespace, id, payload)?;
    let tx = crate::transaction_immediate(conn)?;
    {
        let mut stmt = tx.prepare("SELECT namespace,id,payload_json FROM tarefas_importacao WHERE em_execucao=1 AND namespace IN ('config','importer')")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (ns, existing_id, json) = row?;
            let existing = resources(&ns, &existing_id, &json)?;
            if requested.0 == "*"
                || existing.0 == "*"
                || (requested.0 == existing.0
                    && (requested.1.is_none() || existing.1.is_none() || requested.1 == existing.1))
            {
                return Err(crate::StorageError::Conflict {
                    namespace: ns,
                    id: existing_id,
                });
            }
        }
    }
    save(&tx, namespace, id, true, payload)?;
    tx.commit()?;
    Ok(())
}

fn resources(namespace: &str, id: &str, payload: &str) -> Result<(String, Option<i64>)> {
    let value: serde_json::Value = serde_json::from_str(payload)
        .map_err(|e| crate::StorageError::Pool(format!("Snapshot inválido: {e}")))?;
    let source = if namespace == "config" {
        value["fonte"].as_str().unwrap_or(id)
    } else {
        id
    };
    let upper = source.to_uppercase();
    let family = if upper == "SINCRONIZAR_TUDO" || upper == "DEDUPLICACAO_MASSA" {
        "*"
    } else if upper.starts_with("TSE") {
        "TSE"
    } else if upper.starts_with("CAMARA") || upper == "CEAP" {
        "CEAP"
    } else if upper.contains("QSA") {
        "QSA"
    } else if upper.contains("OAB") {
        "OAB"
    } else {
        &upper
    };
    Ok((
        family.to_owned(),
        if namespace == "config" {
            value["ano"].as_i64()
        } else {
            None
        },
    ))
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct JobHistory {
    pub source: String,
    pub status: String,
    pub details: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Completion {
    pub namespace: String,
    pub id: String,
    pub payload: String,
    pub history: Option<JobHistory>,
}

fn completion_dir(pool: &crate::DbPool) -> Option<std::path::PathBuf> {
    pool.file_path()
        .map(|path| path.with_extension("job-completions"))
}
fn completion_path(
    pool: &crate::DbPool,
    namespace: &str,
    id: &str,
) -> Result<Option<std::path::PathBuf>> {
    if ![namespace, id].iter().all(|part| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }) {
        return Err(crate::StorageError::Pool(
            "Identificador de conclusão inválido".into(),
        ));
    }
    Ok(completion_dir(pool).map(|dir| dir.join(format!("{namespace}--{id}.json"))))
}

/// A conclusão fica consultável antes de disputar a trava de escrita do banco.
/// Em bancos em arquivo, um outbox atômico preserva o resultado entre reinícios.
pub fn queue_completion(pool: &crate::DbPool, completion: &Completion) -> Result<()> {
    use std::io::Write;
    pool.completions
        .lock()
        .map_err(|_| crate::StorageError::Pool("Trava de conclusões indisponível".into()))?
        .insert(
            (completion.namespace.clone(), completion.id.clone()),
            completion.clone(),
        );
    if let Some(path) = completion_path(pool, &completion.namespace, &completion.id)? {
        let parent = path
            .parent()
            .ok_or_else(|| crate::StorageError::Pool("Diretório de conclusão inválido".into()))?;
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer(&mut temporary, completion)
            .map_err(|e| crate::StorageError::Pool(e.to_string()))?;
        temporary.flush()?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(&path)
            .map_err(|e| crate::StorageError::Io(e.error))?;
        std::fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

pub fn load_completion(pool: &crate::DbPool, namespace: &str, id: &str) -> Result<Option<String>> {
    if let Some(completion) = pool
        .completions
        .lock()
        .map_err(|_| crate::StorageError::Pool("Trava de conclusões indisponível".into()))?
        .get(&(namespace.to_owned(), id.to_owned()))
    {
        return Ok(Some(completion.payload.clone()));
    }
    if let Some(path) = completion_path(pool, namespace, id)? {
        match std::fs::File::open(path) {
            Ok(file) => {
                let completion: Completion = serde_json::from_reader(file)
                    .map_err(|e| crate::StorageError::Pool(e.to_string()))?;
                return Ok(Some(completion.payload));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(None)
}

fn flush_completion(pool: &crate::DbPool, completion: &Completion) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = crate::transaction_immediate(&mut conn)?;
    let running: Option<bool> = tx
        .query_row(
            "SELECT em_execucao FROM tarefas_importacao WHERE namespace=?1 AND id=?2",
            [&completion.namespace, &completion.id],
            |r| r.get(0),
        )
        .optional()?;
    let applied = if completion.namespace == "importer" && running.is_some() {
        save_current(
            &tx,
            &completion.namespace,
            &completion.id,
            false,
            &completion.payload,
        )?
    } else {
        save(
            &tx,
            &completion.namespace,
            &completion.id,
            false,
            &completion.payload,
        )?;
        true
    };
    if applied && running != Some(false) {
        if let Some(history) = &completion.history {
            tx.execute("INSERT INTO historico_sincronizacao(fonte,status,detalhes,data_fim) VALUES (?1,?2,?3,CURRENT_TIMESTAMP)",
                rusqlite::params![history.source, history.status, history.details])?;
        }
    }
    tx.commit()?;
    if let Some(path) = completion_path(pool, &completion.namespace, &completion.id)? {
        match std::fs::remove_file(path) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    pool.completions
        .lock()
        .map_err(|_| crate::StorageError::Pool("Trava de conclusões indisponível".into()))?
        .remove(&(completion.namespace.clone(), completion.id.clone()));
    Ok(())
}

/// Reaplica conclusões antes de marcar workers antigos como interrompidos.
pub fn recover_completions(pool: &crate::DbPool) -> Result<usize> {
    let Some(dir) = completion_dir(pool) else {
        return Ok(0);
    };
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|v| v.to_str()) != Some("json") {
            continue;
        }
        let completion: Completion = serde_json::from_reader(std::fs::File::open(path)?)
            .map_err(|e| crate::StorageError::Pool(e.to_string()))?;
        flush_completion(pool, &completion)?;
        count += 1;
    }
    Ok(count)
}

pub async fn complete(pool: &crate::DbPool, completion: Completion) -> Result<()> {
    let queued_pool = pool.clone();
    let queued = completion.clone();
    let queued_result =
        tokio::task::spawn_blocking(move || queue_completion(&queued_pool, &queued))
            .await
            .map_err(|e| crate::StorageError::Pool(e.to_string()))?;
    if let Err(error) = queued_result {
        tracing::error!(job_id = %completion.id, error = %error, "Outbox de conclusão indisponível; tentando persistir no SQLite");
    }
    let mut delay = std::time::Duration::from_millis(100);
    loop {
        let flush_pool = pool.clone();
        let snapshot = completion.clone();
        let result = tokio::task::spawn_blocking(move || flush_completion(&flush_pool, &snapshot))
            .await
            .map_err(|e| crate::StorageError::Pool(e.to_string()))?;
        match result {
            Ok(()) => return Ok(()),
            Err(error) if retryable(&error) => {
                tracing::warn!(job_id = %completion.id, stage = "completion", error = %error, "Conclusão na fila; aguardando SQLite");
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(std::time::Duration::from_secs(5));
            }
            Err(error) => return Err(error),
        }
    }
}

fn retryable(error: &crate::StorageError) -> bool {
    match error {
        crate::StorageError::Sqlite(rusqlite::Error::SqliteFailure(code, _)) => matches!(
            code.code,
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
        ),
        crate::StorageError::Pool(message) => message.contains("Tempo de espera"),
        _ => false,
    }
}

#[cfg(test)]
mod synchronization_regressions {
    use super::*;
    fn file_pool() -> (tempfile::TempDir, crate::DbPool) {
        let directory = tempfile::tempdir().unwrap();
        let pool = crate::DbPool::open_with_size(directory.path().join("test.db"), 4).unwrap();
        crate::run_migrations(&mut pool.get().unwrap()).unwrap();
        (directory, pool)
    }
    fn completion(id: &str) -> Completion {
        Completion {
            namespace: "config".into(),
            id: id.into(),
            payload: r#"{"fonte":"CAMARA_BULK","ano":2024,"status":"CONCLUIDO"}"#.into(),
            history: Some(JobHistory {
                source: "CAMARA_BULK".into(),
                status: "CONCLUIDO".into(),
                details: "done".into(),
            }),
        }
    }
    #[test]
    fn concurrent_admission_and_cross_endpoint_conflicts() {
        let (_directory, pool) = file_pool();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let workers: Vec<_> = (0..2)
            .map(|index| {
                let pool = pool.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    save_exclusive(
                        &mut pool.get().unwrap(),
                        "config",
                        &format!("job{index}"),
                        r#"{"fonte":"CAMARA_BULK","ano":2024}"#,
                    )
                })
            })
            .collect();
        let results: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(crate::StorageError::Conflict { .. })))
                .count(),
            1
        );
        let mut conn = pool.get().unwrap();
        assert!(matches!(
            save_exclusive(
                &mut conn,
                "config",
                "all",
                r#"{"fonte":"SINCRONIZAR_TUDO"}"#
            ),
            Err(crate::StorageError::Conflict { .. })
        ));
        assert!(matches!(
            save_exclusive(&mut conn, "importer", "camara_ceap", "{}"),
            Err(crate::StorageError::Conflict { .. })
        ));
        assert!(save_exclusive(
            &mut conn,
            "config",
            "other-year",
            r#"{"fonte":"CAMARA_API","ano":2022}"#
        )
        .is_ok());
    }
    #[tokio::test]
    async fn queued_completion_is_visible_during_write_lock_and_reads_continue() {
        let (_directory, pool) = file_pool();
        save_exclusive(
            &mut pool.get().unwrap(),
            "config",
            "job",
            r#"{"fonte":"CAMARA_BULK","ano":2024,"status":"PROCESSANDO"}"#,
        )
        .unwrap();
        let mut writer = pool.get().unwrap();
        let tx = crate::transaction_immediate(&mut writer).unwrap();
        tx.execute(
            "INSERT INTO politicos(nome_completo,nome_urna) VALUES ('uncommitted','uncommitted')",
            [],
        )
        .unwrap();
        let worker_pool = pool.clone();
        let worker = tokio::spawn(async move { complete(&worker_pool, completion("job")).await });
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while load_completion(&pool, "config", "job").unwrap().is_none() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let visible = load_completion(&pool, "config", "job").unwrap().unwrap();
        assert!(visible.contains("CONCLUIDO"));
        let read = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            pool.run_blocking(|conn| {
                Ok(conn.query_row("SELECT count(*) FROM politicos", [], |r| r.get::<_, i64>(0))?)
            }),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(read, 0);
        tx.commit().unwrap();
        drop(writer);
        tokio::time::timeout(std::time::Duration::from_secs(3), worker)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let mut conn = pool.get().unwrap();
        assert!(load_completion(&pool, "config", "job").unwrap().is_none());
        assert!(save_exclusive(
            &mut conn,
            "config",
            "next",
            r#"{"fonte":"CAMARA_BULK","ano":2024}"#
        )
        .is_ok());
        let count: i64 = conn
            .query_row("SELECT count(*) FROM historico_sincronizacao", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }
    #[test]
    fn outbox_survives_restart_and_replay_does_not_duplicate_history() {
        let (directory, pool) = file_pool();
        save_exclusive(
            &mut pool.get().unwrap(),
            "config",
            "job",
            r#"{"fonte":"CAMARA_BULK","ano":2024,"status":"PROCESSANDO"}"#,
        )
        .unwrap();
        queue_completion(&pool, &completion("job")).unwrap();
        drop(pool);
        let pool = crate::DbPool::open(directory.path().join("test.db")).unwrap();
        assert_eq!(recover_completions(&pool).unwrap(), 1);
        assert_eq!(recover_interrupted(&pool.get().unwrap()).unwrap(), 0);
        queue_completion(&pool, &completion("job")).unwrap();
        assert_eq!(recover_completions(&pool).unwrap(), 1);
        let conn = pool.get().unwrap();
        assert!(load(&conn, "config", "job")
            .unwrap()
            .unwrap()
            .contains("CONCLUIDO"));
        let count: i64 = conn
            .query_row("SELECT count(*) FROM historico_sincronizacao", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }
    #[test]
    fn pending_completion_restores_missing_snapshot_after_database_restore() {
        let (_directory, pool) = file_pool();
        queue_completion(&pool, &completion("missing")).unwrap();
        assert_eq!(recover_completions(&pool).unwrap(), 1);
        let conn = pool.get().unwrap();
        assert!(load(&conn, "config", "missing")
            .unwrap()
            .unwrap()
            .contains("CONCLUIDO"));
        let count: i64 = conn
            .query_row("SELECT count(*) FROM historico_sincronizacao", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }
}
