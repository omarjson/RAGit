use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{ipc::Channel, AppHandle, Manager};
use zvec_rust::Doc;

use crate::chat;
use crate::engine;
use crate::rag::embed;
use crate::rag::parse::{self, Level};
use crate::rag::store;
use crate::AppState;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgress {
    pub library_id: String,
    pub total: usize,
    pub processed: usize,
    pub current_file: String,
    pub level: u8,
    pub status: String,
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
        IndexerState { jobs: Mutex::new(HashMap::new()) }
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
    let state = app.state::<Arc<AppState>>();
    state.store.add_library(&lib, &lib).map_err(|e| e.to_string())?;

    // Retrieve cursor from DB to skip already-indexed files.
    let cursor = state.store.get_index_cursor(&lib).ok().flatten();

    let ctrl = Arc::new(JobControl {
        paused: AtomicBool::new(false),
        canceled: AtomicBool::new(false),
    });
    state.indexer.jobs.lock().map_err(|e| e.to_string())?.insert(lib.clone(), ctrl.clone());

    let mut files: Vec<_> = parse::collect_files(std::path::Path::new(&path));
    files.sort_by(|a, b| a.to_string_lossy().as_ref().cmp(b.to_string_lossy().as_ref()));

    // If resuming, skip files up to and including the cursor.
    if let Some(ref c) = cursor {
        if let Some(pos) = files.iter().position(|f| f.to_string_lossy().as_ref() >= c.as_str()) {
            files.drain(..pos);
        }
    }

    let total = files.len();
    let files_clone = files.clone();
    let lib_c = lib.clone();
    let ctrl_c = ctrl.clone();

    std::thread::spawn(move || {
        let state = crate::APP_STATE.get().expect("app state");
        let store = &state.store;
        let mut processed = 0usize;

        for f in &files_clone {
            if ctrl_c.canceled.load(Ordering::SeqCst) {
                let _ = on_progress.send(IndexProgress {
                    library_id: lib_c.clone(), total, processed,
                    current_file: String::new(), level,
                    status: "canceled".into(), message: "Job canceled".into(),
                });
                cleanup_job(&lib_c);
                return;
            }
            while ctrl_c.paused.load(Ordering::SeqCst) {
                let _ = on_progress.send(IndexProgress {
                    library_id: lib_c.clone(), total, processed,
                    current_file: String::new(), level,
                    status: "paused".into(), message: "Paused".into(),
                });
                std::thread::sleep(std::time::Duration::from_millis(700));
                if ctrl_c.canceled.load(Ordering::SeqCst) {
                    cleanup_job(&lib_c);
                    return;
                }
            }

            let file_name = f.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let _ = on_progress.send(IndexProgress {
                library_id: lib_c.clone(), total, processed,
                current_file: file_name.clone(), level,
                status: "running".into(),
                message: format!("Indexing {file_name}"),
            });

            match index_one_file(store, &lib_c, f, level) {
                Ok(chunks) => {
                    processed += 1;
                    // Update cursor in DB so resume skips this file.
                    let _ = store.set_index_cursor(&lib_c, Some(&f.to_string_lossy()));
                    let _ = on_progress.send(IndexProgress {
                        library_id: lib_c.clone(), total, processed,
                        current_file: file_name.clone(), level,
                        status: "running".into(),
                        message: format!("{file_name}: {chunks} chunks @ L{level}"),
                    });
                }
                Err(e) => {
                    eprintln!("index error on {file_name}: {e}");
                }
            }
        }

        // Clear cursor when all files are processed.
        let _ = store.set_index_cursor(&lib_c, None);

        let _ = on_progress.send(IndexProgress {
            library_id: lib_c.clone(), total, processed,
            current_file: String::new(), level,
            status: "done".into(),
            message: format!("Indexed {processed}/{total} files"),
        });
        cleanup_job(&lib_c);
    });

    Ok(format!("Started indexing {total} files into '{lib}' at level {level}"))
}

fn cleanup_job(library_id: &str) {
    if let Some(state) = crate::APP_STATE.get() {
        if let Ok(mut jobs) = state.indexer.jobs.lock() {
            jobs.remove(library_id);
        }
    }
}

