use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::Manager;

use crate::AppState;

const LLAMA_RELEASE: &str = "b10066";
const LLAMA_ZIP_URL: &str =
    "https://github.com/ggml-org/llama.cpp/releases/download/b10066/llama-b10066-bin-win-cpu-x64.zip";

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub running: bool,
    pub model_path: Option<String>,
    pub port: Option<u16>,
    pub measured_tps: Option<f64>,
    pub backend: Option<String>,
    pub embed_model: Option<String>,
    pub embed_port: Option<u16>,
}

pub struct EngineState {
    pub child: Mutex<Option<Child>>,
    pub status: Mutex<EngineStatus>,
    pub embed_child: Mutex<Option<Child>>,
    pub start_in_progress: AtomicBool,
    pub embed_dim: Mutex<Option<usize>>,
}

fn engine_dir() -> std::path::PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    dir.push("ragit");
    dir.push("engine");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn server_bin() -> std::path::PathBuf {
    engine_dir().join("llama-server.exe")
}

fn models_dir() -> std::path::PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    dir.push("ragit");
    dir.push("models");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn resolve_model(path: &str) -> std::path::PathBuf {
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);
    models_dir().join(name)
}

fn pick_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .map(|l| l.local_addr().map(|a| a.port()).unwrap_or(11435))
        .unwrap_or(11435)
}

pub fn ensure_binary() -> Result<(), String> {
    if server_bin().exists() {
        return Ok(());
    }
    let zip = engine_dir().join("llama.zip");
    let client = reqwest::blocking::ClientBuilder::new()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let bytes = client
        .get(LLAMA_ZIP_URL)
        .send()
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    std::fs::write(&zip, &bytes).map_err(|e| e.to_string())?;

    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;
    let mut found = false;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(|e| e.to_string())?;
        if f.name().eq_ignore_ascii_case("llama-server.exe") {
            let mut out = std::fs::File::create(server_bin()).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
            found = true;
            break;
        }
    }
    let _ = std::fs::remove_file(&zip);
    if found { Ok(()) } else { Err("llama-server.exe not found in archive".into()) }
}

fn is_listening(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &([127, 0, 0, 1], port).into(),
        Duration::from_secs(2),
    )
    .is_ok()
}

fn measure_speed(app: &tauri::AppHandle, port: u16) -> Option<f64> {
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let client = reqwest::blocking::ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .build()
        .ok()?;
    let body = serde_json::json!({
        "model": "ragit-model",
        "messages": [{"role": "user", "content": "Say hello in one word."}],
        "max_tokens": 8,
        "temperature": 0.0,
    });
    let resp = client.post(&url).json(&body).send().ok()?;
    let json: serde_json::Value = resp.json().ok()?;
    let text = json
        .pointer("/choices/0/message/content")?
        .as_str()
        .unwrap_or("")
        .to_string();
    let tps = if text.is_empty() { 0.0 } else { (text.len() as f64) / 4.0 };
    let est = app.state::<Arc<AppState>>();
    let mut st = est.engine.status.lock().ok()?;
    st.measured_tps = Some(tps);
    st.backend = Some("llama.cpp".into());
    Some(tps)
}

pub fn probe_embed_dim(port: u16) -> Result<usize, String> {
    let url = format!("http://127.0.0.1:{port}/v1/embeddings");
    let client = reqwest::blocking::ClientBuilder::new()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let body = serde_json::json!({
        "model": "ragit-model",
        "input": ["probe"],
        "encoding_format": "base64",
    });
    let resp = client.post(&url).json(&body).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("dimension probe HTTP {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    let b64 = json
        .pointer("/data/0/embedding")
        .and_then(|v| v.as_str())
        .ok_or("no embedding in probe response")?;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    let bytes = STANDARD.decode(b64).map_err(|e| e.to_string())?;
    if bytes.len() % 4 != 0 {
        return Err("probe embedding bytes not aligned to f32".into());
    }
    Ok(bytes.len() / 4)
}

#[tauri::command]
pub fn start_engine(
    app: tauri::AppHandle,
    model_path: String,
    port: Option<u16>,
    gpu_layers: Option<i32>,
    embed_model_path: Option<String>,
    embed_port: Option<u16>,
) -> Result<EngineStatus, String> {
    let state = app.state::<Arc<AppState>>();
    let engine = &state.engine;

    {
        let st = engine.status.lock().map_err(|e| e.to_string())?;
        if st.running {
            return Ok(st.clone());
        }
    }

    if engine.start_in_progress.swap(true, Ordering::SeqCst) {
        return Err("Engine start already in progress".into());
    }

    let result = (|| -> Result<EngineStatus, String> {
        let requested = port.unwrap_or_else(pick_free_port);
        let (child, actual_port) = spawn_server(&model_path, requested, gpu_layers.unwrap_or(-1), "ragit-model", true, 8192)?;

        let dim = probe_embed_dim(actual_port).ok();

        {
            let mut st = engine.status.lock().map_err(|e| e.to_string())?;
            st.running = true;
            st.model_path = Some(model_path.clone());
            st.port = Some(actual_port);
        }
        *engine.child.lock().map_err(|e| e.to_string())? = Some(child);
        if let Some(d) = dim {
            *engine.embed_dim.lock().map_err(|e| e.to_string())? = Some(d);
        }

        if let Some(emb) = embed_model_path {
            let eport = embed_port.unwrap_or_else(pick_free_port);
            match spawn_server(&emb, eport, gpu_layers.unwrap_or(-1), "ragit-embed", true, 8192) {
                Ok((echild, actual_ep)) => {
                    let edim = probe_embed_dim(actual_ep).ok();
                    {
                        let mut st = engine.status.lock().map_err(|e| e.to_string())?;
                        st.embed_model = Some(emb);
                        st.embed_port = Some(actual_ep);
                    }
                    if let Some(d) = edim {
                        *engine.embed_dim.lock().map_err(|e| e.to_string())? = Some(d);
                    }
                    *engine.embed_child.lock().map_err(|e| e.to_string())? = Some(echild);
                }
                Err(e) => {
                    let mut st = engine.status.lock().map_err(|e| e.to_string())?;
                    st.backend = Some(format!("chat up; embed failed: {e}"));
                }
            }
        }

        // Measure speed in background (after brief warm-up).
        let app2 = app.clone();
        let p = actual_port;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(800));
            let _ = measure_speed(&app2, p);
        });

        // Health probe — checks every 3s; kills state if engine disappears.
        let app3 = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(5));
            loop {
                std::thread::sleep(Duration::from_secs(3));
                let state = app3.state::<Arc<AppState>>();
                let dead = {
                    let mut st = match state.engine.status.lock() {
                        Ok(s) => s,
                        Err(_) => { eprintln!("engine mutex poisoned"); break; }
                    };
                    if !st.running {
                        true
                    } else if let Some(p) = st.port {
                        if !is_listening(p) {
                            st.running = false;
                            st.port = None;
                            st.measured_tps = None;
                            st.backend = Some("engine crashed".into());
                            let _ = state.engine.child.lock().ok().and_then(|mut c| c.take());
                            let _ = state.engine.embed_child.lock().ok().and_then(|mut c| c.take());
                            true
                        } else {
                            false
                        }
                    } else {
                        true
                    }
                };
                if dead {
                    break;
                }
            }
        });

        let st = engine.status.lock().map_err(|e| e.to_string())?;
        Ok(st.clone())
    })();

    engine.start_in_progress.store(false, Ordering::SeqCst);
    result
}

