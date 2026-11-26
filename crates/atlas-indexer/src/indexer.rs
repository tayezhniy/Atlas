//! Основной индексатор проекта (раздел 4 ТЗ).
//!
//! Связывает обход файлов, парсинг tree-sitter и запись в atlas-store.
//! Один и тот же код индексирует и весь проект, и отдельные изменившиеся файлы.

use crate::support::{
    atlas_hash, estimate_tokens, is_generated_file, is_generic_name, is_test_file, map_symbol_kind,
    resolve_import_target as resolve_import, simple_name,
};
use crate::walker::{ProjectWalker, WalkedFile, WalkOptions};
use crate::lsp_client::{LspClient, LspRegistry};
use atlas_langs::{LanguageParser, ParsedFile};
use atlas_store::db::Store;
use atlas_store::schema::{Confidence, Edge, EdgeKind, FileMeta, Node, NodeKind};
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde_json;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Статистика индексации.
#[derive(Debug, Clone, Default)]
pub struct IndexStats {
    pub files_scanned: usize,
    pub files_indexed: usize,
    pub symbols_extracted: usize,
    pub imports_extracted: usize,
    pub calls_extracted: usize,
    /// Вызовы, для которых найден вызываемый символ (связь попала в граф).
    pub calls_linked: usize,
    /// Вызовы, которые не удалось разрешить (неоднозначное имя).
    pub calls_unresolved: usize,
    /// Связи с уверенностью `guess`: неоднозначное имя, привязка ко всем кандидатам.
    pub calls_guessed: usize,
    pub errors: usize,
}

/// Основной индексатор проекта (раздел 4 ТЗ).
pub struct ProjectIndexer<'a> {
    store: &'a mut Store,
    parser: LanguageParser,
    root_path: String,
    stats: IndexStats,
    /// Путь файла -> ID узла файла (для разрешения импортов).
    file_node_ids: HashMap<String, Uuid>,
    /// `путь::имя` -> ID узла символа.
    symbol_node_ids: HashMap<String, Uuid>,
    pending_calls: Vec<PendingRef>,
    pending_imports: Vec<PendingImport>,
    pending_extends: Vec<PendingRef>,
    /// Входящие связи из неизменённых файлов в изменённый: (ребро, путь, имя, вид цели)
    preserved_edges: Vec<(Edge, String, String, String)>,
    /// Опциональный LSP-клиент для динамического разрешения вызовов.
    lsp_client: Option<Arc<LspClient>>,
}

/// Максимум кандидатов, на которые ставится связь `guess` при неоднозначном имени.
const MAX_GUESS_CANDIDATES: usize = 3;

struct PendingRef {
    src: Uuid,
    path: String,
    name: String,
}

struct PendingImport {
    src: Uuid,
    from_path: String,
    target: String,
}

/// Увеличивается, когда меняется извлечение символов: старый индекс перечитывается целиком.
const PARSER_VERSION: i64 = 3;

impl<'a> ProjectIndexer<'a> {
    pub fn new(store: &'a mut Store, root_path: &str) -> Self {
        Self {
            store,
            parser: LanguageParser::new(),
            root_path: root_path.to_string(),
            stats: IndexStats::default(),
            file_node_ids: HashMap::new(),
            symbol_node_ids: HashMap::new(),
            pending_calls: Vec::new(),
            pending_imports: Vec::new(),
            pending_extends: Vec::new(),
            preserved_edges: Vec::new(),
            lsp_client: None,
        }
    }

    /// Инициализирует LSP-клиенты для поддерживаемых языков в проекте.
    pub async fn init_lsp_clients(&mut self) -> Result<()> {
        let root_path = PathBuf::from(&self.root_path);
        let client = LspClient::new(&root_path).await?;
        let configs = LspRegistry::all();
        
        for config in configs {
            if root_path.join(config.root_markers[0]).exists() {
                if let Err(e) = client.ensure_started(&config).await {
                    warn!("Failed to start LSP server for {}: {}", config.language, e);
                } else {
                    info!("LSP server started for {}", config.language);
                }
            }
        }
        
        self.lsp_client = Some(Arc::new(client));
        Ok(())
    }

