use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{ipc::Channel, AppHandle, Manager};

use crate::AppState;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChatEvent {
    Start { model: String },
    Token { text: String },
    Done { tokens: usize },
    Error { message: String },
}

#[derive(serde::Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[tauri::command]
pub fn chat_stream(
    app: AppHandle,
    messages: Vec<ChatMessage>,
    on_event: Channel<ChatEvent>,
) -> Result<(), String> {
    let state = app.state::<Arc<AppState>>();
    let port = {
        let st = state.engine.status.lock().map_err(|e| e.to_string())?;
        if !st.running {
            let _ = on_event.send(ChatEvent::Error {
                message: "Engine not running".into(),
            });
            return Err("engine not running".into());
        }
        st.port.unwrap_or(11435)
    };

    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let payload = serde_json::json!({
        "model": "ragit-model",
        "messages": messages
            .iter()
            .map(|m| serde_json::json!({"role": m.role, "content": m.content}))
            .collect::<Vec<_>>(),
        "stream": true,
        "temperature": 0.7,
        "max_tokens": 2048,
    });

    let client = reqwest::blocking::ClientBuilder::new()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = match client.post(&url).json(&payload).send() {
        Ok(r) => r,
        Err(e) => {
            let _ = on_event.send(ChatEvent::Error {
                message: e.to_string(),
            });
            return Err(e.to_string());
        }
    };

    if !resp.status().is_success() {
        let msg = format!("chat HTTP {}", resp.status());
        let _ = on_event.send(ChatEvent::Error { message: msg.clone() });
        return Err(msg);
    }

    let _ = on_event.send(ChatEvent::Start {
        model: "ragit-model".into(),
    });

    let body = resp.text().map_err(|e| {
        let msg = format!("failed to read stream: {e}");
        let _ = on_event.send(ChatEvent::Error { message: msg.clone() });
        msg
    })?;

    let mut tokens = 0usize;
    for line in body.lines() {
        let line = line.trim();
        if !line.starts_with("data:") {
            continue;
        }
        let data = line.trim_start_matches("data:").trim();
        if data == "[DONE]" {
            break;
        }
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
            if let Some(delta) = json
                .pointer("/choices/0/delta/content")
                .and_then(|v| v.as_str())
            {
                if !delta.is_empty() {
                    tokens += 1;
                    let _ = on_event.send(ChatEvent::Token {
                        text: delta.to_string(),
                    });
                }
            }
        }
    }

    let _ = on_event.send(ChatEvent::Done { tokens });
    Ok(())
}

/// Non-streaming completion, used by RAG commands.
pub fn complete_blocking(port: u16, messages: &[ChatMessage]) -> Result<String, String> {
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let payload = serde_json::json!({
        "model": "ragit-model",
        "messages": messages
            .iter()
            .map(|m| serde_json::json!({"role": m.role, "content": m.content}))
            .collect::<Vec<_>>(),
        "stream": false,
        "temperature": 0.7,
        "max_tokens": 2048,
    });
    let client = reqwest::blocking::ClientBuilder::new()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(&url)
        .json(&payload)
        .send()
        .map_err(|e| format!("completion request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("completion HTTP {status}: {text}"));
    }
    let json: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    json
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "completion response missing content".to_string())
}
