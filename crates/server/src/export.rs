//! Exportação em disco temporário: memória limitada a uma linha por vez.
use std::io::Write;
use storage::{
    rusqlite::{types::Value, Connection},
    Result,
};

pub fn table_file(conn: &Connection, table: &str, format: &str) -> Result<tempfile::NamedTempFile> {
    let tx = conn.unchecked_transaction()?;
    let mut statement = tx.prepare(&format!("SELECT * FROM {table}"))?;
    let names = statement
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut file = tempfile::NamedTempFile::new()?;
    let mut rows = statement.query([])?;
    if format == "json" {
        file.write_all(b"[")?;
        let mut first = true;
        while let Some(row) = rows.next()? {
            let mut object = serde_json::Map::new();
            for (i, name) in names.iter().enumerate() {
                let value = match row.get::<_, Value>(i)? {
                    Value::Null => serde_json::Value::Null,
                    Value::Integer(v) => serde_json::json!(v),
                    Value::Real(v) => serde_json::json!(v),
                    Value::Text(v) => serde_json::json!(v),
                    Value::Blob(v) => serde_json::json!(format!("[BLOB {} bytes]", v.len())),
                };
                object.insert(name.clone(), value);
            }
            if !first {
                file.write_all(b",")?;
            }
            first = false;
            serde_json::to_writer(&mut file, &object)
                .map_err(|e| storage::StorageError::Pool(e.to_string()))?;
        }
        file.write_all(b"]")?;
    } else {
        let mut writer = csv::WriterBuilder::new()
            .delimiter(b';')
            .from_writer(&mut file);
        writer
            .write_record(&names)
            .map_err(|e| storage::StorageError::Pool(e.to_string()))?;
        while let Some(row) = rows.next()? {
            let mut values = Vec::with_capacity(names.len());
            for i in 0..names.len() {
                values.push(match row.get::<_, Value>(i)? {
                    Value::Null => String::new(),
                    Value::Integer(v) => v.to_string(),
                    Value::Real(v) => v.to_string(),
                    Value::Text(v) => v,
                    Value::Blob(v) => format!("[BLOB {} bytes]", v.len()),
                });
            }
            writer
                .write_record(values)
                .map_err(|e| storage::StorageError::Pool(e.to_string()))?;
        }
        writer.flush()?;
    }
    file.flush()?;
    drop(rows);
    drop(statement);
    tx.commit()?;
    Ok(file)
}
