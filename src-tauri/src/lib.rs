mod hardware;
mod catalog;
mod download;
mod engine;
mod chat;
mod commands;
mod rag;
mod team;

use std::sync::Arc;
use std::sync::Mutex;

use zvec_rust;

use rag::indexer::IndexerState;
use rag::store::Store;
use rag::vector_db::{self, ZvecPool};
use engine::EngineState;
use team::TeamState;

/// Global shared state — fully owned via Arc, no &'static leaks.
pub struct AppState {
    pub engine: EngineState,
    pub store: Store,
    pub indexer: IndexerState,
    pub zvec: ZvecPool,
    pub team: Mutex<Option<Arc<TeamState>>>,
    pub active_model: Mutex<Option<String>>,
}

/// Global handle for background threads (indexer, scheduler, team).
/// Initialized once in run(); safe because Arc keeps data alive.
pub static APP_STATE: std::sync::OnceLock<Arc<AppState>> = std::sync::OnceLock::new();

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    vector_db::init().expect("zvec_rust::initialize failed");

    let engine_state = engine::init_state();
    let store = Store::new().expect("failed to open rag.db");
    let indexer = IndexerState::new();
    let zvec = ZvecPool::new(768).expect("ZvecPool::new");

    // Crash recovery: reset any files stuck in "indexing" back to "pending".
    if let Ok(n) = store.reset_stuck_files() {
        if n > 0 {
            eprintln!("crash recovery: reset {n} stuck files to pending");
        }
    }

    let app_state = Arc::new(AppState {
        engine: engine_state,
        store,
        indexer,
        zvec,
        team: Mutex::new(None),
        active_model: Mutex::new(None),
    });

    APP_STATE.set(Arc::clone(&app_state)).ok();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::clone(&app_state))
        .invoke_handler(tauri::generate_handler![
            commands::detect_hardware,
            commands::list_models,
            commands::search_hf_models,
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
            rag::migrate_library,
            rag::export::export_library,
            rag::export::import_library,
            team::start_team_server_cmd,
            team::stop_team_server_cmd,
            team::team_status_cmd,
        ])
        .build(tauri::generate_context!())
        .expect("error while building RAGit")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = APP_STATE.get() {
                    let _ = state.zvec.flush_all();
                }
                let _ = zvec_rust::shutdown();
            }
        });
}
