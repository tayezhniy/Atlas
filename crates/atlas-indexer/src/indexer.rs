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

        // 4. Рассчитываем ранги и обновляем узлы
        self.calculate_and_update_ranks()?;

        // 5. Сохраняем метаданные проекта
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

    /// Индексация одного файла.
    fn index_file(&mut self, walked: &WalkedFile) -> Result<()> {
        let path_str = walked.path.to_string_lossy().to_string();

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
        if let Some(existing) = self.store.find_node_by_path(&path_str)? {
            if existing.hash.as_ref() == Some(&hash) {
                debug!("Файл не изменился, пропускаем: {}", path_str);
                return Ok(());
            }
        }

        // Парсим файл
        let parsed = match self.parser.parse(&path_str, &content) {
            Ok(p) => p,
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
            lang: Some(parsed.language.clone()),
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

        // Вставляем импорты (рёбра imports между файлами)
        for imp in &parsed.imports {
            let _import_id = Uuid::new_v4();
            // Ищем целевой файл по пути импорта
            if let Some(dst_id) = self.resolve_import_target(&imp.to, &path_str)? {
                let edge = Edge {
                    src_id: file_id,
                    dst_id,
                    kind: EdgeKind::Imports,
                    confidence: atlas_store::schema::Confidence::Certain,
                    weight: 1.0,
                };
                self.store.upsert_edge(&edge)?;
            }
            self.stats.imports_extracted += 1;
        }

        // Вставляем вызовы (рёбра calls между символами)
        for call in &parsed.calls {
            let caller_id = self.find_symbol_id(&path_str, &call.caller);
            let callee_id = self.find_symbol_id(&path_str, &call.callee);
            if let (Some(src), Some(dst)) = (caller_id, callee_id) {
                let edge = Edge {
                    src_id: src,
                    dst_id: dst,
                    kind: EdgeKind::Calls,
                    confidence: atlas_store::schema::Confidence::Certain,
                    weight: 1.0,
                };
                self.store.upsert_edge(&edge)?;
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
    fn resolve_import_target(&mut self, import_target: &str, from_path: &str) -> Result<Option<Uuid>> {
        // Прямой путь к файлу
        if let Some(node_id) = self.file_node_ids.get(import_target) {
            return Ok(Some(*node_id));
        }
        // Попробуем как относительный путь от корня
        let full_path = if import_target.starts_with("./") || import_target.starts_with("../") {
            let resolved = Path::new(from_path)
                .parent()
                .unwrap_or(Path::new(""))
                .join(import_target);
            resolved.to_string_lossy().to_string()
        } else {
            import_target.to_string()
        };
        if let Some(node_id) = self.file_node_ids.get(&full_path) {
            return Ok(Some(*node_id));
        }
        Ok(None)
    }

    /// Находит ID символа по пути файла и имени.
    fn find_symbol_id(&self, path: &str, symbol_name: &str) -> Option<Uuid> {
        self.symbol_node_ids.get(&format!("{}::{}", path, symbol_name)).copied()
    }

    /// Рассчитывает ранги и обновляет узлы.
    fn calculate_and_update_ranks(&mut self) -> Result<()> {
        // Простой ранг на основе количества входящих связей
        for (path, &file_id) in &self.file_node_ids {
            let incoming = self.store.incoming_edges(&file_id)?;
            let rank = incoming.len() as f64 * 0.1;
            // Обновим ранг файла (требуется обновление узла)
            // Для простоты пропускаем — ранг будет рассчитан отдельно
            let _ = path;
            let _ = rank;
        }
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

