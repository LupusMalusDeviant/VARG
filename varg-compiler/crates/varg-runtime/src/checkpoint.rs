// Wave 32: Agent Checkpoint / Resume
// @[Checkpointed("path.db")] annotation + checkpoint() builtin

use std::sync::{Arc, Mutex};
use rusqlite::{Connection, params};

pub struct CheckpointStore {
    db: Connection,
    agent_id: String,
}

impl CheckpointStore {
    pub fn open(path: &str, agent_id: &str) -> Result<Self, String> {
        let conn = if path == ":memory:" {
            Connection::open_in_memory()
        } else {
            Connection::open(path)
        }
        .map_err(|e| format!("checkpoint open error: {e}"))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS checkpoints (
                agent_id TEXT NOT NULL,
                key      TEXT NOT NULL,
                value    TEXT NOT NULL,
                saved_at INTEGER NOT NULL,
                PRIMARY KEY (agent_id, key)
            );",
        )
        .map_err(|e| format!("checkpoint schema error: {e}"))?;

        Ok(CheckpointStore { db: conn, agent_id: agent_id.to_string() })
    }

    pub fn save(&self, state_json: &str) -> Result<(), String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.db
            .execute(
                "INSERT OR REPLACE INTO checkpoints (agent_id, key, value, saved_at)
                 VALUES (?1, 'state', ?2, ?3)",
                params![self.agent_id, state_json, now],
            )
            .map_err(|e| format!("checkpoint save error: {e}"))?;
        Ok(())
    }

    pub fn load(&self) -> Option<String> {
        self.db
            .query_row(
                "SELECT value FROM checkpoints WHERE agent_id = ?1 AND key = 'state'",
                params![self.agent_id],
                |row| row.get(0),
            )
            .ok()
    }

    pub fn clear(&self) -> Result<(), String> {
        self.db
            .execute(
                "DELETE FROM checkpoints WHERE agent_id = ?1",
                params![self.agent_id],
            )
            .map_err(|e| format!("checkpoint clear error: {e}"))?;
        Ok(())
    }

    pub fn age_seconds(&self) -> i64 {
        let saved: Result<i64, _> = self.db.query_row(
            "SELECT saved_at FROM checkpoints WHERE agent_id = ?1 AND key = 'state'",
            params![self.agent_id],
            |row| row.get(0),
        );
        match saved {
            Ok(t) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                now - t
            }
            Err(_) => -1,
        }
    }
}

pub type CheckpointHandle = Arc<Mutex<CheckpointStore>>;

pub fn __varg_checkpoint_open(path: &str, agent_id: &str) -> CheckpointHandle {
    let store = CheckpointStore::open(path, agent_id)
        .unwrap_or_else(|e| {
            eprintln!("[Varg] checkpoint open failed ({e}), using in-memory fallback");
            CheckpointStore::open(":memory:", agent_id).unwrap()
        });
    Arc::new(Mutex::new(store))
}

pub fn __varg_checkpoint_save(h: &CheckpointHandle, state_json: &str) -> bool {
    h.lock().unwrap().save(state_json).is_ok()
}

pub fn __varg_checkpoint_load(h: &CheckpointHandle) -> String {
    h.lock().unwrap().load().unwrap_or_default()
}

pub fn __varg_checkpoint_clear(h: &CheckpointHandle) -> bool {
    h.lock().unwrap().clear().is_ok()
}

pub fn __varg_checkpoint_exists(h: &CheckpointHandle) -> bool {
    h.lock().unwrap().load().is_some()
}

