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
    let mut s = server.lock().unwrap();
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
    server.lock().unwrap().tools.len() as i64
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
    let s = server.lock().unwrap();

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
        let s = server.lock().unwrap();
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
