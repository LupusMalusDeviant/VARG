// Varg Runtime: Multi-Provider LLM Abstraction (Plan 48)
//
// Supports three LLM providers with automatic detection:
//   1. Ollama (default) — local inference, no API key needed
//   2. OpenAI — GPT models via api.openai.com
//   3. Anthropic — Claude models via api.anthropic.com
//
// Environment variables:
//   VARG_LLM_PROVIDER  - "ollama" (default) | "openai" | "anthropic"
//   VARG_LLM_URL       - Override the base URL
//   VARG_LLM_MODEL     - Override the default model name
//   OPENAI_API_KEY      - Required for OpenAI provider
//   ANTHROPIC_API_KEY   - Required for Anthropic provider

use varg_os_types::Context;
use crate::net::__varg_fetch;
use std::collections::HashMap;

// ─── Provider Detection ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum LlmProvider {
    Ollama,
    OpenAI,
    Anthropic,
}

impl LlmProvider {
    /// Detect provider from VARG_LLM_PROVIDER env var (default: Ollama)
    pub fn detect() -> Self {
        match std::env::var("VARG_LLM_PROVIDER").unwrap_or_default().to_lowercase().as_str() {
            "openai" => LlmProvider::OpenAI,
            "anthropic" | "claude" => LlmProvider::Anthropic,
            _ => LlmProvider::Ollama,
        }
    }

    pub fn base_url(&self) -> String {
        std::env::var("VARG_LLM_URL").unwrap_or_else(|_| match self {
            LlmProvider::Ollama => "http://127.0.0.1:11434".to_string(),
            LlmProvider::OpenAI => "https://api.openai.com".to_string(),
            LlmProvider::Anthropic => "https://api.anthropic.com".to_string(),
        })
    }

    pub fn default_model(&self) -> String {
        std::env::var("VARG_LLM_MODEL").unwrap_or_else(|_| match self {
            LlmProvider::Ollama => "llama3".to_string(),
            LlmProvider::OpenAI => "gpt-4o".to_string(),
            LlmProvider::Anthropic => "claude-sonnet-4-20250514".to_string(),
        })
    }

    pub fn chat_endpoint(&self) -> String {
        let base = self.base_url();
        match self {
            LlmProvider::Ollama => format!("{}/api/chat", base),
            LlmProvider::OpenAI => format!("{}/v1/chat/completions", base),
            LlmProvider::Anthropic => format!("{}/v1/messages", base),
        }
    }

