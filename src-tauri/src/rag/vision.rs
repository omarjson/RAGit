use std::path::Path;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use crate::engine;

/// Describe an image using a vision-capable model loaded in the engine (mmproj).
pub fn describe_image(path: &Path) -> Result<String, String> {
    let port = engine::with_port(|p| Ok(p))?;
    let bytes = std::fs::read(path).map_err(|e| format!("read image: {e}"))?;
    let b64 = B64.encode(bytes);
    let mime = match path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "image/png",
    };
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let client = reqwest::blocking::Client::new();
    let payload = serde_json::json!({
        "model": "ragit-model",
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": "Describe this image in detail for retrieval. Mention any text, objects, charts, and context." },
                { "type": "image_url", "image_url": { "url": format!("data:{mime};base64,{b64}") } }
            ]
        }],
        "stream": false,
        "max_tokens": 512
    });
    let resp = client.post(&url).json(&payload).send().map_err(|e| format!("vision request: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("vision HTTP {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().map_err(|e| format!("vision JSON: {e}"))?;
    json.pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "vision: no content in response".into())
}

/// Transcribe audio/video using a locally-installed whisper.cpp (`whisper-cli`).
pub fn transcribe_media(path: &Path) -> Result<String, String> {
    let out = std::process::Command::new("whisper-cli")
        .arg("-f")
        .arg(path)
        .arg("--no-prints")
        .arg("-otxt")
        .output()
        .map_err(|e| format!("whisper-cli not found or failed: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(format!("whisper-cli error: {stderr}"))
    }
}

/// Extract frames from a video using ffmpeg (if installed) and describe them.
pub fn describe_video_frames(path: &Path) -> Result<String, String> {
    let tmp = std::env::temp_dir().join(format!("ragit_frames_{}.png", uuid::Uuid::new_v4()));
    let frame_pattern = tmp.to_string_lossy().to_string();
    let status = std::process::Command::new("ffmpeg")
        .arg("-i")
        .arg(path)
        .arg("-vf")
        .arg("fps=1/5")
        .arg(&frame_pattern)
        .status()
        .map_err(|e| format!("ffmpeg not found or failed: {e}"))?;
    if !status.success() {
        return Err("ffmpeg frame extraction failed".into());
    }
    let parent = tmp.parent().ok_or("invalid temp path")?;
    let mut descriptions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(parent) {
        for e in entries.flatten() {
            let p = e.path();
            match describe_image(&p) {
                Ok(d) => descriptions.push(d),
                Err(e) => eprintln!("frame describe failed: {e}"),
            }
        }
    }
    let _ = std::fs::remove_dir_all(parent);
    if descriptions.is_empty() {
        Err("no frames could be described".into())
    } else {
        Ok(descriptions.join("\n\n"))
    }
}
