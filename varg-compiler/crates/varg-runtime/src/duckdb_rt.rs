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

pub fn __varg_duckdb_open(path: &str) -> DuckDbHandle {
    let conn = if path == ":memory:" {
        Connection::open_in_memory().expect("Varg runtime error: duckdb_open() failed — could not create an in-memory DuckDB database (out of memory?)")
    } else {
        Connection::open(path).expect("Varg runtime error: duckdb_open() failed — could not open or create the database file (check the path and that you have read/write permissions)")
    };
    Arc::new(Mutex::new(VargDuckDb { conn }))
}

pub fn __varg_duckdb_execute(db: &DuckDbHandle, sql: &str, params: &[String]) {
    let inner = db.lock().unwrap();
    let mut stmt = inner.conn.prepare(sql)
        .expect("Varg runtime error: duckdb_execute() failed — the SQL statement could not be prepared (check for syntax errors in your SQL)");
    stmt.execute(params_from_iter(params.iter().map(|s| s.as_str())))
        .expect("Varg runtime error: duckdb_execute() failed — the SQL statement could not be executed (check parameter count and types match the query placeholders)");
}

pub fn __varg_duckdb_query(db: &DuckDbHandle, sql: &str, params: &[String]) -> Vec<Vec<String>> {
    let inner = db.lock().unwrap();
    let mut stmt = inner.conn.prepare(sql)
        .expect("Varg runtime error: duckdb_query() failed — the SQL query could not be prepared (check for syntax errors in your SQL)");
    let col_count = stmt.column_count();
    let mut rows_out: Vec<Vec<String>> = Vec::new();
    let mut rows = stmt.query(params_from_iter(params.iter().map(|s| s.as_str())))
        .expect("Varg runtime error: duckdb_query() failed — the query could not be executed (check parameter count and types match the query placeholders)");
    while let Some(row) = rows.next().expect("Varg runtime error: duckdb_query() failed — an error occurred while reading query results (the database may have been modified concurrently)") {
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
    rows_out
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
        let db = __varg_duckdb_open(":memory:");
        assert!(db.lock().is_ok());
    }

    #[test]
    fn test_duckdb_execute_create_insert() {
        let db = __varg_duckdb_open(":memory:");
        __varg_duckdb_execute(&db, "CREATE TABLE t (id INTEGER, name VARCHAR)", &[]);
        __varg_duckdb_execute(&db, "INSERT INTO t VALUES (1, 'Alice')", &[]);
        __varg_duckdb_execute(&db, "INSERT INTO t VALUES (2, 'Bob')", &[]);
        let rows = __varg_duckdb_query(&db, "SELECT COUNT(*) FROM t", &[]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], "2");
    }

    #[test]
    fn test_duckdb_query_returns_rows() {
        let db = __varg_duckdb_open(":memory:");
        __varg_duckdb_execute(&db, "CREATE TABLE people (name VARCHAR, age INTEGER)", &[]);
        __varg_duckdb_execute(&db, "INSERT INTO people VALUES ('Alice', 30)", &[]);
        __varg_duckdb_execute(&db, "INSERT INTO people VALUES ('Bob', 25)", &[]);
        let rows = __varg_duckdb_query(&db, "SELECT name, age FROM people ORDER BY age", &[]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], "Bob");
        assert_eq!(rows[0][1], "25");
        assert_eq!(rows[1][0], "Alice");
        assert_eq!(rows[1][1], "30");
    }

    #[test]
    fn test_duckdb_query_with_params() {
        let db = __varg_duckdb_open(":memory:");
        __varg_duckdb_execute(&db, "CREATE TABLE scores (name VARCHAR, score DOUBLE)", &[]);
        __varg_duckdb_execute(&db, "INSERT INTO scores VALUES ('A', 90.0)", &[]);
        __varg_duckdb_execute(&db, "INSERT INTO scores VALUES ('B', 70.0)", &[]);
        let rows = __varg_duckdb_query(&db, "SELECT name FROM scores WHERE score > $1", &["80".to_string()]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], "A");
    }

    #[test]
    fn test_duckdb_close_noop() {
        let db = __varg_duckdb_open(":memory:");
        __varg_duckdb_close(&db); // should not panic
    }

    #[test]
    fn test_duckdb_aggregation() {
        let db = __varg_duckdb_open(":memory:");
        __varg_duckdb_execute(&db, "CREATE TABLE sales (city VARCHAR, amount DOUBLE)", &[]);
        __varg_duckdb_execute(&db, "INSERT INTO sales VALUES ('Berlin', 100.0)", &[]);
        __varg_duckdb_execute(&db, "INSERT INTO sales VALUES ('Berlin', 200.0)", &[]);
        __varg_duckdb_execute(&db, "INSERT INTO sales VALUES ('Munich', 150.0)", &[]);
        let rows = __varg_duckdb_query(&db,
            "SELECT city, SUM(amount) as total FROM sales GROUP BY city ORDER BY city", &[]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], "Berlin");
        assert_eq!(rows[0][1], "300");
    }
}
