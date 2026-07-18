use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{ipc::Channel, AppHandle, Manager};

use crate::chat;
use crate::engine;
use crate::rag::embed;
use crate::rag::parse::{self, Level};
use crate::rag::store::{self, Store};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgress {
    pub library_id: String,
    pub total: usize,
    pub processed: usize,
    pub current_file: String,
    pub level: u8,
    pub status: String, // "running" | "paused" | "done" | "canceled" | "error"
    pub message: String,
}

struct JobControl {
    paused: AtomicBool,
    canceled: AtomicBool,
}

pub struct IndexerState {
    jobs: Mutex<HashMap<String, Arc<JobControl>>>,
}

impl IndexerState {
    pub fn new() -> IndexerState {
        IndexerState {
            jobs: Mutex::new(HashMap::new()),
        }
    }
}

#[tauri::command]
pub fn index_library(
    app: AppHandle,
    path: String,
    library_id: Option<String>,
    level: Option<u8>,
    on_progress: Channel<IndexProgress>,
) -> Result<String, String> {
    let lib = library_id.unwrap_or_else(|| "default".to_string());
    let level = level.unwrap_or(4).clamp(1, 5);
    let store = app.state::<Store>();
    store.add_library(&lib, &lib).map_err(|e| e.to_string())?;

    let ctrl = Arc::new(JobControl {
        paused: AtomicBool::new(false),
        canceled: AtomicBool::new(false),
    });
    app.state::<IndexerState>()
        .jobs
        .lock()
        .unwrap()
        .insert(lib.clone(), ctrl.clone());

    let files = parse::collect_files(std::path::Path::new(&path));
    let total = files.len();
    let files_clone = files.clone();
    let lib_c = lib.clone();
    let ctrl_c = ctrl.clone();
    let on_progress_c = on_progress.clone();

    std::thread::spawn(move || {
        let store = unsafe { crate::APP_STATE.as_ref() }.expect("app state").store;
        let mut processed = 0usize;
        for f in &files_clone {
            if ctrl_c.canceled.load(Ordering::SeqCst) {
                let _ = on_progress_c.send(IndexProgress {
                    library_id: lib_c.clone(),
                    total,
                    processed,
                    current_file: String::new(),
                    level,
                    status: "canceled".into(),
                    message: "Job canceled".into(),
                });
                return;
            }
            while ctrl_c.paused.load(Ordering::SeqCst) {
                let _ = on_progress_c.send(IndexProgress {
                    library_id: lib_c.clone(),
                    total,
                    processed,
                    current_file: String::new(),
                    level,
                    status: "paused".into(),
                    message: "Paused".into(),
                });
                std::thread::sleep(std::time::Duration::from_millis(700));
                if ctrl_c.canceled.load(Ordering::SeqCst) {
                    return;
                }
            }

            let file_name = f
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            let _ = on_progress_c.send(IndexProgress {
                library_id: lib_c.clone(),
                total,
                processed,
                current_file: file_name.clone(),
                level,
                status: "running".into(),
                message: format!("Indexing {file_name}"),
            });

            match index_one_file(store, &lib_c, f, level) {
                Ok(chunks) => {
                    processed += 1;
                    let _ = on_progress_c.send(IndexProgress {
                        library_id: lib_c.clone(),
                        total,
                        processed,
                        current_file: file_name.clone(),
                        level,
                        status: "running".into(),
                        message: format!("{file_name}: {chunks} chunks @ L{level}"),
                    });
                }
                Err(e) => {
                    // Record error but continue with other files.
                    eprintln!("index error on {file_name}: {e}");
                }
            }
        }
        let _ = on_progress_c.send(IndexProgress {
            library_id: lib_c.clone(),
            total,
            processed,
            current_file: String::new(),
            level,
            status: "done".into(),
            message: format!("Indexed {processed}/{total} files"),
        });
    });

    Ok(format!("Started indexing {total} files into '{lib}' at level {level}"))
}

