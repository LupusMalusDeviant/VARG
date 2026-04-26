// Wave 27: Base64 Encoding/Decoding Runtime
//
// Provides base64 encode/decode for strings, files, and binary HTTP downloads.
// Uses the base64 crate (already a dependency via crypto module).

use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::collections::HashMap;

/// Encode a string to base64
pub fn __varg_base64_encode(data: &str) -> String {
    STANDARD.encode(data.as_bytes())
}

/// Decode a base64 string back to UTF-8 string
pub fn __varg_base64_decode(encoded: &str) -> String {
    match STANDARD.decode(encoded) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        Err(e) => format!("[base64_decode error: {}]", e),
    }
}

/// Encode a binary file to base64 string
pub fn __varg_base64_encode_file(path: &str) -> String {
    match std::fs::read(path) {
        Ok(bytes) => STANDARD.encode(&bytes),
        Err(e) => format!("[base64_encode_file error: {}]", e),
    }
}

/// Download a URL as binary and return base64-encoded content
/// Accepts a URL and a headers map (key-value pairs)
pub fn __varg_http_download_base64(url: &str, headers: &HashMap<String, String>) -> String {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new());
    let mut req = client.get(url);
    for (k, v) in headers {
        req = req.header(k.as_str(), v.as_str());
    }
    match req.send() {
        Ok(resp) => match resp.bytes() {
            Ok(bytes) => STANDARD.encode(&bytes),
            Err(e) => format!("[http_download_base64 error: {}]", e),
        },
        Err(e) => format!("[http_download_base64 error: {}]", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode_string() {
        let encoded = __varg_base64_encode("hello world");
        assert_eq!(encoded, "aGVsbG8gd29ybGQ=");
    }

    #[test]
    fn test_base64_decode_string() {
        let decoded = __varg_base64_decode("aGVsbG8gd29ybGQ=");
        assert_eq!(decoded, "hello world");
    }

    #[test]
    fn test_base64_roundtrip() {
        let original = "Varg is a compiled language for AI agents! 🤖";
        let encoded = __varg_base64_encode(original);
        let decoded = __varg_base64_decode(&encoded);
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_base64_decode_invalid() {
        let result = __varg_base64_decode("!!!not-valid-base64!!!");
        assert!(result.starts_with("[base64_decode error:"));
    }

    #[test]
    fn test_base64_encode_empty() {
        assert_eq!(__varg_base64_encode(""), "");
        assert_eq!(__varg_base64_decode(""), "");
    }

    #[test]
    fn test_base64_encode_file_not_found() {
        let result = __varg_base64_encode_file("/nonexistent/file.bin");
        assert!(result.starts_with("[base64_encode_file error:"));
    }

    #[test]
    fn test_base64_encode_file_roundtrip() {
        // Write a temp file, encode it, verify
        let tmp = std::env::temp_dir().join("varg_test_b64.txt");
        std::fs::write(&tmp, "binary test content").unwrap();
        let encoded = __varg_base64_encode_file(tmp.to_str().unwrap());
        let decoded = __varg_base64_decode(&encoded);
        assert_eq!(decoded, "binary test content");
        std::fs::remove_file(tmp).ok();
    }
}
