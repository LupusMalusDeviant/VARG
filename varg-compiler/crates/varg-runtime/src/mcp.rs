// F42: Varg Runtime — MCP Protocol (JSON-RPC Client)
//
// Provides MCP client builtins for compiled Varg programs.
// Connects to external MCP servers via stdio JSON-RPC.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// MCP tool description
#[derive(Clone, Debug)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: String,
}

/// MCP server connection (via child process stdio)
pub struct McpConnection {
    pub command: String,
    pub args: Vec<String>,
    pub is_connected: bool,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    reader: Option<BufReader<ChildStdout>>,
    next_id: u64,
}

impl std::fmt::Debug for McpConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpConnection").field("command", &self.command).field("is_connected", &self.is_connected).finish()
    }
}

impl Drop for McpConnection {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Send a JSON-RPC request and read the response
fn send_request(conn: &mut McpConnection, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
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

    let reader = conn.reader.as_mut().ok_or("MCP stdout not available")?;
    let mut response_line = String::new();
    reader.read_line(&mut response_line).map_err(|e| format!("Failed to read MCP response: {}", e))?;

    let response: serde_json::Value = serde_json::from_str(&response_line)
        .map_err(|e| format!("Invalid JSON-RPC response: {} (raw: {})", e, response_line.trim()))?;

    if let Some(error) = response.get("error") {
        let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
        return Err(format!("MCP error: {}", msg));
    }

    Ok(response.get("result").cloned().unwrap_or(serde_json::Value::Null))
}

/// Send a JSON-RPC notification (no response expected)
fn send_notification(conn: &mut McpConnection, method: &str, params: serde_json::Value) -> Result<(), String> {
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

    let mut conn = McpConnection {
        command: cmd.to_string(),
        args: args.to_vec(),
        is_connected: true,
        child: Some(child),
        stdin: Some(stdin),
        reader: Some(BufReader::new(stdout)),
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

    Ok(conn)
}

/// List available tools from an MCP server
pub fn __varg_mcp_list_tools(conn: &mut McpConnection) -> Result<Vec<McpToolInfo>, String> {
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
pub fn __varg_mcp_call_tool(
    conn: &mut McpConnection,
    tool_name: &str,
    params: &HashMap<String, String>,
) -> Result<String, String> {
    if !conn.is_connected {
        return Err("MCP server not connected".to_string());
    }

    let arguments: serde_json::Value = params.iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
        .collect::<serde_json::Map<String, serde_json::Value>>()
        .into();

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
pub fn __varg_mcp_disconnect(conn: &mut McpConnection) {
    conn.is_connected = false;
    if let Some(ref mut child) = conn.child {
        let _ = child.kill();
        let _ = child.wait();
    }
    conn.child = None;
    conn.stdin = None;
    conn.reader = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_connect_nonexistent_command() {
        let result = __varg_mcp_connect("__nonexistent_mcp_server_xyz__", &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to spawn"));
    }

    #[test]
    fn test_mcp_disconnect_not_connected() {
        // Verify disconnect is safe even without a real connection
        let mut conn = McpConnection {
            command: "test".to_string(),
            args: vec![],
            is_connected: false,
            child: None,
            stdin: None,
            reader: None,
            next_id: 0,
        };
        __varg_mcp_disconnect(&mut conn);
        assert!(!conn.is_connected);
    }

    #[test]
    fn test_mcp_list_tools_not_connected() {
        let mut conn = McpConnection {
            command: "test".to_string(),
            args: vec![],
            is_connected: false,
            child: None,
            stdin: None,
            reader: None,
            next_id: 0,
        };
        assert!(__varg_mcp_list_tools(&mut conn).is_err());
    }

    #[test]
    fn test_mcp_call_tool_not_connected() {
        let mut conn = McpConnection {
            command: "test".to_string(),
            args: vec![],
            is_connected: false,
            child: None,
            stdin: None,
            reader: None,
            next_id: 0,
        };
        assert!(__varg_mcp_call_tool(&mut conn, "test", &HashMap::new()).is_err());
    }
}
