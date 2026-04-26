// F42: Varg Runtime — SQLite Database Driver
//
// Provides SQLite builtins for compiled Varg programs.
// Uses rusqlite with bundled SQLite.

use std::collections::HashMap;
use rusqlite::Connection;

/// SQLite database connection wrapper
pub struct VargDbConnection {
    pub path: String,
    pub conn: Connection,
}

/// Open a SQLite database connection
pub fn __varg_db_open(path: &str) -> VargDbConnection {
    let conn = if path == ":memory:" {
        Connection::open_in_memory().expect("Varg runtime error: db_open() failed — could not create an in-memory SQLite database (out of memory?)")
    } else {
        Connection::open(path).unwrap_or_else(|e| panic!("Varg runtime error: db_open() failed — could not open database file '{}': {} (check the path exists and you have read/write permissions)", path, e))
    };
    VargDbConnection {
        path: path.to_string(),
        conn,
    }
}

/// Execute a SQL statement (INSERT, UPDATE, DELETE, CREATE, etc.)
/// Returns number of affected rows.
pub fn __varg_db_execute(db: &VargDbConnection, sql: &str, params: &[String]) -> Result<i64, String> {
    if sql.is_empty() {
        return Err("Empty SQL statement".to_string());
    }
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter()
        .map(|s| s as &dyn rusqlite::types::ToSql)
        .collect();
    db.conn.execute(sql, param_refs.as_slice())
        .map(|rows| rows as i64)
        .map_err(|e| e.to_string())
}

/// Query the database, returning rows as List<Map<string, string>>
pub fn __varg_db_query(db: &VargDbConnection, sql: &str, params: &[String]) -> Result<Vec<HashMap<String, String>>, String> {
    if sql.is_empty() {
        return Err("Empty SQL query".to_string());
    }
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter()
        .map(|s| s as &dyn rusqlite::types::ToSql)
        .collect();
    let mut stmt = db.conn.prepare(sql).map_err(|e| e.to_string())?;
    let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        let mut map = HashMap::new();
        for (i, name) in col_names.iter().enumerate() {
            // Try string first, then int, then float, then null
            let val: String = row.get::<_, String>(i)
                .or_else(|_| row.get::<_, i64>(i).map(|v| v.to_string()))
                .or_else(|_| row.get::<_, f64>(i).map(|v| v.to_string()))
                .unwrap_or_else(|_| "null".to_string());
            map.insert(name.clone(), val);
        }
        Ok(map)
    }).map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_open_memory() {
        let conn = __varg_db_open(":memory:");
        assert_eq!(conn.path, ":memory:");
    }

    #[test]
    fn test_db_execute_empty_sql() {
        let conn = __varg_db_open(":memory:");
        let result = __varg_db_execute(&conn, "", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_db_query_empty_sql() {
        let conn = __varg_db_open(":memory:");
        let result = __varg_db_query(&conn, "", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_db_create_and_insert() {
        let conn = __varg_db_open(":memory:");
        let r1 = __varg_db_execute(&conn, "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)", &[]);
        assert!(r1.is_ok());

        let r2 = __varg_db_execute(&conn, "INSERT INTO users (name, age) VALUES (?1, ?2)", &["Alice".to_string(), "30".to_string()]);
        assert_eq!(r2.unwrap(), 1);

        let r3 = __varg_db_execute(&conn, "INSERT INTO users (name, age) VALUES (?1, ?2)", &["Bob".to_string(), "25".to_string()]);
        assert_eq!(r3.unwrap(), 1);
    }

    #[test]
    fn test_db_query_returns_rows() {
        let conn = __varg_db_open(":memory:");
        __varg_db_execute(&conn, "CREATE TABLE items (id INTEGER PRIMARY KEY, label TEXT)", &[]).unwrap();
        __varg_db_execute(&conn, "INSERT INTO items (label) VALUES (?1)", &["hello".to_string()]).unwrap();
        __varg_db_execute(&conn, "INSERT INTO items (label) VALUES (?1)", &["world".to_string()]).unwrap();

        let rows = __varg_db_query(&conn, "SELECT id, label FROM items ORDER BY id", &[]).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get("label").unwrap(), "hello");
        assert_eq!(rows[1].get("label").unwrap(), "world");
    }

    #[test]
    fn test_db_query_with_params() {
        let conn = __varg_db_open(":memory:");
        __varg_db_execute(&conn, "CREATE TABLE kv (key TEXT, value TEXT)", &[]).unwrap();
        __varg_db_execute(&conn, "INSERT INTO kv VALUES (?1, ?2)", &["a".to_string(), "1".to_string()]).unwrap();
        __varg_db_execute(&conn, "INSERT INTO kv VALUES (?1, ?2)", &["b".to_string(), "2".to_string()]).unwrap();

        let rows = __varg_db_query(&conn, "SELECT * FROM kv WHERE key = ?1", &["b".to_string()]).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("value").unwrap(), "2");
    }
}
