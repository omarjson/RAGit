pub mod embed;
pub mod export;
pub mod indexer;
pub mod media;
pub mod parse;
pub mod vision;
pub mod store;
pub mod vector_db;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use zvec_rust::{Doc, Fts, MultiQuery, SearchQuery, SubQuery};

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
    _store: &Store,
    library_id: &str,
    query: &str,
    k: usize,
    rerank: bool,
) -> Result<String, String> {
    let emb = engine::with_port(|port| embed::embed(port, query))?;
    let state = crate::APP_STATE.get().ok_or("app not initialized")?;
    let coll = state.zvec.collection_for(library_id)?;

    let hits = if rerank {
        // Hybrid MultiQuery: dense vector + FTS + RRF rerank.
        let mut sub_vec = SubQuery::new().map_err(|e| e.to_string())?;
        sub_vec.set_field_name("embedding").map_err(|e| e.to_string())?;
        sub_vec.set_query_vector(&emb).map_err(|e| e.to_string())?;
        sub_vec.set_num_candidates((k * 4).max(50) as i32).map_err(|e| e.to_string())?;

        let mut fts_inst = Fts::new().map_err(|e| e.to_string())?;
        fts_inst.set_match_string(query).map_err(|e| e.to_string())?;
        let mut sub_fts = SubQuery::new().map_err(|e| e.to_string())?;
        sub_fts.set_field_name("content").map_err(|e| e.to_string())?;
        sub_fts.set_fts(&fts_inst).map_err(|e| e.to_string())?;
        sub_fts.set_num_candidates((k * 4).max(50) as i32).map_err(|e| e.to_string())?;

        let mut mq = MultiQuery::new().map_err(|e| e.to_string())?;
        mq.set_topk(k as i32).map_err(|e| e.to_string())?;
        mq.set_rerank_rrf(60).map_err(|e| e.to_string())?;
        mq.add_sub_query(&sub_vec).map_err(|e| e.to_string())?;
        mq.add_sub_query(&sub_fts).map_err(|e| e.to_string())?;
        coll.multi_query(&mq).map_err(|e| e.to_string())?
    } else {
        let sq = SearchQuery::builder()
            .field_name("embedding")
            .vector(&emb)
            .topk(k as i32)
            .filter(&format!("library_id = '{library_id}'"))
            .build()
            .map_err(|e| e.to_string())?;
        coll.query(&sq).map_err(|e| e.to_string())?
    };

    if hits.is_empty() {
        return Ok(String::new());
    }

    let mut ctx = String::from("=== RETRIEVED CONTEXT ===\n");
    for (i, r) in hits.iter().enumerate() {
        let fname = r.get_string("file_name")
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| "?".to_string());
        let idx = r.get_i64("chunk_index")
            .map_err(|e| e.to_string())?
            .unwrap_or(0);
        let content = r.get_string("content")
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        ctx.push_str(&format!("[{i}] ({fname}#{idx})\n{content}\n\n"));
    }
    Ok(ctx)
}

/// Migrate chunks from the old SQLite chunks table into Zvec for a library.
/// This allows existing indexed data to be used with the new Zvec-based retrieval.
/// Returns the number of chunks migrated.
#[tauri::command]
pub fn migrate_library(library_id: Option<String>) -> Result<usize, String> {
    let lib = library_id.as_deref().unwrap_or("default");
    let state = crate::APP_STATE.get().ok_or("app not initialized")?;

    let rows = state.store.get_chunks_with_embeddings(lib)?;
    if rows.is_empty() {
        return Ok(0);
    }

    let coll = state.zvec.collection_for(lib)?;
    let mut docs = Vec::with_capacity(rows.len());
    for (file_id, file_name, chunk_index, content, emb) in &rows {
        let mut d = Doc::new().map_err(|e| e.to_string())?;
        d.set_pk(&format!("{file_id}_{chunk_index}"));
        d.add_string("library_id", lib).map_err(|e| e.to_string())?;
        d.add_string("file_name", file_name).map_err(|e| e.to_string())?;
        d.add_i64("chunk_index", *chunk_index).map_err(|e| e.to_string())?;
        d.add_string("content", content).map_err(|e| e.to_string())?;
        d.add_i32("level", 4).map_err(|e| e.to_string())?;
        d.add_vector_f32("embedding", emb).map_err(|e| e.to_string())?;
        docs.push(d);
    }

    let refs: Vec<&Doc> = docs.iter().collect();
    coll.insert(&refs).map_err(|e| e.to_string())?;
    coll.flush().map_err(|e| e.to_string())?;

    Ok(rows.len())
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
    let state = APP_STATE.get().ok_or("app not initialized")?;
    let lib = library_id.unwrap_or_else(|| "default".to_string());

    let context = if rerank.unwrap_or(false) {
        retrieve_context_rerank(&state.store, &lib, &message, 5)?
    } else {
        retrieve_context(&state.store, &lib, &message, 5)?
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
        let st = state.engine.status.lock().map_err(|e| e.to_string())?;
        st.port.ok_or("engine not running")?
    };
    chat::complete_blocking(port, &messages)
}
