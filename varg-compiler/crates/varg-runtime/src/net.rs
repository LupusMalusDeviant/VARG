// Varg Runtime: Networking via reqwest::blocking with global connection pool
// Uses OnceLock to share a single Client instance across calls (HTTP/1.1 + HTTP/2
// keep-alive, up to 10 idle connections per host).

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();

fn get_client() -> &'static reqwest::blocking::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .connection_verbose(false)
            .pool_max_idle_per_host(10)
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new())
    })
}

/// Build and send a request; return (status_u16, response_body, response_headers).
fn execute_request(
    url: &str,
    method: &str,
    headers: &HashMap<String, String>,
    body: &str,
) -> Result<(u16, String, HashMap<String, String>), String> {
    let client = get_client();

    let mut req = match method.to_uppercase().as_str() {
        "GET"    => client.get(url),
        "POST"   => client.post(url),
        "PUT"    => client.put(url),
        "DELETE" => client.delete(url),
        "PATCH"  => client.patch(url),
        _        => client.get(url),
    };

    for (k, v) in headers {
        req = req.header(k.as_str(), v.as_str());
    }

    if !body.is_empty() {
        req = req.body(body.to_string());
    }

    let resp = req.send().map_err(|e| format!("Network error: {}", e))?;

    let status = resp.status().as_u16();
    let resp_headers: HashMap<String, String> = resp
        .headers()
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|s| (k.to_string(), s.to_string())))
        .collect();
    let body_text = resp.text().unwrap_or_default();

    Ok((status, body_text, resp_headers))
}

/// Perform an HTTP request and return the response body as a string.
pub fn __varg_fetch(url: &str, method: &str, headers: HashMap<String, String>, body: &str) -> String {
    match execute_request(url, method, &headers, body) {
        Ok((_, body_text, _)) => body_text,
        Err(e) => format!("{{ \"error\": \"{}\" }}", e),
    }
}

/// Perform an HTTP request and return JSON with status, headers, and body.
/// Returns: {"status": 200, "body": "...", "headers": {"content-type": "..."}}
pub fn __varg_http_request(
    url: &str,
    method: &str,
    headers: HashMap<String, String>,
    body: &str,
) -> Result<String, String> {
    let (status, body_text, resp_headers) = execute_request(url, method, &headers, body)?;
    Ok(serde_json::json!({
        "status": status,
        "body": body_text,
        "headers": resp_headers
    })
    .to_string())
}

/// Streaming fetch — reads response line by line and returns all lines as Vec<String>.
pub fn __varg_fetch_stream(
    url: &str,
    method: &str,
    headers: HashMap<String, String>,
    body: &str,
) -> Vec<String> {
    use std::io::{BufRead, BufReader};

    let client = get_client();
    let mut req = match method.to_uppercase().as_str() {
        "POST" => client.post(url),
        _      => client.get(url),
    };
    for (k, v) in &headers {
        req = req.header(k.as_str(), v.as_str());
    }
    if !body.is_empty() {
        req = req.body(body.to_string());
    }

    match req.send() {
        Ok(resp) => {
            let reader = BufReader::new(resp);
            reader.lines().filter_map(Result::ok).collect()
        }
        Err(e) => {
            eprintln!("[VargOS] Stream error: {}", e);
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_http_client_reuse() {
        let c1 = get_client() as *const _;
        let c2 = get_client() as *const _;
        assert_eq!(c1, c2, "get_client() must return the same instance (pointer equality)");
    }

    #[test]
    fn test_fetch_error_returns_string() {
        // An invalid URL should return an error string, never panic.
        let result = __varg_fetch(
            "http://this.domain.does.not.exist.varg.invalid/test",
            "GET",
            HashMap::new(),
            "",
        );
        // Must be a non-empty string and must NOT panic
        assert!(!result.is_empty(), "fetch error should return a non-empty error string");
    }

    #[test]
    fn test_http_request_error_returns_err() {
        let result = __varg_http_request(
            "http://this.domain.does.not.exist.varg.invalid/test",
            "GET",
            HashMap::new(),
            "",
        );
        assert!(result.is_err(), "http_request to invalid URL should return Err");
    }

    #[test]
    fn test_fetch_stream_error_returns_empty_vec() {
        let result = __varg_fetch_stream(
            "http://this.domain.does.not.exist.varg.invalid/test",
            "GET",
            HashMap::new(),
            "",
        );
        // Should not panic; returns empty vec on network failure
        assert!(
            result.is_empty(),
            "fetch_stream error should return an empty Vec, got: {:?}",
            result
        );
    }
}
