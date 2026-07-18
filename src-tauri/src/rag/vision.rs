use std::path::Path;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use crate::engine;

/// Describe an image using a vision-capable model loaded in the engine (mmproj).
/// Returns None if the engine is not running or the model lacks vision.
pub fn describe_image(path: &Path) -> Option<String> {
    let port = engine::with_port(|p| Ok(p)).ok()?;
    let bytes = std::fs::read(path).ok()?;
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
    let resp = client.post(&url).json(&payload).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().ok()?;
    json.pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Transcribe audio/video using a locally-installed whisper.cpp (`whisper-cli`).
/// Returns None if the binary is not on PATH (graceful degradation).
pub fn transcribe_media(path: &Path) -> Option<String> {
    let out = std::process::Command::new("whisper-cli")
        .arg("-f")
        .arg(path)
        .arg("--no-prints")
        .arg("-otxt")
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        None
    }
}

/// Extract frames from a video using ffmpeg (if installed) and describe them.
/// Falls back to None when ffmpeg is unavailable.
pub fn describe_video_frames(path: &Path) -> Option<String> {
    let tmp = std::env::temp_dir().join(format!("ragit_frames_{}.png", uuid::Uuid::new_v4()));
    let frame_pattern = tmp.to_string_lossy().to_string();
    let status = std::process::Command::new("ffmpeg")
        .arg("-i")
        .arg(path)
        .arg("-vf")
        .arg("fps=1/5") // one frame every 5 seconds
        .arg(&frame_pattern)
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    let mut descriptions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(tmp.parent().unwrap()) {
        for e in entries.flatten() {
            let p = e.path();
            if let Some(d) = describe_image(&p) {
                descriptions.push(d);
            }
        }
    }
    let _ = std::fs::remove_dir_all(tmp.parent().unwrap());
    if descriptions.is_empty() {
        None
    } else {
        Some(descriptions.join("\n\n"))
    }
}
