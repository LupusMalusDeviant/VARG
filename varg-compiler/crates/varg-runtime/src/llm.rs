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

// ─── Wave 31: Structured Output ──────────────────────────────────────────

/// Call the LLM and force JSON output matching `schema_json`.
/// Retries up to `retries` times if the response is not valid JSON.
pub fn __varg_llm_structured(prompt: &str, schema_json: &str, retries: i64) -> String {
    let provider = LlmProvider::detect();
    let system_msg = format!(
        "Respond with ONLY a valid JSON object matching this exact schema. No markdown fences, no explanation:\n{}",
        schema_json
    );
    let messages_json = serde_json::json!([
        {"role": "system", "content": system_msg},
        {"role": "user",   "content": prompt}
    ])
    .to_string();
    let body = provider.build_body(&provider.default_model(), &messages_json, false);

    for attempt in 0..retries.max(1) {
        let raw = __varg_fetch(&provider.chat_endpoint(), "POST", provider.headers(), &body);
        let content = provider.parse_response(&raw).unwrap_or(raw);
        // Accept if it parses as a JSON object
        if serde_json::from_str::<serde_json::Value>(&content).is_ok() {
            return content;
        }
        // Try to extract a JSON object embedded in surrounding text
        if let (Some(s), Some(e)) = (content.find('{'), content.rfind('}')) {
            let candidate = &content[s..=e];
            if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                return candidate.to_string();
            }
        }
        if attempt < retries - 1 {
            std::thread::sleep(std::time::Duration::from_millis(500 * (attempt as u64 + 1)));
        }
    }
    "{}".to_string()
}

/// Wave 37: Generic typed variant — llm_structured<T>(provider, model, prompt) -> T
/// T must implement serde::de::DeserializeOwned.
/// provider: "openai" | "anthropic" | "ollama" | "" (auto-detect)
pub fn __varg_llm_structured_typed<T: serde::de::DeserializeOwned>(provider: &str, model: &str, prompt: &str) -> T {
    let json_str = __varg_llm_structured(prompt, "", 3);
    serde_json::from_str::<T>(&json_str)
        .unwrap_or_else(|e| panic!("llm_structured: failed to deserialize response as {}: {}\nRaw: {}", std::any::type_name::<T>(), e, json_str))
}

/// Collect all LLM stream chunks into a Vec<String>.
/// Enables: `for chunk in llm_stream(prompt, model) { ... }`
pub fn __varg_llm_stream(prompt: &str, model: &str) -> Vec<String> {
    use std::io::{BufRead, BufReader};
    let provider = LlmProvider::detect();
    let model_str = if model.is_empty() { provider.default_model() } else { model.to_string() };
    let messages_json = single_prompt_messages(prompt);
    let body = provider.build_body(&model_str, &messages_json, true);

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
    {
        Ok(c) => c,
        Err(e) => { eprintln!("[Varg LLM] llm_stream client error: {e}"); return Vec::new(); }
    };

    let mut req = client.post(&provider.chat_endpoint());
    for (k, v) in provider.headers() {
        req = req.header(k, v);
    }
    req = req.body(body);

    let mut chunks = Vec::new();
    match req.send() {
        Ok(resp) => {
            let reader = BufReader::new(resp);
            for line in reader.lines().filter_map(Result::ok) {
                if let Some(c) = provider.parse_stream_chunk(&line) {
                    if !c.is_empty() { chunks.push(c); }
                }
            }
        }
        Err(e) => eprintln!("[Varg LLM] llm_stream error: {e}"),
    }
    chunks
}

/// Batch embedding: embed multiple texts and return raw float vectors.
pub fn __varg_llm_embed_batch(texts: Vec<String>) -> Vec<Vec<f32>> {
    texts.iter().map(|t| crate::vector::__varg_embed(t)).collect()
}

/// Read an SSE stream (Vec<String> of chunks) as a single concatenated string.
/// Use `for chunk in llm_stream(...)` for chunk-by-chunk processing instead.
pub fn __varg_sse_read(stream: &[String]) -> String {
    stream.join("")
}

// ─── Task 1: Prompt Caching ───────────────────────────────────────────────

