use crate::walker::{ProjectWalker, WalkedFile};
use atlas_core::ranking::Ranker;
use atlas_langs::LanguageParser;
use atlas_store::{db::Store, schema::{Edge, EdgeKind, FileMeta, Node, NodeKind}};
use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use std::path::Path;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;
use tracing::{info, warn, debug};

/// Статистика индексации.
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub files_scanned: usize,
    pub files_indexed: usize,
    pub symbols_extracted: usize,
    pub imports_extracted: usize,
    pub calls_extracted: usize,
    pub errors: usize,
}

/// Основной индексатор проекта (раздел 4 ТЗ).
/// Связывает обход файлов, парсинг tree-sitter и запись в atlas-store.
pub struct ProjectIndexer<'a> {
    store: &'a mut Store,
    parser: LanguageParser,
    ranker: Ranker,
    root_path: String,
    stats: IndexStats,
    // Кэш ID узлов по путям файлов для построения рёбер contains
    file_node_ids: HashMap<String, Uuid>,
    // Кэш ID узлов символов по qualified имени для построения рёбер calls
    symbol_node_ids: HashMap<String, Uuid>,
    // Вызовы и импорты разрешаются после обхода всех файлов: цель может быть ещё не проиндексирована
    pending_calls: Vec<PendingCall>,
    pending_imports: Vec<PendingImport>,
    // Входящие связи из неизменённых файлов в изменённый: (ребро, путь, имя и вид цели)
    preserved_edges: Vec<(Edge, String, String, String)>,
}

struct PendingCall {
    src: Uuid,
    path: String,
    callee: String,
}

struct PendingImport {
    src: Uuid,
    from_path: String,
    target: String,
}

impl<'a> ProjectIndexer<'a> {
    pub fn new(store: &'a mut Store, root_path: &str) -> Self {
        Self {
            store,
            parser: LanguageParser::new(),
            ranker: Ranker::new(),
            root_path: root_path.to_string(),
            stats: IndexStats::default(),
            file_node_ids: HashMap::new(),
            symbol_node_ids: HashMap::new(),
            pending_calls: Vec::new(),
            pending_imports: Vec::new(),
            preserved_edges: Vec::new(),
        }
    }

    /// Полная индексация проекта: обход, парсинг, запись в БД.
    pub fn index_project(&mut self) -> Result<IndexStats> {
        info!("Начало индексации проекта: {}", self.root_path);

        // 1. Создаём или обновляем запись проекта
        self.upsert_project_record()?;

        // 2. Обходим файлы
        let options = crate::walker::WalkOptions {
            root: Path::new(&self.root_path).to_path_buf(),
            ..Default::default()
        };
        let walker = ProjectWalker::new(options);
        let files = walker.walk()?;
        self.stats.files_scanned = files.len();
        info!("Найдено файлов для индексации: {}", files.len());

        // 3. Индексируем каждый файл
        for walked in &files {
            if let Err(e) = self.index_file(walked) {
                warn!("Ошибка индексации файла {}: {}", walked.path.display(), e);
                self.stats.errors += 1;
            }
        }

        // Файлы, удалённые с диска, убираем из индекса вместе со связями
        let on_disk: std::collections::HashSet<String> =
            files.iter().map(|f| self.relative_path(&f.path)).collect();
        for indexed in self.store.file_paths()? {
            if !on_disk.contains(&indexed) {
                self.store.delete_nodes_by_path(&indexed)?;
            }
        }

        // 4. Разрешаем связи, когда все узлы уже в базе
        self.resolve_pending()?;

        // 5. Рассчитываем ранги и обновляем узлы
        self.calculate_and_update_ranks()?;

        // 6. Сохраняем метаданные проекта
        self.store.conn_mut().execute(
            "UPDATE project SET last_indexed_at = ?1 WHERE root_path = ?2",
            params![Utc::now().to_rfc3339(), self.root_path],
        )?;

        info!(
            "Индексация завершена: {} файлов, {} символов, {} импортов, {} вызовов",
            self.stats.files_indexed,
            self.stats.symbols_extracted,
            self.stats.imports_extracted,
            self.stats.calls_extracted,
        );

        Ok(self.stats.clone())
    }

