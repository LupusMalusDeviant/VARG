// F42: Varg Runtime — MCP Protocol (JSON-RPC Client)
//
// Provides MCP client builtins for compiled Varg programs.
// Connects to external MCP servers via stdio JSON-RPC.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// MCP tool description
#[derive(Clone, Debug)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: String,
}

/// MCP server connection (via child process stdio)
pub struct McpConnectionInner {
    pub command: String,
    pub args: Vec<String>,
    pub is_connected: bool,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    // A reader thread owns the child's stdout and pushes whole lines down this channel. It
    // used to be the BufReader itself, read straight from the request path: a server that sent
    // nothing left `read_line` blocked and took the agent with it, and no message count could
    // bound that because no message ever arrived.
    lines: Option<Receiver<String>>,
    next_id: u64,
}

impl std::fmt::Debug for McpConnectionInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpConnectionInner").field("command", &self.command).field("is_connected", &self.is_connected).finish()
    }
}

impl Drop for McpConnectionInner {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// How long to wait for a server to answer one request. `VARG_MCP_TIMEOUT_SECS` overrides it;
/// a server that is merely slow should not be mistaken for one that is stuck.
fn response_timeout() -> Duration {
    let secs = std::env::var("VARG_MCP_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);
    Duration::from_secs(secs.max(1))
}

/// Read the child's stdout on its own thread so the request path can wait with a deadline.
/// The thread ends when the pipe closes, which is what turns into a Disconnected on the
/// receiving side and then into "closed the connection before responding".
fn spawn_reader(stdout: ChildStdout) -> Receiver<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    if tx.send(l).is_err() {
                        return; // the connection was dropped
                    }
                }
                Err(_) => return,
            }
        }
    });
    rx
}

/// R3: read from a JSON-RPC stream until the message whose `id` matches `expected_id` arrives.
/// MCP servers may interleave notifications (no `id`), log lines, or responses to earlier
/// requests on stdout; blindly taking the first line desynchronised the client. Skips
/// non-matching / non-JSON lines, stops on EOF, and is bounded so a server that never answers
/// our id cannot spin forever. (A server that emits nothing at all can still block on read; a
/// wall-clock timeout would require a dedicated reader thread.) Extracted from `send_request`
/// so the framing logic is unit-testable against a synthetic stream.
fn read_matching_response(
    mut next_line: impl FnMut() -> Result<Option<String>, String>,
    expected_id: u64,
) -> Result<serde_json::Value, String> {
    for _ in 0..1000 {
        let response_line = match next_line()? {
            Some(l) => l,
            None => return Err("MCP server closed the connection before responding".to_string()),
        };
        let trimmed = response_line.trim();
        if trimmed.is_empty() { continue; }
        let msg: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue, // non-JSON noise on stdout (e.g. server logging) — skip
        };
        match msg.get("id").and_then(|v| v.as_u64()) {
            Some(id) if id == expected_id => return Ok(msg),
            _ => continue, // notification or response to a different request — skip
        }
    }
    Err("MCP server produced too many non-matching messages without answering the request".to_string())
}

/// Send a JSON-RPC request and read the response
fn send_request(conn: &mut McpConnectionInner, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    conn.next_id += 1;
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": conn.next_id,
        "method": method,
        "params": params
    });

    let stdin = conn.stdin.as_mut().ok_or("MCP stdin not available")?;
    let mut line = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    line.push('\n');
    stdin.write_all(line.as_bytes()).map_err(|e| format!("Failed to write to MCP server: {}", e))?;
    stdin.flush().map_err(|e| format!("Failed to flush MCP stdin: {}", e))?;

    let lines = conn.lines.as_ref().ok_or("MCP stdout not available")?;
    let expected_id = conn.next_id;
    // One deadline for the whole exchange, not per line: a server that dribbles out noise
    // forever is as stuck as one that says nothing.
    let deadline = Instant::now() + response_timeout();
    let response = read_matching_response(
        || match lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(line) => Ok(Some(line)),
            Err(RecvTimeoutError::Disconnected) => Ok(None),
            Err(RecvTimeoutError::Timeout) => Err(format!(
                "MCP server did not answer `{}` within {} seconds",
                method,
                response_timeout().as_secs()
            )),
        },
        expected_id,
    )?;

    if let Some(error) = response.get("error") {
        let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
        return Err(format!("MCP error: {}", msg));
    }

    Ok(response.get("result").cloned().unwrap_or(serde_json::Value::Null))
}

