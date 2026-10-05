use rusqlite::Connection;
use crate::error::Result;

pub struct Migration {
    pub version: i32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "create_politicos_candidaturas_bens",
        sql: "
            CREATE TABLE IF NOT EXISTS politicos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sq_candidato TEXT UNIQUE,
                cpf_mascarado TEXT,
                nome_completo TEXT NOT NULL,
                nome_urna TEXT NOT NULL,
                data_nascimento TEXT,
                grau_instrucao TEXT,
                ocupacao TEXT,
                foto_blob BLOB,
                foto_mime TEXT DEFAULT 'image/jpeg',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS candidaturas (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                politico_id INTEGER REFERENCES politicos(id),
                ano_eleicao INTEGER NOT NULL,
                cargo TEXT NOT NULL,
                numero_urna INTEGER,
                sigla_partido TEXT NOT NULL,
                uf TEXT NOT NULL,
                municipio TEXT,
                situacao_totalizacao TEXT,
                total_bens_declarados REAL DEFAULT 0.0,
                UNIQUE(politico_id, ano_eleicao, cargo)
            );

            CREATE TABLE IF NOT EXISTS bens_candidato (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                candidatura_id INTEGER REFERENCES candidaturas(id),
                tipo_bem TEXT,
                descricao TEXT,
                valor_declarado REAL NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_candidaturas_politico ON candidaturas(politico_id);
            CREATE INDEX IF NOT EXISTS idx_candidaturas_eleicao ON candidaturas(ano_eleicao, cargo, uf);
            CREATE INDEX IF NOT EXISTS idx_bens_candidatura ON bens_candidato(candidatura_id);
        ",
    },
];

pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );",
        [],
    )?;

    for migration in MIGRATIONS {
        let applied: bool = conn
            .query_row(
                "SELECT 1 FROM _migrations WHERE version = ?1",
                [migration.version],
                |_| Ok(()),
            )
            .map(|_| true)
            .unwrap_or(false);

        if !applied {
            let tx = conn.transaction()?;
            tx.execute_batch(migration.sql)?;
            tx.execute(
                "INSERT INTO _migrations (version, name) VALUES (?1, ?2)",
                (migration.version, migration.name),
            )?;
            tx.commit()?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::DbPool;

    #[test]
    fn test_migrations_politicos_candidaturas_bens() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;

        run_migrations(&mut conn)?;

        // Ensure tables exist and can store/retrieve data
        let foto_sample = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        conn.execute(
            "INSERT INTO politicos (sq_candidato, cpf_mascarado, nome_completo, nome_urna, foto_blob)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            ("12345", "***.456.789-**", "FULANO DE TAL", "FULANO", &foto_sample),
        )?;

        let politico_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, numero_urna, sigla_partido, uf, municipio)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (politico_id, 2024, "PREFEITO", 99, "PARTIDO", "SP", "SAO PAULO"),
        )?;

        let candidatura_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO bens_candidato (candidatura_id, tipo_bem, descricao, valor_declarado)
             VALUES (?1, ?2, ?3, ?4)",
            (candidatura_id, "VEICULO", "AUTOMOVEL FIAT UNO", 35000.0),
        )?;

        let count: i64 = conn.query_row("SELECT count(*) FROM bens_candidato", [], |r| r.get(0))?;
        assert_eq!(count, 1);

        // Verify idempotency
        run_migrations(&mut conn)?;

        Ok(())
    }
}
