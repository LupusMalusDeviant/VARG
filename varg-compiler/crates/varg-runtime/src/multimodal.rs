// Wave 34: Multi-Modal Types — Image and Audio primitives

use base64::{Engine as _, engine::general_purpose::STANDARD};

// ── Image ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VargImage {
    pub path: String,
    pub data: Vec<u8>,
    pub format: String,
}

pub fn __varg_image_load(path: &str) -> VargImage {
    let data = std::fs::read(path).unwrap_or_default();
    let format = path.rsplit('.').next().unwrap_or("").to_lowercase();
    VargImage { path: path.to_string(), data, format }
}

pub fn __varg_image_from_base64(b64: &str, format: &str) -> VargImage {
    let data = STANDARD.decode(b64).unwrap_or_default();
    VargImage { path: String::new(), data, format: format.to_string() }
}

pub fn __varg_image_to_base64(img: &VargImage) -> String {
    STANDARD.encode(&img.data)
}

pub fn __varg_image_format(img: &VargImage) -> String {
    img.format.clone()
}

pub fn __varg_image_size_bytes(img: &VargImage) -> i64 {
    img.data.len() as i64
}

// ── Audio ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VargAudio {
    pub path: String,
    pub data: Vec<u8>,
    pub format: String,
}

pub fn __varg_audio_load(path: &str) -> VargAudio {
    let data = std::fs::read(path).unwrap_or_default();
    let format = path.rsplit('.').next().unwrap_or("").to_lowercase();
    VargAudio { path: path.to_string(), data, format }
}

pub fn __varg_audio_to_base64(audio: &VargAudio) -> String {
    STANDARD.encode(&audio.data)
}

pub fn __varg_audio_format(audio: &VargAudio) -> String {
    audio.format.clone()
}

pub fn __varg_audio_size_bytes(audio: &VargAudio) -> i64 {
    audio.data.len() as i64
}

// ── Vision LLM ────────────────────────────────────────────────────────────

/// Call a vision-capable LLM with an image + text prompt.
pub fn __varg_llm_vision(img: &VargImage, prompt: &str, model: &str) -> String {
    use crate::net::__varg_fetch;
    use crate::llm::LlmProvider;

    let b64 = __varg_image_to_base64(img);
    let mime = match img.format.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png"          => "image/png",
        "gif"          => "image/gif",
        "webp"         => "image/webp",
        _              => "image/jpeg",
    };

    let provider = LlmProvider::detect();
    let model_name = if model.is_empty() { provider.default_model() } else { model.to_string() };
    let headers = provider.headers();
    let url = provider.chat_endpoint();

    let body = match provider {
        LlmProvider::OpenAI => serde_json::json!({
            "model": model_name,
            "messages": [{"role":"user","content":[
                {"type":"text","text":prompt},
                {"type":"image_url","image_url":{"url":format!("data:{mime};base64,{b64}")}}
            ]}]
        }),
        LlmProvider::Anthropic => serde_json::json!({
            "model": model_name,
            "max_tokens": 1024,
            "messages": [{"role":"user","content":[
                {"type":"image","source":{"type":"base64","media_type":mime,"data":b64}},
                {"type":"text","text":prompt}
            ]}]
        }),
        _ => serde_json::json!({
            "model": model_name,
            "messages": [{"role":"user","content":format!("[Image:{mime}] {prompt}")}]
        }),
    }
    .to_string();

    __varg_fetch(&url, "POST", headers, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_from_base64_roundtrip() {
        let original = "SGVsbG8gV29ybGQ="; // "Hello World"
        let img = __varg_image_from_base64(original, "png");
        assert_eq!(__varg_image_to_base64(&img), original);
    }

    #[test]
    fn test_image_format() {
        let img = __varg_image_from_base64("SGU=", "jpeg");
        assert_eq!(__varg_image_format(&img), "jpeg");
    }

    #[test]
    fn test_image_size_bytes() {
        let img = __varg_image_from_base64("SGVsbG8=", "png"); // 5 bytes
        assert_eq!(__varg_image_size_bytes(&img), 5);
    }

    #[test]
    fn test_image_load_missing_path() {
        let img = __varg_image_load("/nonexistent/file.png");
        assert!(img.data.is_empty());
    }

    #[test]
    fn test_audio_load_missing_path() {
        let audio = __varg_audio_load("/nonexistent/audio.mp3");
        assert!(audio.data.is_empty());
        assert_eq!(__varg_audio_format(&audio), "mp3");
    }

    #[test]
    fn test_audio_to_base64() {
        let mut audio = VargAudio { path: String::new(), data: b"RIFF".to_vec(), format: "wav".to_string() };
        let b64 = __varg_audio_to_base64(&audio);
        assert!(!b64.is_empty());
        audio.data.clear();
        assert_eq!(__varg_audio_size_bytes(&audio), 0);
    }
}