fn prepare_file(
    store: &store::Store,
    library_id: &str,
    path: &std::path::Path,
) -> Result<(String, i64, String), String> {
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

    let file_name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let file_id = store
        .upsert_file(library_id, &path.to_string_lossy(), &file_name, &hash)
        .map_err(|e| e.to_string())?;

    store
        .update_file_status(file_id, "indexing", None, None, None)
        .map_err(|e| e.to_string())?;

    Ok((text, file_id, file_name))
}

fn chunk_and_summarize(text: &str, target: u8) -> (Vec<String>, u8) {
    let mut achieved = 1u8;
    let mut chunks = parse::chunk_by_level(text, Level::Raw);
    if target >= 2 {
        chunks = parse::chunk_by_level(text, Level::Structure);
        achieved = 2;
    }

    if target >= 3 {
        if let Some(port) = engine::embed_port().or_else(|| engine::with_port(|p| Ok(p)).ok()) {
            let summaries: Vec<String> = chunks.iter()
                .map(|c| summarize_chunk(port, c).unwrap_or_else(|| c.clone()))
                .collect();
            chunks = summaries;
            achieved = 3;
        }
    }

    if target >= 5 {
        chunks = chunks.iter().map(|c| {
            let ents = parse::extract_entities(c).join(", ");
            if ents.is_empty() { c.clone() } else { format!("{c}\n[entities: {ents}]") }
        }).collect();
        achieved = 5;
    }

    (chunks, achieved)
}

fn embed_chunks(chunks: &[String]) -> Vec<Option<Vec<f32>>> {
    if let Some(port) = engine::embed_port() {
        let texts: Vec<&str> = chunks.iter().map(|c| c.as_str()).collect();
        match embed::embed_batch(port, &texts) {
            Ok(vecs) => vecs.into_iter().map(Some).collect(),
            Err(e) => {
                eprintln!("batch embed failed: {e}");
                vec![None; chunks.len()]
            }
        }
    } else {
        vec![None; chunks.len()]
    }
}

fn persist_chunks(
    store: &store::Store,
    library_id: &str,
    file_id: i64,
    file_name: &str,
    target: u8,
    achieved: u8,
    chunks: &[String],
    embeddings: &[Option<Vec<f32>>],
) -> Result<usize, String> {
    let mut chunk_count = 0usize;
    let mut zvec_docs: Vec<Doc> = Vec::new();

    for (i, c) in chunks.iter().enumerate() {
        let emb = embeddings.get(i).and_then(|e| e.as_ref());
        let final_level = if target >= 4 && emb.is_some() { 4 } else { achieved.min(3) };
        let blob = emb.map(|v| store::float_vec_to_blob(v));

        store
            .add_chunk_enriched(library_id, file_id, file_name, i as i64, final_level as i64, c, blob)
            .map_err(|e| e.to_string())?;
        chunk_count += 1;

        if let Some(e) = emb {
            let mut d = Doc::new().map_err(|e| e.to_string())?;
            let pk = format!("{file_id}_{i}");
            d.set_pk(&pk);
            d.add_string("library_id", library_id).map_err(|e| e.to_string())?;
            d.add_string("file_name", file_name).map_err(|e| e.to_string())?;
            d.add_i64("chunk_index", i as i64).map_err(|e| e.to_string())?;
            d.add_string("content", c).map_err(|e| e.to_string())?;
            d.add_i32("level", final_level as i32).map_err(|e| e.to_string())?;
            d.add_vector_f32("embedding", e).map_err(|e| e.to_string())?;
            zvec_docs.push(d);
        }
    }

    if !zvec_docs.is_empty() {
        let state = crate::APP_STATE.get().expect("app state");
        let coll = state.zvec.collection_for(library_id)?;
        let refs: Vec<&Doc> = zvec_docs.iter().collect();
        coll.insert(&refs).map_err(|e| e.to_string())?;
        coll.flush().map_err(|e| e.to_string())?;
        coll.optimize().map_err(|e| e.to_string())?;
    }

    store
        .update_file_status(file_id, "done", None, Some(achieved as i64), Some(chunk_count as i64))
        .map_err(|e| e.to_string())?;
    Ok(chunk_count)
}