    /// Путь файла относительно корня проекта, с разделителем `/`.
    fn relative_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.root_path)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// Индексация одного файла.
    fn index_file(&mut self, walked: &WalkedFile) -> Result<()> {
        // В индексе хранятся пути относительно корня проекта (раздел 5.4 ТЗ)
        let path_str = self.relative_path(&walked.path);

        // Читаем содержимое файла
        let content = match std::fs::read_to_string(&walked.path) {
            Ok(c) => c,
            Err(e) => {
                warn!("Не удалось прочитать файл {}: {}", path_str, e);
                self.stats.errors += 1;
                return Ok(());
            }
        };

        // Вычисляем хеш содержимого
        let hash = atlas_hash(&content);

        // Проверяем, нужно ли переиндексировать (по хешу)
        if let Some(existing) = self.store.find_file_node_by_path(&path_str)? {
            if existing.hash.as_ref() == Some(&hash) {
                debug!("Файл не изменился, пропускаем: {}", path_str);
                // Узел остаётся прежним, но его id нужен для связей из других файлов
                self.file_node_ids.insert(path_str.clone(), existing.id);
                return Ok(());
            }
            // Файл изменился: убираем старые узлы и связи, чтобы не копились дубликаты.
            // Связи из других (неизменённых) файлов запоминаем: сами они не пересоберутся.
            for (edge, name, kind) in self.store.incoming_edges_from_other_files(&path_str)? {
                self.preserved_edges.push((edge, path_str.clone(), name, kind));
            }
            self.store.delete_nodes_by_path(&path_str)?;
        }

        // Парсим файл. Неподдерживаемый язык — не ошибка: файл попадает в индекс как узел без символов
        let parsed = match self.parser.parse(&path_str, &content) {
            Ok(p) => p,
            Err(e) if e.to_string().contains("unsupported language") => atlas_langs::ParsedFile {
                path: path_str.clone(),
                language: walked.language.clone().unwrap_or_default(),
                symbols: Vec::new(),
                imports: Vec::new(),
                calls: Vec::new(),
            },
            Err(e) => {
                warn!("Ошибка парсинга файла {}: {}", path_str, e);
                self.stats.errors += 1;
                return Ok(());
            }
        };

        // Создаём узел файла
        let file_id = Uuid::new_v4();
        let file_node = Node {
            id: file_id,
            kind: NodeKind::File,
            path: path_str.clone(),
            name: walked.path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            parent_id: self.parent_id_for_path(&path_str)?,
            lang: if parsed.language.is_empty() { None } else { Some(parsed.language.clone()) },
            hash: Some(hash),
            line_start: None,
            line_end: None,
            signature: None,
            doc_summary: None,
            tokens_est: estimate_tokens(&content),
            rank: 0.0,
        };
        self.store.upsert_node(&file_node)?;
        self.file_node_ids.insert(path_str.clone(), file_id);

        // Метаданные файла
        let (size, mtime) = match walked.path.metadata() {
            Ok(m) => (m.len() as i64, m.modified().ok().map(|t| t.into())),
            Err(_) => (0, None),
        };
        let file_meta: FileMeta = FileMeta {
            node_id: file_id,
            size,
            mtime,
            git_changes_30d: 0,
            last_commit: None,
            is_test: is_test_file(&path_str),
            is_generated: is_generated_file(&path_str),
        };
        self.store.upsert_file_meta(&file_meta)?;

        // Вставляем символы
        for sym in &parsed.symbols {
            let symbol_id = Uuid::new_v4();
            let symbol_node = Node {
                id: symbol_id,
                kind: map_symbol_kind(&sym.kind),
                path: path_str.clone(),
                name: sym.name.clone(),
                parent_id: Some(file_id),
                lang: Some(parsed.language.clone()),
                hash: None,
                line_start: Some(sym.line_start as i32),
                line_end: Some(sym.line_end as i32),
                signature: sym.signature.clone(),
                doc_summary: sym.doc.clone(),
                tokens_est: estimate_tokens(&sym.signature.clone().unwrap_or_default()),
                rank: 0.0,
            };
            self.store.upsert_node(&symbol_node)?;
            self.symbol_node_ids.insert(
                format!("{}::{}", path_str, sym.name),
                symbol_id,
            );
            self.stats.symbols_extracted += 1;
        }

        // Импорты: связь файл -> файл, разрешается в конце обхода
        for imp in &parsed.imports {
            self.pending_imports.push(PendingImport {
                src: file_id,
                from_path: path_str.clone(),
                target: imp.to.clone(),
            });
            self.stats.imports_extracted += 1;
        }

        // Вызовы: вызывающий — самый внутренний символ файла, чей диапазон строк содержит вызов
        for call in &parsed.calls {
            let caller = parsed
                .symbols
                .iter()
                .filter(|sy| {
                    matches!(sy.kind, atlas_langs::SymbolKind::Function | atlas_langs::SymbolKind::Method)
                        && sy.line_start <= call.line
                        && call.line <= sy.line_end
                })
                .min_by_key(|sy| sy.line_end - sy.line_start);
            if is_generic_name(&simple_name(&call.callee)) {
                // `new`, `default`, `get`… есть почти у каждого типа; без анализа типов связь была бы догадкой
                self.stats.calls_extracted += 1;
                continue;
            }
            if let Some(caller) = caller {
                if let Some(src) = self.find_symbol_id(&path_str, &caller.name) {
                    self.pending_calls.push(PendingCall {
                        src,
                        path: path_str.clone(),
                        callee: simple_name(&call.callee),
                    });
                }
            }
            self.stats.calls_extracted += 1;
        }

        self.stats.files_indexed += 1;
        Ok(())
    }

    /// Вставляет или обновляет запись проекта.
    fn upsert_project_record(&mut self) -> Result<()> {
        // Проверяем, существует ли проект
        let existing: Option<String> = self.store.conn().query_row(
            "SELECT id FROM project WHERE root_path = ?1",
            params![&self.root_path],
            |row| row.get(0),
        ).optional()?;

        let project_id = match existing {
            Some(id) => id,
            None => Uuid::new_v4().to_string(),
        };

        self.store.conn().execute(
            "INSERT OR REPLACE INTO project (id, root_path, schema_version, parser_version, last_indexed_at, name, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                project_id,
                &self.root_path,
                1, // schema_version
                1, // parser_version
                Utc::now().to_rfc3339(),
                Path::new(&self.root_path).file_name().unwrap_or_default().to_string_lossy(),
                Utc::now().to_rfc3339(),
            ],
        )?;
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
        // Ищем существующий узел каталога
        if let Some(node) = self.store.find_node_by_path(parent)? {
            return Ok(Some(node.id));
        }
        // Создаём узел каталога (рекурсивно для родителей)
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
        self.file_node_ids.insert(parent.to_string(), dir_id);
        Ok(Some(dir_id))
    }

    /// Разрешает цель импорта в ID узла файла.
    /// Пробует путь как есть, относительный путь от импортирующего файла и имя модуля
    /// (`pkg.mod` -> `pkg/mod.py`) с типичными расширениями и индексными файлами.
    fn resolve_import_target(&mut self, import_target: &str, from_path: &str) -> Result<Option<Uuid>> {
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
                // Каталоги тоже лежат в file_node_ids: у файла в имени есть расширение
                let has_ext = Path::new(&candidate).extension().is_some();
                if !has_ext {
                    continue;
                }
                if let Some(id) = self.file_node_ids.get(&candidate) {
                    return Ok(Some(*id));
                }
            }
        }
        Ok(None)
    }

    /// Находит ID символа по пути файла и имени.
    fn find_symbol_id(&self, path: &str, symbol_name: &str) -> Option<Uuid> {
        self.symbol_node_ids.get(&format!("{}::{}", path, symbol_name)).copied()
    }

    /// Превращает собранные вызовы и импорты в рёбра графа.
    /// Вызов в том же файле — `certain`; единственный символ с таким именем в проекте — `likely`;
    /// несколько кандидатов — связь не создаётся (лучше пропуск, чем неверная связь).
    fn resolve_pending(&mut self) -> Result<()> {
        use atlas_store::schema::Confidence;

        let imports = std::mem::take(&mut self.pending_imports);
        for imp in imports {
            if let Some(dst) = self.resolve_import_target(&imp.target, &imp.from_path)? {
                if dst != imp.src {
                    self.store.upsert_edge(&Edge {
                        src_id: imp.src,
                        dst_id: dst,
                        kind: EdgeKind::Imports,
                        confidence: Confidence::Certain,
                        weight: 1.0,
                    })?;
                }
            }
        }

        // Входящие связи из неизменённых файлов: перепривязываем к новым узлам по пути и имени.
        // Если символ переименован или удалён, связь исчезает — так и должно быть.
        for (edge, path, name, kind) in std::mem::take(&mut self.preserved_edges) {
            let new_dst = if kind == "file" {
                self.file_node_ids.get(&path).copied()
            } else {
                self.find_symbol_id(&path, &name)
            };
            if let Some(dst_id) = new_dst {
                self.store.upsert_edge(&Edge { dst_id, ..edge })?;
            }
        }

        let calls = std::mem::take(&mut self.pending_calls);
        for call in calls {
            let same_file = self.find_symbol_id(&call.path, &call.callee);
            let (dst, confidence) = if let Some(id) = same_file {
                (id, Confidence::Certain)
            } else {
                let candidates = self.store.symbols_by_name(&call.callee)?;
                if candidates.len() == 1 {
                    (candidates[0].id, Confidence::Likely)
                } else {
                    continue;
                }
            };
            if dst != call.src {
                self.store.upsert_edge(&Edge {
                    src_id: call.src,
                    dst_id: dst,
                    kind: EdgeKind::Calls,
                    confidence,
                    weight: 1.0,
                })?;
            }
        }
        Ok(())
    }

    /// Рассчитывает ранги и обновляет узлы.
    fn calculate_and_update_ranks(&mut self) -> Result<()> {
        self.store.recompute_ranks()?;
        Ok(())
    }
}

