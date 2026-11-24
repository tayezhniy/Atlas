//! Библиотека оболочки Tauri для Atlas UI (раздел 12 ТЗ).
//! Команды связывают интерфейс с ядром (atlas-store, atlas-indexer).

use atlas_store::Store;
use std::path::{Path, PathBuf};

fn db_path(root: &Path) -> PathBuf {
    root.join(".atlas").join("index.db")
}

/// Проверка живости моста UI ↔ ядро.
#[tauri::command]
pub fn atlas_core_ping() -> &'static str {
    "pong"
}

/// Открывает базу индекса проекта и возвращает сводку.
#[tauri::command]
pub async fn atlas_store_open(path: String) -> Result<String, String> {
    let root = Path::new(&path);
    let db = db_path(root);
    if !db.exists() {
        return Err(format!("Индекс не найден: {}. Сначала запустите индексацию.", db.display()));
    }
    let store = Store::open(&db).map_err(|e| e.to_string())?;
    let info = store.project_info().map_err(|e| e.to_string())?;
    Ok(format!("Открыта база {} ({:?})", db.display(), info))
}

/// Полная индексация проекта из интерфейса.
#[tauri::command]
pub async fn atlas_index_project(path: String) -> Result<String, String> {
    use atlas_indexer::ProjectIndexer;

    let root = Path::new(&path);
    let db = db_path(root);
    if let Some(parent) = db.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut store = Store::open(&db).map_err(|e| e.to_string())?;
    let stats = ProjectIndexer::new(&mut store, &path)
        .index_project()
        .map_err(|e| e.to_string())?;

    Ok(format!(
        "Проиндексировано: {} файлов, {} символов, {} импортов, {} вызовов, ошибок {}",
        stats.files_indexed, stats.symbols_extracted, stats.imports_extracted, stats.calls_extracted, stats.errors
    ))
}

/// Состояние индекса для UI.
#[tauri::command]
pub async fn atlas_doctor(path: String) -> Result<String, String> {
    let root = Path::new(&path);
    let db = db_path(root);
    if !db.exists() {
        return Ok("Индекс не найден".to_string());
    }
    let store = Store::open(&db).map_err(|e| e.to_string())?;
    let integrity = store.integrity_check().map_err(|e| e.to_string())?;
    let info = store.project_info().map_err(|e| e.to_string())?;
    let nodes = store.count_by_kind().map_err(|e| e.to_string())?;
    let edges = store.count_edges_by_kind().map_err(|e| e.to_string())?;

    let total_nodes: i64 = nodes.iter().map(|(_, n)| n).sum();
    let node_parts: Vec<String> = nodes.iter().map(|(k, n)| format!("{k} {n}")).collect();
    let edge_parts: Vec<String> = edges.iter().map(|(k, n)| format!("{k} {n}")).collect();

    Ok(format!(
        "Целостность: {integrity}\nПроект: {info:?}\nУзлов: {total_nodes} ({})\nСвязи: {}",
        node_parts.join(", "),
        edge_parts.join(", ")
    ))
}

/// Запуск приложения Tauri.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            atlas_core_ping,
            atlas_store_open,
            atlas_index_project,
            atlas_doctor,
        ])
        .run(tauri::generate_context!())
        .expect("ошибка запуска приложения Atlas");
}