    /// Улучшает разрешение вызовов с помощью LSP call hierarchy.
    /// Улучшает граф вызовов с помощью LSP call hierarchy.
    /// Проходит по всем функциям/методам в проекте и запрашивает входящие/исходящие вызовы.
    async fn enhance_calls_with_lsp(&mut self) -> Result<()> {
        let Some(lsp_client) = &self.lsp_client else {
            return Ok(());
        };

        info!("Enhancing call graph with LSP call hierarchy...");

        // Получаем все символы-функции/методы из БД
        let symbols = self.store.nodes_of_kinds(&[NodeKind::Function, NodeKind::Method])?;

        let mut enhanced_count = 0;
        for symbol in symbols {
            let Some(line_start) = symbol.line_start else { continue };
            let path = Path::new(&symbol.path);

            // Используем начало диапазона символа для позиции
            let line = line_start as u32;
            let character = 0;

            // Получаем входящие вызовы (кто вызывает эту функцию)
            let Ok(incoming) = lsp_client.incoming_calls(path, line, character).await else {
                warn!("LSP не отвечает, дополнение графа остановлено");
                break;
            };
            {
                for inc in incoming {
                    if let Some(dst_id) = self.find_symbol_id(&inc.callee_path, &inc.callee) {
                        if let Some(caller_id) = self.find_symbol_id(&inc.caller_path, &inc.caller) {
                            if caller_id != dst_id {
                                self.store.upsert_edge(&Edge {
                                    src_id: caller_id,
                                    dst_id,
                                    kind: EdgeKind::Calls,
                                    confidence: Confidence::Likely,
                                    weight: 1.0,
                                })?;
                                enhanced_count += 1;
                            }
                        }
                    }
                }
            }

            // Получаем исходящие вызовы (что вызывает эта функция)
            if let Ok(outgoing) = lsp_client.outgoing_calls(path, line, character).await {
                for out in outgoing {
                    if let Some(caller_id) = self.find_symbol_id(&out.caller_path, &out.caller) {
                        if let Some(callee_id) = self.find_symbol_id(&out.callee_path, &out.callee) {
                            if caller_id != callee_id {
                                self.store.upsert_edge(&Edge {
                                    src_id: caller_id,
                                    dst_id: callee_id,
                                    kind: EdgeKind::Calls,
                                    confidence: Confidence::Likely,
                                    weight: 1.0,
                                })?;
                                enhanced_count += 1;
                            }
                        }
                    }
                }
            }
        }

        if enhanced_count > 0 {
            self.stats.calls_linked += enhanced_count;
            info!("LSP enhanced call graph with {} additional edges", enhanced_count);
        }

        Ok(())
    }

    /// Полная индексация проекта: обход, парсинг, запись в БД.
    pub fn index_project(&mut self) -> Result<IndexStats> {
        info!("Начало индексации проекта: {}", self.root_path);
        
        // Инициализируем LSP-клиенты асинхронно
        let rt = tokio::runtime::Runtime::new()?;
        // LSP экспериментален и запускает внешние программы, поэтому только по явной просьбе
        if std::env::var("ATLAS_LSP").is_ok_and(|v| v == "1") {
            if let Err(e) = rt.block_on(self.init_lsp_clients()) {
                warn!("Failed to initialize LSP clients: {}", e);
            }
        }
        
        self.upsert_project_record()?;
        self.drop_outdated_index()?;
        self.preload_maps()?;

        // Читаем настройки проекта
        let mut custom_exclusions: Vec<String> = Vec::new();
        let mut max_file_size: u64 = 1_048_576;
        if let Ok(settings) = self.store.all_settings() {
            for (key, value) in settings {
                if key == "exclusions" {
                    if let Ok(excl) = serde_json::from_str::<Vec<String>>(&value) {
                        custom_exclusions = excl;
                    }
                } else if key == "maxFileSize" {
                    if let Ok(size) = value.parse::<u64>() {
                        max_file_size = size;
                    }
                }
            }
        }

        let options = WalkOptions {
            root: PathBuf::from(&self.root_path),
            max_file_size,
            custom_exclusions,
            ..Default::default()
        };
        let files = ProjectWalker::new(options).walk()?;
        self.stats.files_scanned = files.len();
        info!("Найдено файлов для индексации: {}", files.len());

        for walked in &files {
            if let Err(e) = self.index_walked_file(walked) {
                warn!("Ошибка индексации файла {}: {}", walked.path.display(), e);
                self.stats.errors += 1;
            }
        }

        self.delete_missing_files(&files)?;
        self.finish()?;
        // только после полного обхода: частичное обновление версию не меняет
        self.store.conn().execute(
            "UPDATE project SET parser_version = ?1 WHERE root_path = ?2",
            params![PARSER_VERSION, self.root_path],
        )?;

        // LSP-расширение: улучшаем граф вызовов через call hierarchy
        if self.lsp_client.is_some() {
            if let Err(e) = rt.block_on(self.enhance_calls_with_lsp()) {
                warn!("LSP call hierarchy enhancement failed: {}", e);
            }
        }

        Ok(self.stats.clone())
    }

