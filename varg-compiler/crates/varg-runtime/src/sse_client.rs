// Wave 28: SSE (Server-Sent Events) Client
//
// A blocking SSE client for consuming streaming responses from LLM APIs
// (Anthropic, OpenAI, etc.). Each call to `sse_client_next` returns the
// next `data:` event payload, or an empty string on EOF.
//
// Protocol: https://html.spec.whatwg.org/multipage/server-sent-events.html
// We parse only `data:` lines, joining multi-line data with '\n'.
// Lines starting with `event:`, `id:`, `retry:`, or `:` (comments) are ignored.

use reqwest::blocking::Response;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};

pub struct SseClient {
    reader: BufReader<Response>,
    closed: bool,
}

impl std::fmt::Debug for SseClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SseClient")
            .field("closed", &self.closed)
            .finish()
    }
}

pub type SseClientHandle = Arc<Mutex<SseClient>>;

/// Open an SSE stream to the given URL.
/// Caller provides custom headers (e.g. Authorization, x-api-key).
/// Accept and Cache-Control are set automatically.
pub fn __varg_sse_client_connect(
    url: &str,
    headers: HashMap<String, String>,
) -> Result<SseClientHandle, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(None) // streaming: no total timeout
        .build()
        .map_err(|e| format!("SSE client build error: {}", e))?;

    let mut req = client
        .get(url)
        .header("Accept", "text/event-stream")
        .header("Cache-Control", "no-cache");
    for (k, v) in &headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let resp = req
        .send()
        .map_err(|e| format!("SSE connect error: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("SSE HTTP error: {}", resp.status()));
    }
    Ok(Arc::new(Mutex::new(SseClient {
        reader: BufReader::new(resp),
        closed: false,
    })))
}

