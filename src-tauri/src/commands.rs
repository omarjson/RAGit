use crate::catalog;
use crate::download;
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
pub fn greet(name: String, state: State<AppState>) -> String {
    let mut active = state.active_model.lock().unwrap();
    *active = Some(name.clone());
    format!("Hello, {name}! RAGit is running.")
}