    /// Инкрементальная индексация конкретных файлов (пути абсолютные).
    pub fn index_paths(&mut self, paths: Vec<PathBuf>) -> Result<IndexStats> {
        self.upsert_project_record()?;
        self.preload_maps()?;
        self.stats.files_scanned = paths.len();
        for path in &paths {
            if !path.is_file() {
                continue;
            }
            let walked = WalkedFile {
                path: path.clone(),
                size: path.metadata().map(|m| m.len()).unwrap_or(0),
                language: None,
                too_large: false,
            };
            if let Err(e) = self.index_walked_file(&walked) {
                warn!("Ошибка индексации файла {}: {}", path.display(), e);
                self.stats.errors += 1;
            }
        }
        self.finish()?;
        Ok(self.stats.clone())
    }

    /// Индексация одного файла из уже прочитанного содержимого (наблюдатель, тесты).
    pub fn index_content(&mut self, rel_path: &str, content: &str) -> Result<IndexStats> {
        self.upsert_project_record()?;
        self.preload_maps()?;
        self.stats.files_scanned = 1;
        let abs = PathBuf::from(&self.root_path).join(rel_path);
        let abs = abs.is_file().then_some(abs);
        if self.index_content_inner(rel_path, content, abs.as_deref())? {
            self.stats.files_indexed = 1;
        }
        self.finish()?;
        Ok(self.stats.clone())
    }

    /// Забывает файлы, разобранные прежней версией парсеров: их хеш не изменился,
    /// и без этого они не были бы прочитаны заново.
    fn drop_outdated_index(&mut self) -> Result<()> {
        let version: i64 = self.store.conn().query_row(
            "SELECT parser_version FROM project WHERE root_path = ?1",
            params![self.root_path],
            |row| row.get(0),
        )?;
        if version != PARSER_VERSION {
            for node in self.store.file_nodes()? {
                self.store.delete_nodes_by_path(&node.path)?;
            }
        }
        Ok(())
    }

