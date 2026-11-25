//! Сквозные тесты индексатора на настоящих файлах во временном каталоге.
//! Раньше индексация молча давала пустой индекс (несовпадение типов в `node`, пути с корнем,
//! вызывающий всегда «unknown»); эти тесты не дают этому вернуться.
use atlas_indexer::ProjectIndexer;
use atlas_store::Store;
use std::fs;
use std::path::Path;

fn write(root: &Path, rel: &str, body: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, body).unwrap();
}

fn project() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_string_lossy().to_string();
    write(dir.path(), "a.py", "def helper():\n    return 1\n\ndef main():\n    return helper()\n");
    write(dir.path(), "pkg/b.py", "from a import main\n\nclass Runner:\n    def go(self):\n        return main()\n");
    write(dir.path(), "NOTES.md", "# заметки\n");
    (dir, root)
}

fn index(store: &mut Store, root: &str) -> atlas_indexer::IndexStats {
    ProjectIndexer::new(store, root).index_project().unwrap()
}

fn edges(store: &Store) -> Vec<(String, String, String, String)> {
    let mut stmt = store
        .conn()
        .prepare(
            "SELECT s.name, d.name, e.kind, e.confidence FROM edge e
             JOIN node s ON s.id = e.src_id JOIN node d ON d.id = e.dst_id ORDER BY 1, 2, 3",
        )
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn indexes_files_symbols_and_edges() {
    let (dir, root) = project();
    let mut store = Store::open(dir.path().join(".atlas-test.db")).unwrap();
    let stats = index(&mut store, &root);
    assert_eq!(stats.errors, 0, "{stats:?}");

    // Пути относительные, а файл без поддерживаемого языка всё равно есть как узел
    assert!(store.find_file_node_by_path("a.py").unwrap().is_some());
    assert!(store.find_file_node_by_path("pkg/b.py").unwrap().is_some());
    assert!(store.find_file_node_by_path("NOTES.md").unwrap().is_some());

    let e = edges(&store);
    assert!(e.contains(&("main".into(), "helper".into(), "calls".into(), "certain".into())), "{e:?}");
    assert!(e.contains(&("go".into(), "main".into(), "calls".into(), "likely".into())), "{e:?}");
    assert!(e.contains(&("b.py".into(), "a.py".into(), "imports".into(), "certain".into())), "{e:?}");

    // Самый востребованный символ получает наибольший ранг
    let main = &store.symbols_by_name("main").unwrap()[0];
    assert!(main.rank > 0.0);
}

#[test]
fn full_text_search_finds_symbols() {
    let (dir, root) = project();
    let mut store = Store::open(dir.path().join(".atlas-test.db")).unwrap();
    index(&mut store, &root);
    let found = store.search_nodes("help", 10).unwrap();
    assert!(found.iter().any(|n| n.name == "helper"), "{found:?}");
    assert!(store.search_nodes("!!!", 10).unwrap().is_empty());
}

#[test]
fn reindex_is_idempotent_and_keeps_edges() {
    let (dir, root) = project();
    let mut store = Store::open(dir.path().join(".atlas-test.db")).unwrap();
    index(&mut store, &root);
    let before = edges(&store);
    let nodes_before: i64 = store.conn().query_row("SELECT COUNT(*) FROM node", [], |r| r.get(0)).unwrap();

    let again = index(&mut store, &root);
    assert_eq!(again.files_indexed, 0, "ничего не менялось: {again:?}");
    let nodes_after: i64 = store.conn().query_row("SELECT COUNT(*) FROM node", [], |r| r.get(0)).unwrap();
    assert_eq!(nodes_before, nodes_after);
    assert_eq!(before, edges(&store));
}

#[test]
fn changed_file_is_replaced_without_duplicates() {
    let (dir, root) = project();
    let mut store = Store::open(dir.path().join(".atlas-test.db")).unwrap();
    index(&mut store, &root);

    write(dir.path(), "a.py", "def helper():\n    return 2\n\ndef renamed():\n    return helper()\n");
    let stats = index(&mut store, &root);
    assert_eq!(stats.files_indexed, 1);
    assert!(store.symbols_by_name("main").unwrap().is_empty(), "старый символ должен исчезнуть");
    assert_eq!(store.symbols_by_name("renamed").unwrap().len(), 1);
    assert_eq!(store.symbols_by_name("helper").unwrap().len(), 1, "дубликатов быть не должно");
    assert!(store.search_nodes("main", 10).unwrap().iter().all(|n| n.name != "main"), "FTS должен забыть удалённое");
}

#[test]
fn deleted_file_leaves_the_index() {
    let (dir, root) = project();
    let mut store = Store::open(dir.path().join(".atlas-test.db")).unwrap();
    index(&mut store, &root);
    fs::remove_file(dir.path().join("pkg/b.py")).unwrap();
    index(&mut store, &root);
    assert!(store.find_file_node_by_path("pkg/b.py").unwrap().is_none());
    assert!(store.symbols_by_name("go").unwrap().is_empty());
}

#[test]
fn generic_method_names_do_not_create_edges() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_string_lossy().to_string();
    write(
        dir.path(),
        "lib.rs",
        "struct A;\nimpl A {\n    fn new() -> A { A }\n}\nfn build() -> A {\n    A::new()\n}\n",
    );
    let mut store = Store::open(dir.path().join(".atlas-test.db")).unwrap();
    index(&mut store, &root);
    assert!(edges(&store).iter().all(|e| e.1 != "new"), "{:?}", edges(&store));
}