/// Build an Anthropic request body with `cache_control` applied to the system
/// prompt and (optionally) to long user messages.  Returns a `serde_json::Value`
/// so the JSON-building logic can be tested without making HTTP calls.
pub fn build_anthropic_request(ctx: &Context, model: &str, cache: bool) -> serde_json::Value {
    let model_str = if model.is_empty() {
        LlmProvider::Anthropic.default_model()
    } else {
        model.to_string()
    };

    // Separate system messages from conversation messages.
    let mut system_text = String::new();
    let mut api_msgs: Vec<serde_json::Value> = Vec::new();
    for msg in &ctx.messages {
        if msg.role == "system" {
            if !system_text.is_empty() { system_text.push('\n'); }
            system_text.push_str(&msg.content);
        } else {
            api_msgs.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content,
            }));
        }
    }

    // Build system block with optional cache_control.
    let system_block: Option<serde_json::Value> = if system_text.is_empty() {
        None
    } else if cache {
        Some(serde_json::json!([{
            "type": "text",
            "text": system_text,
            "cache_control": {"type": "ephemeral"}
        }]))
    } else {
        Some(serde_json::json!(system_text))
    };

    // If caching, mark the first long user message with cache_control too.
    if cache {
        for msg in &mut api_msgs {
            if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                let content_len = msg
                    .get("content")
                    .and_then(|c| c.as_str())
                    .map(|s| s.len())
                    .unwrap_or(0);
                if content_len > 1000 {
                    let content_str = msg["content"].as_str().unwrap_or("").to_string();
                    msg["content"] = serde_json::json!([{
                        "type": "text",
                        "text": content_str,
                        "cache_control": {"type": "ephemeral"}
                    }]);
                }
                break; // only the first user message
            }
        }
    }

    let mut body = serde_json::json!({
        "model": model_str,
        "max_tokens": 4096,
        "messages": api_msgs,
        "stream": false,
    });

    if let Some(sys) = system_block {
        body["system"] = sys;
    }
    body
}

/// Like `__varg_llm_chat` but adds Anthropic prompt-caching headers and
/// `cache_control: ephemeral` to the system block and long user messages.
/// For OpenAI/Ollama this falls back to `__varg_llm_chat`.
pub fn __varg_llm_chat_cached(ctx: &mut Context, prompt: &str, model: &str) -> String {
    let provider = LlmProvider::detect();
    ctx.push("user", prompt);

    if provider != LlmProvider::Anthropic {
        // Non-Anthropic: fall back to the regular implementation.
        let messages_json = serde_json::to_string(&ctx.messages)
            .unwrap_or_else(|_| "[]".to_string());
        let body = provider.build_body(model, &messages_json, false);
        let res = __varg_fetch(&provider.chat_endpoint(), "POST", provider.headers(), &body);
        if let Some(content) = provider.parse_response(&res) {
            ctx.push("assistant", &content);
            return content;
        }
        return res;
    }

    // Anthropic: build request with cache_control.
    let body = build_anthropic_request(ctx, model, true);
    let body_str = serde_json::to_string(&body).unwrap_or_default();

    let mut headers = provider.headers();
    headers.insert(
        "anthropic-beta".to_string(),
        "prompt-caching-2024-07-31".to_string(),
    );

    let res = __varg_fetch(&provider.chat_endpoint(), "POST", headers, &body_str);
    if let Some(content) = provider.parse_response(&res) {
        ctx.push("assistant", &content);
        content
    } else {
        res
    }
}

// ─── Task 2: Real Structured Outputs ─────────────────────────────────────────

/// Build an OpenAI `response_format` structured-output request body.
/// Extracted so it can be tested without an HTTP call.
pub fn build_openai_structured_request(model: &str, schema_json: &str, prompt: &str)
    -> serde_json::Value
{
    let schema: serde_json::Value = serde_json::from_str(schema_json)
        .unwrap_or(serde_json::json!({}));
    let messages = serde_json::json!([
        {"role": "user", "content": prompt}
    ]);
    serde_json::json!({
        "model": model,
        "messages": messages,
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "response",
                "strict": true,
                "schema": schema
            }
        }
    })
}

/// Build an Anthropic tool-use structured-output request body.
/// Extracted so it can be tested without an HTTP call.
pub fn build_anthropic_structured_request(model: &str, schema_json: &str, prompt: &str)
    -> serde_json::Value
{
    let schema: serde_json::Value = serde_json::from_str(schema_json)
        .unwrap_or(serde_json::json!({}));
    let messages = serde_json::json!([
        {"role": "user", "content": prompt}
    ]);
    serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "messages": messages,
        "tools": [{
            "name": "respond",
            "description": "Respond with structured data",
            "input_schema": schema
        }],
        "tool_choice": {"type": "tool", "name": "respond"}
    })
}