    pub fn headers(&self) -> HashMap<String, String> {
        let mut h = HashMap::new();
        h.insert("Content-Type".to_string(), "application/json".to_string());
        match self {
            LlmProvider::Ollama => {},
            LlmProvider::OpenAI => {
                if let Ok(key) = std::env::var("OPENAI_API_KEY") {
                    h.insert("Authorization".to_string(), format!("Bearer {}", key));
                }
            },
            LlmProvider::Anthropic => {
                if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
                    h.insert("x-api-key".to_string(), key);
                    h.insert("anthropic-version".to_string(), "2023-06-01".to_string());
                }
            },
        }
        h
    }

    /// Build request body for a list of messages
    pub fn build_body(&self, model: &str, messages_json: &str, stream: bool) -> String {
        let model = if model.is_empty() { self.default_model() } else { model.to_string() };
        match self {
            LlmProvider::Ollama => {
                format!(
                    "{{\"model\": \"{}\", \"messages\": {}, \"stream\": {}}}",
                    model, messages_json, stream
                )
            },
            LlmProvider::OpenAI => {
                format!(
                    "{{\"model\": \"{}\", \"messages\": {}, \"stream\": {}}}",
                    model, messages_json, stream
                )
            },
            LlmProvider::Anthropic => {
                // Anthropic format: extract system message, remaining become messages
                // Messages format: [{"role": "user", "content": "..."}]
                // System is a top-level field, not in messages
                let msgs: Vec<serde_json::Value> = serde_json::from_str(messages_json)
                    .unwrap_or_default();
                let mut system_text = String::new();
                let mut api_msgs: Vec<serde_json::Value> = Vec::new();
                for msg in &msgs {
                    let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
                    let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    if role == "system" {
                        if !system_text.is_empty() { system_text.push('\n'); }
                        system_text.push_str(content);
                    } else {
                        api_msgs.push(serde_json::json!({
                            "role": role,
                            "content": content,
                        }));
                    }
                }
                let msgs_str = serde_json::to_string(&api_msgs).unwrap_or_else(|_| "[]".to_string());
                if system_text.is_empty() {
                    format!(
                        "{{\"model\": \"{}\", \"max_tokens\": 4096, \"messages\": {}, \"stream\": {}}}",
                        model, msgs_str, stream
                    )
                } else {
                    let safe_system = system_text.replace('\"', "\\\"").replace('\n', "\\n");
                    format!(
                        "{{\"model\": \"{}\", \"max_tokens\": 4096, \"system\": \"{}\", \"messages\": {}, \"stream\": {}}}",
                        model, safe_system, msgs_str, stream
                    )
                }
            },
        }
    }

    /// Extract content from a non-streaming response
    pub fn parse_response(&self, response: &str) -> Option<String> {
        let json: serde_json::Value = serde_json::from_str(response).ok()?;
        match self {
            LlmProvider::Ollama => {
                json.get("message")?.get("content")?.as_str().map(|s| s.to_string())
            },
            LlmProvider::OpenAI => {
                json.get("choices")?
                    .get(0)?
                    .get("message")?
                    .get("content")?
                    .as_str()
                    .map(|s| s.to_string())
            },
            LlmProvider::Anthropic => {
                json.get("content")?
                    .get(0)?
                    .get("text")?
                    .as_str()
                    .map(|s| s.to_string())
            },
        }
    }

    /// Extract content from a streaming chunk (SSE/NDJSON line)
    pub fn parse_stream_chunk(&self, line: &str) -> Option<String> {
        match self {
            LlmProvider::Ollama => {
                let json: serde_json::Value = serde_json::from_str(line).ok()?;
                json.get("message")?.get("content")?.as_str().map(|s| s.to_string())
            },
            LlmProvider::OpenAI => {
                // SSE format: "data: {...}"
                let data = line.strip_prefix("data: ")?;
                if data.trim() == "[DONE]" { return None; }
                let json: serde_json::Value = serde_json::from_str(data).ok()?;
                json.get("choices")?
                    .get(0)?
                    .get("delta")?
                    .get("content")?
                    .as_str()
                    .map(|s| s.to_string())
            },
            LlmProvider::Anthropic => {
                // SSE format: "data: {...}" with event types
                let data = line.strip_prefix("data: ")?;
                let json: serde_json::Value = serde_json::from_str(data).ok()?;
                // content_block_delta events contain the text
                if json.get("type")?.as_str()? == "content_block_delta" {
                    json.get("delta")?.get("text")?.as_str().map(|s| s.to_string())
                } else {
                    None
                }
            },
        }
    }
}

// ─── Public API (backward-compatible signatures) ──────────────────────

/// Build a JSON messages array from a single prompt
fn single_prompt_messages(prompt: &str) -> String {
    let safe = prompt.replace('\\', "\\\\").replace('\"', "\\\"").replace('\n', "\\n");
    format!("[{{\"role\": \"user\", \"content\": \"{}\"}}]", safe)
}

/// Non-streaming LLM inference (single prompt → single response)
pub fn __varg_llm_infer(prompt: &str, model: &str) -> String {
    let provider = LlmProvider::detect();
    let messages_json = single_prompt_messages(prompt);
    let body = provider.build_body(model, &messages_json, false);
    let res = __varg_fetch(&provider.chat_endpoint(), "POST", provider.headers(), &body);
    provider.parse_response(&res).unwrap_or(res)
}

/// Non-streaming LLM chat with context (multi-turn conversation)
pub fn __varg_llm_chat(ctx: &mut Context, prompt: &str, model: &str) -> String {
    let provider = LlmProvider::detect();
    ctx.push("user", prompt);
    let messages_json = serde_json::to_string(&ctx.messages).unwrap_or_else(|_| "[]".to_string());
    let body = provider.build_body(model, &messages_json, false);
    let res = __varg_fetch(&provider.chat_endpoint(), "POST", provider.headers(), &body);
    if let Some(content) = provider.parse_response(&res) {
        ctx.push("assistant", &content);
        content
    } else {
        res
    }
}