/// Index a single file up to `target` level, returning chunk count.
fn index_one_file(
    store: &Store,
    library_id: &str,
    path: &std::path::Path,
    target: u8,
) -> Result<usize, String> {
    let text = parse::parse_file(path)
        .filter(|t| !t.trim().is_empty())
        .or_else(|| parse::parse_media(path))
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| "unsupported, empty, or tool-unavailable file".to_string())?;

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    let hash = format!("{:x}", hasher.finish());

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let file_id = store
        .upsert_file(library_id, &path.to_string_lossy(), &file_name, &hash)
        .map_err(|e| e.to_string())?;

    store
        .update_file_status(file_id, "indexing", None, None, None)
        .map_err(|e| e.to_string())?;

    let mut achieved = 1u8;

    // L1: raw chunks.
    let mut chunks = parse::chunk_by_level(&text, Level::Raw);
    // L2: structure-aware.
    if target >= 2 {
        chunks = parse::chunk_by_level(&text, Level::Structure);
        achieved = 2;
    }

    // L3: summaries via LLM (needs engine); otherwise stay at L2.
    let mut summary_chunks: Vec<String> = Vec::new();
    if target >= 3 {
        if let Ok(emb_port) = engine::with_port(|p| Ok(p)) {
            let _ = emb_port;
            if let Some(port) = engine_port() {
                for c in &chunks {
                    let summary = summarize_chunk(port, c);
                    summary_chunks.push(summary.unwrap_or_else(|| c.clone()));
                }
                chunks = summary_chunks;
                achieved = 3;
            }
        }
    }

    // L5: entities appended.
    if target >= 5 {
        chunks = chunks
            .iter()
            .map(|c| {
                let ents = parse::extract_entities(c).join(", ");
                if ents.is_empty() {
                    c.clone()
                } else {
                    format!("{c}\n[entities: {ents}]")
                }
            })
            .collect();
        achieved = 5;
    }

    // L4: embeddings (needs engine). Store final enriched chunks with embeddings.
    let mut chunk_count = 0usize;
    for (i, c) in chunks.iter().enumerate() {
        let emb = if target >= 4 {
            engine::with_port(|port| embed::embed(port, c)).ok()
        } else {
            None
        };
        let final_level = if target >= 4 && emb.is_some() {
            4
        } else {
            achieved.min(3)
        };
        let blob = emb.as_deref().map(store::float_vec_to_blob);
        store
            .add_chunk_enriched(library_id, file_id, &file_name, i as i64, final_level as i64, c, blob)
            .map_err(|e| e.to_string())?;
        chunk_count += 1;
    }

    store
        .update_file_status(
            file_id,
            "done",
            None,
            Some(achieved as i64),
            Some(chunk_count as i64),
        )
        .map_err(|e| e.to_string())?;
    Ok(chunk_count)
}

fn engine_port() -> Option<u16> {
    unsafe { crate::APP_STATE.as_ref() }
        .and_then(|s| {
            let st = s.engine.status.lock().unwrap();
            st.port
        })
}

fn summarize_chunk(port: u16, chunk: &str) -> Option<String> {
    let msg = vec![chat::ChatMessage {
        role: "user".into(),
        content: format!(
            "Summarize the following passage in ONE concise sentence. \
             Reply with only the summary.\n\n{chunk}"
        ),
    }];
    chat::complete_blocking(port, &msg).ok().map(|s| {
        let s = s.trim().to_string();
        if s.is_empty() {
            chunk.to_string()
        } else {
            format!("[summary: {s}]\n{chunk}")
        }
    })
}

#[tauri::command]
pub fn pause_index(library_id: Option<String>) -> Result<(), String> {
    let lib = library_id.unwrap_or_else(|| "default".to_string());
    let jobs = unsafe { crate::APP_STATE.as_ref() }
        .ok_or("app not initialized")?
        .indexer
        .jobs
        .lock()
        .unwrap();
    if let Some(ctrl) = jobs.get(&lib) {
        ctrl.paused.store(true, Ordering::SeqCst);
        Ok(())
    } else {
        Err(format!("no active job for '{lib}'"))
    }
}

