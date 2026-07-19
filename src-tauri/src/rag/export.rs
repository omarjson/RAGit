use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ExportChunk {
    file_name: String,
    chunk_index: i64,
    level: i64,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct ExportFile {
    path: String,
    file_name: String,
    content_hash: String,
    status: String,
    level: i64,
    chunks: Vec<ExportChunk>,
}

#[derive(Serialize, Deserialize)]
struct ExportLibrary {
    library_id: String,
    version: u32,
    files: Vec<ExportFile>,
}

#[tauri::command]
pub fn export_library(path: String, library_id: Option<String>) -> Result<String, String> {
    let lib = library_id.unwrap_or_else(|| "default".to_string());
    let store = &crate::APP_STATE.get()
        .ok_or("app not initialized")?
        .store;
    let files = store.get_files(&lib).map_err(|e| e.to_string())?;
    let mut out_files = Vec::new();
    for f in files {
        let fname = f.file_name.clone();
        let chunks = store
            .get_chunks_for_file(f.id)
            .map_err(|e| e.to_string())?;
        out_files.push(ExportFile {
            path: f.path,
            file_name: fname.clone(),
            content_hash: f.content_hash,
            status: f.status,
            level: f.level,
            chunks: chunks
                .into_iter()
                .map(|(idx, level, content)| ExportChunk {
                    file_name: fname.clone(),
                    chunk_index: idx,
                    level,
                    content,
                })
                .collect(),
        });
    }
    let export = ExportLibrary {
        library_id: lib.clone(),
        version: 1,
        files: out_files,
    };
    let json = serde_json::to_string_pretty(&export).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(format!("Exported library '{lib}' to {path}"))
}

#[tauri::command]
pub fn import_library(path: String, library_id: Option<String>) -> Result<String, String> {
    let lib = library_id.unwrap_or_else(|| "default".to_string());
    let store = &crate::APP_STATE.get()
        .ok_or("app not initialized")?
        .store;
    store.add_library(&lib, &lib).map_err(|e| e.to_string())?;
    let text = std::fs::read_to_string(Path::new(&path)).map_err(|e| e.to_string())?;
    let export: ExportLibrary = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut file_count = 0;
    let mut chunk_count = 0;
    let reembed = crate::engine::embed_port().is_some();
    for f in export.files {
        let file_id = store
            .upsert_file(&lib, &f.path, &f.file_name, &f.content_hash)
            .map_err(|e| e.to_string())?;
        store
            .update_file_status(file_id, "done", None, Some(f.level), Some(f.chunks.len() as i64))
            .map_err(|e| e.to_string())?;
        for c in f.chunks {
            let emb = if reembed {
                match crate::engine::with_port(|p| crate::rag::embed::embed(p, &c.content)) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        eprintln!("import re-embed failed for chunk {}: {e}", c.chunk_index);
                        None
                    }
                }
            } else {
                None
            };
            let blob = emb.as_deref().map(crate::rag::store::float_vec_to_blob);
            store
                .add_chunk_enriched(&lib, file_id, &c.file_name, c.chunk_index, c.level, &c.content, blob)
                .map_err(|e| e.to_string())?;
            chunk_count += 1;
        }
        file_count += 1;
    }
    Ok(format!(
        "Imported {file_count} files, {chunk_count} chunks into '{lib}'{}",
        if reembed { " (re-embedded)" } else { " (no embeddings — re-index to enable search)" }
    ))
}