impl Default for IndexStats {
    fn default() -> Self {
        Self {
            files_scanned: 0,
            files_indexed: 0,
            symbols_extracted: 0,
            imports_extracted: 0,
            calls_extracted: 0,
            errors: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Вспомогательные функции
// ---------------------------------------------------------------------------

fn atlas_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Имена методов, которые есть у множества типов: связь по такому имени почти всегда ложная.
fn is_generic_name(name: &str) -> bool {
    const GENERIC: [&str; 24] = [
        "new", "default", "from", "into", "clone", "fmt", "drop", "eq", "hash", "len", "get", "set",
        "push", "pop", "insert", "remove", "next", "iter", "map", "to_string", "as_str", "unwrap",
        "ok", "is_empty",
    ];
    GENERIC.contains(&name)
}

/// Убирает `.` и `..` из пути, не обращаясь к файловой системе.
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

/// Последний сегмент имени вызова: `self.helper` -> `helper`, `a::b::run` -> `run`.
fn simple_name(callee: &str) -> String {
    callee
        .rsplit(|c: char| c == '.' || c == ':')
        .next()
        .unwrap_or(callee)
        .trim()
        .to_string()
}

fn estimate_tokens(text: &str) -> i32 {
    // Примерная оценка: 1 токен ≈ 4 символа
    (text.len() / 4).max(1) as i32
}

fn is_test_file(path: &str) -> bool {
    path.contains("test") || path.contains("spec")
}

fn is_generated_file(path: &str) -> bool {
    path.contains("generated") || path.contains("build/") || path.contains("target/")
}

fn map_symbol_kind(kind: &atlas_langs::SymbolKind) -> NodeKind {
    match kind {
        atlas_langs::SymbolKind::Function => NodeKind::Function,
        atlas_langs::SymbolKind::Method => NodeKind::Method,
        atlas_langs::SymbolKind::Class => NodeKind::Class,
        atlas_langs::SymbolKind::Interface => NodeKind::Interface,
        atlas_langs::SymbolKind::Constant => NodeKind::Constant,
        atlas_langs::SymbolKind::Type => NodeKind::Type,
    }
}

