use atlas_store::{db::Store, schema::{Edge, EdgeKind, FileMeta, Node, NodeKind, Confidence}};
use anyhow::Result;
use std::path::Path;
use uuid::Uuid;
use atlas_langs::{LanguageParser, SymbolKind};

/// Инкрементальное обновление индекса (раздел 4.3 ТЗ).
/// Файл пересчитывается только если изменился хеш содержимого.
pub struct IncrementalUpdater<'a> {
    store: &'a mut Store,
    parser: LanguageParser,
    root_path: String,
}

impl<'a> IncrementalUpdater<'a> {
    pub fn new(store: &'a mut Store, root_path: &str) -> Self {
        Self {
            store,
            parser: LanguageParser::new(),
            root_path: root_path.to_string(),
        }
    }

    /// Обновляет индекс для одного файла.
    /// Возвращает true, если файл был переиндексирован.
    pub fn update_file(&mut self, rel_path: &str, content: &str) -> Result<bool> {
        // Вычисляем хеш нового содержимого
        let new_hash = atlas_hash(content);

        // Проверяем, есть ли файл в индексе
        let existing = self.store.find_file_node_by_path(rel_path)?;

        if let Some(file_node) = existing {
            // Файл есть в индексе — проверяем хеш
            if file_node.hash.as_ref() == Some(&new_hash) {
                return Ok(false); // Не изменился
            }
            // Файл изменился: удаляем старые узлы и связи, запоминаем входящие связи
            let mut preserved_edges = Vec::new();
            for (edge, name, kind) in self.store.incoming_edges_from_other_files(rel_path)? {
                preserved_edges.push((edge, rel_path.to_string(), name, kind));
            }
            self.store.delete_nodes_by_path(rel_path)?;

            // Переиндексируем
            self.reindex_file(rel_path, content, new_hash, preserved_edges)?;
        } else {
            // Новый файл: просто индексируем
            self.reindex_file(rel_path, content, new_hash, Vec::new())?;
        }

        // Пересчитываем ранги
        self.store.recompute_ranks()?;
        Ok(true)
    }

    /// Читает файл с диска (путь относительно корня проекта) и обновляет индекс,
    /// если содержимое изменилось. Возвращает true, если файл был переиндексирован.
    pub fn update_file_from_disk(&mut self, rel_path: &str) -> Result<bool> {
        let full = Path::new(&self.root_path).join(rel_path);
        let content = match std::fs::read_to_string(&full) {
            Ok(c) => c,
            Err(_) if !full.exists() => {
                // Файл удалён: убираем его узлы и связи
                self.store.delete_nodes_by_path(rel_path)?;
                self.store.recompute_ranks()?;
                return Ok(true);
            }
            Err(e) => return Err(e.into()),
        };
        self.update_file(rel_path, &content)
    }

