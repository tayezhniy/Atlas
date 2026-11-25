//! Интеграционные тесты хранилища индекса (раздел 5 ТЗ).
//!
//! Проверяем миграции, запись/чтение узлов и рёбер, FTS-поиск, удаление
//! по пути, журнал вызовов и настройки — без обращения к файловой системе
//! проекта (база живёт в tempdir).

use atlas_store::schema::{Confidence, Edge, EdgeKind, FileMeta, Node, NodeKind};
use atlas_store::Store;
use uuid::Uuid;

fn node(kind: NodeKind, path: &str, name: &str) -> Node {
    Node {
        id: Uuid::new_v4(),
        kind,
        path: path.to_string(),
        name: name.to_string(),
        parent_id: None,
        lang: Some("rust".into()),
        hash: Some("h1".into()),
        line_start: Some(1),
        line_end: Some(10),
        signature: Some("fn main()".into()),
        doc_summary: Some("точка входа".into()),
        tokens_est: 5,
        rank: 0.0,
    }
}

fn store_in_tempdir() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("index.db");
    let store = Store::open(&path).expect("open store");
    (dir, store)
}

#[test]
fn migrations_create_schema_and_are_idempotent() {
    let (_dir, store) = store_in_tempdir();
    assert_eq!(store.integrity_check().unwrap(), "ok");
    // Повторное открытие той же базы не должно падать: миграции идемпотентны
    let path = _dir.path().join("index.db");
    drop(store);
    let store = Store::open(&path).expect("reopen");
    assert_eq!(store.integrity_check().unwrap(), "ok");
    assert_eq!(store.file_count().unwrap(), 0);
}

#[test]
fn upsert_node_and_read_back() {
    let (_dir, store) = store_in_tempdir();
    let n = node(NodeKind::Function, "src/lib.rs", "load");
    store.upsert_node(&n).unwrap();

    let got = store.get_node(&n.id).unwrap().expect("узел не найден");
    assert_eq!(got.name, "load");
    assert_eq!(got.kind, NodeKind::Function);
    assert_eq!(got.path, "src/lib.rs");

    // Повторный upsert с тем же id обновляет, а не дублирует
    let mut updated = n.clone();
    updated.signature = Some("fn load(path: &str)".into());
    store.upsert_node(&updated).unwrap();
    let got = store.get_node(&n.id).unwrap().expect("узел не найден");
    assert_eq!(got.signature.as_deref(), Some("fn load(path: &str)"));
    assert_eq!(store.file_count().unwrap(), 0, "символ — не файл");
}

#[test]
fn edges_and_incoming_outgoing() {
    let (_dir, store) = store_in_tempdir();
    let file = node(NodeKind::File, "src/lib.rs", "lib.rs");
    let func = node(NodeKind::Function, "src/lib.rs", "load");
    store.upsert_node(&file).unwrap();
    store.upsert_node(&func).unwrap();
    store
        .upsert_edge(&Edge {
            src_id: file.id,
            dst_id: func.id,
            kind: EdgeKind::Contains,
            confidence: Confidence::Certain,
            weight: 1.0,
        })
        .unwrap();

    assert_eq!(store.outgoing_edges(&file.id).unwrap().len(), 1);
    assert_eq!(store.incoming_edges(&func.id).unwrap().len(), 1);
    let counts = store.count_edges_by_kind().unwrap();
    assert_eq!(counts, vec![("contains".to_string(), 1)]);
}

#[test]
fn fts_search_finds_symbol_by_prefix() {
    let (_dir, store) = store_in_tempdir();
    let n = node(NodeKind::Function, "src/payment.rs", "process_payment");
    store.upsert_node(&n).unwrap();

    let found = store.search_nodes("process_pay", 10).unwrap();
    assert!(
        found.iter().any(|x| x.id == n.id),
        "префиксный поиск не нашёл символ: {found:?}"
    );
    // Другой запрос не должен находить этот узел
    assert!(store.search_nodes("nonexistent_zzz", 10).unwrap().is_empty());
}

#[test]
fn symbols_by_name_skips_files_and_dirs() {
    let (_dir, store) = store_in_tempdir();
    let dir = node(NodeKind::Dir, "src", "src");
    let file = node(NodeKind::File, "src/lib.rs", "utils");
    let func = node(NodeKind::Function, "src/lib.rs", "utils");
    for n in [&dir, &file, &func] {
        store.upsert_node(n).unwrap();
    }
    let found = store.symbols_by_name("utils").unwrap();
    assert_eq!(found.len(), 1, "файлы и каталоги не должны попадать: {found:?}");
    assert_eq!(found[0].id, func.id);
}

#[test]
fn delete_nodes_by_path_cascades_to_edges() {
    let (_dir, store) = store_in_tempdir();
    let file = node(NodeKind::File, "src/gone.rs", "gone.rs");
    let func = node(NodeKind::Function, "src/gone.rs", "run");
    let other = node(NodeKind::Function, "src/keep.rs", "keep");
    for n in [&file, &func, &other] {
        store.upsert_node(n).unwrap();
    }
    store
        .upsert_edge(&Edge {
            src_id: file.id,
            dst_id: func.id,
            kind: EdgeKind::Contains,
            confidence: Confidence::Certain,
            weight: 1.0,
        })
        .unwrap();

    store.delete_nodes_by_path("src/gone.rs").unwrap();
    assert!(store.get_node(&file.id).unwrap().is_none());
    assert!(store.get_node(&func.id).unwrap().is_none());
    assert!(store.get_node(&other.id).unwrap().is_some());
    assert!(store.incoming_edges(&func.id).unwrap().is_empty());
}

#[test]
fn file_meta_and_settings_roundtrip() {
    let (_dir, store) = store_in_tempdir();
    let file = node(NodeKind::File, "src/lib.rs", "lib.rs");
    store.upsert_node(&file).unwrap();
    store
        .upsert_file_meta(&FileMeta {
            node_id: file.id,
            size: 1234,
            mtime: None,
            git_changes_30d: 7,
            last_commit: Some("abc123".into()),
            is_test: false,
            is_generated: false,
        })
        .unwrap();

    store.set_setting("exclude", "vendor").unwrap();
    assert_eq!(store.get_setting("exclude").unwrap().as_deref(), Some("vendor"));
    assert!(store.all_settings().unwrap().iter().any(|(k, v)| k == "exclude" && v == "vendor"));
}

#[test]
fn call_log_is_written_and_summarised() {
    let (_dir, store) = store_in_tempdir();
    store.log_call("s1", "search", "{}", 120, "n1,n2").unwrap();
    store.log_call("s1", "read", "{}", 80, "n2").unwrap();

    let summary = store.call_log_summary().unwrap();
    assert_eq!(summary.len(), 2);
    assert!(summary.iter().any(|(tool, calls, tokens)| tool == "search" && *calls == 1 && *tokens == 120));
    assert_eq!(store.recent_calls(10).unwrap().len(), 2);
}