    fn preload_maps(&mut self) -> Result<()> {
        self.file_node_ids.clear();
        self.symbol_node_ids.clear();
        for node in self.store.file_nodes()? {
            self.file_node_ids.insert(node.path.clone(), node.id);
        }
        for node in self.store.symbol_nodes()? {
            self.symbol_node_ids
                .insert(format!("{}::{}", node.path, node.name), node.id);
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        self.resolve_pending()?;
        atlas_core::ranking::recompute_ranks(self.store)?;
        self.store.conn().execute(
            "UPDATE project SET last_indexed_at = ?1, schema_version = 2
             WHERE root_path = ?2",
            params![Utc::now().to_rfc3339(), self.root_path],
        )?;
        info!(
            "Индексация завершена: {} файлов, {} символов, {} импортов, {} вызовов",
            self.stats.files_indexed,
            self.stats.symbols_extracted,
            self.stats.imports_extracted,
            self.stats.calls_extracted,
        );
        Ok(())
    }

    /// Удаляет из индекса файлы, которых больше нет на диске.
    fn delete_missing_files(&mut self, files: &[WalkedFile]) -> Result<()> {
        let on_disk: HashSet<String> = files.iter().map(|f| self.relative_path(&f.path)).collect();
        for node in self.store.file_nodes()? {
            if !on_disk.contains(&node.path) {
                self.store.delete_nodes_by_path(&node.path)?;
                self.file_node_ids.remove(&node.path);
            }
        }
        self.store.clear_empty_dirs()?;
        Ok(())
    }

    /// Путь файла относительно корня проекта, с разделителем `/`.
    fn relative_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.root_path)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn index_walked_file(&mut self, walked: &WalkedFile) -> Result<()> {
        let path_str = self.relative_path(&walked.path);
        // Крупные файлы не разбираем, но узел в индексе сохраняем (раздел 4.4 ТЗ)
        if walked.too_large {
            if self.index_oversized_file(&path_str, walked)? {
                self.stats.files_indexed += 1;
            }
            return Ok(());
        }
        let content = match std::fs::read_to_string(&walked.path) {
            Ok(c) => c,
            Err(e) => {
                warn!("Не удалось прочитать файл {}: {}", path_str, e);
                self.stats.errors += 1;
                return Ok(());
            }
        };
        if self.index_content_inner(&path_str, &content, Some(&walked.path))? {
            self.stats.files_indexed += 1;
        }
        Ok(())
    }

    /// Узел для файла больше порога: без чтения и разбора содержимого.
    fn index_oversized_file(&mut self, path_str: &str, walked: &WalkedFile) -> Result<bool> {
        let mtime = walked
            .path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let hash = format!("oversized:{}:{}", walked.size, mtime);
        if let Some(existing) = self.store.find_file_node_by_path(path_str)? {
            if existing.hash.as_ref() == Some(&hash) {
                self.file_node_ids.insert(path_str.to_string(), existing.id);
                return Ok(false);
            }
            for (edge, name, kind) in self.store.incoming_edges_from_other_files(path_str)? {
                self.preserved_edges
                    .push((edge, path_str.to_string(), name, kind));
            }
            self.store.delete_nodes_by_path(path_str)?;
        }
        let file_id = Uuid::new_v4();
        let parent_id = self.ensure_dir(path_str)?;
        self.store.upsert_node(&Node {
            id: file_id,
            kind: NodeKind::File,
            path: path_str.to_string(),
            name: Path::new(path_str)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            parent_id,
            lang: walked.language.clone(),
            hash: Some(hash),
            line_start: None,
            line_end: None,
            signature: None,
            doc_summary: Some("файл больше лимита: содержимое не разбиралось".to_string()),
            tokens_est: (walked.size / 4) as i32,
            rank: 0.0,
        })?;
        self.file_node_ids.insert(path_str.to_string(), file_id);
        self.store.upsert_file_meta(&FileMeta {
            node_id: file_id,
            size: walked.size as i64,
            mtime: None,
            git_changes_30d: 0,
            last_commit: None,
            is_test: is_test_file(path_str),
            is_generated: is_generated_file(path_str),
        })?;
        Ok(true)
    }

