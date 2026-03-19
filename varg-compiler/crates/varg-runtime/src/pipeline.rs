// Wave 24: Reactive Agent Pipelines
//
// Event bus for inter-agent communication.
// Supports: event_emit, event_on, pipeline_create, pipeline_step, pipeline_run.
// No external dependencies — pure Rust implementation.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// An event in the event bus
#[derive(Debug, Clone)]
pub struct Event {
    pub name: String,
    pub data: HashMap<String, String>,
    pub timestamp: u64,
}

/// Event handler: takes event data, returns result string
type EventHandler = Arc<dyn Fn(&HashMap<String, String>) -> String + Send + Sync>;

struct HandlerEntry {
    handler: EventHandler,
}

impl std::fmt::Debug for HandlerEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HandlerEntry")
    }
}

/// Event bus for pub/sub messaging
#[derive(Debug)]
pub struct EventBus {
    pub name: String,
    handlers: HashMap<String, Vec<HandlerEntry>>,
    event_log: Vec<Event>,
}

pub type EventBusHandle = Arc<Mutex<EventBus>>;

/// Create a new event bus
pub fn __varg_event_bus_new(name: &str) -> EventBusHandle {
    Arc::new(Mutex::new(EventBus {
        name: name.to_string(),
        handlers: HashMap::new(),
        event_log: Vec::new(),
    }))
}

/// Register an event handler
pub fn __varg_event_on(
    bus: &EventBusHandle,
    event_name: &str,
    handler: EventHandler,
) {
    let mut b = bus.lock().unwrap();
    b.handlers.entry(event_name.to_string())
        .or_insert_with(Vec::new)
        .push(HandlerEntry { handler });
}

/// Emit an event (calls all registered handlers)
pub fn __varg_event_emit(
    bus: &EventBusHandle,
    event_name: &str,
    data: &HashMap<String, String>,
) -> Vec<String> {
    let b = bus.lock().unwrap();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Log the event
    // (can't mutably borrow while iterating handlers, so we collect results first)
    let results: Vec<String> = if let Some(handlers) = b.handlers.get(event_name) {
        handlers.iter().map(|h| (h.handler)(data)).collect()
    } else {
        Vec::new()
    };

    drop(b);

    // Log event after releasing lock
    let mut b = bus.lock().unwrap();
    b.event_log.push(Event {
        name: event_name.to_string(),
        data: data.clone(),
        timestamp,
    });

    results
}

/// Get event count in the log
pub fn __varg_event_count(bus: &EventBusHandle) -> i64 {
    bus.lock().unwrap().event_log.len() as i64
}

// ===== Pipeline =====

/// A pipeline step
#[derive(Clone)]
pub struct PipelineStep {
    pub name: String,
    pub handler: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

impl std::fmt::Debug for PipelineStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PipelineStep").field("name", &self.name).finish()
    }
}

/// A pipeline of sequential steps
#[derive(Debug)]
pub struct Pipeline {
    pub name: String,
    pub steps: Vec<PipelineStep>,
}

pub type PipelineHandle = Arc<Mutex<Pipeline>>;

/// Create a new pipeline
pub fn __varg_pipeline_new(name: &str) -> PipelineHandle {
    Arc::new(Mutex::new(Pipeline {
        name: name.to_string(),
        steps: Vec::new(),
    }))
}

/// Add a step to the pipeline
pub fn __varg_pipeline_add_step(
    pipeline: &PipelineHandle,
    name: &str,
    handler: Arc<dyn Fn(&str) -> String + Send + Sync>,
) {
    let mut p = pipeline.lock().unwrap();
    p.steps.push(PipelineStep {
        name: name.to_string(),
        handler,
    });
}

/// Run the pipeline, passing output of each step as input to the next
pub fn __varg_pipeline_run(pipeline: &PipelineHandle, initial_input: &str) -> String {
    let p = pipeline.lock().unwrap();
    let mut current = initial_input.to_string();
    for step in &p.steps {
        current = (step.handler)(&current);
    }
    current
}

/// Get step count
pub fn __varg_pipeline_step_count(pipeline: &PipelineHandle) -> i64 {
    pipeline.lock().unwrap().steps.len() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_bus_new() {
        let bus = __varg_event_bus_new("test");
        let b = bus.lock().unwrap();
        assert_eq!(b.name, "test");
        assert!(b.handlers.is_empty());
    }

    #[test]
    fn test_event_emit_and_handle() {
        let bus = __varg_event_bus_new("test");
        let handler = Arc::new(|data: &HashMap<String, String>| {
            format!("handled: {}", data.get("msg").unwrap_or(&"none".to_string()))
        });
        __varg_event_on(&bus, "message", handler);

        let data = HashMap::from([("msg".to_string(), "hello".to_string())]);
        let results = __varg_event_emit(&bus, "message", &data);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "handled: hello");
        assert_eq!(__varg_event_count(&bus), 1);
    }

    #[test]
    fn test_event_multiple_handlers() {
        let bus = __varg_event_bus_new("test");
        let h1 = Arc::new(|_: &HashMap<String, String>| "h1".to_string());
        let h2 = Arc::new(|_: &HashMap<String, String>| "h2".to_string());
        __varg_event_on(&bus, "tick", h1);
        __varg_event_on(&bus, "tick", h2);

        let results = __varg_event_emit(&bus, "tick", &HashMap::new());
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_event_no_handler() {
        let bus = __varg_event_bus_new("test");
        let results = __varg_event_emit(&bus, "unknown", &HashMap::new());
        assert!(results.is_empty());
        assert_eq!(__varg_event_count(&bus), 1); // event is still logged
    }

    #[test]
    fn test_pipeline_new() {
        let pipe = __varg_pipeline_new("data_pipeline");
        let p = pipe.lock().unwrap();
        assert_eq!(p.name, "data_pipeline");
        assert!(p.steps.is_empty());
    }

    #[test]
    fn test_pipeline_run() {
        let pipe = __varg_pipeline_new("transform");
        __varg_pipeline_add_step(&pipe, "uppercase",
            Arc::new(|input: &str| input.to_uppercase()));
        __varg_pipeline_add_step(&pipe, "exclaim",
            Arc::new(|input: &str| format!("{}!", input)));

        let result = __varg_pipeline_run(&pipe, "hello");
        assert_eq!(result, "HELLO!");
        assert_eq!(__varg_pipeline_step_count(&pipe), 2);
    }

    #[test]
    fn test_pipeline_empty() {
        let pipe = __varg_pipeline_new("empty");
        let result = __varg_pipeline_run(&pipe, "passthrough");
        assert_eq!(result, "passthrough");
    }
}