/// Open an SSE stream via POST (for APIs like Anthropic that require POST).
pub fn __varg_sse_client_post(
    url: &str,
    headers: HashMap<String, String>,
    body: &str,
) -> Result<SseClientHandle, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(None)
        .build()
        .map_err(|e| format!("SSE client build error: {}", e))?;

    let mut req = client
        .post(url)
        .header("Accept", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .body(body.to_string());
    for (k, v) in &headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let resp = req
        .send()
        .map_err(|e| format!("SSE connect error: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("SSE HTTP error: {}", resp.status()));
    }
    Ok(Arc::new(Mutex::new(SseClient {
        reader: BufReader::new(resp),
        closed: false,
    })))
}

/// Read the next SSE `data:` event. Returns:
/// - Ok(payload) — payload string (multi-line joined with '\n')
/// - Ok("") — stream closed / EOF
/// - Err(msg) — transport error
pub fn __varg_sse_client_next(handle: &SseClientHandle) -> Result<String, String> {
    let mut client = handle.lock().map_err(|e| format!("SSE lock poisoned: {}", e))?;
    if client.closed {
        return Ok(String::new());
    }

    let mut data_lines: Vec<String> = Vec::new();
    loop {
        let mut line = String::new();
        match client.reader.read_line(&mut line) {
            Ok(0) => {
                client.closed = true;
                if data_lines.is_empty() {
                    return Ok(String::new());
                }
                return Ok(data_lines.join("\n"));
            }
            Ok(_) => {
                let trimmed = line.trim_end_matches(|c: char| c == '\n' || c == '\r');
                if trimmed.is_empty() {
                    if !data_lines.is_empty() {
                        return Ok(data_lines.join("\n"));
                    }
                    continue;
                }
                if let Some(rest) = trimmed.strip_prefix("data:") {
                    // Spec: single space after `data:` is stripped if present
                    let payload = rest.strip_prefix(' ').unwrap_or(rest);
                    data_lines.push(payload.to_string());
                }
                // Ignore event:, id:, retry:, and comments (lines starting with :)
            }
            Err(e) => return Err(format!("SSE read error: {}", e)),
        }
    }
}

/// Close the SSE stream. Subsequent `next` calls will return empty string.
pub fn __varg_sse_client_close(handle: &SseClientHandle) -> Result<(), String> {
    let mut client = handle.lock().map_err(|e| format!("SSE lock poisoned: {}", e))?;
    client.closed = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    // Note: These are unit tests for the SSE parser logic only.
    // Full integration tests would require a local SSE server.
    // We test the parser by constructing a mock reader.

    use std::io::{BufRead, BufReader, Cursor};

    /// Replicates the parser logic from __varg_sse_client_next for unit testing
    /// without needing a real Response object.
    fn parse_next_event<R: BufRead>(reader: &mut R) -> Result<String, String> {
        let mut data_lines: Vec<String> = Vec::new();
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    if data_lines.is_empty() {
                        return Ok(String::new());
                    }
                    return Ok(data_lines.join("\n"));
                }
                Ok(_) => {
                    let trimmed = line.trim_end_matches(|c: char| c == '\n' || c == '\r');
                    if trimmed.is_empty() {
                        if !data_lines.is_empty() {
                            return Ok(data_lines.join("\n"));
                        }
                        continue;
                    }
                    if let Some(rest) = trimmed.strip_prefix("data:") {
                        let payload = rest.strip_prefix(' ').unwrap_or(rest);
                        data_lines.push(payload.to_string());
                    }
                }
                Err(e) => return Err(format!("read: {}", e)),
            }
        }
    }

    #[test]
    fn test_sse_parse_single_data_event() {
        let input = "data: hello world\n\n";
        let mut reader = BufReader::new(Cursor::new(input));
        let event = parse_next_event(&mut reader).unwrap();
        assert_eq!(event, "hello world");
    }

    #[test]
    fn test_sse_parse_multiple_events() {
        let input = "data: first\n\ndata: second\n\n";
        let mut reader = BufReader::new(Cursor::new(input));
        let e1 = parse_next_event(&mut reader).unwrap();
        let e2 = parse_next_event(&mut reader).unwrap();
        assert_eq!(e1, "first");
        assert_eq!(e2, "second");
    }

    #[test]
    fn test_sse_parse_multiline_data() {
        let input = "data: line one\ndata: line two\n\n";
        let mut reader = BufReader::new(Cursor::new(input));
        let event = parse_next_event(&mut reader).unwrap();
        assert_eq!(event, "line one\nline two");
    }

    #[test]
    fn test_sse_ignores_event_id_retry() {
        let input = "event: message\nid: 1\nretry: 3000\ndata: payload\n\n";
        let mut reader = BufReader::new(Cursor::new(input));
        let event = parse_next_event(&mut reader).unwrap();
        assert_eq!(event, "payload");
    }

    #[test]
    fn test_sse_ignores_comments() {
        let input = ": this is a comment\ndata: real data\n\n";
        let mut reader = BufReader::new(Cursor::new(input));
        let event = parse_next_event(&mut reader).unwrap();
        assert_eq!(event, "real data");
    }

    #[test]
    fn test_sse_eof_returns_empty() {
        let input = "";
        let mut reader = BufReader::new(Cursor::new(input));
        let event = parse_next_event(&mut reader).unwrap();
        assert_eq!(event, "");
    }

    #[test]
    fn test_sse_parses_json_payload() {
        let input = "data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"Hello\"}}\n\n";
        let mut reader = BufReader::new(Cursor::new(input));
        let event = parse_next_event(&mut reader).unwrap();
        assert_eq!(event, "{\"type\":\"content_block_delta\",\"delta\":{\"text\":\"Hello\"}}");
    }

    #[test]
    fn test_sse_data_without_space_after_colon() {
        let input = "data:no-space\n\n";
        let mut reader = BufReader::new(Cursor::new(input));
        let event = parse_next_event(&mut reader).unwrap();
        assert_eq!(event, "no-space");
    }
}
