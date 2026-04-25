// Varg Runtime: Networking via ureq (blocking, no tokio dependency — avoids nested-runtime deadlock)

use std::collections::HashMap;
use std::time::Duration;

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(30))
        .timeout(Duration::from_secs(180))
        .build()
}

/// Perform an HTTP request and return the response body as a string.
pub fn __varg_fetch(url: &str, method: &str, headers: HashMap<String, String>, body: &str) -> String {
    let agent = http_agent();
    let mut req = match method.to_uppercase().as_str() {
        "GET"    => agent.get(url),
        "POST"   => agent.post(url),
        "PUT"    => agent.put(url),
        "DELETE" => agent.delete(url),
        "PATCH"  => agent.patch(url),
        _        => agent.get(url),
    };
    for (k, v) in &headers {
        req = req.set(k, v);
    }
    let result = if body.is_empty() { req.call() } else { req.send_string(body) };
    match result {
        Ok(resp)                         => resp.into_string().unwrap_or_default(),
        Err(ureq::Error::Status(_, resp)) => resp.into_string().unwrap_or_default(),
        Err(e)                           => format!("{{ \"error\": \"Network error: {}\" }}", e),
    }
}

/// Perform an HTTP request and return JSON with status, headers, and body.
/// Returns: {"status": 200, "body": "...", "headers": {"content-type": "..."}}
pub fn __varg_http_request(url: &str, method: &str, headers: HashMap<String, String>, body: &str) -> Result<String, String> {
    let agent = http_agent();
    let mut req = match method.to_uppercase().as_str() {
        "GET"    => agent.get(url),
        "POST"   => agent.post(url),
        "PUT"    => agent.put(url),
        "DELETE" => agent.delete(url),
        "PATCH"  => agent.patch(url),
        _        => agent.get(url),
    };
    for (k, v) in &headers {
        req = req.set(k, v);
    }
    let result = if body.is_empty() { req.call() } else { req.send_string(body) };

    let (status, resp_headers, body_text) = match result {
        Ok(resp) => {
            let status = resp.status();
            let hdrs = collect_headers(&resp);
            let text = resp.into_string().unwrap_or_default();
            (status, hdrs, text)
        }
        Err(ureq::Error::Status(status, resp)) => {
            let hdrs = collect_headers(&resp);
            let text = resp.into_string().unwrap_or_default();
            (status, hdrs, text)
        }
        Err(e) => return Err(format!("Network error: {}", e)),
    };
    Ok(serde_json::json!({
        "status": status,
        "body": body_text,
        "headers": resp_headers
    }).to_string())
}

fn collect_headers(resp: &ureq::Response) -> HashMap<String, String> {
    resp.headers_names()
        .into_iter()
        .filter_map(|name| resp.header(&name).map(|v| (name, v.to_string())))
        .collect()
}

/// Streaming fetch — reads response line by line, parsing SSE/NDJSON and printing content.
pub fn __varg_fetch_stream(url: &str, method: &str, headers: HashMap<String, String>, body: &str) {
    use std::io::{BufRead, BufReader, Write};

    let agent = http_agent();
    let mut req = match method.to_uppercase().as_str() {
        "POST" => agent.post(url),
        _      => agent.get(url),
    };
    for (k, v) in &headers {
        req = req.set(k, v);
    }
    let result = if body.is_empty() { req.call() } else { req.send_string(body) };
    match result {
        Ok(resp) => {
            let reader = BufReader::new(resp.into_reader());
            for line in reader.lines().filter_map(Result::ok) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                    if let Some(content) = json.get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        print!("{}", content);
                        std::io::stdout().flush().unwrap();
                    }
                }
            }
        }
        Err(e) => eprintln!("[VargOS] Stream error: {}", e),
    }
    println!();
}
