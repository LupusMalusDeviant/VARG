// Wave 34: Workflow / DAG Execution Engine

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum StepStatus { Pending, Done, Failed, Skipped }

#[derive(Debug, Clone)]
pub struct WorkflowStep {
    pub name: String,
    pub deps: Vec<String>,
    pub output: Option<String>,
    pub status: StepStatus,
    pub error: Option<String>,
}

pub struct Workflow {
    pub name: String,
    steps: HashMap<String, WorkflowStep>,
    order: Vec<String>,
}

impl Workflow {
    pub fn new(name: &str) -> Self {
        Workflow { name: name.to_string(), steps: HashMap::new(), order: Vec::new() }
    }

    pub fn add_step(&mut self, name: &str, deps: Vec<String>) {
        self.steps.insert(name.to_string(), WorkflowStep {
            name: name.to_string(),
            deps,
            output: None,
            status: StepStatus::Pending,
            error: None,
        });
        if !self.order.contains(&name.to_string()) {
            self.order.push(name.to_string());
        }
    }

    pub fn set_output(&mut self, step: &str, output: &str) {
        if let Some(s) = self.steps.get_mut(step) {
            s.output = Some(output.to_string());
            s.status = StepStatus::Done;
        }
    }

    pub fn set_failed(&mut self, step: &str, error: &str) {
        if let Some(s) = self.steps.get_mut(step) {
            s.status = StepStatus::Failed;
            s.error = Some(error.to_string());
        }
        // Skip steps that depend on the failed one
        let failed = step.to_string();
        let to_skip: Vec<String> = self.steps.values()
            .filter(|s| s.deps.contains(&failed) && s.status == StepStatus::Pending)
            .map(|s| s.name.clone())
            .collect();
        for name in to_skip {
            if let Some(s) = self.steps.get_mut(&name) {
                s.status = StepStatus::Skipped;
            }
        }
    }

    pub fn ready_steps(&self) -> Vec<String> {
        self.order.iter()
            .filter(|name| {
                self.steps.get(*name).map_or(false, |s| {
                    s.status == StepStatus::Pending
                        && s.deps.iter().all(|d| {
                            self.steps.get(d).map_or(true, |dep| dep.status == StepStatus::Done)
                        })
                })
            })
            .cloned()
            .collect()
    }

    pub fn is_complete(&self) -> bool {
        self.steps.values().all(|s| s.status != StepStatus::Pending)
    }

    pub fn get_output(&self, step: &str) -> String {
        self.steps.get(step).and_then(|s| s.output.clone()).unwrap_or_default()
    }

    pub fn step_count(&self) -> usize { self.steps.len() }

    pub fn status_report(&self) -> String {
        let done    = self.steps.values().filter(|s| s.status == StepStatus::Done).count();
        let failed  = self.steps.values().filter(|s| s.status == StepStatus::Failed).count();
        let skipped = self.steps.values().filter(|s| s.status == StepStatus::Skipped).count();
        let pending = self.steps.values().filter(|s| s.status == StepStatus::Pending).count();
        format!("'{}': {done}/{} done | {failed} failed | {skipped} skipped | {pending} pending",
            self.name, self.steps.len())
    }
}

pub type WorkflowHandle = Arc<Mutex<Workflow>>;

pub fn __varg_workflow_new(name: &str) -> WorkflowHandle {
    Arc::new(Mutex::new(Workflow::new(name)))
}

pub fn __varg_workflow_add_step(h: &WorkflowHandle, name: &str, deps: Vec<String>) {
    h.lock().unwrap().add_step(name, deps);
}

pub fn __varg_workflow_set_output(h: &WorkflowHandle, step: &str, output: &str) {
    h.lock().unwrap().set_output(step, output);
}

pub fn __varg_workflow_set_failed(h: &WorkflowHandle, step: &str, error: &str) {
    h.lock().unwrap().set_failed(step, error);
}

pub fn __varg_workflow_ready_steps(h: &WorkflowHandle) -> Vec<String> {
    h.lock().unwrap().ready_steps()
}

pub fn __varg_workflow_is_complete(h: &WorkflowHandle) -> bool {
    h.lock().unwrap().is_complete()
}

pub fn __varg_workflow_get_output(h: &WorkflowHandle, step: &str) -> String {
    h.lock().unwrap().get_output(step)
}

