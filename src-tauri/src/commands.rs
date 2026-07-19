use std::sync::Arc;

use crate::catalog;
use crate::hardware;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn detect_hardware() -> hardware::HardwareInfo {
    hardware::probe()
}

#[tauri::command]
pub fn list_models() -> Vec<catalog::CatalogModel> {
    catalog::load_catalog()
}

#[tauri::command]
pub fn search_hf_models(query: String) -> Vec<catalog::SearchHit> {
    catalog::search_hf(query)
}

#[tauri::command]
pub fn greet(name: String, state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let mut active = state.active_model.lock().map_err(|e| e.to_string())?;
    *active = Some(name.clone());
    Ok(format!("Hello, {name}! RAGit is running."))
}