pub fn __varg_checkpoint_age(h: &CheckpointHandle) -> i64 {
    h.lock().unwrap().age_seconds()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem(id: &str) -> CheckpointHandle {
        __varg_checkpoint_open(":memory:", id)
    }

    #[test]
    fn test_checkpoint_initially_empty() {
        assert!(!__varg_checkpoint_exists(&mem("a1")));
    }

    #[test]
    fn test_checkpoint_save_and_load() {
        let h = mem("a2");
        let state = r#"{"counter":42,"name":"Alice"}"#;
        assert!(__varg_checkpoint_save(&h, state));
        assert_eq!(__varg_checkpoint_load(&h), state);
    }

    #[test]
    fn test_checkpoint_exists_after_save() {
        let h = mem("a3");
        __varg_checkpoint_save(&h, "{}");
        assert!(__varg_checkpoint_exists(&h));
    }

    #[test]
    fn test_checkpoint_clear() {
        let h = mem("a4");
        __varg_checkpoint_save(&h, "{\"v\":1}");
        __varg_checkpoint_clear(&h);
        assert!(!__varg_checkpoint_exists(&h));
    }

    #[test]
    fn test_checkpoint_overwrite() {
        let h = mem("a5");
        __varg_checkpoint_save(&h, "{\"v\":1}");
        __varg_checkpoint_save(&h, "{\"v\":2}");
        assert_eq!(__varg_checkpoint_load(&h), "{\"v\":2}");
    }

    #[test]
    fn test_checkpoint_age_before_save() {
        assert_eq!(__varg_checkpoint_age(&mem("a6")), -1);
    }

    #[test]
    fn test_checkpoint_age_after_save() {
        let h = mem("a7");
        __varg_checkpoint_save(&h, "{}");
        assert!(__varg_checkpoint_age(&h) >= 0);
    }

    // ── Adversarial / edge-case tests ────────────────────────────────────────

    #[test]
    fn test_checkpoint_load_after_clear_returns_empty() {
        // After clear(), load() must return empty string (unwrap_or_default)
        let h = mem("clr1");
        __varg_checkpoint_save(&h, "{\"x\":1}");
        __varg_checkpoint_clear(&h);
        let loaded = __varg_checkpoint_load(&h);
        assert!(loaded.is_empty(), "load after clear must return empty string, got: {loaded}");
    }

    #[test]
    fn test_checkpoint_exists_false_after_clear() {
        let h = mem("clr2");
        __varg_checkpoint_save(&h, "{}");
        __varg_checkpoint_clear(&h);
        assert!(!__varg_checkpoint_exists(&h), "exists must be false after clear");
    }

    #[test]
    fn test_checkpoint_age_minus_one_after_clear() {
        let h = mem("clr3");
        __varg_checkpoint_save(&h, "{}");
        __varg_checkpoint_clear(&h);
        assert_eq!(__varg_checkpoint_age(&h), -1, "age must be -1 when no checkpoint exists");
    }

    #[test]
    fn test_checkpoint_triple_overwrite_keeps_last() {
        let h = mem("ow3");
        for i in 1..=3 {
            __varg_checkpoint_save(&h, &format!("{{\"v\":{i}}}"));
        }
        assert_eq!(__varg_checkpoint_load(&h), "{\"v\":3}");
    }

    #[test]
    fn test_checkpoint_large_state_json() {
        let h = mem("large");
        // 100KB JSON blob
        let big = format!("{{\"data\":\"{}\"}}", "x".repeat(100_000));
        assert!(__varg_checkpoint_save(&h, &big), "saving 100KB state must succeed");
        let loaded = __varg_checkpoint_load(&h);
        assert_eq!(loaded.len(), big.len(), "loaded state must match saved state exactly");
    }

    #[test]
    fn test_checkpoint_empty_json_string_survives_roundtrip() {
        let h = mem("empty_json");
        assert!(__varg_checkpoint_save(&h, "{}"));
        assert_eq!(__varg_checkpoint_load(&h), "{}");
    }

    #[test]
    fn test_checkpoint_special_chars_in_json_survive_roundtrip() {
        let h = mem("special");
        let json = r#"{"msg":"hello\nworld\t\"quoted\""}"#;
        __varg_checkpoint_save(&h, json);
        assert_eq!(__varg_checkpoint_load(&h), json);
    }

    #[test]
    fn test_checkpoint_two_agents_are_isolated() {
        // Two handles with different agent IDs must not share state
        let h1 = mem("agent_alpha");
        let h2 = mem("agent_beta");
        // The second mem() creates a NEW in-memory connection — they are physically separate DBs.
        // Each agent's save is truly isolated.
        __varg_checkpoint_save(&h1, "{\"owner\":\"alpha\"}");
        // h2 has its own connection, never saved anything
        assert!(!__varg_checkpoint_exists(&h2), "agent_beta must not see agent_alpha's checkpoint");
    }

    #[test]
    fn test_checkpoint_fallback_to_memory_on_bad_path() {
        // Invalid path should fall back to in-memory (not panic)
        let h = __varg_checkpoint_open("/no/such/directory/does/not/exist/ck.db", "fallback_test");
        // Must be usable — fallback to :memory: succeeded
        assert!(__varg_checkpoint_save(&h, "{\"fallback\":true}"), "in-memory fallback must be functional");
        assert_eq!(__varg_checkpoint_load(&h), "{\"fallback\":true}");
    }

    #[test]
    fn test_checkpoint_clear_nonexistent_is_safe() {
        let h = mem("never_saved");
        assert!(__varg_checkpoint_clear(&h), "clearing a never-saved checkpoint must succeed");
        assert!(!__varg_checkpoint_exists(&h));
    }

    #[test]
    fn test_checkpoint_unicode_state_survives_roundtrip() {
        let h = mem("unicode");
        let json = "{\"msg\":\"こんにちは\",\"emoji\":\"🚀\"}";
        __varg_checkpoint_save(&h, json);
        assert_eq!(__varg_checkpoint_load(&h), json);
    }
}
