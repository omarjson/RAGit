use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use serde::Serialize;
use tauri::Manager;

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

/// Resolve a model path that may be a bare filename or "models/..." prefix
/// against the real models directory.
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

/// Ensure the llama-server binary exists, downloading + extracting on first use.
pub fn ensure_binary() -> Result<(), String> {
    if server_bin().exists() {
        return Ok(());
    }
    let zip = engine_dir().join("llama.zip");
    let client = reqwest::blocking::Client::new();
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
    if found {
        Ok(())
    } else {
        Err("llama-server.exe not found in archive".into())
    }
}

fn is_listening(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

/// Probe tokens/sec by issuing a tiny completion and timing it.
fn measure_speed(app: &tauri::AppHandle, port: u16) -> Option<f64> {
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let client = reqwest::blocking::Client::new();
    let body = serde_json::json!({
        "model": "ragit-model",
        "messages": [{"role": "user", "content": "Say hello in one word."}],
        "max_tokens": 8,
        "temperature": 0.0,
    });
    let resp = client.post(&url).json(&body).send().ok()?;
    let json: serde_json::Value = resp.json().ok()?;
    let text = json
        .get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?
        .as_str()
        .unwrap_or("")
        .to_string();
    // Coarse estimate: ~4 chars/token.
    let tps = if text.is_empty() { 0.0 } else { (text.len() as f64) / 4.0 };
    let est = app.state::<EngineState>();
    let mut st = est.status.lock().unwrap();
    st.measured_tps = Some(tps);
    st.backend = Some("llama.cpp".into());
    Some(tps)
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
    let state = app.state::<EngineState>();
    {
        let st = state.status.lock().unwrap();
        if st.running {
            return Ok(st.clone());
        }
    }

    let port = port.unwrap_or_else(pick_free_port);
    let (child, port) = spawn_server(&model_path, port, gpu_layers.unwrap_or(-1), "ragit-model", true, 8192)?;
    {
        let mut st = state.status.lock().unwrap();
        st.running = true;
        st.model_path = Some(model_path);
        st.port = Some(port);
    }
    *state.child.lock().unwrap() = Some(child);

    // Optionally launch a dedicated embedding engine on its own port.
    if let Some(emb) = embed_model_path {
        let eport = embed_port.unwrap_or_else(pick_free_port);
        match spawn_server(&emb, eport, gpu_layers.unwrap_or(-1), "ragit-embed", true, 8192) {
            Ok((echild, eport)) => {
                let mut st = state.status.lock().unwrap();
                st.embed_model = Some(emb);
                st.embed_port = Some(eport);
                *state.embed_child.lock().unwrap() = Some(echild);
            }
            Err(e) => {
                let mut st = state.status.lock().unwrap();
                st.backend = Some(format!("chat up; embed failed: {e}"));
            }
        }
    }

    // Measure speed in background.
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(800));
        let _ = measure_speed(&app2, port);
    });

    let st = state.status.lock().unwrap();
    Ok(st.clone())
}

#[tauri::command]
pub fn stop_engine(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<EngineState>();
    if let Some(mut child) = state.child.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    if let Some(mut child) = state.embed_child.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let mut st = state.status.lock().unwrap();
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
    let state = app.state::<EngineState>();
    let st = state.status.lock().unwrap();
    let snapshot = st.clone();
    snapshot
}

/// Run a closure with the current engine port. Errors if engine not running.
pub fn with_port<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce(u16) -> Result<T, String>,
{
    use crate::APP_STATE;
    let state = unsafe { APP_STATE.as_ref() }.ok_or("app not initialized")?;
    let st = state.engine.status.lock().unwrap();
    let port = st.port.ok_or("engine not running")?;
    f(port)
}

/// Port to use for embeddings: a dedicated embed engine if running, else the
/// chat engine (which also has `--embedding` enabled).
pub fn embed_port() -> Option<u16> {
    use crate::APP_STATE;
    let state = unsafe { APP_STATE.as_ref() }?;
    let st = state.engine.status.lock().unwrap();
    st.embed_port.or(st.port)
}

/// Spawn a llama-server instance; returns the child and the port it listens on.
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
    let mut command = Command::new(&bin);
    command
        .arg("-m")
        .arg(resolved)
        .arg("--port")
        .arg(port.to_string())
        .arg("--host")
        .arg("127.0.0.1")
        .arg("-ngl")
        .arg(gpu_layers.to_string())
        .arg("-c")
        .arg(ctx.to_string())
        .arg("--alias")
        .arg(alias);
    if embedding {
        command.arg("--embedding");
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = command
        .spawn()
        .map_err(|e| format!("failed to start engine: {e}"))?;
    // Wait until it listens.
    for _ in 0..40 {
        if is_listening(port) {
            return Ok((child, port));
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Err(format!("engine on port {port} did not start in time"))
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
    }
}
