// Wave 25: Agent Orchestration
//
// Fan-out/fan-in patterns for multi-agent workflows.
// Typed task queues, parallel execution, result aggregation.
// No external dependencies — pure Rust with std threads.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

/// A task in the orchestration system
#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub input: String,
    pub status: TaskStatus,
    pub result: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
}

/// Orchestrator manages parallel task execution
#[derive(Debug)]
pub struct Orchestrator {
    pub name: String,
    pub tasks: Vec<Task>,
}

pub type OrchestratorHandle = Arc<Mutex<Orchestrator>>;

/// Create a new orchestrator
pub fn __varg_orchestrator_new(name: &str) -> OrchestratorHandle {
    Arc::new(Mutex::new(Orchestrator {
        name: name.to_string(),
        tasks: Vec::new(),
    }))
}

/// Fan-out: execute multiple functions in parallel, return all results
pub fn __varg_fan_out(
    inputs: &[String],
    handler: Arc<dyn Fn(&str) -> String + Send + Sync>,
) -> Vec<String> {
    let handles: Vec<_> = inputs.iter().map(|input| {
        let input = input.clone();
        let handler = handler.clone();
        thread::spawn(move || handler(&input))
    }).collect();

    handles.into_iter()
        .map(|h| h.join().unwrap_or_else(|_| "error".to_string()))
        .collect()
}

/// Fan-in: merge multiple results into one using a reducer
pub fn __varg_fan_in(
    results: &[String],
    reducer: Arc<dyn Fn(&[String]) -> String + Send + Sync>,
) -> String {
    reducer(results)
}

/// Add a task to the orchestrator
pub fn __varg_orchestrator_add_task(
    orch: &OrchestratorHandle,
    id: &str,
    input: &str,
) {
    let mut o = orch.lock().unwrap();
    o.tasks.push(Task {
        id: id.to_string(),
        input: input.to_string(),
        status: TaskStatus::Pending,
        result: None,
    });
}

/// Run all pending tasks in parallel using the given handler
pub fn __varg_orchestrator_run_all(
    orch: &OrchestratorHandle,
    handler: Arc<dyn Fn(&str) -> String + Send + Sync>,
) {
    let inputs: Vec<(usize, String)> = {
        let o = orch.lock().unwrap();
        o.tasks.iter().enumerate()
            .filter(|(_, t)| t.status == TaskStatus::Pending)
            .map(|(i, t)| (i, t.input.clone()))
            .collect()
    };

    let handles: Vec<_> = inputs.into_iter().map(|(idx, input)| {
        let handler = handler.clone();
        let orch = orch.clone();
        thread::spawn(move || {
            // Mark as running
            {
                let mut o = orch.lock().unwrap();
                o.tasks[idx].status = TaskStatus::Running;
            }
            let result = handler(&input);
            // Mark as completed
            {
                let mut o = orch.lock().unwrap();
                o.tasks[idx].status = TaskStatus::Completed;
                o.tasks[idx].result = Some(result);
            }
        })
    }).collect();

    for h in handles {
        let _ = h.join();
    }
}

/// Get results from all completed tasks
pub fn __varg_orchestrator_results(orch: &OrchestratorHandle) -> Vec<HashMap<String, String>> {
    let o = orch.lock().unwrap();
    o.tasks.iter().map(|t| {
        let mut m = HashMap::new();
        m.insert("id".to_string(), t.id.clone());
        m.insert("input".to_string(), t.input.clone());
        m.insert("status".to_string(), format!("{:?}", t.status));
        if let Some(ref r) = t.result {
            m.insert("result".to_string(), r.clone());
        }
        m
    }).collect()
}

/// Get task count
pub fn __varg_orchestrator_task_count(orch: &OrchestratorHandle) -> i64 {
    orch.lock().unwrap().tasks.len() as i64
}

/// Get completed task count
pub fn __varg_orchestrator_completed_count(orch: &OrchestratorHandle) -> i64 {
    orch.lock().unwrap().tasks.iter()
        .filter(|t| t.status == TaskStatus::Completed)
        .count() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_new() {
        let orch = __varg_orchestrator_new("test");
        let o = orch.lock().unwrap();
        assert_eq!(o.name, "test");
        assert!(o.tasks.is_empty());
    }

    #[test]
    fn test_fan_out() {
        let handler = Arc::new(|input: &str| format!("processed: {}", input));
        let inputs = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let results = __varg_fan_out(&inputs, handler);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], "processed: a");
        assert_eq!(results[1], "processed: b");
        assert_eq!(results[2], "processed: c");
    }

    #[test]
    fn test_fan_in() {
        let results = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let reducer = Arc::new(|items: &[String]| items.join("+"));
        let merged = __varg_fan_in(&results, reducer);
        assert_eq!(merged, "1+2+3");
    }

    #[test]
    fn test_orchestrator_add_and_run() {
        let orch = __varg_orchestrator_new("test");
        __varg_orchestrator_add_task(&orch, "t1", "hello");
        __varg_orchestrator_add_task(&orch, "t2", "world");
        assert_eq!(__varg_orchestrator_task_count(&orch), 2);

        let handler = Arc::new(|input: &str| input.to_uppercase());
        __varg_orchestrator_run_all(&orch, handler);

        assert_eq!(__varg_orchestrator_completed_count(&orch), 2);
        let results = __varg_orchestrator_results(&orch);
        assert_eq!(results[0].get("result").unwrap(), "HELLO");
        assert_eq!(results[1].get("result").unwrap(), "WORLD");
    }

    #[test]
    fn test_fan_out_parallel_execution() {
        // Verify parallel execution by timing
        let handler = Arc::new(|input: &str| {
            std::thread::sleep(std::time::Duration::from_millis(10));
            format!("done: {}", input)
        });
        let inputs: Vec<String> = (0..5).map(|i| i.to_string()).collect();
        let start = std::time::Instant::now();
        let results = __varg_fan_out(&inputs, handler);
        let elapsed = start.elapsed();
        assert_eq!(results.len(), 5);
        // Parallel should be much faster than 50ms (5 * 10ms serial)
        assert!(elapsed.as_millis() < 40);
    }
}