fn index_one_file(
    store: &store::Store,
    library_id: &str,
    path: &std::path::Path,
    target: u8,
) -> Result<usize, String> {
    let (text, file_id, file_name) = prepare_file(store, library_id, path)?;
    let (chunks, achieved) = chunk_and_summarize(&text, target);
    let embeddings = if target >= 4 { embed_chunks(&chunks) } else { vec![None; chunks.len()] };
    persist_chunks(store, library_id, file_id, &file_name, target, achieved, &chunks, &embeddings)
}

fn summarize_chunk(port: u16, chunk: &str) -> Option<String> {
    let msg = vec![chat::ChatMessage {
        role: "user".into(),
        content: format!(
            "Summarize the following passage in ONE concise sentence. Reply with only the summary.\n\n{chunk}"
        ),
    }];
    chat::complete_blocking(port, &msg).ok().map(|s| {
        let s = s.trim().to_string();
        if s.is_empty() { chunk.to_string() } else { format!("[summary: {s}]\n{chunk}") }
    })
}

#[tauri::command]
pub fn pause_index(library_id: Option<String>) -> Result<(), String> {
    let lib = library_id.unwrap_or_else(|| "default".to_string());
    let jobs = crate::APP_STATE.get()
        .ok_or("app not initialized")?
        .indexer.jobs.lock().map_err(|e| e.to_string())?;
    jobs.get(&lib).ok_or_else(|| format!("no active job for '{lib}'"))?
        .paused.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn resume_index(library_id: Option<String>) -> Result<(), String> {
    let lib = library_id.unwrap_or_else(|| "default".to_string());
    let jobs = crate::APP_STATE.get()
        .ok_or("app not initialized")?
        .indexer.jobs.lock().map_err(|e| e.to_string())?;
    jobs.get(&lib).ok_or_else(|| format!("no active job for '{lib}'"))?
        .paused.store(false, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn cancel_index(library_id: Option<String>) -> Result<(), String> {
    let lib = library_id.unwrap_or_else(|| "default".to_string());
    let jobs = crate::APP_STATE.get()
        .ok_or("app not initialized")?
        .indexer.jobs.lock().map_err(|e| e.to_string())?;
    jobs.get(&lib).ok_or_else(|| format!("no active job for '{lib}'"))?
        .canceled.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn list_indexed_files(library_id: Option<String>) -> Result<Vec<store::FileRecord>, String> {
    let lib = library_id.unwrap_or_else(|| "default".to_string());
    let store = &crate::APP_STATE.get()
        .ok_or("app not initialized")?
        .store;
    store.get_files(&lib).map_err(|e| e.to_string())
}

pub fn team_rag_answer(
    store: &store::Store,
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
            content: format!("Answer using the following context. Cite sources by file name.\n\n{ctx}"),
        });
    }
    messages.push(chat::ChatMessage {
        role: "user".into(),
        content: message.to_string(),
    });
    let port = engine::with_port(|p| Ok(p))?;
    chat::complete_blocking(port, &messages)
}

#[tauri::command]
pub fn set_scheduler(
    library_id: Option<String>,
    enabled: bool,
    interval_secs: Option<u64>,
) -> Result<String, String> {
    let lib = library_id.unwrap_or_else(|| "default".to_string());
    let interval = interval_secs.unwrap_or(60).max(10);
    let state = crate::APP_STATE.get()
        .ok_or("app not initialized")?;
    let store = state.store.clone();
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
        let pending: Vec<_> = files.into_iter()
            .filter(|f| f.status == "pending" || f.status == "error")
            .collect();
        if pending.is_empty() { continue; }
        for f in pending {
            let path = std::path::Path::new(&f.path);
            let _ = index_one_file(&store, &lib_c, path, 4);
        }
    });
    Ok(format!(
        "Scheduler enabled for '{lib}' every {interval}s (re-indexes pending/error files)"
    ))
}
