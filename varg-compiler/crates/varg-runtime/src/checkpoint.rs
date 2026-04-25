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
}
