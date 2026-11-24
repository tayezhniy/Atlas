//! Библиотека оболочки Tauri для Atlas UI (раздел 12 ТЗ).
//! Команды связывают интерфейс с ядром (atlas-store, atlas-indexer, atlas-core).

use atlas_core::search as core_search;
use atlas_store::db::{read_project_lines, Store};
use serde_json::json;
use std::path::{Path, PathBuf};

fn db_path(root: &Path) -> PathBuf {
    root.join(".atlas").join("index.db")
}

fn open_store(root: &Path) -> Result<Store, String> {
    let db = db_path(root);
    if !db.exists() {
        return Err(format!("Индекс не найден: {}. Сначала запустите индексацию.", db.display()));
    }
    Store::open(&db).map_err(|e| e.to_string())
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
    let mut store = open_store(root)?;
    let info = store.project_info().map_err(|e| e.to_string())?;
    Ok(format!("Открыта база {} ({:?})", db_path(root).display(), info))
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
    let mut store = open_store(root)?;
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

/// Поиск по индексу (FTS5 + ранг). Возвращает JSON-список символов.
#[tauri::command]
pub async fn atlas_search(path: String, query: String, limit: Option<usize>) -> Result<String, String> {
    let root = Path::new(&path);
    let mut store = open_store(root)?;
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let results = core_search(&store, &query, limit).map_err(|e| e.to_string())?;
    let items: Vec<serde_json::Value> = results
        .into_iter()
        .map(|r| {
            json!({
                "id": r.node.id.to_string(),
                "kind": r.node.kind.to_string(),
                "path": r.node.path,
                "name": r.node.name,
                "lang": r.node.lang,
                "line_start": r.node.line_start,
                "line_end": r.node.line_end,
                "signature": r.node.signature,
                "doc": r.node.doc_summary,
                "score": r.score,
                "snippet": r.snippet,
            })
        })
        .collect();
    Ok(json!({ "query": query, "total": items.len(), "items": items }).to_string())
}

/// Обзор проекта (L0): языки, файлы, топ-хабы. Возвращает JSON.
#[tauri::command]
pub async fn atlas_overview(path: String) -> Result<String, String> {
    let root = Path::new(&path);
    let mut store = open_store(root)?;
    let files = store.nodes_of_kinds(&[atlas_store::schema::NodeKind::File]).map_err(|e| e.to_string())?;
    let mut langs: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for f in &files {
        *langs.entry(f.lang.clone().unwrap_or_else(|| "other".into())).or_default() += 1;
    }
    let top: Vec<serde_json::Value> = files
        .iter()
        .take(10)
        .map(|f| {
            json!({
                "path": f.path,
                "name": f.name,
                "lang": f.lang,
                "rank": f.rank,
                "tokens_est": f.tokens_est,
            })
        })
        .collect();
    Ok(json!({
        "root": path,
        "files": files.len(),
        "langs": langs,
        "top": top,
    }).to_string())
}

/// Символы одного файла (L2). Возвращает JSON-список.
#[tauri::command]
pub async fn atlas_outline(path: String, file_path: String) -> Result<String, String> {
    let root = Path::new(&path);
    let mut store = open_store(root)?;
    let nodes = store.nodes_in_file(&file_path).map_err(|e| e.to_string())?;
    let items: Vec<serde_json::Value> = nodes
        .into_iter()
        .map(|n| {
            json!({
                "id": n.id.to_string(),
                "kind": n.kind.to_string(),
                "name": n.name,
                "path": n.path,
                "line_start": n.line_start,
                "line_end": n.line_end,
                "signature": n.signature,
                "doc": n.doc_summary,
            })
        })
        .collect();
    Ok(json!({ "path": file_path, "symbols": items }).to_string())
}

/// Чтение диапазона строк файла (L3). Возвращает JSON со строками.
#[tauri::command]
pub async fn atlas_read(
    path: String,
    file_path: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<String, String> {
    let root = Path::new(&path);
    let lines = read_project_lines(&path, &file_path).map_err(|e| e.to_string())?;
    let total = lines.len();
    let start = start_line.unwrap_or(1).max(1);
    let end = end_line.unwrap_or(total).max(start).min(total);
    let slice: Vec<serde_json::Value> = (start..=end)
        .map(|n| json!({ "line": n, "text": lines[n - 1] }))
        .collect();
    Ok(json!({ "path": file_path, "total": total, "start": start, "end": end, "lines": slice }).to_string())
}

/// Связи символа или файла (L4). Возвращает JSON со входящими и исходящими рёбрами.
#[tauri::command]
pub async fn atlas_neighbors(path: String, target: String) -> Result<String, String> {
    let root = Path::new(&path);
    let mut store = open_store(root)?;
    // target — либо путь к файлу, либо имя символа
    let node = if let Some(f) = store.find_file_node_by_path(&target).map_err(|e| e.to_string())? {
        f
    } else {
        let syms = store.symbols_by_name(&target).map_err(|e| e.to_string())?;
        syms.into_iter().next().ok_or_else(|| format!("symbol «{target}» не найден"))?
    };
    let outgoing: Vec<serde_json::Value> = store
        .outgoing_edges(&node.id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|e| {
            let other = store.get_node(&e.dst_id).ok().flatten()?;
            Some(json!({
                "kind": e.kind.to_string(),
                "direction": "out",
                "target_id": other.id.to_string(),
                "target_name": other.name,
                "target_path": other.path,
                "confidence": e.confidence.to_string(),
            }))
        })
        .collect();
    let incoming: Vec<serde_json::Value> = store
        .incoming_edges(&node.id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|e| {
            let other = store.get_node(&e.src_id).ok().flatten()?;
            Some(json!({
                "kind": e.kind.to_string(),
                "direction": "in",
                "target_id": other.id.to_string(),
                "target_name": other.name,
                "target_path": other.path,
                "confidence": e.confidence.to_string(),
            }))
        })
        .collect();
    Ok(json!({
        "node": {
            "id": node.id.to_string(),
            "name": node.name,
            "kind": node.kind.to_string(),
            "path": node.path,
        },
        "outgoing": outgoing,
        "incoming": incoming,
    }).to_string())
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
            atlas_search,
            atlas_overview,
            atlas_outline,
            atlas_read,
            atlas_neighbors,
        ])
        .run(tauri::generate_context!())
        .expect("ошибка запуска приложения Atlas");
}