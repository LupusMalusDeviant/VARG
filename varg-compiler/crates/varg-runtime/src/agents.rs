// Wave 22b: Agent registry — the live state behind the dashboard's agent list.
//
// Deliberately process-global rather than handle-based, unlike graph/trace/memory. A spawned
// agent has to register itself from inside its own thread, and threading a handle through every
// `spawn` site would put the bookkeeping in the user's program instead of under it.
//
// Status is maintained by the generated dispatcher: an agent is Starting while its `on_start`
// runs, Idle while it waits on its mailbox, Running while it handles a message, Error if a
// handler panicked, and Stopped once the mailbox closes.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static AGENT_COUNTER: AtomicU64 = AtomicU64::new(1);

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    Starting,
    Idle,
    Running,
    Error,
    Stopped,
}

impl AgentStatus {
    fn as_str(&self) -> &'static str {
        match self {
            AgentStatus::Starting => "starting",
            AgentStatus::Idle => "idle",
            AgentStatus::Running => "running",
            AgentStatus::Error => "error",
            AgentStatus::Stopped => "stopped",
        }
    }

    fn from_str(s: &str) -> Option<AgentStatus> {
        match s {
            "starting" => Some(AgentStatus::Starting),
            "idle" => Some(AgentStatus::Idle),
            "running" => Some(AgentStatus::Running),
            "error" => Some(AgentStatus::Error),
            "stopped" => Some(AgentStatus::Stopped),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentRecord {
    pub id: u64,
    pub name: String,
    pub status: AgentStatus,
    pub started_at: u64,
    pub updated_at: u64,
    pub handled: u64,
    pub last_message: Option<String>,
    pub last_error: Option<String>,
}

fn registry() -> &'static Mutex<Vec<AgentRecord>> {
    static REGISTRY: OnceLock<Mutex<Vec<AgentRecord>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

fn with_registry<T>(f: impl FnOnce(&mut Vec<AgentRecord>) -> T) -> T {
    let mut guard = registry().lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

/// Register an agent and return its id. Called by the generated code, once per spawn.
pub fn __varg_agent_register(name: &str) -> i64 {
    let id = AGENT_COUNTER.fetch_add(1, Ordering::SeqCst);
    let now = now_millis();
    with_registry(|rs| {
        rs.push(AgentRecord {
            id,
            name: name.to_string(),
            status: AgentStatus::Starting,
            started_at: now,
            updated_at: now,
            handled: 0,
            last_message: None,
            last_error: None,
        })
    });
    id as i64
}

/// Move an agent to a new status. An unknown status string is ignored rather than panicking:
/// this runs on the agent's own thread, where a panic would take the agent down.
pub fn __varg_agent_set_status(id: i64, status: &str) {
    let Some(next) = AgentStatus::from_str(status) else { return };
    with_registry(|rs| {
        if let Some(r) = rs.iter_mut().find(|r| r.id == id as u64) {
            r.status = next;
            r.updated_at = now_millis();
        }
    });
}

/// Record that a message was handled, moving the agent back to Idle.
pub fn __varg_agent_handled(id: i64, method: &str) {
    with_registry(|rs| {
        if let Some(r) = rs.iter_mut().find(|r| r.id == id as u64) {
            r.handled += 1;
            r.last_message = Some(method.to_string());
            r.status = AgentStatus::Idle;
            r.updated_at = now_millis();
        }
    });
}

/// Record a handler failure. The agent stays alive; the dashboard shows why it faulted.
pub fn __varg_agent_failed(id: i64, method: &str, error: &str) {
    with_registry(|rs| {
        if let Some(r) = rs.iter_mut().find(|r| r.id == id as u64) {
            r.status = AgentStatus::Error;
            r.last_message = Some(method.to_string());
            r.last_error = Some(error.to_string());
            r.updated_at = now_millis();
        }
    });
}

/// Serialise one record. Built through serde_json rather than string concatenation so quotes,
/// backslashes and control characters in an agent name or error message cannot break the payload
/// the dashboard parses.
fn record_json(r: &AgentRecord) -> serde_json::Value {
    serde_json::json!({
        "id": r.id,
        "name": r.name,
        "status": r.status.as_str(),
        "started_at": r.started_at,
        "updated_at": r.updated_at,
        "handled": r.handled,
        "last_message": r.last_message,
        "last_error": r.last_error,
    })
}

/// The whole registry as a JSON array, oldest first. This is what the dashboard reads.
pub fn __varg_agents_list() -> String {
    with_registry(|rs| {
        let items: Vec<serde_json::Value> = rs.iter().map(record_json).collect();
        serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string())
    })
}

pub fn __varg_agents_count() -> i64 {
    with_registry(|rs| rs.len() as i64)
}

/// How many agents are in a given status — the dashboard's headline numbers.
pub fn __varg_agents_count_by_status(status: &str) -> i64 {
    let Some(want) = AgentStatus::from_str(status) else { return 0 };
    with_registry(|rs| rs.iter().filter(|r| r.status == want).count() as i64)
}

/// Drop every record. Only for tests and for a long-running host that wants a clean slate.
pub fn __varg_agents_clear() {
    with_registry(|rs| rs.clear());
}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    /// The registry is process-global, so tests share it. Each one works against the agents it
    /// registers itself rather than assuming an empty registry.
    fn ids_in(json: &str) -> Vec<u64> {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        v.as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_u64().unwrap())
            .collect()
    }

    fn record_for(json: &str, id: i64) -> serde_json::Value {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        v.as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"].as_u64().unwrap() == id as u64)
            .cloned()
            .expect("registered agent must appear in the listing")
    }

    #[test]
    fn a_registered_agent_appears_with_its_starting_status() {
        let id = __varg_agent_register("worker");
        let rec = record_for(&__varg_agents_list(), id);
        assert_eq!(rec["name"], "worker");
        assert_eq!(rec["status"], "starting");
        assert_eq!(rec["handled"], 0);
        assert!(rec["last_message"].is_null());
        assert!(ids_in(&__varg_agents_list()).contains(&(id as u64)));
    }

    #[test]
    fn handling_a_message_counts_it_and_returns_to_idle() {
        let id = __varg_agent_register("counter");
        __varg_agent_set_status(id, "running");
        __varg_agent_handled(id, "process");
        __varg_agent_handled(id, "process");
        let rec = record_for(&__varg_agents_list(), id);
        assert_eq!(rec["status"], "idle");
        assert_eq!(rec["handled"], 2);
        assert_eq!(rec["last_message"], "process");
    }

    #[test]
    fn a_failed_handler_is_recorded_without_losing_the_agent() {
        let id = __varg_agent_register("fragile");
        __varg_agent_failed(id, "explode", "boom");
        let rec = record_for(&__varg_agents_list(), id);
        assert_eq!(rec["status"], "error");
        assert_eq!(rec["last_message"], "explode");
        assert_eq!(rec["last_error"], "boom");
    }

    /// An unknown status must not panic: this runs on the agent's own thread, where a panic
    /// would take the agent down over a bookkeeping mistake.
    #[test]
    fn an_unknown_status_is_ignored() {
        let id = __varg_agent_register("steady");
        __varg_agent_set_status(id, "not-a-status");
        assert_eq!(record_for(&__varg_agents_list(), id)["status"], "starting");
    }

    /// Names and errors go through serde_json, so quotes and newlines cannot break the payload.
    #[test]
    fn awkward_text_stays_parseable() {
        let id = __varg_agent_register("say \"hi\"\nnow");
        __varg_agent_failed(id, "m", "tab\there \\ backslash");
        let listing = __varg_agents_list();
        let rec = record_for(&listing, id);
        assert_eq!(rec["name"], "say \"hi\"\nnow");
        assert_eq!(rec["last_error"], "tab\there \\ backslash");
    }

    #[test]
    fn counts_by_status_track_transitions() {
        let id = __varg_agent_register("mover");
        let running_before = __varg_agents_count_by_status("running");
        __varg_agent_set_status(id, "running");
        assert_eq!(__varg_agents_count_by_status("running"), running_before + 1);
        __varg_agent_set_status(id, "stopped");
        assert_eq!(__varg_agents_count_by_status("running"), running_before);
        assert_eq!(__varg_agents_count_by_status("not-a-status"), 0);
        assert!(__varg_agents_count() >= 1);
    }
}