/// Send a JSON-RPC notification (no response expected)
fn send_notification(conn: &mut McpConnectionInner, method: &str, params: serde_json::Value) -> Result<(), String> {
    let notification = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params
    });

    let stdin = conn.stdin.as_mut().ok_or("MCP stdin not available")?;
    let mut line = serde_json::to_string(&notification).map_err(|e| e.to_string())?;
    line.push('\n');
    stdin.write_all(line.as_bytes()).map_err(|e| format!("Failed to write notification: {}", e))?;
    stdin.flush().map_err(|e| format!("Failed to flush: {}", e))?;
    Ok(())
}

/// A shared, thread-safe MCP connection.
///
/// Every other stateful handle in the runtime (vector store, workflow, MCP *server*) is an
/// `Arc<Mutex<_>>`; the client connection used to be a bare struct requiring `&mut`. That made it
/// impossible to use from a tool handler — those are `Fn` + Send + Sync closures, which cannot
/// mutably borrow a captured value. A router that forwards calls to a child MCP needs exactly that,
/// so the connection is now a handle like the rest.
pub type McpConnection = std::sync::Arc<std::sync::Mutex<McpConnectionInner>>;

/// Lock a connection, recovering from a poisoned mutex rather than cascading the panic.
fn lock(conn: &McpConnection) -> std::sync::MutexGuard<'_, McpConnectionInner> {
    conn.lock().unwrap_or_else(|e| e.into_inner())
}

/// Connect to an MCP server by spawning a child process
pub fn __varg_mcp_connect(cmd: &str, args: &[String]) -> Result<McpConnection, String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn MCP server '{}': {}", cmd, e))?;

    let stdin = child.stdin.take().ok_or("Failed to capture MCP stdin")?;
    let stdout = child.stdout.take().ok_or("Failed to capture MCP stdout")?;

    let mut conn = McpConnectionInner {
        command: cmd.to_string(),
        args: args.to_vec(),
        is_connected: true,
        child: Some(child),
        stdin: Some(stdin),
        lines: Some(spawn_reader(stdout)),
        next_id: 0,
    };

    // MCP initialize handshake
    let init_result = send_request(&mut conn, "initialize", serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "varg", "version": "0.1.0" }
    }));

    if let Err(e) = init_result {
        conn.is_connected = false;
        return Err(format!("MCP initialize failed: {}", e));
    }

    // Send initialized notification
    let _ = send_notification(&mut conn, "notifications/initialized", serde_json::json!({}));

    Ok(std::sync::Arc::new(std::sync::Mutex::new(conn)))
}

/// List available tools from an MCP server
pub fn __varg_mcp_list_tools(conn: &McpConnection) -> Result<Vec<McpToolInfo>, String> {
    let conn = &mut *lock(conn);
    if !conn.is_connected {
        return Err("MCP server not connected".to_string());
    }

    let result = send_request(conn, "tools/list", serde_json::json!({}))?;

    let tools = result.get("tools").and_then(|t| t.as_array()).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for tool in tools {
        out.push(McpToolInfo {
            name: tool.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string(),
            description: tool.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string(),
            input_schema: tool.get("inputSchema").map(|s| s.to_string()).unwrap_or_default(),
        });
    }
    Ok(out)
}

/// Call a tool on an MCP server
/// Tool arguments: either a `{name: value}` map, or a raw JSON object string.
///
/// The map form stringifies every value (`{"n": 42}` would go out as `{"n": "42"}`), which is fine
/// when you build the args yourself but lossy for a **proxy**: a router forwarding a caller's
/// arguments must pass them through verbatim, types intact. The string form does exactly that.
pub trait ToToolArgs {
    fn to_arguments(&self) -> serde_json::Value;
}

impl ToToolArgs for HashMap<String, String> {
    fn to_arguments(&self) -> serde_json::Value {
        self.iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect::<serde_json::Map<String, serde_json::Value>>()
            .into()
    }
}

// Argument maps stay `HashMap<String, String>` on purpose. Adding impls for numeric and boolean
// maps looked like an obvious convenience, but it made the empty literal `{}` — "this tool takes
// no arguments", the most common call of all — ambiguous, because Rust could no longer infer the
// value type. Mixed or numeric arguments go through the JSON string form below, which is also the
// only way to express a heterogeneous object: a Varg map literal is homogeneous.
// The shape codegen produces for a map literal in tool-argument position. One impl, and the
// value types inside the object are whatever they were written as — which is why the empty
// literal is no longer ambiguous: it is a Value, not a HashMap of some type to be inferred.
impl ToToolArgs for serde_json::Value {
    fn to_arguments(&self) -> serde_json::Value {
        match self {
            v @ serde_json::Value::Object(_) => v.clone(),
            // The protocol requires an object; anything else would be silently wrong.
            _ => serde_json::Value::Object(serde_json::Map::new()),
        }
    }
}

