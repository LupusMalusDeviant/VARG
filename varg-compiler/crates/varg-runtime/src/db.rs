// Varg Runtime: Embedded KV Database (JSON file MVP)
// TODO: Plan 10B - Upgrade to embedded SurrealDB when ready

pub fn __varg_query(query: &str) -> String {
    let db_path = ".varg_memory.json";
    let mut db: std::collections::HashMap<String, String> = std::fs::read_to_string(db_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let parts: Vec<&str> = query.split_whitespace().collect();
    if parts.is_empty() {
        return "[]".to_string();
    }

    let cmd = parts[0].to_uppercase();
    if cmd == "SET" && parts.len() >= 3 {
        let key = parts[1];
        let val = parts[2..].join(" ");
        db.insert(key.to_string(), val);
        let _ = std::fs::write(db_path, serde_json::to_string(&db).unwrap());
        return "{\"status\": \"ok\"}".to_string();
    } else if cmd == "GET" && parts.len() == 2 {
        let key = parts[1];
        if let Some(val) = db.get(key) {
            if serde_json::from_str::<serde_json::Value>(val).is_ok() {
                return val.to_string();
            } else {
                return format!("\"{}\"", val.replace("\"", "\\\""));
            }
        } else {
            return "null".to_string();
        }
    }

    serde_json::to_string(&db).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_empty() {
        let result = __varg_query("");
        assert_eq!(result, "[]");
    }
}
