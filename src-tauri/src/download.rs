use std::io::Write;
use std::path::PathBuf;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{ipc::Channel, AppHandle};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadEvent {
    Started { url: String, total_bytes: u64 },
    Progress { downloaded: u64, total_bytes: u64 },
    Verified { sha256: String },
    Finished { path: String },
    Error { message: String },
}

fn models_dir() -> PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("ragit");
    dir.push("models");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// Resolve and download a GGUF file from Hugging Face.
/// `repo` e.g. "Qwen/Qwen2.5-7B-Instruct-GGUF"
/// `filename` e.g. "Qwen2.5-7B-Instruct-Q4_K_M.gguf"
#[tauri::command]
pub fn download_model(
    app: AppHandle,
    repo: String,
    filename: String,
    expected_sha256: Option<String>,
    on_event: Channel<DownloadEvent>,
) {
    let url = format!(
        "https://huggingface.co/{}/resolve/main/{}",
        repo, filename
    );

    let dest = models_dir().join(&filename);
    // Prevent path traversal: only keep the final component.
    let dest = match dest.file_name() {
        Some(name) => models_dir().join(name),
        None => {
            let _ = on_event.send(DownloadEvent::Error {
                message: "Invalid filename".into(),
            });
            return;
        }
    };

    let client = reqwest::blocking::Client::builder()
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new());

    let mut resp = match client.get(&url).send() {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            let _ = on_event.send(DownloadEvent::Error {
                message: format!("HTTP {}", r.status()),
            });
            return;
        }
        Err(e) => {
            let _ = on_event.send(DownloadEvent::Error {
                message: e.to_string(),
            });
            return;
        }
    };

    let total = resp.content_length().unwrap_or(0);
    let _ = on_event.send(DownloadEvent::Started {
        url: url.clone(),
        total_bytes: total,
    });

    let mut hasher = Sha256::new();

    let mut file = match std::fs::File::create(&dest) {
        Ok(f) => f,
        Err(e) => {
            let _ = on_event.send(DownloadEvent::Error {
                message: e.to_string(),
            });
            return;
        }
    };

    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = match std::io::Read::read(&mut resp, &mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                let _ = on_event.send(DownloadEvent::Error {
                    message: e.to_string(),
                });
                let _ = std::fs::remove_file(&dest);
                return;
            }
        };
        let chunk = &buf[..n];
        hasher.update(chunk);
        if let Err(e) = file.write_all(chunk) {
            let _ = on_event.send(DownloadEvent::Error {
                message: e.to_string(),
            });
            let _ = std::fs::remove_file(&dest);
            return;
        }
        downloaded += n as u64;
        if total > 0 {
            let _ = on_event.send(DownloadEvent::Progress {
                downloaded,
                total_bytes: total,
            });
        }
    }

    let sha = format!("{:x}", hasher.finalize());
    if let Some(exp) = expected_sha256 {
        if exp.to_lowercase() != sha {
            let _ = std::fs::remove_file(&dest);
            let _ = on_event.send(DownloadEvent::Error {
                message: "SHA256 mismatch — file removed".into(),
            });
            return;
        }
    }

    let _ = on_event.send(DownloadEvent::Verified { sha256: sha });
    let _ = on_event.send(DownloadEvent::Finished {
        path: dest.to_string_lossy().to_string(),
    });
    let _ = app;
}

#[allow(dead_code)]
pub fn model_path(filename: &str) -> Option<PathBuf> {
    let p = models_dir().join(filename);
    if p.exists() { Some(p) } else { None }
}
