// F42: Varg Runtime — WebSocket Support
//
// Provides WebSocket builtins for compiled Varg programs.
// Uses tungstenite (blocking) for WebSocket client.

use tungstenite::{connect, Message, WebSocket};
use tungstenite::stream::MaybeTlsStream;
use std::net::TcpStream;

/// WebSocket connection handle
pub struct VargWebSocket {
    pub url: String,
    pub is_connected: bool,
    socket: Option<WebSocket<MaybeTlsStream<TcpStream>>>,
}

impl std::fmt::Debug for VargWebSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VargWebSocket").field("url", &self.url).field("is_connected", &self.is_connected).finish()
    }
}

/// WebSocket message
#[derive(Clone, Debug)]
pub enum VargWsMessage {
    Text(String),
    Binary(Vec<u8>),
    Close,
}

/// Connect to a WebSocket server (client-side, blocking)
pub fn __varg_ws_connect(url: &str) -> Result<VargWebSocket, String> {
    let (socket, _response) = connect(url)
        .map_err(|e| format!("WebSocket connect failed '{}': {}", url, e))?;
    Ok(VargWebSocket {
        url: url.to_string(),
        is_connected: true,
        socket: Some(socket),
    })
}

/// Send a text message over WebSocket
pub fn __varg_ws_send(ws: &mut VargWebSocket, message: &str) -> Result<(), String> {
    if !ws.is_connected {
        return Err("WebSocket not connected".to_string());
    }
    let socket = ws.socket.as_mut().ok_or("WebSocket handle not available")?;
    socket.send(Message::Text(message.to_string()))
        .map_err(|e| format!("WebSocket send failed: {}", e))
}

/// Receive a message from WebSocket (blocking)
pub fn __varg_ws_receive(ws: &mut VargWebSocket) -> Result<String, String> {
    if !ws.is_connected {
        return Err("WebSocket not connected".to_string());
    }
    let socket = ws.socket.as_mut().ok_or("WebSocket handle not available")?;
    loop {
        let msg = socket.read().map_err(|e| format!("WebSocket receive failed: {}", e))?;
        match msg {
            Message::Text(text) => return Ok(text),
            Message::Binary(data) => return Ok(String::from_utf8_lossy(&data).to_string()),
            Message::Close(_) => {
                ws.is_connected = false;
                return Err("WebSocket closed by remote".to_string());
            }
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {
                // Skip control frames, continue reading
                continue;
            }
        }
    }
}

/// Close WebSocket connection
pub fn __varg_ws_close(ws: &mut VargWebSocket) {
    ws.is_connected = false;
    if let Some(mut socket) = ws.socket.take() {
        let _ = socket.close(None);
    }
}

/// SSE Writer for server-side events (placeholder — real SSE server needs axum integration)
pub struct VargSseWriter {
    pub is_open: bool,
}

/// Create an SSE stream writer
pub fn __varg_sse_stream() -> VargSseWriter {
    VargSseWriter { is_open: true }
}

/// Send an SSE event
pub fn __varg_sse_send(writer: &VargSseWriter, event: &str, data: &str) -> Result<(), String> {
    if !writer.is_open {
        return Err("SSE stream closed".to_string());
    }
    let _ = (event, data);
    Ok(())
}

/// Close an SSE stream
pub fn __varg_sse_close(writer: &mut VargSseWriter) {
    writer.is_open = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_connect_invalid_url() {
        let result = __varg_ws_connect("ws://127.0.0.1:1");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("WebSocket connect failed"));
    }

    #[test]
    fn test_ws_send_not_connected() {
        let mut ws = VargWebSocket {
            url: "ws://test".to_string(),
            is_connected: false,
            socket: None,
        };
        assert!(__varg_ws_send(&mut ws, "hello").is_err());
    }

    #[test]
    fn test_ws_receive_not_connected() {
        let mut ws = VargWebSocket {
            url: "ws://test".to_string(),
            is_connected: false,
            socket: None,
        };
        assert!(__varg_ws_receive(&mut ws).is_err());
    }

    #[test]
    fn test_ws_close_without_socket() {
        let mut ws = VargWebSocket {
            url: "ws://test".to_string(),
            is_connected: true,
            socket: None,
        };
        __varg_ws_close(&mut ws);
        assert!(!ws.is_connected);
    }

    #[test]
    fn test_sse_lifecycle() {
        let mut writer = __varg_sse_stream();
        assert!(writer.is_open);
        assert!(__varg_sse_send(&writer, "update", "data1").is_ok());
        __varg_sse_close(&mut writer);
        assert!(!writer.is_open);
        assert!(__varg_sse_send(&writer, "update", "data2").is_err());
    }
}