/// Streaming LLM chat with context
pub fn __varg_llm_chat_stream(ctx: &mut Context, prompt: &str, model: &str) {
    let provider = LlmProvider::detect();
    ctx.push("user", prompt);
    let messages_json = serde_json::to_string(&ctx.messages).unwrap_or_else(|_| "[]".to_string());
    let body = provider.build_body(model, &messages_json, true);
    __varg_llm_fetch_stream(&provider, &provider.chat_endpoint(), &body);
    ctx.push("assistant", "[STREAMED_REPLY]");
}

/// Streaming LLM inference (single prompt)
pub fn __varg_llm_infer_stream(prompt: &str, model: &str) {
    let provider = LlmProvider::detect();
    let messages_json = single_prompt_messages(prompt);
    let body = provider.build_body(model, &messages_json, true);
    __varg_llm_fetch_stream(&provider, &provider.chat_endpoint(), &body);
}

/// Provider-aware streaming fetch — uses provider-specific chunk parsing
fn __varg_llm_fetch_stream(provider: &LlmProvider, url: &str, body: &str) {
    use std::io::{BufRead, BufReader, Write};

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new());
    let mut builder = client.post(url);
    for (k, v) in &provider.headers() {
        builder = builder.header(k.as_str(), v.as_str());
    }
    builder = builder.body(body.to_string());

    match builder.send() {
        Ok(response) => {
            let reader = BufReader::new(response);
            for line in reader.lines().filter_map(Result::ok) {
                if line.is_empty() { continue; }
                if let Some(content) = provider.parse_stream_chunk(&line) {
                    print!("{}", content);
                    std::io::stdout().flush().unwrap();
                }
            }
        }
        Err(e) => eprintln!("[Varg LLM] Stream error: {}", e),
    }
    println!();
}

/// Create a new conversation context
pub fn __varg_create_context(id: &str) -> Context {
    Context::new(id)
}

/// Create a RAG context from data
pub fn __varg_context_from(data: &str) -> Context {
    let mut ctx = Context::new("rag_context");
    ctx.push("system", &format!("Use the following context to answer:\n{}", data));
    ctx
}