    fn reindex_file(
        &mut self,
        rel_path: &str,
        content: &str,
        hash: String,
        preserved_edges: Vec<(Edge, String, String, String)>,
    ) -> Result<()> {
        // Парсим файл
        let parsed = match self.parser.parse(rel_path, content) {
            Ok(p) => p,
            Err(e) if e.to_string().contains("unsupported language") => {
                // Неподдерживаемый язык — создаём узел файла без символов
                let file_id = Uuid::new_v4();
                let file_node = Node {
                    id: file_id,
                    kind: NodeKind::File,
                    path: rel_path.to_string(),
                    name: Path::new(rel_path).file_name().unwrap_or_default().to_string_lossy().to_string(),
                    parent_id: self.parent_id_for_path(rel_path)?,
                    lang: None,
                    hash: Some(hash),
                    line_start: None,
                    line_end: None,
                    signature: None,
                    doc_summary: None,
                    tokens_est: estimate_tokens(content) as i32,
                    rank: 0.0,
                };
                self.store.upsert_node(&file_node)?;
                let file_meta = FileMeta {
                    node_id: file_id,
                    size: content.len() as i64,
                    mtime: None,
                    git_changes_30d: 0,
                    last_commit: None,
                    is_test: is_test_file(rel_path),
                    is_generated: is_generated_file(rel_path),
                };
                self.store.upsert_file_meta(&file_meta)?;
                return Ok(());
            }
            Err(e) => {
                return Err(e);
            }
        };

        // Создаём узел файла
        let file_id = Uuid::new_v4();
        let file_node = Node {
            id: file_id,
            kind: NodeKind::File,
            path: rel_path.to_string(),
            name: Path::new(rel_path).file_name().unwrap_or_default().to_string_lossy().to_string(),
            parent_id: self.parent_id_for_path(rel_path)?,
            lang: if parsed.language.is_empty() { None } else { Some(parsed.language.clone()) },
            hash: Some(hash),
            line_start: None,
            line_end: None,
            signature: None,
            doc_summary: None,
            tokens_est: estimate_tokens(content) as i32,
            rank: 0.0,
        };
        self.store.upsert_node(&file_node)?;

        // Метаданные файла
        let file_meta = FileMeta {
            node_id: file_id,
            size: content.len() as i64,
            mtime: None,
            git_changes_30d: 0,
            last_commit: None,
            is_test: is_test_file(rel_path),
            is_generated: is_generated_file(rel_path),
        };
        self.store.upsert_file_meta(&file_meta)?;

        // Вставляем символы
        let mut symbol_ids = std::collections::HashMap::new();
        for sym in &parsed.symbols {
            let symbol_id = Uuid::new_v4();
            let symbol_node = Node {
                id: symbol_id,
                kind: map_symbol_kind(&sym.kind),
                path: rel_path.to_string(),
                name: sym.name.clone(),
                parent_id: Some(file_id),
                lang: Some(parsed.language.clone()),
                hash: None,
                line_start: Some(sym.line_start as i32),
                line_end: Some(sym.line_end as i32),
                signature: sym.signature.clone(),
                doc_summary: sym.doc.clone(),
                tokens_est: estimate_tokens(&sym.signature.clone().unwrap_or_default()) as i32,
                rank: 0.0,
            };
            self.store.upsert_node(&symbol_node)?;
            symbol_ids.insert(
                format!("{}::{}", rel_path, sym.name),
                symbol_id,
            );
        }

        // Импорты
        for imp in &parsed.imports {
            if let Some(dst) = self.resolve_import_target(&imp.to, rel_path)? {
                if dst != file_id {
                    self.store.upsert_edge(&Edge {
                        src_id: file_id,
                        dst_id: dst,
                        kind: EdgeKind::Imports,
                        confidence: Confidence::Certain,
                        weight: 1.0,
                    })?;
                }
            }
        }

        // Вызовы
        for call in &parsed.calls {
            if is_generic_name(&simple_name(&call.callee)) {
                continue;
            }
            let caller = parsed
                .symbols
                .iter()
                .filter(|sy| {
                    matches!(sy.kind, SymbolKind::Function | SymbolKind::Method)
                        && sy.line_start <= call.line
                        && call.line <= sy.line_end
                })
                .min_by_key(|sy| sy.line_end - sy.line_start);

            if let Some(caller) = caller {
                if let Some(src) = symbol_ids.get(&format!("{}::{}", rel_path, caller.name)) {
                    // В том же файле
                    if let Some(dst) = symbol_ids.get(&format!("{}::{}", rel_path, simple_name(&call.callee))) {
                        if *dst != *src {
                            self.store.upsert_edge(&Edge {
                                src_id: *src,
                                dst_id: *dst,
                                kind: EdgeKind::Calls,
                                confidence: Confidence::Certain,
                                weight: 1.0,
                            })?;
                        }
                    } else {
                        // В другом файле
                        let candidates = self.store.symbols_by_name(&simple_name(&call.callee))?;
                        if candidates.len() == 1 && candidates[0].id != *src {
                            self.store.upsert_edge(&Edge {
                                src_id: *src,
                                dst_id: candidates[0].id,
                                kind: EdgeKind::Calls,
                                confidence: Confidence::Likely,
                                weight: 1.0,
                            })?;
                        }
                    }
                }
            }
        }

        // Восстанавливаем входящие связи из неизменённых файлов
        for (edge, path, name, kind) in preserved_edges {
            let new_dst = if kind == "file" {
                // Для файлов ищем по пути
                self.store.find_file_node_by_path(&path)?.map(|n| n.id)
            } else {
                // Для символов ищем по пути и имени
                self.store.find_node_by_path(&path)?
                    .filter(|n| n.name == name)
                    .map(|n| n.id)
            };
            if let Some(dst_id) = new_dst {
                self.store.upsert_edge(&Edge { dst_id, ..edge })?;
            }
        }

        Ok(())
    }

    /// Вычисляет ID родительского каталога для пути.
    fn parent_id_for_path(&mut self, path: &str) -> Result<Option<Uuid>> {
        let parent = Path::new(path).parent()
            .and_then(|p| p.to_str())
            .unwrap_or("");
        if parent.is_empty() || parent == "." {
            return Ok(None);
        }
        if let Some(node) = self.store.find_node_by_path(parent)? {
            return Ok(Some(node.id));
        }
        let dir_id = Uuid::new_v4();
        let dir_node = Node {
            id: dir_id,
            kind: NodeKind::Dir,
            path: parent.to_string(),
            name: Path::new(parent).file_name().unwrap_or_default().to_string_lossy().to_string(),
            parent_id: self.parent_id_for_path(parent)?,
            lang: None,
            hash: None,
            line_start: None,
            line_end: None,
            signature: None,
            doc_summary: None,
            tokens_est: 0,
            rank: 0.0,
        };
        self.store.upsert_node(&dir_node)?;
        Ok(Some(dir_id))
    }