    /// Индексирует один файл. Возвращает true, если файл действительно переиндексирован.
    fn index_content_inner(
        &mut self,
        path_str: &str,
        content: &str,
        abs_path: Option<&Path>,
    ) -> Result<bool> {
        let hash = atlas_hash(content);

        if let Some(existing) = self.store.find_file_node_by_path(path_str)? {
            if existing.hash.as_ref() == Some(&hash) {
                debug!("Файл не изменился, пропускаем: {}", path_str);
                self.file_node_ids.insert(path_str.to_string(), existing.id);
                return Ok(false);
            }
            // Файл изменился: запоминаем входящие связи из неизменённых файлов
            for (edge, name, kind) in self.store.incoming_edges_from_other_files(path_str)? {
                self.preserved_edges
                    .push((edge, path_str.to_string(), name, kind));
            }
            self.store.delete_nodes_by_path(path_str)?;
        }

        // Неподдерживаемый язык — не ошибка: файл попадает в индекс как узел без символов
        let parsed = match self.parser.parse(path_str, content) {
            Ok(p) => p,
            Err(e) if e.to_string().contains("unsupported language") => ParsedFile {
                path: path_str.to_string(),
                language: String::new(),
                symbols: Vec::new(),
                imports: Vec::new(),
                calls: Vec::new(),
                extends: Vec::new(),
            },
            Err(e) => {
                warn!("Ошибка парсинга файла {}: {}", path_str, e);
                self.stats.errors += 1;
                return Ok(false);
            }
        };

        let file_id = Uuid::new_v4();
        let parent_id = self.ensure_dir(path_str)?;
        let file_node = Node {
            id: file_id,
            kind: NodeKind::File,
            path: path_str.to_string(),
            name: Path::new(path_str)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            parent_id,
            lang: (!parsed.language.is_empty()).then(|| parsed.language.clone()),
            hash: Some(hash),
            line_start: None,
            line_end: None,
            signature: None,
            doc_summary: None,
            tokens_est: estimate_tokens(content),
            rank: 0.0,
        };
        self.store.upsert_node(&file_node)?;
        self.file_node_ids.insert(path_str.to_string(), file_id);

        let (size, mtime) = match abs_path.and_then(|p| p.metadata().ok()) {
            Some(m) => (m.len() as i64, m.modified().ok().map(|t| t.into())),
            None => (content.len() as i64, None),
        };
        self.store.upsert_file_meta(&FileMeta {
            node_id: file_id,
            size,
            mtime,
            git_changes_30d: 0,
            last_commit: None,
            is_test: is_test_file(path_str),
            is_generated: is_generated_file(path_str),
        })?;

        for sym in &parsed.symbols {
            let symbol_id = Uuid::new_v4();
            let symbol_node = Node {
                id: symbol_id,
                kind: map_symbol_kind(&sym.kind),
                path: path_str.to_string(),
                name: sym.name.clone(),
                parent_id: Some(file_id),
                lang: Some(parsed.language.clone()),
                hash: None,
                line_start: Some(sym.line_start as i32),
                line_end: Some(sym.line_end as i32),
                signature: sym.signature.clone(),
                doc_summary: sym.doc.clone(),
                tokens_est: estimate_tokens(
                    &sym.signature.clone().unwrap_or_else(|| sym.name.clone()),
                ),
                rank: 0.0,
            };
            self.store.upsert_node(&symbol_node)?;
            self.symbol_node_ids
                .insert(format!("{}::{}", path_str, sym.name), symbol_id);
            self.stats.symbols_extracted += 1;
        }

        for imp in &parsed.imports {
            self.pending_imports.push(PendingImport {
                src: file_id,
                from_path: path_str.to_string(),
                target: imp.to.clone(),
            });
            self.stats.imports_extracted += 1;
        }

        // Вызовы: вызывающий — самый внутренний символ файла, содержащий строку вызова
        for call in &parsed.calls {
            let caller = parsed
                .symbols
                .iter()
                .filter(|sy| {
                    matches!(
                        sy.kind,
                        atlas_langs::SymbolKind::Function | atlas_langs::SymbolKind::Method
                    ) && sy.line_start <= call.line
                        && call.line <= sy.line_end
                })
                .min_by_key(|sy| sy.line_end - sy.line_start);
            self.stats.calls_extracted += 1;
            let callee = simple_name(&call.callee);
            if is_generic_name(&callee) {
                continue;
            }
            if let Some(caller) = caller {
                if let Some(src) = self.find_symbol_id(path_str, &caller.name) {
                    self.pending_calls.push(PendingRef {
                        src,
                        path: path_str.to_string(),
                        name: callee,
                    });
                }
            }
        }

        for heritage in &parsed.extends {
            if let Some(src) = self.find_symbol_id(path_str, &heritage.child) {
                self.pending_extends.push(PendingRef {
                    src,
                    path: path_str.to_string(),
                    name: heritage.parent.clone(),
                });
            }
        }

        Ok(true)
    }