// ─── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_detect_default() {
        // Without env var, defaults to Ollama
        std::env::remove_var("VARG_LLM_PROVIDER");
        assert_eq!(LlmProvider::detect(), LlmProvider::Ollama);
    }

    #[test]
    fn test_provider_detect_openai() {
        std::env::set_var("VARG_LLM_PROVIDER", "openai");
        assert_eq!(LlmProvider::detect(), LlmProvider::OpenAI);
        std::env::remove_var("VARG_LLM_PROVIDER");
    }

    #[test]
    fn test_provider_detect_anthropic() {
        std::env::set_var("VARG_LLM_PROVIDER", "anthropic");
        assert_eq!(LlmProvider::detect(), LlmProvider::Anthropic);
        std::env::remove_var("VARG_LLM_PROVIDER");
    }

    #[test]
    fn test_provider_detect_claude_alias() {
        std::env::set_var("VARG_LLM_PROVIDER", "claude");
        assert_eq!(LlmProvider::detect(), LlmProvider::Anthropic);
        std::env::remove_var("VARG_LLM_PROVIDER");
    }

    #[test]
    fn test_ollama_base_url() {
        std::env::remove_var("VARG_LLM_URL");
        let p = LlmProvider::Ollama;
        assert_eq!(p.base_url(), "http://127.0.0.1:11434");
        assert_eq!(p.chat_endpoint(), "http://127.0.0.1:11434/api/chat");
    }

    #[test]
    fn test_openai_base_url() {
        std::env::remove_var("VARG_LLM_URL");
        let p = LlmProvider::OpenAI;
        assert_eq!(p.base_url(), "https://api.openai.com");
        assert_eq!(p.chat_endpoint(), "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn test_anthropic_base_url() {
        std::env::remove_var("VARG_LLM_URL");
        let p = LlmProvider::Anthropic;
        assert_eq!(p.base_url(), "https://api.anthropic.com");
        assert_eq!(p.chat_endpoint(), "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn test_url_override() {
        std::env::set_var("VARG_LLM_URL", "http://custom:8080");
        let p = LlmProvider::OpenAI;
        assert_eq!(p.base_url(), "http://custom:8080");
        assert_eq!(p.chat_endpoint(), "http://custom:8080/v1/chat/completions");
        std::env::remove_var("VARG_LLM_URL");
    }

    #[test]
    fn test_ollama_parse_response() {
        let p = LlmProvider::Ollama;
        let resp = r#"{"message":{"role":"assistant","content":"Hello!"}}"#;
        assert_eq!(p.parse_response(resp), Some("Hello!".to_string()));
    }

    #[test]
    fn test_openai_parse_response() {
        let p = LlmProvider::OpenAI;
        let resp = r#"{"choices":[{"message":{"content":"Hello from GPT!"}}]}"#;
        assert_eq!(p.parse_response(resp), Some("Hello from GPT!".to_string()));
    }

    #[test]
    fn test_anthropic_parse_response() {
        let p = LlmProvider::Anthropic;
        let resp = r#"{"content":[{"type":"text","text":"Hello from Claude!"}]}"#;
        assert_eq!(p.parse_response(resp), Some("Hello from Claude!".to_string()));
    }

    #[test]
    fn test_ollama_parse_stream_chunk() {
        let p = LlmProvider::Ollama;
        let chunk = r#"{"message":{"content":"Hi"}}"#;
        assert_eq!(p.parse_stream_chunk(chunk), Some("Hi".to_string()));
    }

    #[test]
    fn test_openai_parse_stream_chunk() {
        let p = LlmProvider::OpenAI;
        let chunk = r#"data: {"choices":[{"delta":{"content":"Hi"}}]}"#;
        assert_eq!(p.parse_stream_chunk(chunk), Some("Hi".to_string()));
    }

    #[test]
    fn test_openai_stream_done() {
        let p = LlmProvider::OpenAI;
        assert_eq!(p.parse_stream_chunk("data: [DONE]"), None);
    }

    #[test]
    fn test_anthropic_parse_stream_chunk() {
        let p = LlmProvider::Anthropic;
        let chunk = r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"Hi"}}"#;
        assert_eq!(p.parse_stream_chunk(chunk), Some("Hi".to_string()));
    }

    #[test]
    fn test_anthropic_stream_non_delta_event() {
        let p = LlmProvider::Anthropic;
        let chunk = r#"data: {"type":"message_start","message":{}}"#;
        assert_eq!(p.parse_stream_chunk(chunk), None);
    }

    #[test]
    fn test_ollama_body_format() {
        let p = LlmProvider::Ollama;
        std::env::remove_var("VARG_LLM_MODEL");
        let body = p.build_body("llama3", "[{\"role\":\"user\",\"content\":\"hi\"}]", false);
        assert!(body.contains("\"model\": \"llama3\""));
        assert!(body.contains("\"stream\": false"));
    }

    #[test]
    fn test_anthropic_body_extracts_system() {
        let p = LlmProvider::Anthropic;
        let msgs = r#"[{"role":"system","content":"You are helpful"},{"role":"user","content":"hi"}]"#;
        let body = p.build_body("claude-sonnet-4-20250514", msgs, false);
        assert!(body.contains("\"system\""));
        assert!(body.contains("You are helpful"));
        assert!(body.contains("\"max_tokens\": 4096"));
    }

    #[test]
    fn test_openai_headers_with_key() {
        std::env::set_var("OPENAI_API_KEY", "sk-test123");
        let p = LlmProvider::OpenAI;
        let headers = p.headers();
        assert_eq!(headers.get("Authorization"), Some(&"Bearer sk-test123".to_string()));
        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn test_anthropic_headers_with_key() {
        std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-test");
        let p = LlmProvider::Anthropic;
        let headers = p.headers();
        assert_eq!(headers.get("x-api-key"), Some(&"sk-ant-test".to_string()));
        assert_eq!(headers.get("anthropic-version"), Some(&"2023-06-01".to_string()));
        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_single_prompt_messages() {
        let msgs = single_prompt_messages("Hello \"world\"");
        assert!(msgs.contains("\\\"world\\\""));
        assert!(msgs.contains("\"role\": \"user\""));
    }
}