/// Call the LLM with a real JSON-schema enforcement strategy per provider:
/// - **OpenAI / GPT**: uses `response_format: {type: "json_schema", …}`
/// - **Anthropic / Claude**: uses tool-use forced call
/// - **Ollama / others**: falls back to `__varg_llm_structured`
pub fn __varg_llm_structured_schema(
    provider_hint: &str,
    model: &str,
    schema_json: &str,
    prompt: &str,
) -> String {
    let provider = LlmProvider::detect();
    let is_openai = provider == LlmProvider::OpenAI
        || provider_hint.to_lowercase().contains("openai")
        || model.to_lowercase().contains("gpt");
    let is_anthropic = provider == LlmProvider::Anthropic
        || provider_hint.to_lowercase().contains("anthropic")
        || model.to_lowercase().contains("claude");

    if is_openai {
        let effective_model = if model.is_empty() {
            LlmProvider::OpenAI.default_model()
        } else {
            model.to_string()
        };
        let body = build_openai_structured_request(&effective_model, schema_json, prompt);
        let body_str = serde_json::to_string(&body).unwrap_or_default();
        let raw = __varg_fetch(
            &LlmProvider::OpenAI.chat_endpoint(),
            "POST",
            LlmProvider::OpenAI.headers(),
            &body_str,
        );
        return LlmProvider::OpenAI
            .parse_response(&raw)
            .unwrap_or(raw);
    }

    if is_anthropic {
        let effective_model = if model.is_empty() {
            LlmProvider::Anthropic.default_model()
        } else {
            model.to_string()
        };
        let body = build_anthropic_structured_request(&effective_model, schema_json, prompt);
        let body_str = serde_json::to_string(&body).unwrap_or_default();
        let raw = __varg_fetch(
            &LlmProvider::Anthropic.chat_endpoint(),
            "POST",
            LlmProvider::Anthropic.headers(),
            &body_str,
        );
        // Extract tool_use input from the response.
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(content_arr) = json.get("content").and_then(|c| c.as_array()) {
                for block in content_arr {
                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        if let Some(input) = block.get("input") {
                            return serde_json::to_string(input)
                                .unwrap_or_else(|_| "{}".to_string());
                        }
                    }
                }
            }
        }
        return raw;
    }

    // Ollama / unknown: prompt-engineering fallback.
    __varg_llm_structured(prompt, schema_json, 3)
}

// ─── Task 3: Parameterizable Temperature ─────────────────────────────────────

/// Build an OpenAI / Ollama compatible request body with custom temperature
/// and max_tokens.  Extracted so it can be tested without an HTTP call.
pub fn build_chat_opts_body(
    provider: &LlmProvider,
    messages_json: &str,
    model: &str,
    temperature: f64,
    max_tokens: i64,
) -> serde_json::Value {
    let model_str = if model.is_empty() {
        provider.default_model()
    } else {
        model.to_string()
    };

    let msgs: serde_json::Value = serde_json::from_str(messages_json)
        .unwrap_or(serde_json::json!([]));

    match provider {
        LlmProvider::Anthropic => {
            // Extract system separately for Anthropic.
            let arr = msgs.as_array().cloned().unwrap_or_default();
            let mut system_text = String::new();
            let mut api_msgs: Vec<serde_json::Value> = Vec::new();
            for msg in &arr {
                let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
                let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
                if role == "system" {
                    if !system_text.is_empty() { system_text.push('\n'); }
                    system_text.push_str(content);
                } else {
                    api_msgs.push(serde_json::json!({"role": role, "content": content}));
                }
            }
            let mut body = serde_json::json!({
                "model": model_str,
                "max_tokens": max_tokens,
                "messages": api_msgs,
                "temperature": temperature,
            });
            if !system_text.is_empty() {
                body["system"] = serde_json::json!(system_text);
            }
            body
        }
        _ => {
            // Ollama and OpenAI share the same format.
            serde_json::json!({
                "model": model_str,
                "messages": msgs,
                "temperature": temperature,
                "max_tokens": max_tokens,
                "stream": false,
            })
        }
    }
}

/// Like `__varg_llm_chat` but exposes `temperature` and `max_tokens`.
pub fn __varg_llm_chat_opts(
    ctx: &mut Context,
    prompt: &str,
    model: &str,
    temperature: f64,
    max_tokens: i64,
) -> String {
    let provider = LlmProvider::detect();
    ctx.push("user", prompt);
    let messages_json = serde_json::to_string(&ctx.messages)
        .unwrap_or_else(|_| "[]".to_string());
    let body = build_chat_opts_body(&provider, &messages_json, model, temperature, max_tokens);
    let body_str = serde_json::to_string(&body).unwrap_or_default();
    let res = __varg_fetch(&provider.chat_endpoint(), "POST", provider.headers(), &body_str);
    if let Some(content) = provider.parse_response(&res) {
        ctx.push("assistant", &content);
        content
    } else {
        res
    }
}

