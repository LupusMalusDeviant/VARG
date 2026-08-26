// Wave 40: DuckDB analytical SQL builtins
//
// DuckDB can query Parquet/CSV files natively via SQL:
//   SELECT city, AVG(score) FROM 'data.parquet' GROUP BY city
//
// Return type: Vec<Vec<String>> — rows as ordered string columns.
// OCAP: all operations except duckdb_close require DbAccess token.

use duckdb::{Connection, params_from_iter};
use std::sync::{Arc, Mutex};

pub struct VargDuckDb {
    conn: Connection,
}

pub type DuckDbHandle = Arc<Mutex<VargDuckDb>>;

/// Open a database, or say why it could not be opened.
///
/// These used to `expect`, so a path that could not be opened and a query with a typo in it both
/// took the whole program down — the most ordinary mistake anyone makes with SQL was
/// unrecoverable, and the message came out as a Rust panic rather than something the program
/// could report or retry.
pub fn __varg_duckdb_open(path: &str) -> Result<DuckDbHandle, String> {
    let conn = if path == ":memory:" {
        Connection::open_in_memory()
            .map_err(|e| format!("duckdb_open: could not create an in-memory database: {}", e))?
    } else {
        Connection::open(path)
            .map_err(|e| format!("duckdb_open: could not open `{}`: {}", path, e))?
    };
    Ok(Arc::new(Mutex::new(VargDuckDb { conn })))
}

pub fn __varg_duckdb_execute(
    db: &DuckDbHandle,
    sql: &str,
    params: &[String],
) -> Result<i64, String> {
    let inner = db.lock().unwrap_or_else(|e| e.into_inner());
    let mut stmt = inner
        .conn
        .prepare(sql)
        .map_err(|e| format!("duckdb_execute: {}", e))?;
    let affected = stmt
        .execute(params_from_iter(params.iter().map(|s| s.as_str())))
        .map_err(|e| format!("duckdb_execute: {}", e))?;
    Ok(affected as i64)
}

pub fn __varg_duckdb_query(
    db: &DuckDbHandle,
    sql: &str,
    params: &[String],
) -> Result<Vec<Vec<String>>, String> {
    let inner = db.lock().unwrap_or_else(|e| e.into_inner());
    let mut stmt = inner
        .conn
        .prepare(sql)
        .map_err(|e| format!("duckdb_query: {}", e))?;
    let mut rows_out: Vec<Vec<String>> = Vec::new();
    let mut rows = stmt
        .query(params_from_iter(params.iter().map(|s| s.as_str())))
        .map_err(|e| format!("duckdb_query: {}", e))?;
    while let Some(row) = rows.next().map_err(|e| format!("duckdb_query: {}", e))? {
        // duckdb's `column_count()` panics unless the statement has been executed, so read it
        // from the (now executed) statement via the row, not from `stmt` before `query()`.
        let col_count = row.as_ref().column_count();
        let mut row_vec = Vec::with_capacity(col_count);
        for i in 0..col_count {
            let val: duckdb::types::Value = row.get(i).unwrap_or(duckdb::types::Value::Null);
            row_vec.push(match val {
                duckdb::types::Value::Null        => "null".to_string(),
                duckdb::types::Value::Boolean(b)  => b.to_string(),
                duckdb::types::Value::TinyInt(n)  => n.to_string(),
                duckdb::types::Value::SmallInt(n) => n.to_string(),
                duckdb::types::Value::Int(n)      => n.to_string(),
                duckdb::types::Value::BigInt(n)   => n.to_string(),
                duckdb::types::Value::Float(f)    => f.to_string(),
                duckdb::types::Value::Double(f)   => f.to_string(),
                duckdb::types::Value::Text(s)     => s,
                other                             => format!("{:?}", other),
            });
        }
        rows_out.push(row_vec);
    }
    Ok(rows_out)
}

pub fn __varg_duckdb_close(_db: &DuckDbHandle) {
    // Arc handles cleanup on last reference drop.
    // This builtin is exposed for Varg semantic clarity.
}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duckdb_open_memory() {
        let db = __varg_duckdb_open(":memory:").unwrap();
        assert!(db.lock().is_ok());
    }

    #[test]
    fn test_duckdb_execute_create_insert() {
        let db = __varg_duckdb_open(":memory:").unwrap();
        __varg_duckdb_execute(&db, "CREATE TABLE t (id INTEGER, name VARCHAR)", &[]).unwrap();
        __varg_duckdb_execute(&db, "INSERT INTO t VALUES (1, 'Alice')", &[]).unwrap();
        __varg_duckdb_execute(&db, "INSERT INTO t VALUES (2, 'Bob')", &[]).unwrap();
        let rows = __varg_duckdb_query(&db, "SELECT COUNT(*) FROM t", &[]).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], "2");
    }

    #[test]
    fn test_duckdb_query_returns_rows() {
        let db = __varg_duckdb_open(":memory:").unwrap();
        __varg_duckdb_execute(&db, "CREATE TABLE people (name VARCHAR, age INTEGER)", &[]).unwrap();
        __varg_duckdb_execute(&db, "INSERT INTO people VALUES ('Alice', 30)", &[]).unwrap();
        __varg_duckdb_execute(&db, "INSERT INTO people VALUES ('Bob', 25)", &[]).unwrap();
        let rows = __varg_duckdb_query(&db, "SELECT name, age FROM people ORDER BY age", &[]).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], "Bob");
        assert_eq!(rows[0][1], "25");
        assert_eq!(rows[1][0], "Alice");
        assert_eq!(rows[1][1], "30");
    }

    #[test]
    fn test_duckdb_query_with_params() {
        let db = __varg_duckdb_open(":memory:").unwrap();
        __varg_duckdb_execute(&db, "CREATE TABLE scores (name VARCHAR, score DOUBLE)", &[]).unwrap();
        __varg_duckdb_execute(&db, "INSERT INTO scores VALUES ('A', 90.0)", &[]).unwrap();
        __varg_duckdb_execute(&db, "INSERT INTO scores VALUES ('B', 70.0)", &[]).unwrap();
        let rows = __varg_duckdb_query(&db, "SELECT name FROM scores WHERE score > $1", &["80".to_string()]).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], "A");
    }

    #[test]
    fn test_duckdb_close_noop() {
        let db = __varg_duckdb_open(":memory:").unwrap();
        __varg_duckdb_close(&db); // should not panic
    }

    #[test]
    fn test_duckdb_aggregation() {
        let db = __varg_duckdb_open(":memory:").unwrap();
        __varg_duckdb_execute(&db, "CREATE TABLE sales (city VARCHAR, amount DOUBLE)", &[]).unwrap();
        __varg_duckdb_execute(&db, "INSERT INTO sales VALUES ('Berlin', 100.0)", &[]).unwrap();
        __varg_duckdb_execute(&db, "INSERT INTO sales VALUES ('Berlin', 200.0)", &[]).unwrap();
        __varg_duckdb_execute(&db, "INSERT INTO sales VALUES ('Munich', 150.0)", &[]).unwrap();
        let rows = __varg_duckdb_query(&db,
            "SELECT city, SUM(amount) as total FROM sales GROUP BY city ORDER BY city", &[]).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], "Berlin");
        assert_eq!(rows[0][1], "300");
    }
}