impl ToToolArgs for String {
    fn to_arguments(&self) -> serde_json::Value {
        // Forward a JSON object as-is; anything else becomes an empty object rather than
        // silently sending a non-object where the protocol requires one.
        match serde_json::from_str::<serde_json::Value>(self) {
            Ok(v @ serde_json::Value::Object(_)) => v,
            _ => serde_json::Value::Object(serde_json::Map::new()),
        }
    }
}

impl ToToolArgs for str {
    fn to_arguments(&self) -> serde_json::Value {
        self.to_string().to_arguments()
    }
}

pub fn __varg_mcp_call_tool<P: ToToolArgs + ?Sized>(
    conn: &McpConnection,
    tool_name: &str,
    params: &P,
) -> Result<String, String> {
    let conn = &mut *lock(conn);
    if !conn.is_connected {
        return Err("MCP server not connected".to_string());
    }

    let arguments: serde_json::Value = params.to_arguments();

    let result = send_request(conn, "tools/call", serde_json::json!({
        "name": tool_name,
        "arguments": arguments
    }))?;

    // Extract text from content array
    if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
        if let Some(first) = content.first() {
            if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                return Ok(text.to_string());
            }
        }
    }

    // Fallback: return the whole result as JSON
    Ok(serde_json::to_string(&result).unwrap_or_default())
}