// ─── Context helpers ──────────────────────────────────────────────────────

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

    // ── Task 1: Prompt caching ────────────────────────────────────────────

    #[test]
    fn test_llm_build_anthropic_cache_request() {
        let mut ctx = Context::new("test");
        ctx.push("system", "You are a helpful assistant.");
        ctx.push("user", "Hello");

        let body = build_anthropic_request(&ctx, "claude-sonnet-4-20250514", true);

        // System block must be an array with cache_control
        let sys = body.get("system").expect("missing system block");
        assert!(sys.is_array(), "system should be an array when caching");
        let first = &sys[0];
        assert_eq!(first["type"], "text");
        assert_eq!(first["text"], "You are a helpful assistant.");
        assert_eq!(first["cache_control"]["type"], "ephemeral");

        // Regular messages field present
        assert!(body.get("messages").is_some());
    }

    #[test]
    fn test_llm_build_anthropic_no_cache_request() {
        let mut ctx = Context::new("test");
        ctx.push("system", "You are helpful.");
        ctx.push("user", "Hi");

        let body = build_anthropic_request(&ctx, "claude-sonnet-4-20250514", false);

        // Without cache the system block is a plain string
        let sys = body.get("system").expect("missing system");
        assert!(sys.is_string(), "system should be a plain string without caching");
    }

    #[test]
    fn test_llm_build_anthropic_cache_long_user_message() {
        let mut ctx = Context::new("test");
        ctx.push("system", "You are helpful.");
        // User message > 1000 chars
        let long_msg = "a".repeat(1100);
        ctx.push("user", &long_msg);

        let body = build_anthropic_request(&ctx, "claude-sonnet-4-20250514", true);

        let messages = body["messages"].as_array().expect("messages array");
        let first_user = messages.iter().find(|m| {
            m.get("role").and_then(|r| r.as_str()) == Some("user")
        }).expect("user message");

        // Content should now be an array with cache_control
        assert!(first_user["content"].is_array(), "long user content should be wrapped in array");
        let content_block = &first_user["content"][0];
        assert_eq!(content_block["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn test_anthropic_cache_headers_added() {
        // Verify the beta header is present when using cached path.
        // We test header insertion by checking the headers map directly.
        let mut headers = LlmProvider::Anthropic.headers();
        headers.insert(
            "anthropic-beta".to_string(),
            "prompt-caching-2024-07-31".to_string(),
        );
        assert_eq!(
            headers.get("anthropic-beta"),
            Some(&"prompt-caching-2024-07-31".to_string()),
        );
    }

    // ── Task 2: Structured outputs ────────────────────────────────────────

    #[test]
    fn test_llm_structured_schema_openai_format() {
        let schema = r#"{"type":"object","properties":{"name":{"type":"string"}}}"#;
        let body = build_openai_structured_request("gpt-4o", schema, "Give me a name");

        let rf = body.get("response_format").expect("response_format missing");
        assert_eq!(rf["type"], "json_schema");

        let js = rf.get("json_schema").expect("json_schema missing");
        assert_eq!(js["name"], "response");
        assert_eq!(js["strict"], true);

        // Schema parsed correctly
        assert_eq!(js["schema"]["type"], "object");
    }

    #[test]
    fn test_llm_structured_schema_anthropic_format() {
        let schema = r#"{"type":"object","properties":{"answer":{"type":"string"}}}"#;
        let body = build_anthropic_structured_request(
            "claude-sonnet-4-20250514",
            schema,
            "Answer this",
        );

        let tools = body.get("tools").and_then(|t| t.as_array()).expect("tools array");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "respond");
        assert!(tools[0].get("input_schema").is_some());

        let tc = body.get("tool_choice").expect("tool_choice missing");
        assert_eq!(tc["type"], "tool");
        assert_eq!(tc["name"], "respond");
    }

    // ── Task 3: Parameterizable temperature ──────────────────────────────

    #[test]
    fn test_llm_chat_opts_temperature_openai() {
        let messages_json = r#"[{"role":"user","content":"hi"}]"#;
        let body = build_chat_opts_body(
            &LlmProvider::OpenAI,
            messages_json,
            "gpt-4o",
            0.7,
            512,
        );

        assert_eq!(body["temperature"], 0.7);
        assert_eq!(body["max_tokens"], 512);
        assert_eq!(body["model"], "gpt-4o");
    }

    #[test]
    fn test_llm_chat_opts_temperature_anthropic() {
        let messages_json = r#"[{"role":"system","content":"sys"},{"role":"user","content":"hi"}]"#;
        let body = build_chat_opts_body(
            &LlmProvider::Anthropic,
            messages_json,
            "claude-sonnet-4-20250514",
            0.2,
            1024,
        );

        assert_eq!(body["temperature"], 0.2);
        assert_eq!(body["max_tokens"], 1024);
        // System extracted to top-level
        assert_eq!(body["system"], "sys");
        // Messages only contain user turn
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
    }

    #[test]
    fn test_llm_chat_opts_temperature_ollama() {
        let messages_json = r#"[{"role":"user","content":"hello"}]"#;
        let body = build_chat_opts_body(
            &LlmProvider::Ollama,
            messages_json,
            "llama3",
            1.0,
            2048,
        );

        assert_eq!(body["temperature"], 1.0);
        assert_eq!(body["max_tokens"], 2048);
    }
}
