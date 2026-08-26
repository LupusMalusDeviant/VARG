// Wave 23: Varg Runtime — MCP Server Mode
//
// Allows Varg agents to expose methods as MCP tools.
// Implements JSON-RPC over stdio (standard MCP transport).
// Tools are registered with name, description, and a handler function.

use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};

/// A registered MCP tool with its handler
#[derive(Clone)]
pub struct McpServerTool {
    pub name: String,
    pub description: String,
    pub parameters: Vec<McpParamInfo>,
    /// Handler function: takes JSON args string, returns JSON result string
    pub handler: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

impl std::fmt::Debug for McpServerTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServerTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct McpParamInfo {
    pub name: String,
    pub param_type: String,
    pub description: String,
    pub required: bool,
}

/// MCP Server state
#[derive(Debug, Clone)]
pub struct McpServer {
    pub name: String,
    pub version: String,
    pub tools: Vec<McpServerTool>,
}

pub type McpServerHandle = Arc<Mutex<McpServer>>;

/// Create a new MCP server
pub fn __varg_mcp_server_new(name: &str, version: &str) -> McpServerHandle {
    Arc::new(Mutex::new(McpServer {
        name: name.to_string(),
        version: version.to_string(),
        tools: Vec::new(),
    }))
}

/// Register a tool with the MCP server
pub fn __varg_mcp_server_add_tool(
    server: &McpServerHandle,
    name: &str,
    description: &str,
    params: Vec<McpParamInfo>,
    handler: Arc<dyn Fn(&str) -> String + Send + Sync>,
) {
    let mut s = server.lock().unwrap_or_else(|e| e.into_inner());
    s.tools.push(McpServerTool {
        name: name.to_string(),
        description: description.to_string(),
        parameters: params,
        handler,
    });
}

/// Register a simple tool (string args -> string result) — used by codegen
pub fn __varg_mcp_server_register(
    server: &McpServerHandle,
    name: &str,
    description: &str,
    handler: Arc<dyn Fn(&str) -> String + Send + Sync>,
) {
    __varg_mcp_server_add_tool(server, name, description, vec![], handler);
}

/// Get tool count
pub fn __varg_mcp_server_tool_count(server: &McpServerHandle) -> i64 {
    server.lock().unwrap_or_else(|e| e.into_inner()).tools.len() as i64
}

/// Remove a tool by name at runtime (dynamic hot-unplug). Returns `true` if a tool was removed.
/// After this, the tool no longer appears in `tools/list` and calls to it are rejected — the
/// building block for a router MCP that swaps child capabilities on and off.
pub fn __varg_mcp_server_remove_tool(server: &McpServerHandle, name: &str) -> bool {
    let mut s = server.lock().unwrap_or_else(|e| e.into_inner());
    let before = s.tools.len();
    s.tools.retain(|t| t.name != name);
    s.tools.len() != before
}

/// Whether a tool with this name is currently registered.
pub fn __varg_mcp_server_has_tool(server: &McpServerHandle, name: &str) -> bool {
    server.lock().unwrap_or_else(|e| e.into_inner()).tools.iter().any(|t| t.name == name)
}

#[cfg(test)]
mod remove_tests {
    use super::*;

