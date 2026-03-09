// Varg Runtime: Networking via reqwest (replaces curl subprocess)

use std::collections::HashMap;

/// Perform an HTTP request using reqwest (blocking).
/// Replaces the old curl subprocess approach with proper error handling.
pub fn __varg_fetch(url: &str, method: &str, headers: HashMap<String, String>, body: &str) -> String {
    let client = reqwest::blocking::Client::new();
    let mut builder = match method.to_uppercase().as_str() {
        "GET" => client.get(url),
        "POST" => client.post(url),
        "PUT" => client.put(url),
        "DELETE" => client.delete(url),
        "PATCH" => client.patch(url),
        _ => client.get(url),
    };

    for (k, v) in &headers {
        builder = builder.header(k.as_str(), v.as_str());
    }

    if !body.is_empty() {
        builder = builder.body(body.to_string());
    }

    match builder.send() {
        Ok(response) => {
            match response.text() {
                Ok(text) => text,
                Err(e) => format!("{{ \"error\": \"Response read error: {}\" }}", e),
            }
        }
        Err(e) => format!("{{ \"error\": \"Network error: {}\" }}", e),
    }
}

/// Streaming fetch — reads response line by line, parsing SSE/NDJSON and printing content.
pub fn __varg_fetch_stream(url: &str, method: &str, headers: HashMap<String, String>, body: &str) {
    use std::io::{BufRead, BufReader, Write};

    let client = reqwest::blocking::Client::new();
    let mut builder = match method.to_uppercase().as_str() {
        "POST" => client.post(url),
        _ => client.get(url),
    };

    for (k, v) in &headers {
        builder = builder.header(k.as_str(), v.as_str());
    }

    if !body.is_empty() {
        builder = builder.body(body.to_string());
    }

    match builder.send() {
        Ok(response) => {
            let reader = BufReader::new(response);
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
