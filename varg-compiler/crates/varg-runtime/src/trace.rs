// Wave 22: Agent Observability & Tracing
//
// Lightweight span-based tracing for Varg agents.
// OpenTelemetry-compatible span format, no external dependencies.
// Supports hierarchical spans, events, attributes, and JSON export.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

static SPAN_COUNTER: AtomicU64 = AtomicU64::new(1);

fn now_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

#[derive(Debug, Clone)]
pub struct SpanEvent {
    pub name: String,
    pub timestamp: u64,
    pub attributes: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Span {
    pub span_id: u64,
    pub parent_id: Option<u64>,
    pub name: String,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub attributes: HashMap<String, String>,
    pub events: Vec<SpanEvent>,
    pub status: SpanStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpanStatus {
    Ok,
    Error(String),
    Running,
}

#[derive(Debug, Clone)]
pub struct Tracer {
    pub name: String,
    pub spans: Vec<Span>,
    pub active_span: Option<u64>,
}

pub type TracerHandle = Arc<Mutex<Tracer>>;

/// Create a new tracer for an agent/service
pub fn __varg_trace_start(name: &str) -> TracerHandle {
    Arc::new(Mutex::new(Tracer {
        name: name.to_string(),
        spans: Vec::new(),
        active_span: None,
    }))
}

/// Begin a new span (child of current active span if any)
pub fn __varg_trace_span(tracer: &TracerHandle, name: &str) -> i64 {
    let mut t = tracer.lock().unwrap();
    let span_id = SPAN_COUNTER.fetch_add(1, Ordering::SeqCst);
    let parent_id = t.active_span;
    t.spans.push(Span {
        span_id,
        parent_id,
        name: name.to_string(),
        start_time: now_micros(),
        end_time: None,
        attributes: HashMap::new(),
        events: Vec::new(),
        status: SpanStatus::Running,
    });
    t.active_span = Some(span_id);
    span_id as i64
}

/// End a span (marks it as completed)
pub fn __varg_trace_end(tracer: &TracerHandle, span_id: i64) {
    let mut t = tracer.lock().unwrap();
    let sid = span_id as u64;
    if let Some(span) = t.spans.iter_mut().find(|s| s.span_id == sid) {
        span.end_time = Some(now_micros());
        if span.status == SpanStatus::Running {
            span.status = SpanStatus::Ok;
        }
        // Restore parent as active span
        t.active_span = span.parent_id;
    }
}

/// End a span with error status
pub fn __varg_trace_error(tracer: &TracerHandle, span_id: i64, error_msg: &str) {
    let mut t = tracer.lock().unwrap();
    let sid = span_id as u64;
    if let Some(span) = t.spans.iter_mut().find(|s| s.span_id == sid) {
        span.end_time = Some(now_micros());
        span.status = SpanStatus::Error(error_msg.to_string());
        t.active_span = span.parent_id;
    }
}

/// Add an event to the current active span
pub fn __varg_trace_event(tracer: &TracerHandle, name: &str, attrs: &HashMap<String, String>) {
    let mut t = tracer.lock().unwrap();
    if let Some(active_id) = t.active_span {
        if let Some(span) = t.spans.iter_mut().find(|s| s.span_id == active_id) {
            span.events.push(SpanEvent {
                name: name.to_string(),
                timestamp: now_micros(),
                attributes: attrs.clone(),
            });
        }
    }
}

/// Set an attribute on the current active span
pub fn __varg_trace_set_attr(tracer: &TracerHandle, key: &str, value: &str) {
    let mut t = tracer.lock().unwrap();
    if let Some(active_id) = t.active_span {
        if let Some(span) = t.spans.iter_mut().find(|s| s.span_id == active_id) {
            span.attributes.insert(key.to_string(), value.to_string());
        }
    }
}

/// Get count of completed spans
pub fn __varg_trace_span_count(tracer: &TracerHandle) -> i64 {
    tracer.lock().unwrap().spans.len() as i64
}

/// Export all spans as JSON string (OpenTelemetry-compatible format)
pub fn __varg_trace_export(tracer: &TracerHandle) -> String {
    let t = tracer.lock().unwrap();
    let mut spans_json = Vec::new();

    for span in &t.spans {
        let status_str = match &span.status {
            SpanStatus::Ok => "\"OK\"".to_string(),
            SpanStatus::Error(msg) => format!("{{\"error\": {:?}}}", msg),
            SpanStatus::Running => "\"RUNNING\"".to_string(),
        };

        let attrs: Vec<String> = span.attributes.iter()
            .map(|(k, v)| format!("{:?}: {:?}", k, v))
            .collect();

        let events: Vec<String> = span.events.iter()
            .map(|e| {
                let ea: Vec<String> = e.attributes.iter()
                    .map(|(k, v)| format!("{:?}: {:?}", k, v))
                    .collect();
                format!("{{\"name\": {:?}, \"timestamp\": {}, \"attributes\": {{{}}}}}",
                    e.name, e.timestamp, ea.join(", "))
            })
            .collect();

        let duration = span.end_time.unwrap_or(now_micros()) - span.start_time;

        spans_json.push(format!(
            "{{\"span_id\": {}, \"parent_id\": {}, \"name\": {:?}, \"duration_us\": {}, \"status\": {}, \"attributes\": {{{}}}, \"events\": [{}]}}",
            span.span_id,
            span.parent_id.map(|id| id.to_string()).unwrap_or_else(|| "null".to_string()),
            span.name,
            duration,
            status_str,
            attrs.join(", "),
            events.join(", ")
        ));
    }

    format!("{{\"tracer\": {:?}, \"spans\": [{}]}}", t.name, spans_json.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_start() {
        let tracer = __varg_trace_start("test_agent");
        let t = tracer.lock().unwrap();
        assert_eq!(t.name, "test_agent");
        assert!(t.spans.is_empty());
        assert!(t.active_span.is_none());
    }

    #[test]
    fn test_trace_span_lifecycle() {
        let tracer = __varg_trace_start("test");
        let span_id = __varg_trace_span(&tracer, "process_request");
        assert!(span_id > 0);

        {
            let t = tracer.lock().unwrap();
            assert_eq!(t.spans.len(), 1);
            assert_eq!(t.active_span, Some(span_id as u64));
            assert_eq!(t.spans[0].status, SpanStatus::Running);
        }

        __varg_trace_end(&tracer, span_id);

        let t = tracer.lock().unwrap();
        assert_eq!(t.spans[0].status, SpanStatus::Ok);
        assert!(t.spans[0].end_time.is_some());
        assert!(t.active_span.is_none());
    }

    #[test]
    fn test_trace_nested_spans() {
        let tracer = __varg_trace_start("test");
        let parent = __varg_trace_span(&tracer, "parent");
        let child = __varg_trace_span(&tracer, "child");

        {
            let t = tracer.lock().unwrap();
            assert_eq!(t.spans[1].parent_id, Some(parent as u64));
            assert_eq!(t.active_span, Some(child as u64));
        }

        __varg_trace_end(&tracer, child);
        {
            let t = tracer.lock().unwrap();
            assert_eq!(t.active_span, Some(parent as u64)); // restored to parent
        }

        __varg_trace_end(&tracer, parent);
        let t = tracer.lock().unwrap();
        assert!(t.active_span.is_none());
    }

    #[test]
    fn test_trace_error() {
        let tracer = __varg_trace_start("test");
        let span_id = __varg_trace_span(&tracer, "failing_op");
        __varg_trace_error(&tracer, span_id, "connection timeout");

        let t = tracer.lock().unwrap();
        assert_eq!(t.spans[0].status, SpanStatus::Error("connection timeout".to_string()));
    }

    #[test]
    fn test_trace_event_and_attr() {
        let tracer = __varg_trace_start("test");
        let _span_id = __varg_trace_span(&tracer, "db_query");

        __varg_trace_set_attr(&tracer, "db.system", "sqlite");
        __varg_trace_set_attr(&tracer, "db.statement", "SELECT * FROM users");

        let attrs = HashMap::from([("rows".to_string(), "42".to_string())]);
        __varg_trace_event(&tracer, "query_complete", &attrs);

        let t = tracer.lock().unwrap();
        assert_eq!(t.spans[0].attributes.get("db.system").unwrap(), "sqlite");
        assert_eq!(t.spans[0].events.len(), 1);
        assert_eq!(t.spans[0].events[0].name, "query_complete");
    }

    #[test]
    fn test_trace_export_json() {
        let tracer = __varg_trace_start("my_agent");
        let s = __varg_trace_span(&tracer, "process");
        __varg_trace_set_attr(&tracer, "input", "test");
        __varg_trace_end(&tracer, s);

        let json = __varg_trace_export(&tracer);
        assert!(json.contains("\"tracer\": \"my_agent\""));
        assert!(json.contains("\"name\": \"process\""));
        assert!(json.contains("\"status\": \"OK\""));
        assert!(json.contains("\"input\": \"test\""));
    }

    #[test]
    fn test_trace_span_count() {
        let tracer = __varg_trace_start("test");
        assert_eq!(__varg_trace_span_count(&tracer), 0);
        let s1 = __varg_trace_span(&tracer, "a");
        __varg_trace_end(&tracer, s1);
        let s2 = __varg_trace_span(&tracer, "b");
        __varg_trace_end(&tracer, s2);
        assert_eq!(__varg_trace_span_count(&tracer), 2);
    }
}
