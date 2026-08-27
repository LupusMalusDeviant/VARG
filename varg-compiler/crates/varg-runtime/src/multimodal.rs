// Wave 34: Multi-Modal Types — Image and Audio primitives

use base64::{Engine as _, engine::general_purpose::STANDARD};

// ── Image ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VargImage {
    pub path: String,
    pub data: Vec<u8>,
    pub format: String,
}

/// Fallible: a path that cannot be read used to yield an *empty* VargImage with the format guessed
/// from the extension, so a typo in a filename produced a zero-byte image that looked valid
/// all the way to the vision model.
pub fn __varg_image_load(path: &str) -> Result<VargImage, String> {
    let data = std::fs::read(path)
        .map_err(|e| format!("cannot read `{}`: {}", path, e))?;
    let format = path.rsplit('.').next().unwrap_or("").to_lowercase();
    Ok(VargImage { path: path.to_string(), data, format })
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

/// Fallible: a path that cannot be read used to yield an *empty* VargAudio with the format guessed
/// from the extension, so a typo in a filename produced a zero-byte image that looked valid
/// all the way to the vision model.
pub fn __varg_audio_load(path: &str) -> Result<VargAudio, String> {
    let data = std::fs::read(path)
        .map_err(|e| format!("cannot read `{}`: {}", path, e))?;
    let format = path.rsplit('.').next().unwrap_or("").to_lowercase();
    Ok(VargAudio { path: path.to_string(), data, format })
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

    // Returns a bare String, so a transport failure arrives as the reply text — the shape
    // `__varg_fetch` itself used to have. See `fetch_or_error_text` in llm.rs.
    match __varg_fetch(&url, "POST", headers, &body) {
        Ok(text) => text,
        Err(e) => e,
    }
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

    /// A path that cannot be read is an error, not an empty file. These used to assert the
    /// opposite — that a missing image silently produced a zero-byte one — which is how a typo
    /// in a filename could reach a vision model looking like a valid image.
    #[test]
    fn test_image_load_missing_path_is_an_error() {
        let err = __varg_image_load("/nonexistent/file.png").unwrap_err();
        assert!(err.contains("/nonexistent/file.png"), "the message should name the path: {}", err);
    }

    #[test]
    fn test_audio_load_missing_path_is_an_error() {
        assert!(__varg_audio_load("/nonexistent/audio.mp3").is_err());
    }

    #[test]
    fn test_image_load_reads_a_real_file() {
        let dir = std::env::temp_dir().join("varg_multimodal_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("probe.png");
        std::fs::write(&path, b"pretend-png").unwrap();
        let img = __varg_image_load(path.to_str().unwrap()).expect("a readable file must load");
        assert_eq!(__varg_image_size_bytes(&img), 11);
        assert_eq!(__varg_image_format(&img), "png");
        let _ = std::fs::remove_file(&path);
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