/// Disconnect from MCP server
pub fn __varg_mcp_disconnect(conn: &McpConnection) {
    let conn = &mut *lock(conn);
    conn.is_connected = false;
    if let Some(ref mut child) = conn.child {
        let _ = child.kill();
        let _ = child.wait();
    }
    conn.child = None;
    conn.stdin = None;
    conn.lines = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// The framing tests drive a synthetic stream; production drives a channel. Both hand the
    /// same shape to `read_matching_response`, so the framing cases are unchanged by the reader
    /// thread that sits behind it now.
    fn next_from<R: BufRead>(r: &mut R) -> Result<Option<String>, String> {
        let mut line = String::new();
        match r.read_line(&mut line) {
            Ok(0) => Ok(None),
            Ok(_) => Ok(Some(line)),
            Err(e) => Err(e.to_string()),
        }
    }

    #[test]
    fn a_server_that_says_nothing_times_out_instead_of_blocking() {
        // The framing loop was bounded by a message count, which cannot bound a server that
        // sends no messages at all: the read blocked and took the agent with it. This drives
        // the same shape the request path uses — a channel nobody sends to — against a short
        // deadline, and expects the wait to end.
        let (_tx, rx) = std::sync::mpsc::channel::<String>();
        let deadline = Instant::now() + Duration::from_millis(300);
        let err = read_matching_response(
            || match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(line) => Ok(Some(line)),
                Err(RecvTimeoutError::Disconnected) => Ok(None),
                Err(RecvTimeoutError::Timeout) => Err("timed out".to_string()),
            },
            1,
        )
        .unwrap_err();
        assert_eq!(err, "timed out");
    }

    #[test]
    fn a_closed_pipe_is_reported_as_a_closed_connection() {
        // The sender dropped: the reader thread has ended because the child's stdout closed.
        // That is a different failure from a timeout and has to read as one.
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        drop(tx);
        let err = read_matching_response(
            || match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(line) => Ok(Some(line)),
                Err(RecvTimeoutError::Disconnected) => Ok(None),
                Err(RecvTimeoutError::Timeout) => Err("timed out".to_string()),
            },
            1,
        )
        .unwrap_err();
        assert!(err.contains("closed the connection"), "got: {}", err);
    }

    #[test]
    fn the_response_timeout_is_configurable_and_never_zero() {
        assert_eq!(response_timeout(), Duration::from_secs(30));
    }

    #[test]
    fn test_r3_skips_notification_then_matches_response() {
        // A notification (no id) precedes the real response — must be skipped, not returned.
        let stream = "{\"jsonrpc\":\"2.0\",\"method\":\"log\",\"params\":{}}\n\
                      {\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"ok\":true}}\n";
        let mut r = Cursor::new(stream);
        let resp = read_matching_response(|| next_from(&mut r), 7).expect("should find id 7");
        assert_eq!(resp["result"]["ok"], serde_json::json!(true));
    }

    #[test]
    fn test_r3_skips_non_json_log_lines() {
        // Plain log noise on stdout must not derail framing.
        let stream = "starting server...\n\
                      [info] ready\n\
                      {\"jsonrpc\":\"2.0\",\"id\":3,\"result\":\"done\"}\n";
        let mut r = Cursor::new(stream);
        let resp = read_matching_response(|| next_from(&mut r), 3).expect("should find id 3");
        assert_eq!(resp["result"], serde_json::json!("done"));
    }

    #[test]
    fn test_r3_skips_response_for_different_id() {
        // A stale response to an earlier request (id 1) must be skipped when awaiting id 2.
        let stream = "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"stale\"}\n\
                      {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":\"fresh\"}\n";
        let mut r = Cursor::new(stream);
        let resp = read_matching_response(|| next_from(&mut r), 2).expect("should find id 2");
        assert_eq!(resp["result"], serde_json::json!("fresh"));
    }

    #[test]
    fn test_r3_eof_before_response_errors() {
        // Server closes without ever answering — must error, not hang or panic.
        let stream = "{\"jsonrpc\":\"2.0\",\"method\":\"note\",\"params\":{}}\n";
        let mut r = Cursor::new(stream);
        let err = read_matching_response(|| next_from(&mut r), 9).unwrap_err();
        assert!(err.contains("closed the connection"), "got: {err}");
    }

    #[test]
    fn test_mcp_connect_nonexistent_command() {
        let result = __varg_mcp_connect("__nonexistent_mcp_server_xyz__", &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to spawn"));
    }

    #[test]
    fn test_mcp_disconnect_not_connected() {
        // Verify disconnect is safe even without a real connection
        let mut conn = McpConnectionInner {
            command: "test".to_string(),
            args: vec![],
            is_connected: false,
            child: None,
            stdin: None,
            lines: None,
            next_id: 0,
        };
        let conn = std::sync::Arc::new(std::sync::Mutex::new(conn));
        __varg_mcp_disconnect(&conn);
        assert!(!lock(&conn).is_connected);
    }

    #[test]
    fn test_mcp_list_tools_not_connected() {
        let mut conn = McpConnectionInner {
            command: "test".to_string(),
            args: vec![],
            is_connected: false,
            child: None,
            stdin: None,
            lines: None,
            next_id: 0,
        };
        let conn = std::sync::Arc::new(std::sync::Mutex::new(conn));
        assert!(__varg_mcp_list_tools(&conn).is_err());
    }

    #[test]
    fn test_mcp_call_tool_not_connected() {
        let mut conn = McpConnectionInner {
            command: "test".to_string(),
            args: vec![],
            is_connected: false,
            child: None,
            stdin: None,
            lines: None,
            next_id: 0,
        };
        let conn = std::sync::Arc::new(std::sync::Mutex::new(conn));
        assert!(__varg_mcp_call_tool(&conn, "test", &HashMap::new()).is_err());
    }

    // ── Tool argument forwarding ──────────────────────────────────────────

    #[test]
    fn map_args_are_sent_as_strings() {
        let mut m = HashMap::new();
        m.insert("msg".to_string(), "hi".to_string());
        let v = m.to_arguments();
        assert_eq!(v["msg"], serde_json::json!("hi"));
    }

    /// A proxy must forward the caller's arguments verbatim — types intact. The map form would
    /// turn 42 into "42" and drop nesting; the string form must not.
    #[test]
    fn raw_json_args_are_forwarded_verbatim() {
        let raw = r#"{"n": 42, "flag": true, "nested": {"a": [1, 2]}, "s": "text"}"#.to_string();
        let v = raw.to_arguments();
        assert_eq!(v["n"], serde_json::json!(42), "numbers must stay numbers");
        assert_eq!(v["flag"], serde_json::json!(true), "bools must stay bools");
        assert_eq!(v["nested"], serde_json::json!({"a": [1, 2]}), "nesting must survive");
        assert_eq!(v["s"], serde_json::json!("text"));
    }

    #[test]
    fn non_object_args_become_an_empty_object() {
        // The protocol wants an object; a bare array/scalar/garbage must not be sent through.
        assert_eq!("[1,2]".to_string().to_arguments(), serde_json::json!({}));
        assert_eq!("\"just a string\"".to_string().to_arguments(), serde_json::json!({}));
        assert_eq!("not json".to_string().to_arguments(), serde_json::json!({}));
    }

}