    fn resolve_import_target(&self, import_target: &str, from_path: &str) -> Result<Option<Uuid>> {
        const SUFFIXES: [&str; 10] = [
            "", ".py", "/__init__.py", ".ts", ".tsx", ".js", ".jsx", "/index.ts", "/index.js", ".rs",
        ];
        let target = import_target.trim();
        if target.is_empty() {
            return Ok(None);
        }
        let mut bases: Vec<String> = Vec::new();
        if target.starts_with("./") || target.starts_with("../") {
            let dir = Path::new(from_path).parent().unwrap_or(Path::new(""));
            bases.push(normalize_path(&dir.join(target)));
        } else {
            bases.push(normalize_path(Path::new(target)));
            if !target.contains('/') {
                bases.push(target.replace('.', "/"));
            }
        }
        for base in &bases {
            for suffix in SUFFIXES {
                let candidate = format!("{}{}", base, suffix);
                let has_ext = Path::new(&candidate).extension().is_some();
                if !has_ext {
                    continue;
                }
                if let Some(node) = self.store.find_file_node_by_path(&candidate)? {
                    return Ok(Some(node.id));
                }
            }
        }
        Ok(None)
    }
}

// Вспомогательные функции (дублированы из indexer.rs для самостоятельности)
fn atlas_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn is_generic_name(name: &str) -> bool {
    const GENERIC: [&str; 24] = [
        "new", "default", "from", "into", "clone", "fmt", "drop", "eq", "hash", "len", "get", "set",
        "push", "pop", "insert", "remove", "next", "iter", "map", "to_string", "as_str", "unwrap",
        "ok", "is_empty",
    ];
    GENERIC.contains(&name)
}

fn normalize_path(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop();
            }
            other => parts.push(other.as_os_str().to_string_lossy().to_string()),
        }
    }
    parts.join("/")
}

fn simple_name(callee: &str) -> String {
    callee
        .rsplit(|c: char| c == '.' || c == ':')
        .next()
        .unwrap_or(callee)
        .trim()
        .to_string()
}

fn estimate_tokens(text: &str) -> i32 {
    (text.len() / 4).max(1) as i32
}

fn is_test_file(path: &str) -> bool {
    path.contains("test") || path.contains("spec")
}

fn is_generated_file(path: &str) -> bool {
    path.contains("generated") || path.contains("build/") || path.contains("target/")
}

fn map_symbol_kind(kind: &SymbolKind) -> NodeKind {
    match kind {
        SymbolKind::Function => NodeKind::Function,
        SymbolKind::Method => NodeKind::Method,
        SymbolKind::Class => NodeKind::Class,
        SymbolKind::Interface => NodeKind::Interface,
        SymbolKind::Constant => NodeKind::Constant,
        SymbolKind::Type => NodeKind::Type,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_hash_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("i.db")).unwrap();
        let mut upd = IncrementalUpdater::new(&mut store, dir.path().to_str().unwrap());

        let content = "def foo():\n    return 1\n";
        assert!(upd.update_file("a.py", content).unwrap(), "первый раз индексируется");
        assert!(!upd.update_file("a.py", content).unwrap(), "повторный — пропуск по хешу");
    }

    #[test]
    fn changed_file_is_reindexed_and_count_stays_stable() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("i.db")).unwrap();
        let root = dir.path().to_str().unwrap().to_string();

        {
            let mut upd = IncrementalUpdater::new(&mut store, &root);
            upd.update_file("a.py", "def foo():\n    return 1\n").unwrap();
        }
        let before = store.count_by_kind().unwrap();
        let foo_before: i64 = before
            .iter()
            .find(|(k, _)| k == "function")
            .map(|(_, n)| *n)
            .unwrap_or(0);
        assert_eq!(foo_before, 1);

        {
            let mut upd = IncrementalUpdater::new(&mut store, &root);
            // Меняем тело: символ тот же, число узлов не должно расти (нет дублей)
            assert!(upd.update_file("a.py", "def foo():\n    return 2\n").unwrap());
        }
        let after = store.count_by_kind().unwrap();
        let foo_after: i64 = after
            .iter()
            .find(|(k, _)| k == "function")
            .map(|(_, n)| *n)
            .unwrap_or(0);
        assert_eq!(foo_after, 1, "переиндексация не должна плодить дубли символов");
    }

    #[test]
    fn deleted_file_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("i.db")).unwrap();
        let root = dir.path().to_str().unwrap().to_string();

        std::fs::write(dir.path().join("a.py"), "def foo():\n    return 1\n").unwrap();
        {
            let mut upd = IncrementalUpdater::new(&mut store, &root);
            assert!(upd.update_file_from_disk("a.py").unwrap());
        }
        assert!(store.find_file_node_by_path("a.py").unwrap().is_some());

        std::fs::remove_file(dir.path().join("a.py")).unwrap();
        {
            let mut upd = IncrementalUpdater::new(&mut store, &root);
            assert!(upd.update_file_from_disk("a.py").unwrap());
        }
        assert!(store.find_file_node_by_path("a.py").unwrap().is_none(), "удалённый файл уходит из индекса");
    }
}