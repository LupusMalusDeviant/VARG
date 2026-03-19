// Wave 26: Self-Improving Agent Loop
//
// Feedback loop that stores successes/failures and retrieves
// past solutions for similar tasks. Builds on memory + vector store.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::memory::{MemoryHandle, __varg_memory_open, __varg_memory_store, __varg_memory_recall};

/// A learning record
#[derive(Debug, Clone)]
pub struct LearningRecord {
    pub task: String,
    pub result: LearningOutcome,
    pub content: String,
    pub iteration: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LearningOutcome {
    Success,
    Failure(String),
}

/// Self-improving agent state
#[derive(Debug)]
pub struct SelfImprover {
    pub name: String,
    pub memory: MemoryHandle,
    pub iterations: u64,
    pub successes: u64,
    pub failures: u64,
    pub max_retries: u64,
}

pub type SelfImproverHandle = Arc<Mutex<SelfImprover>>;

/// Create a new self-improving agent wrapper
pub fn __varg_self_improver_new(name: &str, max_retries: i64) -> SelfImproverHandle {
    Arc::new(Mutex::new(SelfImprover {
        name: name.to_string(),
        memory: __varg_memory_open(&format!("{}_learnings", name)),
        iterations: 0,
        successes: 0,
        failures: 0,
        max_retries: max_retries as u64,
    }))
}

/// Record a successful attempt
pub fn __varg_self_improver_record_success(
    improver: &SelfImproverHandle,
    task: &str,
    solution: &str,
) {
    let mut si = improver.lock().unwrap();
    si.iterations += 1;
    si.successes += 1;
    let meta = HashMap::from([
        ("outcome".to_string(), "success".to_string()),
        ("iteration".to_string(), si.iterations.to_string()),
    ]);
    __varg_memory_store(&si.memory, &format!("SUCCESS [{}]: {}", task, solution), &meta);
}

/// Record a failed attempt
pub fn __varg_self_improver_record_failure(
    improver: &SelfImproverHandle,
    task: &str,
    error: &str,
) {
    let mut si = improver.lock().unwrap();
    si.iterations += 1;
    si.failures += 1;
    let meta = HashMap::from([
        ("outcome".to_string(), "failure".to_string()),
        ("iteration".to_string(), si.iterations.to_string()),
    ]);
    __varg_memory_store(&si.memory, &format!("FAILURE [{}]: {}", task, error), &meta);
}

/// Recall past learnings for a similar task
pub fn __varg_self_improver_recall(
    improver: &SelfImproverHandle,
    task: &str,
    top_k: i64,
) -> Vec<HashMap<String, String>> {
    let si = improver.lock().unwrap();
    __varg_memory_recall(&si.memory, task, top_k)
}

/// Get success rate as percentage
pub fn __varg_self_improver_success_rate(improver: &SelfImproverHandle) -> i64 {
    let si = improver.lock().unwrap();
    if si.iterations == 0 { return 0; }
    ((si.successes as f64 / si.iterations as f64) * 100.0) as i64
}

/// Get iteration count
pub fn __varg_self_improver_iterations(improver: &SelfImproverHandle) -> i64 {
    improver.lock().unwrap().iterations as i64
}

/// Get stats as a map
pub fn __varg_self_improver_stats(improver: &SelfImproverHandle) -> HashMap<String, String> {
    let si = improver.lock().unwrap();
    HashMap::from([
        ("name".to_string(), si.name.clone()),
        ("iterations".to_string(), si.iterations.to_string()),
        ("successes".to_string(), si.successes.to_string()),
        ("failures".to_string(), si.failures.to_string()),
        ("success_rate".to_string(), format!("{}%",
            if si.iterations == 0 { 0 } else { (si.successes * 100 / si.iterations) })),
        ("max_retries".to_string(), si.max_retries.to_string()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_self_improver_new() {
        let si = __varg_self_improver_new("test_agent", 3);
        let s = si.lock().unwrap();
        assert_eq!(s.name, "test_agent");
        assert_eq!(s.iterations, 0);
        assert_eq!(s.max_retries, 3);
    }

    #[test]
    fn test_record_success_and_failure() {
        let si = __varg_self_improver_new("test", 3);
        __varg_self_improver_record_success(&si, "sort array", "used quicksort");
        __varg_self_improver_record_failure(&si, "parse JSON", "invalid syntax");
        __varg_self_improver_record_success(&si, "fetch API", "used reqwest");

        assert_eq!(__varg_self_improver_iterations(&si), 3);
        assert_eq!(__varg_self_improver_success_rate(&si), 66); // 2/3
    }

    #[test]
    fn test_recall_learnings() {
        let si = __varg_self_improver_new("test", 3);
        __varg_self_improver_record_success(&si, "sort algorithm", "used merge sort for stability");
        __varg_self_improver_record_success(&si, "search algorithm", "binary search on sorted data");
        __varg_self_improver_record_failure(&si, "parse CSV", "missed escape handling");

        let results = __varg_self_improver_recall(&si, "sorting algorithm", 2);
        assert_eq!(results.len(), 2);
        // Results should contain _content field from memory
        assert!(results[0].contains_key("_content"));
    }

    #[test]
    fn test_stats() {
        let si = __varg_self_improver_new("agent", 5);
        __varg_self_improver_record_success(&si, "t1", "ok");
        __varg_self_improver_record_success(&si, "t2", "ok");
        __varg_self_improver_record_failure(&si, "t3", "err");

        let stats = __varg_self_improver_stats(&si);
        assert_eq!(stats.get("name").unwrap(), "agent");
        assert_eq!(stats.get("iterations").unwrap(), "3");
        assert_eq!(stats.get("successes").unwrap(), "2");
        assert_eq!(stats.get("failures").unwrap(), "1");
        assert_eq!(stats.get("success_rate").unwrap(), "66%");
    }

    #[test]
    fn test_success_rate_zero_iterations() {
        let si = __varg_self_improver_new("test", 3);
        assert_eq!(__varg_self_improver_success_rate(&si), 0);
    }
}