    #[test]
    fn remove_tool_hot_unplugs() {
        let srv = __varg_mcp_server_new("router", "1.0");
        __varg_mcp_server_register(&srv, "echo", "echoes", Arc::new(|a: &str| a.to_string()));
        __varg_mcp_server_register(&srv, "ping", "pong", Arc::new(|_: &str| "pong".to_string()));
        assert_eq!(__varg_mcp_server_tool_count(&srv), 2);
        assert!(__varg_mcp_server_has_tool(&srv, "echo"));
        // Remove one → count drops, tool gone, second remove is a no-op.
        assert!(__varg_mcp_server_remove_tool(&srv, "echo"));
        assert!(!__varg_mcp_server_has_tool(&srv, "echo"));
        assert_eq!(__varg_mcp_server_tool_count(&srv), 1);
        assert!(!__varg_mcp_server_remove_tool(&srv, "echo"));
        // tools/list no longer advertises the removed tool.
        let s = srv.lock().unwrap();
        let list = generate_tools_list(&s);
        assert!(!list.contains("echo"));
        assert!(list.contains("ping"));
    }
}

/// Generate the JSON schema for tools/list response
fn generate_tools_list(server: &McpServer) -> String {
    let tools: Vec<String> = server.tools.iter().map(|tool| {
        let params: Vec<String> = tool.parameters.iter().map(|p| {
            format!("{:?}: {{\"type\": {:?}, \"description\": {:?}}}",
                p.name, p.param_type, p.description)
        }).collect();

        let required: Vec<String> = tool.parameters.iter()
            .filter(|p| p.required)
            .map(|p| format!("{:?}", p.name))
            .collect();

        format!(
            "{{\"name\": {:?}, \"description\": {:?}, \"inputSchema\": {{\"type\": \"object\", \"properties\": {{{}}}, \"required\": [{}]}}}}",
            tool.name, tool.description, params.join(", "), required.join(", ")
        )
    }).collect();

    format!("[{}]", tools.join(", "))
}

/// Handle a single JSON-RPC request, return response string
pub fn __varg_mcp_server_handle_request(server: &McpServerHandle, request: &str) -> String {
    let s = server.lock().unwrap_or_else(|e| e.into_inner());

    // Minimal JSON parsing (no serde dependency needed)
    let id = extract_json_field(request, "id").unwrap_or("null".to_string());
    let method = extract_json_string(request, "method").unwrap_or_default();

    match method.as_str() {
        "initialize" => {
            format!(
                "{{\"jsonrpc\": \"2.0\", \"id\": {}, \"result\": {{\"protocolVersion\": \"2024-11-05\", \"capabilities\": {{\"tools\": {{}}}}, \"serverInfo\": {{\"name\": {:?}, \"version\": {:?}}}}}}}",
                id, s.name, s.version
            )
        }
        "notifications/initialized" => {
            // Notification — no response needed
            String::new()
        }
        "tools/list" => {
            let tools = generate_tools_list(&s);
            format!(
                "{{\"jsonrpc\": \"2.0\", \"id\": {}, \"result\": {{\"tools\": {}}}}}",
                id, tools
            )
        }
        "tools/call" => {
            let tool_name = extract_json_string(request, "name")
                .or_else(|| {
                    // Try params.name
                    let params = extract_json_field(request, "params").unwrap_or_default();
                    extract_json_string(&params, "name")
                })
                .unwrap_or_default();
            let arguments = extract_json_field(request, "arguments")
                .or_else(|| {
                    let params = extract_json_field(request, "params").unwrap_or_default();
                    extract_json_field(&params, "arguments")
                })
                .unwrap_or_else(|| "{}".to_string());

            if let Some(tool) = s.tools.iter().find(|t| t.name == tool_name) {
                let result = (tool.handler)(&arguments);
                format!(
                    "{{\"jsonrpc\": \"2.0\", \"id\": {}, \"result\": {{\"content\": [{{\"type\": \"text\", \"text\": {}}}]}}}}",
                    id, json_escape(&result)
                )
            } else {
                format!(
                    "{{\"jsonrpc\": \"2.0\", \"id\": {}, \"error\": {{\"code\": -32601, \"message\": \"Tool not found: {}\"}}}}",
                    id, tool_name
                )
            }
        }
        _ => {
            format!(
                "{{\"jsonrpc\": \"2.0\", \"id\": {}, \"error\": {{\"code\": -32601, \"message\": \"Method not found: {}\"}}}}",
                id, method
            )
        }
    }
}

/// Run the MCP server on stdio (blocking)
pub fn __varg_mcp_server_run(server: &McpServerHandle) {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let reader = stdin.lock();

    for line in reader.lines() {
        match line {
            Ok(line) if line.trim().is_empty() => continue,
            Ok(line) => {
                let response = __varg_mcp_server_handle_request(server, &line);
                if !response.is_empty() {
                    let mut out = stdout.lock();
                    let _ = writeln!(out, "{}", response);
                    let _ = out.flush();
                }
            }
            Err(_) => break,
        }
    }
}

// Simple JSON field extraction helpers (no serde needed)

fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let pos = json.find(&pattern)?;
    let after = &json[pos + pattern.len()..];
    // Skip : and whitespace
    let after = after.trim_start();
    let after = after.strip_prefix(':')?;
    let after = after.trim_start();
    // Read string value
    if after.starts_with('"') {
        let after = &after[1..];
        let end = after.find('"')?;
        Some(after[..end].to_string())
    } else {
        None
    }
}

