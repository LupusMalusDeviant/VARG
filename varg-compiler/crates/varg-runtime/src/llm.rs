// Varg Runtime: LLM Provider (configurable via environment variables)
//
// Environment variables:
//   VARG_LLM_PROVIDER  - "ollama" (default) | "openai" | "anthropic"
//   VARG_LLM_URL       - Override the base URL (default: http://127.0.0.1:11434)
//   VARG_LLM_MODEL     - Override the model name

use varg_os_types::Context;
use crate::net::{__varg_fetch, __varg_fetch_stream};
use std::collections::HashMap;

fn llm_base_url() -> String {
    std::env::var("VARG_LLM_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string())
}

/// Non-streaming LLM inference (single prompt → single response)
pub fn __varg_llm_infer(prompt: &str, model: &str) -> String {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    let safe_prompt = prompt.replace("\"", "\\\"");
    let body = format!(
        "{{\"model\": \"{}\", \"messages\": [{{\"role\": \"user\", \"content\": \"{}\"}}], \"stream\": false}}",
        model, safe_prompt
    );
    let base_url = llm_base_url();
    let res = __varg_fetch(&format!("{}/api/chat", base_url), "POST", headers, &body);
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&res) {
        if let Some(content) = json.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_str()) {
            return content.to_string();
        }
    }
    res
}

/// Non-streaming LLM chat with context (multi-turn conversation)
pub fn __varg_llm_chat(ctx: &mut Context, prompt: &str, model: &str) -> String {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    ctx.push("user", prompt);
    let messages_json = serde_json::to_string(&ctx.messages).unwrap_or_else(|_| "[]".to_string());
    let body = format!(
        "{{\"model\": \"{}\", \"messages\": {}, \"stream\": false}}",
        model, messages_json
    );
    let base_url = llm_base_url();
    let res = __varg_fetch(&format!("{}/api/chat", base_url), "POST", headers, &body);
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&res) {
        if let Some(content) = json.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_str()) {
            ctx.push("assistant", content);
            return content.to_string();
        }
    }
    res
}

/// Streaming LLM chat with context
pub fn __varg_llm_chat_stream(ctx: &mut Context, prompt: &str, model: &str) {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    ctx.push("user", prompt);
    let messages_json = serde_json::to_string(&ctx.messages).unwrap_or_else(|_| "[]".to_string());
    let body = format!(
        "{{\"model\": \"{}\", \"messages\": {}, \"stream\": true}}",
        model, messages_json
    );
    let base_url = llm_base_url();
    __varg_fetch_stream(&format!("{}/api/chat", base_url), "POST", headers, &body);
    ctx.push("assistant", "[STREAMED_REPLY]");
}

/// Streaming LLM inference (single prompt)
pub fn __varg_llm_infer_stream(prompt: &str, model: &str) {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    let safe_prompt = prompt.replace("\"", "\\\"");
    let body = format!(
        "{{\"model\": \"{}\", \"messages\": [{{\"role\": \"user\", \"content\": \"{}\"}}], \"stream\": true}}",
        model, safe_prompt
    );
    let base_url = llm_base_url();
    __varg_fetch_stream(&format!("{}/api/chat", base_url), "POST", headers, &body);
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