#[tauri::command]
pub fn resume_index(library_id: Option<String>) -> Result<(), String> {
    let lib = library_id.unwrap_or_else(|| "default".to_string());
    let jobs = unsafe { crate::APP_STATE.as_ref() }
        .ok_or("app not initialized")?
        .indexer
        .jobs
        .lock()
        .unwrap();
    if let Some(ctrl) = jobs.get(&lib) {
        ctrl.paused.store(false, Ordering::SeqCst);
        Ok(())
    } else {
        Err(format!("no active job for '{lib}'"))
    }
}

#[tauri::command]
pub fn cancel_index(library_id: Option<String>) -> Result<(), String> {
    let lib = library_id.unwrap_or_else(|| "default".to_string());
    let jobs = unsafe { crate::APP_STATE.as_ref() }
        .ok_or("app not initialized")?
        .indexer
        .jobs
        .lock()
        .unwrap();
    if let Some(ctrl) = jobs.get(&lib) {
        ctrl.canceled.store(true, Ordering::SeqCst);
        ctrl.paused.store(false, Ordering::SeqCst);
        Ok(())
    } else {
        Err(format!("no active job for '{lib}'"))
    }
}

#[tauri::command]
pub fn list_indexed_files(library_id: Option<String>) -> Result<Vec<store::FileRecord>, String> {
    let lib = library_id.unwrap_or_else(|| "default".to_string());
    let store = unsafe { crate::APP_STATE.as_ref() }
        .ok_or("app not initialized")?
        .store;
    store.get_files(&lib).map_err(|e| e.to_string())
}

/// Blocking RAG answer used by the Team Mode HTTP server.
pub fn team_rag_answer(
    store: &Store,
    library_id: &str,
    message: &str,
    rerank: bool,
) -> Result<String, String> {
    let ctx = if rerank {
        crate::rag::retrieve_context_rerank(store, library_id, message, 5)?
    } else {
        crate::rag::retrieve_context(store, library_id, message, 5)?
    };
    let mut messages = Vec::new();
    if !ctx.is_empty() {
        messages.push(chat::ChatMessage {
            role: "system".into(),
            content: format!(
                "Answer using the following context. Cite sources by file name.\n\n{ctx}"
            ),
        });
    }
    messages.push(chat::ChatMessage {
        role: "user".into(),
        content: message.to_string(),
    });
    let port = engine::with_port(|p| Ok(p))?;
    chat::complete_blocking(port, &messages)
}

/// Simple scheduler: when enabled, periodically re-indexes files left in
/// `pending`/`error` state for the given library (e.g. after a crash).
#[tauri::command]
pub fn set_scheduler(
    library_id: Option<String>,
    enabled: bool,
    interval_secs: Option<u64>,
) -> Result<String, String> {
    let lib = library_id.unwrap_or_else(|| "default".to_string());
    let interval = interval_secs.unwrap_or(60).max(10);
    let store = unsafe { crate::APP_STATE.as_ref() }
        .ok_or("app not initialized")?
        .store;
    let lib_c = lib.clone();

    if !enabled {
        return Ok(format!("Scheduler disabled for '{lib}'"));
    }

    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(interval));
        let files = match store.get_files(&lib_c) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let pending: Vec<_> = files
            .into_iter()
            .filter(|f| f.status == "pending" || f.status == "error")
            .collect();
        if pending.is_empty() {
            continue;
        }
        for f in pending {
            let path = std::path::Path::new(&f.path);
            let _ = index_one_file(store, &lib_c, path, 4);
        }
    });
    Ok(format!(
        "Scheduler enabled for '{lib}' every {interval}s (re-indexes pending/error files)"
    ))
}