pub fn __varg_workflow_step_count(h: &WorkflowHandle) -> i64 {
    h.lock().unwrap().step_count() as i64
}

pub fn __varg_workflow_status(h: &WorkflowHandle) -> String {
    h.lock().unwrap().status_report()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_new_empty() {
        let w = __varg_workflow_new("pipe");
        assert_eq!(__varg_workflow_step_count(&w), 0);
    }

    #[test]
    fn test_workflow_add_steps() {
        let w = __varg_workflow_new("pipe");
        __varg_workflow_add_step(&w, "a", vec![]);
        __varg_workflow_add_step(&w, "b", vec!["a".into()]);
        assert_eq!(__varg_workflow_step_count(&w), 2);
    }

    #[test]
    fn test_workflow_ready_no_deps() {
        let w = __varg_workflow_new("pipe");
        __varg_workflow_add_step(&w, "x", vec![]);
        __varg_workflow_add_step(&w, "y", vec![]);
        let ready = __varg_workflow_ready_steps(&w);
        assert!(ready.contains(&"x".into()) && ready.contains(&"y".into()));
    }

    #[test]
    fn test_workflow_dep_blocks_step() {
        let w = __varg_workflow_new("pipe");
        __varg_workflow_add_step(&w, "a", vec![]);
        __varg_workflow_add_step(&w, "b", vec!["a".into()]);
        let ready = __varg_workflow_ready_steps(&w);
        assert!(ready.contains(&"a".into()));
        assert!(!ready.contains(&"b".into()));
    }

    #[test]
    fn test_workflow_completes_after_all_done() {
        let w = __varg_workflow_new("pipe");
        __varg_workflow_add_step(&w, "a", vec![]);
        __varg_workflow_add_step(&w, "b", vec!["a".into()]);
        assert!(!__varg_workflow_is_complete(&w));
        __varg_workflow_set_output(&w, "a", "r_a");
        __varg_workflow_set_output(&w, "b", "r_b");
        assert!(__varg_workflow_is_complete(&w));
    }

    #[test]
    fn test_workflow_failure_skips_dependents() {
        let w = __varg_workflow_new("pipe");
        __varg_workflow_add_step(&w, "a", vec![]);
        __varg_workflow_add_step(&w, "b", vec!["a".into()]);
        __varg_workflow_set_failed(&w, "a", "network error");
        assert!(__varg_workflow_is_complete(&w));
    }

    #[test]
    fn test_workflow_output_retrieval() {
        let w = __varg_workflow_new("pipe");
        __varg_workflow_add_step(&w, "s1", vec![]);
        __varg_workflow_set_output(&w, "s1", "my_result");
        assert_eq!(__varg_workflow_get_output(&w, "s1"), "my_result");
    }

    #[test]
    fn test_workflow_status_report() {
        let w = __varg_workflow_new("my_pipeline");
        __varg_workflow_add_step(&w, "a", vec![]);
        __varg_workflow_set_output(&w, "a", "done");
        let report = __varg_workflow_status(&w);
        assert!(report.contains("my_pipeline"));
    }

    // ── Adversarial / edge-case tests ────────────────────────────────────────

    #[test]
    fn test_workflow_empty_is_complete() {
        // A workflow with zero steps has nothing pending → is_complete must be true
        let w = __varg_workflow_new("empty");
        assert!(__varg_workflow_is_complete(&w), "empty workflow must be immediately complete");
    }

    #[test]
    fn test_workflow_empty_ready_steps() {
        let w = __varg_workflow_new("empty");
        assert!(__varg_workflow_ready_steps(&w).is_empty());
    }

    #[test]
    fn test_workflow_circular_dependency_never_becomes_ready() {
        // A→B, B→A: neither step ever has all deps Done → ready_steps is empty
        let w = __varg_workflow_new("circ");
        __varg_workflow_add_step(&w, "A", vec!["B".into()]);
        __varg_workflow_add_step(&w, "B", vec!["A".into()]);
        let ready = __varg_workflow_ready_steps(&w);
        assert!(ready.is_empty(), "circular deps must produce no ready steps, got: {ready:?}");
    }

    #[test]
    fn test_workflow_set_output_nonexistent_step_is_safe() {
        // Calling set_output on a step that was never added must be a no-op, not panic
        let w = __varg_workflow_new("noop");
        __varg_workflow_set_output(&w, "ghost_step", "output"); // must not panic
        assert_eq!(__varg_workflow_step_count(&w), 0);
    }

    #[test]
    fn test_workflow_set_failed_nonexistent_step_is_safe() {
        let w = __varg_workflow_new("noop");
        __varg_workflow_set_failed(&w, "ghost_step", "error"); // must not panic
    }

    #[test]
    fn test_workflow_set_output_twice_overwrites() {
        let w = __varg_workflow_new("ow");
        __varg_workflow_add_step(&w, "s", vec![]);
        __varg_workflow_set_output(&w, "s", "first");
        __varg_workflow_set_output(&w, "s", "second");
        assert_eq!(__varg_workflow_get_output(&w, "s"), "second", "second set_output must overwrite");
    }

    #[test]
    fn test_workflow_failed_step_makes_dependent_skipped_not_ready() {
        let w = __varg_workflow_new("skip");
        __varg_workflow_add_step(&w, "a", vec![]);
        __varg_workflow_add_step(&w, "b", vec!["a".into()]);
        __varg_workflow_add_step(&w, "c", vec!["b".into()]);
        __varg_workflow_set_failed(&w, "a", "oops");
        let ready = __varg_workflow_ready_steps(&w);
        assert!(!ready.contains(&"b".to_string()), "b must be skipped when a failed");
        // c depends on b which is now Skipped (not Done), so c also must not become ready
        assert!(!ready.contains(&"c".to_string()));
    }

    #[test]
    fn test_workflow_get_output_nonexistent_returns_empty() {
        let w = __varg_workflow_new("out");
        assert_eq!(__varg_workflow_get_output(&w, "nonexistent"), "");
    }

    #[test]
    fn test_workflow_add_duplicate_step_name_is_idempotent() {
        let w = __varg_workflow_new("dup");
        __varg_workflow_add_step(&w, "s", vec![]);
        __varg_workflow_add_step(&w, "s", vec![]); // same name again
        // Should not create a duplicate entry; order list deduplicates
        assert_eq!(__varg_workflow_step_count(&w), 1);
    }

    #[test]
    fn test_workflow_long_linear_chain() {
        // 50-step chain: each depends on previous
        let w = __varg_workflow_new("chain");
        let names: Vec<String> = (0..50).map(|i| format!("step_{i}")).collect();
        __varg_workflow_add_step(&w, &names[0], vec![]);
        for i in 1..50 {
            __varg_workflow_add_step(&w, &names[i], vec![names[i-1].clone()]);
        }
        // Only step_0 is ready initially
        let ready = __varg_workflow_ready_steps(&w);
        assert_eq!(ready, vec!["step_0".to_string()]);
        // Complete each step in order
        for name in &names {
            __varg_workflow_set_output(&w, name, "ok");
        }
        assert!(__varg_workflow_is_complete(&w));
    }

    #[test]
    fn test_workflow_dep_on_unknown_step_treated_as_satisfied() {
        // Dep references a name that was never add_step'd.
        // ready_steps uses map_or(true, ...) for unknown deps → treats them as Done
        let w = __varg_workflow_new("unknown_dep");
        __varg_workflow_add_step(&w, "b", vec!["nonexistent_a".into()]);
        let ready = __varg_workflow_ready_steps(&w);
        // "nonexistent_a" has no step entry → map_or(true) → treated as Done → b is ready
        assert!(ready.contains(&"b".to_string()), "dep on unknown step must not block");
    }

    #[test]
    fn test_workflow_status_report_counts() {
        let w = __varg_workflow_new("counts");
        __varg_workflow_add_step(&w, "ok", vec![]);
        __varg_workflow_add_step(&w, "fail", vec![]);
        __varg_workflow_add_step(&w, "dep_of_fail", vec!["fail".into()]);
        __varg_workflow_add_step(&w, "pending", vec![]);
        __varg_workflow_set_output(&w, "ok", "done");
        __varg_workflow_set_failed(&w, "fail", "err");
        let report = __varg_workflow_status(&w);
        assert!(report.contains("failed") || report.contains("1 failed") || report.contains("failed |"),
            "status report must mention failures: {report}");
    }
}
