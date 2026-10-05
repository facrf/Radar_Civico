use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use rusqlite::{Connection, OpenFlags};

use crate::error::{Result, StorageError};

static MEM_DB_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn apply_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA cache_size = -64000;
         PRAGMA temp_store = MEMORY;
         PRAGMA mmap_size = 30000000000;
         PRAGMA foreign_keys = ON;"
    )?;
    Ok(())
}

#[derive(Clone)]
enum DbTarget {
    File(PathBuf),
    SharedMemory(String),
}

#[derive(Clone)]
pub struct DbPool {
    target: DbTarget,
    pool: Arc<Mutex<Vec<Connection>>>,
    max_size: usize,
    // Keep at least one connection alive for shared-memory databases so it doesn't get dropped
    _anchor: Option<Arc<Mutex<Connection>>>,
}

pub struct PooledConnection {
    pool: DbPool,
    conn: Option<Connection>,
}

impl Deref for PooledConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        self.conn.as_ref().expect("Connection is always present until drop")
    }
}

impl DerefMut for PooledConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.conn.as_mut().expect("Connection is always present until drop")
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            if let Ok(mut pool) = self.pool.pool.lock() {
                if pool.len() < self.pool.max_size {
                    pool.push(conn);
                }
            }
        }
    }
}

impl DbPool {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::open_with_size(path, 16)
    }

    pub fn open_with_size<P: AsRef<Path>>(path: P, max_size: usize) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        if let Some(parent) = path_buf.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    StorageError::Pool(format!("Failed to create database directory: {e}"))
                })?;
            }
        }

        let initial_conn = Connection::open(&path_buf)?;
        apply_pragmas(&initial_conn)?;

        let pool = Arc::new(Mutex::new(vec![initial_conn]));
        Ok(Self {
            target: DbTarget::File(path_buf),
            pool,
            max_size,
            _anchor: None,
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::open_in_memory_with_size(16)
    }

    pub fn open_in_memory_with_size(max_size: usize) -> Result<Self> {
        let counter = MEM_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let uri = format!("file:memdb_{counter}?mode=memory&cache=shared");

        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_URI;

        let anchor_conn = Connection::open_with_flags(&uri, flags)?;
        apply_pragmas(&anchor_conn)?;

        let first_conn = Connection::open_with_flags(&uri, flags)?;
        apply_pragmas(&first_conn)?;

        let pool = Arc::new(Mutex::new(vec![first_conn]));
        Ok(Self {
            target: DbTarget::SharedMemory(uri),
            pool,
            max_size,
            _anchor: Some(Arc::new(Mutex::new(anchor_conn))),
        })
    }

    pub fn get(&self) -> Result<PooledConnection> {
        let mut pool_guard = self.pool.lock().map_err(|_| {
            StorageError::Pool("Poisoned connection pool lock".to_string())
        })?;

        let conn = if let Some(c) = pool_guard.pop() {
            c
        } else {
            self.create_connection()?
        };

        Ok(PooledConnection {
            pool: self.clone(),
            conn: Some(conn),
        })
    }

    fn create_connection(&self) -> Result<Connection> {
        let conn = match &self.target {
            DbTarget::File(path) => Connection::open(path)?,
            DbTarget::SharedMemory(uri) => {
                let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_URI;
                Connection::open_with_flags(uri, flags)?
            }
        };
        apply_pragmas(&conn)?;
        Ok(conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_pool_pragmas_and_shared_state() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let conn1 = pool.get()?;

        // Verify foreign_keys = ON
        let fk: i32 = conn1.query_row("PRAGMA foreign_keys;", [], |r| r.get(0))?;
        assert_eq!(fk, 1);

        // Verify synchronous = NORMAL (1)
        let sync: i32 = conn1.query_row("PRAGMA synchronous;", [], |r| r.get(0))?;
        assert_eq!(sync, 1);

        // Verify cache_size = -64000
        let cache_size: i32 = conn1.query_row("PRAGMA cache_size;", [], |r| r.get(0))?;
        assert_eq!(cache_size, -64000);

        // Verify temp_store = MEMORY (2)
        let temp_store: i32 = conn1.query_row("PRAGMA temp_store;", [], |r| r.get(0))?;
        assert_eq!(temp_store, 2);

        // Create table in conn1
        conn1.execute("CREATE TABLE teste (id INTEGER PRIMARY KEY, valor TEXT);", [])?;
        conn1.execute("INSERT INTO teste (id, valor) VALUES (1, 'radar');", [])?;
        drop(conn1);

        // Read from conn2 and ensure shared in-memory state
        let conn2 = pool.get()?;
        let valor: String = conn2.query_row("SELECT valor FROM teste WHERE id = 1;", [], |r| r.get(0))?;
        assert_eq!(valor, "radar");

        Ok(())
    }

    #[test]
    fn test_file_pool_pragmas_and_wal() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("test_radar_{}.db", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));

        let pool = DbPool::open(&db_path)?;
        let conn = pool.get()?;

        let journal: String = conn.query_row("PRAGMA journal_mode;", [], |r| r.get(0))?;
        assert_eq!(journal.to_lowercase(), "wal");

        let sync: i32 = conn.query_row("PRAGMA synchronous;", [], |r| r.get(0))?;
        assert_eq!(sync, 1);

        let cache_size: i32 = conn.query_row("PRAGMA cache_size;", [], |r| r.get(0))?;
        assert_eq!(cache_size, -64000);

        let temp_store: i32 = conn.query_row("PRAGMA temp_store;", [], |r| r.get(0))?;
        assert_eq!(temp_store, 2);

        let mmap_size: i64 = conn.query_row("PRAGMA mmap_size;", [], |r| r.get(0))?;
        assert!(mmap_size > 0);

        drop(conn);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