#[tauri::command]
pub fn stop_engine(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<Arc<AppState>>();
    let engine = &state.engine;
    if let Some(mut child) = engine.child.lock().ok().and_then(|mut c| c.take()) {
        let _ = child.kill();
        let _ = child.wait();
    }
    if let Some(mut child) = engine.embed_child.lock().ok().and_then(|mut c| c.take()) {
        let _ = child.kill();
        let _ = child.wait();
    }
    let mut st = engine.status.lock().map_err(|e| e.to_string())?;
    st.running = false;
    st.port = None;
    st.model_path = None;
    st.measured_tps = None;
    st.backend = None;
    st.embed_model = None;
    st.embed_port = None;
    Ok(())
}

#[tauri::command]
pub fn engine_status(app: tauri::AppHandle) -> EngineStatus {
    let state = app.state::<Arc<AppState>>();
    state.engine.status.lock().ok().map(|s| s.clone()).unwrap_or(EngineStatus {
        running: false, model_path: None, port: None,
        measured_tps: None, backend: None, embed_model: None, embed_port: None,
    })
}

pub fn with_port_from(state: &crate::AppState) -> Result<u16, String> {
    let st = state.engine.status.lock().map_err(|e| e.to_string())?;
    st.port.ok_or("engine not running".into())
}

pub fn embed_port_from(state: &crate::AppState) -> Option<u16> {
    let st = state.engine.status.lock().ok()?;
    st.embed_port
}

pub fn embed_dim_from(state: &crate::AppState) -> Option<usize> {
    let d = state.engine.embed_dim.lock().ok()?;
    *d
}

/// Run a closure with the current engine port. Errors if engine not running.
pub fn with_port<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce(u16) -> Result<T, String>,
{
    use crate::APP_STATE;
    let state = APP_STATE.get().ok_or("app not initialized")?;
    let port = with_port_from(state)?;
    f(port)
}

/// Port of the dedicated embedding engine (no fallback to chat model).
pub fn embed_port() -> Option<u16> {
    let state = crate::APP_STATE.get()?;
    embed_port_from(state)
}

/// Stored embedding dimension, probed on engine start.
pub fn embed_dim() -> Option<usize> {
    let state = crate::APP_STATE.get()?;
    embed_dim_from(state)
}

/// Spawn a llama-server instance; retries nearby ports on conflict.
fn spawn_server(
    model_path: &str,
    port: u16,
    gpu_layers: i32,
    alias: &str,
    embedding: bool,
    ctx: usize,
) -> Result<(Child, u16), String> {
    ensure_binary()?;
    let bin = server_bin();
    let resolved = resolve_model(model_path);
    if !resolved.exists() {
        return Err(format!("model not found: {}", resolved.display()));
    }

    for attempt in 0..10 {
        let p = port + attempt;
        let mut command = Command::new(&bin);
        command
            .arg("-m").arg(&resolved)
            .arg("--port").arg(p.to_string())
            .arg("--host").arg("127.0.0.1")
            .arg("-ngl").arg(gpu_layers.to_string())
            .arg("-c").arg(ctx.to_string())
            .arg("--alias").arg(alias);
        if embedding {
            command.arg("--embedding");
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                if attempt < 9 {
                    continue;
                }
                return Err(format!("failed to start engine: {e}"));
            }
        };

        for _ in 0..40 {
            if is_listening(p) {
                return Ok((child, p));
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        // Didn't start in time on this port; kill and try next.
        let _ = child.kill();
        let _ = child.wait();
    }
    Err("engine did not listen on any port within time".into())
}

pub fn init_state() -> EngineState {
    EngineState {
        child: Mutex::new(None),
        embed_child: Mutex::new(None),
        status: Mutex::new(EngineStatus {
            running: false,
            model_path: None,
            port: None,
            measured_tps: None,
            backend: None,
            embed_model: None,
            embed_port: None,
        }),
        start_in_progress: AtomicBool::new(false),
        embed_dim: Mutex::new(None),
    }
}
