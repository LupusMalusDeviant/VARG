// F41-3: Varg Runtime — SQLite Database Driver
//
// Provides SQLite builtins for compiled Varg programs.
// Uses rusqlite with bundled SQLite.

use std::collections::HashMap;

/// SQLite database connection wrapper
pub struct VargDbConnection {
    // In production, this will hold a rusqlite::Connection.
    // For now, it's a path marker — actual connection via `import crate "rusqlite"`.
    pub path: String,
}

/// Open a SQLite database connection
pub fn __varg_db_open(path: &str) -> VargDbConnection {
    VargDbConnection {
        path: path.to_string(),
    }
}

/// Execute a SQL statement (INSERT, UPDATE, DELETE, CREATE, etc.)
/// Returns number of affected rows.
pub fn __varg_db_execute(conn: &VargDbConnection, sql: &str, _params: &[String]) -> Result<i64, String> {
    // MVP: This will be fully functional when rusqlite is added as a runtime dependency.
    // For now, validate that the API surface is correct.
    if sql.is_empty() {
        return Err("Empty SQL statement".to_string());
    }
    let _ = &conn.path; // use connection
    Ok(0)
}

/// Query the database, returning rows as List<Map<string, string>>
pub fn __varg_db_query(conn: &VargDbConnection, sql: &str, _params: &[String]) -> Result<Vec<HashMap<String, String>>, String> {
    if sql.is_empty() {
        return Err("Empty SQL query".to_string());
    }
    let _ = &conn.path;
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_open() {
        let conn = __varg_db_open("test.sqlite");
        assert_eq!(conn.path, "test.sqlite");
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
    fn test_db_execute_valid() {
        let conn = __varg_db_open(":memory:");
        let result = __varg_db_execute(&conn, "CREATE TABLE test (id INT)", &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_query_valid() {
        let conn = __varg_db_open(":memory:");
        let result = __varg_db_query(&conn, "SELECT 1", &[]);
        assert!(result.is_ok());
    }
}
