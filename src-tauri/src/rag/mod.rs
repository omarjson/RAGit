pub mod embed;
pub mod export;
pub mod indexer;
pub mod media;
pub mod parse;
pub mod vision;
pub mod store;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::engine;
use store::Store;

/// Index a file or directory into a library.
/// Returns (files_indexed, chunks_indexed).
pub fn index_path(store: &Store, library_id: &str, path: &str) -> Result<(usize, usize), String> {
    store.add_library(library_id, library_id).map_err(|e| e.to_string())?;
    let root = std::path::Path::new(path);
    let files = parse::collect_files(root);
    let mut files_n = 0usize;
    let mut chunks_n = 0usize;
    for f in files {
        let text = match parse::parse_file(&f) {
            Some(t) if !t.trim().is_empty() => t,
            _ => continue,
        };
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let hash = format!("{:x}", hasher.finish());
        let file_name = f
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let file_id = store
            .add_file(library_id, &f.to_string_lossy(), &file_name, &hash)
            .map_err(|e| e.to_string())?;
        files_n += 1;

        let chunks = parse::chunk_text(&text, 1000, 100);
        for (i, c) in chunks.iter().enumerate() {
            match engine::with_port(|port| embed::embed(port, c)) {
                Ok(emb) => {
                    store
                        .add_chunk(library_id, file_id, &file_name, i as i64, c, &emb)
                        .map_err(|e| e.to_string())?;
                    chunks_n += 1;
                }
                Err(e) => {
                    // Embedding failed for this chunk; skip but report.
                    eprintln!("embed error (chunk {i}): {e}");
                }
            }
        }
    }
    Ok((files_n, chunks_n))
}

/// Retrieve top-k chunks for a query and format as context string.
pub fn retrieve_context(
    store: &Store,
    library_id: &str,
    query: &str,
    k: usize,
) -> Result<String, String> {
    retrieve_context_impl(store, library_id, query, k, false)
}

/// Retrieve with reranking: over-fetch candidates by vector similarity, then
/// re-rank by lexical overlap (keyword match) between query and chunk.
pub fn retrieve_context_rerank(
    store: &Store,
    library_id: &str,
    query: &str,
    k: usize,
) -> Result<String, String> {
    retrieve_context_impl(store, library_id, query, k, true)
}

fn retrieve_context_impl(
    store: &Store,
    library_id: &str,
    query: &str,
    k: usize,
    rerank: bool,
) -> Result<String, String> {
    let emb = engine::with_port(|port| embed::embed(port, query))?;
    let fetch = if rerank { (k * 4).max(12) } else { k };
    let mut hits = store.search(library_id, &emb, fetch).map_err(|e| e.to_string())?;
    if hits.is_empty() {
        return Ok(String::new());
    }
    if rerank {
        let q_tokens: Vec<String> = tokenize(query);
        for h in hits.iter_mut() {
            let c_tokens = tokenize(&h.content);
            let overlap = lexical_overlap(&q_tokens, &c_tokens);
            h.score = 0.6 * h.score + 0.4 * overlap;
        }
        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(k);
    }
    let mut ctx = String::from("=== RETRIEVED CONTEXT ===\n");
    for (i, h) in hits.iter().enumerate() {
        ctx.push_str(&format!(
            "[{i}] ({}#{})\n{}\n\n",
            h.file_name, h.chunk_index, h.content
        ));
    }
    Ok(ctx)
}

fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3)
        .map(|w| w.to_lowercase())
        .collect()
}

fn lexical_overlap(a: &[String], b: &[String]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let set: std::collections::HashSet<&String> = b.iter().collect();
    let matched = a.iter().filter(|t| set.contains(t)).count();
    matched as f32 / a.len() as f32
}

#[tauri::command]
pub fn rag_chat(
    _app: tauri::AppHandle,
    message: String,
    library_id: Option<String>,
    history: Option<Vec<crate::chat::ChatMessage>>,
    rerank: Option<bool>,
) -> Result<String, String> {
    use crate::APP_STATE;
    use crate::chat;
    let state = unsafe { APP_STATE.as_ref() }.ok_or("app not initialized")?;
    let lib = library_id.unwrap_or_else(|| "default".to_string());

    let context = if rerank.unwrap_or(false) {
        retrieve_context_rerank(state.store, &lib, &message, 5)?
    } else {
        retrieve_context(state.store, &lib, &message, 5)?
    };
    let mut messages = history.unwrap_or_default();
    if !context.is_empty() {
        messages.insert(
            0,
            chat::ChatMessage {
                role: "system".into(),
                content: format!(
                    "Answer using the following context. Cite sources by file name.\n\n{context}"
                ),
            },
        );
    }
    messages.push(chat::ChatMessage {
        role: "user".into(),
        content: message,
    });

    let port = {
        let st = state.engine.status.lock().unwrap();
        st.port.ok_or("engine not running")?
    };
    chat::complete_blocking(port, &messages)
}