    /// Вставляет или обновляет запись проекта.
    fn upsert_project_record(&mut self) -> Result<()> {
        let existing: Option<String> = self
            .store
            .conn()
            .query_row(
                "SELECT id FROM project WHERE root_path = ?1",
                params![&self.root_path],
                |row| row.get(0),
            )
            .optional()?;
        let project_id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
        self.store.conn().execute(
            "INSERT OR REPLACE INTO project (id, root_path, schema_version, parser_version, last_indexed_at, name, created_at)
             VALUES (?1, ?2, 2, COALESCE((SELECT parser_version FROM project WHERE id = ?1), ?5), ?3, ?4,
                     COALESCE((SELECT created_at FROM project WHERE id = ?1), ?3))",
            params![
                project_id,
                &self.root_path,
                Utc::now().to_rfc3339(),
                Path::new(&self.root_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                PARSER_VERSION,
            ],
        )?;
        Ok(())
    }

    /// Гарантирует наличие узлов каталогов для пути, возвращает ID родителя.
    fn ensure_dir(&mut self, path: &str) -> Result<Option<Uuid>> {
        let parent = match Path::new(path).parent().and_then(|p| p.to_str()) {
            Some(p) if !p.is_empty() && p != "." => p.to_string(),
            _ => return Ok(None),
        };
        if let Some(node) = self.store.find_dir_node_by_path(&parent)? {
            return Ok(Some(node.id));
        }
        let dir_id = Uuid::new_v4();
        let dir_parent = self.ensure_dir(&parent)?;
        let dir_node = Node {
            id: dir_id,
            kind: NodeKind::Dir,
            path: parent.clone(),
            name: Path::new(&parent)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            parent_id: dir_parent,
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

    fn find_symbol_id(&self, path: &str, symbol_name: &str) -> Option<Uuid> {
        self.symbol_node_ids
            .get(&format!("{}::{}", path, symbol_name))
            .copied()
    }

    fn resolve_import_target(&self, target: &str, from_path: &str) -> Option<Uuid> {
        resolve_import(&self.file_node_ids, target, from_path)
    }

    /// Превращает собранные вызовы, импорты и наследование в рёбра графа.
    fn resolve_pending(&mut self) -> Result<()> {
        let imports = std::mem::take(&mut self.pending_imports);
        // Карта «файл -> файлы, которые он импортирует»: нужна, чтобы сузить
        // кандидатов на роль вызываемого символа (раздел 4.5 ТЗ).
        let mut imports_by_file: HashMap<String, Vec<Uuid>> = HashMap::new();
        for imp in imports {
            if let Some(dst) = self.resolve_import_target(&imp.target, &imp.from_path) {
                imports_by_file.entry(imp.from_path.clone()).or_default().push(dst);
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

        // Входящие связи из неизменённых файлов: перепривязываем к новым узлам
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

        for call in std::mem::take(&mut self.pending_calls) {
            let same_file = self.find_symbol_id(&call.path, &call.name);
            let (dst, confidence) = if let Some(id) = same_file {
                (id, Confidence::Certain)
            } else {
                let candidates = self.store.symbols_by_name(&call.name)?;
                // Сначала сужаем кандидатов до файлов, которые вызывающий файл
                // импортирует: так имя `run` разрешается в `helper::run`, а не
                // теряется из-за одноимённых методов в других модулях.
                let imported: Vec<Uuid> = imports_by_file
                    .get(&call.path)
                    .cloned()
                    .unwrap_or_default();
                let scoped: Vec<Node> = candidates
                    .iter()
                    .filter(|n| {
                        self.file_node_ids
                            .get(&n.path)
                            .map(|f| imported.contains(f))
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();
                if scoped.len() == 1 {
                    (scoped[0].id, Confidence::Likely)
                } else if candidates.len() == 1 {
                    (candidates[0].id, Confidence::Likely)
                } else {
                    // Имена вида `run`/`new` живут в нескольких модулях. Сначала
                    // пробуем соседей по каталогу, затем — единственного кандидата
                    // «функция/метод» (конструкторы и классы отбрасываем).
                    let caller_dir = Path::new(&call.path)
                        .parent()
                        .and_then(|p| p.to_str())
                        .unwrap_or("");
                    let same_dir: Vec<&Node> = candidates
                        .iter()
                        .filter(|n| {
                            Path::new(&n.path)
                                .parent()
                                .and_then(|p| p.to_str())
                                .unwrap_or("")
                                == caller_dir
                        })
                        .collect();
                    let callable: Vec<&Node> = candidates
                        .iter()
                        .filter(|n| {
                            matches!(n.kind, NodeKind::Function | NodeKind::Method)
                        })
                        .collect();
                    if same_dir.len() == 1 {
                        (same_dir[0].id, Confidence::Guess)
                    } else if callable.len() == 1 {
                        (callable[0].id, Confidence::Guess)
                    } else if candidates.len() <= MAX_GUESS_CANDIDATES {
                        // Неоднозначное имя с небольшим числом кандидатов:
                        // привязываем ко всем, но с уверенностью `guess` —
                        // потребитель видит метку и может отфильтровать.
                        for cand in &candidates {
                            if cand.id == call.src {
                                continue;
                            }
                            self.store.upsert_edge(&Edge {
                                src_id: call.src,
                                dst_id: cand.id,
                                kind: EdgeKind::Calls,
                                confidence: Confidence::Guess,
                                weight: 1.0,
                            })?;
                            self.stats.calls_linked += 1;
                            self.stats.calls_guessed += 1;
                        }
                        continue;
                    } else {
                        self.stats.calls_unresolved += 1;
                        continue;
                    }
                }
            };
            self.stats.calls_linked += 1;
            if dst == call.src {
                continue;
            }
            self.store.upsert_edge(&Edge {
                src_id: call.src,
                dst_id: dst,
                kind: EdgeKind::Calls,
                confidence,
                weight: 1.0,
            })?;
            // Тест, вызывающий символ, дополнительно порождает ребро `tests`
            if is_test_file(&call.path) {
                self.store.upsert_edge(&Edge {
                    src_id: call.src,
                    dst_id: dst,
                    kind: EdgeKind::Tests,
                    confidence: Confidence::Likely,
                    weight: 1.0,
                })?;
            }
        }

        for ext in std::mem::take(&mut self.pending_extends) {
            if let Some(dst) = self.find_symbol_id(&ext.path, &ext.name) {
                if dst != ext.src {
                    self.store.upsert_edge(&Edge {
                        src_id: ext.src,
                        dst_id: dst,
                        kind: EdgeKind::Extends,
                        confidence: Confidence::Certain,
                        weight: 1.0,
                    })?;
                    continue;
                }
            }
            let candidates = self.store.symbols_by_name(&ext.name)?;
            if candidates.len() == 1 && candidates[0].id != ext.src {
                self.store.upsert_edge(&Edge {
                    src_id: ext.src,
                    dst_id: candidates[0].id,
                    kind: EdgeKind::Extends,
                    confidence: Confidence::Likely,
                    weight: 1.0,
                })?;
            }
        }

        // LSP-расширение: если есть LSP-клиент, пробуем улучшить вызовы через call hierarchy
        if let Some(_lsp_client) = &self.lsp_client {
            // Вызываем enhance_calls_with_lsp для улучшения графа вызовов
            // pending_calls уже обработаны статически, но мы можем добавить
            // дополнительные связи, найденные через LSP call hierarchy
            debug!("LSP client available for enhanced call resolution");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_estimated_by_chars_not_bytes() {
        // Кириллица: 8 символов -> 2 токена, а не 16/4 + запас
        assert_eq!(estimate_tokens("привет!!"), 2);
        assert_eq!(estimate_tokens(""), 1);
    }

    #[test]
    fn generic_and_simple_names() {
        assert!(is_generic_name("get"));
        assert_eq!(simple_name("self.helper"), "helper");
        assert_eq!(simple_name("a::b::run"), "run");
    }
}