#[test]
fn incoming_edges_survive_when_target_file_changes() {
    let (dir, root) = project();
    let mut store = Store::open(dir.path().join(".atlas-test.db")).unwrap();
    index(&mut store, &root);

    // b.py не менялся и пропускается; связи go -> main и b.py -> a.py должны пережить правку a.py
    write(dir.path(), "a.py", "def helper():\n    return 1\n\ndef main():\n    return helper() + 1\n");
    let stats = index(&mut store, &root);
    assert_eq!(stats.files_indexed, 1, "{stats:?}");
    let e = edges(&store);
    assert!(e.contains(&("go".into(), "main".into(), "calls".into(), "likely".into())), "{e:?}");
    assert!(e.contains(&("b.py".into(), "a.py".into(), "imports".into(), "certain".into())), "{e:?}");

    // Переименование: связь на исчезнувший символ не должна остаться висеть
    write(dir.path(), "a.py", "def helper():\n    return 1\n\ndef primary():\n    return helper()\n");
    index(&mut store, &root);
    let e = edges(&store);
    assert!(e.iter().all(|x| x.1 != "main"), "{e:?}");
    assert!(e.contains(&("b.py".into(), "a.py".into(), "imports".into(), "certain".into())), "{e:?}");
}

#[test]
fn ambiguous_call_names_link_all_candidates_as_guess() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_string_lossy().to_string();
    write(dir.path(), "a.py", "def run():\n    return 1\n");
    write(dir.path(), "b.py", "def run():\n    return 2\n");
    // Вызов без импорта: имя `run` неоднозначно, но кандидатов немного
    write(dir.path(), "c.py", "def go():\n    return run()\n");
    let mut store = Store::open(dir.path().join(".atlas-test.db")).unwrap();
    let stats = index(&mut store, &root);

    let e = edges(&store);
    let guesses: Vec<&(String, String, String, String)> = e
        .iter()
        .filter(|x| x.2 == "calls" && x.3 == "guess")
        .collect();
    assert_eq!(guesses.len(), 2, "ожидались две связи guess: {e:?}");
    assert!(guesses.iter().any(|x| x.1 == "run"), "{e:?}");
    assert!(stats.calls_guessed >= 2, "{stats:?}");
    assert_eq!(stats.calls_unresolved, 0, "кандидатов мало, связи должны появиться: {stats:?}");
}

#[test]
fn call_stats_report_linked_and_unresolved() {
    let (dir, root) = project();
    let mut store = Store::open(dir.path().join(".atlas-test.db")).unwrap();
    let stats = index(&mut store, &root);
    assert_eq!(stats.calls_linked + stats.calls_unresolved, stats.calls_extracted, "{stats:?}");
    assert!(stats.calls_linked > 0, "{stats:?}");
}
