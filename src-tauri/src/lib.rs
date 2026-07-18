mod hardware;
mod catalog;
mod download;
mod engine;
mod chat;
mod commands;
mod rag;
mod team;

use std::sync::Mutex;

use rag::indexer::IndexerState;
use rag::store::Store;
use team::TeamState;

#[derive(Default)]
pub struct AppState {
    pub active_model: Mutex<Option<String>>,
}

pub struct GlobalState {
    pub engine: &'static engine::EngineState,
    pub store: &'static Store,
    pub indexer: &'static IndexerState,
    pub team: Mutex<Option<std::sync::Arc<TeamState>>>,
}

// Global handles used by engine::with_port and RAG pipeline.
// Written exactly once during run(); read-only afterwards.
#[allow(static_mut_refs)]
pub static mut APP_STATE: Option<GlobalState> = None;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let engine_state = engine::init_state();
    let store = Store::new().expect("failed to open rag.db");
    let indexer = IndexerState::new();

    let engine_static: &'static engine::EngineState = Box::leak(Box::new(engine_state));
    let store_static: &'static Store = Box::leak(Box::new(store));
    let indexer_static: &'static IndexerState = Box::leak(Box::new(indexer));

    unsafe {
        APP_STATE = Some(GlobalState {
            engine: engine_static,
            store: store_static,
            indexer: indexer_static,
            team: Mutex::new(None),
        });
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .manage(engine_static)
        .manage(store_static)
        .manage(indexer_static)
        .invoke_handler(tauri::generate_handler![
            commands::detect_hardware,
            commands::list_models,
            download::download_model,
            engine::start_engine,
            engine::stop_engine,
            engine::engine_status,
            chat::chat_stream,
            commands::greet,
            rag::indexer::index_library,
            rag::indexer::pause_index,
            rag::indexer::resume_index,
            rag::indexer::cancel_index,
            rag::indexer::list_indexed_files,
            rag::indexer::set_scheduler,
            rag::rag_chat,
            rag::export::export_library,
            rag::export::import_library,
            team::start_team_server_cmd,
            team::stop_team_server_cmd,
            team::team_status_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running RAGit");
}
