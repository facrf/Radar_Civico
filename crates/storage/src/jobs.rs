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
    Ok(conn
        .query_row(
            "SELECT payload_json FROM tarefas_importacao WHERE namespace=?1 AND id=?2",
            [namespace, id],
            |r| r.get(0),
        )
        .optional()?)
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