fn extract_json_field(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let pos = json.find(&pattern)?;
    let after = &json[pos + pattern.len()..];
    let after = after.trim_start();
    let after = after.strip_prefix(':')?;
    let after = after.trim_start();

    if after.starts_with('"') {
        // String value
        let after = &after[1..];
        let end = after.find('"')?;
        Some(format!("\"{}\"", &after[..end]))
    } else if after.starts_with('{') || after.starts_with('[') {
        // Object or array — find matching bracket
        let open = after.as_bytes()[0];
        let close = if open == b'{' { b'}' } else { b']' };
        let mut depth = 0;
        for (i, ch) in after.bytes().enumerate() {
            if ch == open { depth += 1; }
            if ch == close { depth -= 1; }
            if depth == 0 {
                return Some(after[..=i].to_string());
            }
        }
        None
    } else {
        // Number, bool, null
        let end = after.find(|c: char| c == ',' || c == '}' || c == ']').unwrap_or(after.len());
        Some(after[..end].trim().to_string())
    }
}

fn json_escape(s: &str) -> String {
    format!("{:?}", s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_new() {
        let server = __varg_mcp_server_new("test_agent", "1.0.0");
        let s = server.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(s.name, "test_agent");
        assert_eq!(s.version, "1.0.0");
        assert!(s.tools.is_empty());
    }

    #[test]
    fn test_mcp_server_register_tool() {
        let server = __varg_mcp_server_new("test", "1.0");
        let handler = Arc::new(|_args: &str| "hello".to_string());
        __varg_mcp_server_register(&server, "greet", "Say hello", handler);
        assert_eq!(__varg_mcp_server_tool_count(&server), 1);
    }

    #[test]
    fn test_mcp_server_initialize() {
        let server = __varg_mcp_server_new("my_agent", "0.1.0");
        let resp = __varg_mcp_server_handle_request(&server,
            r#"{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}"#);
        assert!(resp.contains("\"protocolVersion\""));
        assert!(resp.contains("\"my_agent\""));
    }

    #[test]
    fn test_mcp_server_tools_list() {
        let server = __varg_mcp_server_new("test", "1.0");
        let handler = Arc::new(|_: &str| "ok".to_string());
        __varg_mcp_server_register(&server, "search", "Search documents", handler);

        let resp = __varg_mcp_server_handle_request(&server,
            r#"{"jsonrpc": "2.0", "id": 2, "method": "tools/list"}"#);
        assert!(resp.contains("\"search\""));
        assert!(resp.contains("\"Search documents\""));
    }

    #[test]
    fn test_mcp_server_tools_call() {
        let server = __varg_mcp_server_new("test", "1.0");
        let handler = Arc::new(|args: &str| format!("echoed: {}", args));
        __varg_mcp_server_register(&server, "echo", "Echo input", handler);

        let resp = __varg_mcp_server_handle_request(&server,
            r#"{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "echo", "arguments": {"msg": "hello"}}}"#);
        assert!(resp.contains("echoed:"));
        assert!(resp.contains("\"id\": 3"));
    }

    #[test]
    fn test_mcp_server_tool_not_found() {
        let server = __varg_mcp_server_new("test", "1.0");
        let resp = __varg_mcp_server_handle_request(&server,
            r#"{"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "nonexistent", "arguments": {}}}"#);
        assert!(resp.contains("\"error\""));
        assert!(resp.contains("Tool not found"));
    }

    #[test]
    fn test_mcp_server_unknown_method() {
        let server = __varg_mcp_server_new("test", "1.0");
        let resp = __varg_mcp_server_handle_request(&server,
            r#"{"jsonrpc": "2.0", "id": 5, "method": "unknown/method"}"#);
        assert!(resp.contains("Method not found"));
    }
}

// ── Wave 23: JSON-RPC helpers for the generated server loop ───────────────────
//
// An `@[McpTool]` method already gets a schema and a CLI dispatch arm from vargc. What was
// missing is the protocol itself, so a real MCP client can call it.
//
// The envelope and parsing live here rather than in generated code: they are the same for every
// program, and here they can be tested directly. The *dispatch* has to stay in generated code —
// it calls a method on the entry agent, and a `Fn + Send + Sync` handler could not hold `&mut`
// to it.
//
// These use serde_json rather than the hand-rolled extraction above. The older helpers predate
// the dependency being available; a real client sends nested objects and escaped strings, which
// substring scanning gets wrong.

fn rpc_id(request: &serde_json::Value) -> serde_json::Value {
    request.get("id").cloned().unwrap_or(serde_json::Value::Null)
}

/// The JSON-RPC method name, or "" if the request is not parseable.
pub fn __varg_mcp_rpc_method(request: &str) -> String {
    serde_json::from_str::<serde_json::Value>(request)
        .ok()
        .and_then(|v| v.get("method").and_then(|m| m.as_str()).map(String::from))
        .unwrap_or_default()
}

/// The request id, rendered as JSON so it can be echoed back verbatim (it may be a number,
/// a string, or absent).
pub fn __varg_mcp_rpc_id(request: &str) -> String {
    serde_json::from_str::<serde_json::Value>(request)
        .map(|v| rpc_id(&v).to_string())
        .unwrap_or_else(|_| "null".to_string())
}

/// `params.name` of a tools/call request.
pub fn __varg_mcp_rpc_tool_name(request: &str) -> String {
    serde_json::from_str::<serde_json::Value>(request)
        .ok()
        .and_then(|v| {
            v.get("params")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .map(String::from)
        })
        .unwrap_or_default()
}

/// One argument of a tools/call request, as text. Numbers and booleans are rendered without
/// quotes so the generated code can parse them into the declared parameter type; a missing
/// argument yields "".
pub fn __varg_mcp_rpc_argument(request: &str, name: &str) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(request) else {
        return String::new();
    };
    let arg = v
        .get("params")
        .and_then(|p| p.get("arguments"))
        .and_then(|a| a.get(name));
    match arg {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn envelope(id: &str, body: serde_json::Value) -> String {
    let id_value: serde_json::Value =
        serde_json::from_str(id).unwrap_or(serde_json::Value::Null);
    serde_json::json!({ "jsonrpc": "2.0", "id": id_value, "result": body }).to_string()
}

pub fn __varg_mcp_rpc_initialize(id: &str, name: &str, version: &str) -> String {
    envelope(
        id,
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": name, "version": version }
        }),
    )
}

/// `tools_json` is the array vargc already builds from the `@[McpTool]` annotations.
pub fn __varg_mcp_rpc_tools_list(id: &str, tools_json: &str) -> String {
    let tools: serde_json::Value =
        serde_json::from_str(tools_json).unwrap_or(serde_json::Value::Array(vec![]));
    envelope(id, serde_json::json!({ "tools": tools }))
}

/// A tool result. MCP wraps it in a content array; the text is whatever the method returned.
pub fn __varg_mcp_rpc_result(id: &str, text: &str) -> String {
    envelope(
        id,
        serde_json::json!({ "content": [{ "type": "text", "text": text }] }),
    )
}

pub fn __varg_mcp_rpc_error(id: &str, code: i64, message: &str) -> String {
    let id_value: serde_json::Value =
        serde_json::from_str(id).unwrap_or(serde_json::Value::Null);
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id_value,
        "error": { "code": code, "message": message }
    })
    .to_string()
}

// ── Wave 23: JSON-RPC helper tests ───────────────────────────────────────────
#[cfg(test)]
mod rpc_tests {
    use super::*;

    fn call_request() -> String {
        // What a real client sends: nested params, a string and a number argument, and text
        // containing the very characters substring scanning gets wrong.
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "search_docs",
                "arguments": { "query": "say \"hi\", {braces}", "top_k": 5, "exact": true }
            }
        })
        .to_string()
    }

    #[test]
    fn method_and_id_are_read_back() {
        let r = call_request();
        assert_eq!(__varg_mcp_rpc_method(&r), "tools/call");
        assert_eq!(__varg_mcp_rpc_id(&r), "7");
        assert_eq!(__varg_mcp_rpc_tool_name(&r), "search_docs");
    }

    /// A string id must survive as a string, and a missing id as null — the client matches
    /// responses on it, so echoing it back in the wrong shape loses the reply.
    #[test]
    fn ids_keep_their_json_shape() {
        let s = serde_json::json!({"id": "abc", "method": "tools/list"}).to_string();
        assert_eq!(__varg_mcp_rpc_id(&s), "\"abc\"");
        let none = serde_json::json!({"method": "tools/list"}).to_string();
        assert_eq!(__varg_mcp_rpc_id(&none), "null");
        assert_eq!(__varg_mcp_rpc_id("not json at all"), "null");
    }

    #[test]
    fn arguments_come_out_as_text_the_caller_can_parse() {
        let r = call_request();
        // Strings unquoted, so they can be used directly.
        assert_eq!(__varg_mcp_rpc_argument(&r, "query"), "say \"hi\", {braces}");
        // Non-strings keep their JSON rendering, so an int parameter can parse them.
        assert_eq!(__varg_mcp_rpc_argument(&r, "top_k"), "5");
        assert_eq!(__varg_mcp_rpc_argument(&r, "exact"), "true");
        // A missing argument is empty rather than an error: the generated code applies the
        // parameter's own default handling.
        assert_eq!(__varg_mcp_rpc_argument(&r, "absent"), "");
    }

    #[test]
    fn envelopes_are_valid_json_rpc() {
        let init = __varg_mcp_rpc_initialize("1", "demo", "0.1.0");
        let v: serde_json::Value = serde_json::from_str(&init).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 1);
        assert_eq!(v["result"]["serverInfo"]["name"], "demo");
        assert!(v["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn tools_list_carries_the_generated_schema() {
        let tools = r#"[{"name":"search_docs","description":"Search","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}}]"#;
        let out = __varg_mcp_rpc_tools_list("2", tools);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let listed = &v["result"]["tools"][0];
        assert_eq!(listed["name"], "search_docs");
        // The schema must arrive as an object, not as a string containing JSON.
        assert!(listed["inputSchema"]["properties"]["query"].is_object());
        assert_eq!(listed["inputSchema"]["required"][0], "query");
    }

    /// Result text is escaped by serde, so a tool returning quotes or newlines cannot break the
    /// response the client parses.
    #[test]
    fn result_text_survives_awkward_output() {
        let out = __varg_mcp_rpc_result("3", "line1\nsaid \"ok\"\tdone");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["result"]["content"][0]["type"], "text");
        assert_eq!(v["result"]["content"][0]["text"], "line1\nsaid \"ok\"\tdone");
    }

    #[test]
    fn errors_use_the_error_member() {
        let out = __varg_mcp_rpc_error("4", -32601, "Tool not found: nope");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["error"]["code"], -32601);
        assert_eq!(v["error"]["message"], "Tool not found: nope");
        assert!(v.get("result").is_none());
    }
}
