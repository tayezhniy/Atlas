use crate::error::{Result, StoreError};
use crate::schema::{Edge, EdgeKind, FileMeta, Node, NodeKind};
use rusqlite::{Connection, params};
use std::path::Path;

/// Обёртка над соединением с базой данных индекса.
pub struct Store {
    conn: Connection,
}

/// Алиас для удобства работы с транзакциями.
pub type StoreConnection<'a> = rusqlite::Connection;

impl Store {
    /// Открывает (или создаёт) базу данных по указанному пути и применяет миграции.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        // Включаем WAL для mejores характеристик параллельного чтения
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        // Нужно, чтобы INSERT OR REPLACE удалял старую запись и из FTS-индекса
        conn.pragma_update(None, "recursive_triggers", "ON")?;

        Self::run_migrations(&conn)?;
        Ok(Self { conn })
    }

    /// Возвращает внутреннее соединение (для транзакций и продвинутых операций).
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Возвращает изменяемое borrowing на соединение.
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Выполняет все миграции схемы (раздел 5.4 ТЗ: миграции выполняются автоматически).
    fn run_migrations(conn: &Connection) -> Result<()> {
        conn.execute_batch(include_str!("migrations/001_initial.sql"))
            .map_err(|e| StoreError::Migration(e.to_string()))?;
        Ok(())
    }

    /// Вставляет узел в таблицу `node`.
    pub fn upsert_node(&self, node: &Node) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO node (id, kind, path, name, parent_id, lang, hash, line_start, line_end, signature, doc_summary, tokens_est, rank)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                node.id.to_string(),
                node.kind.to_string(),
                node.path,
                node.name,
                node.parent_id.map(|id| id.to_string()),
                node.lang,
                node.hash,
                node.line_start,
                node.line_end,
                node.signature,
                node.doc_summary,
                node.tokens_est,
                node.rank,
            ],
        )?;
        Ok(())
    }

    /// Вставляет ребро в таблицу `edge`.
    /// Если один из узлов не существует (например, был удалён при переиндексации),
    /// ребро пропускается вместо возврата ошибки внешнего ключа.
    pub fn upsert_edge(&self, edge: &Edge) -> Result<()> {
        if !self.node_exists(&edge.src_id)? || !self.node_exists(&edge.dst_id)? {
            return Ok(());
        }
        self.conn.execute(
            "INSERT OR REPLACE INTO edge (src_id, dst_id, kind, confidence, weight)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                edge.src_id.to_string(),
                edge.dst_id.to_string(),
                edge.kind.to_string(),
                edge.confidence.to_string(),
                edge.weight,
            ],
        )?;
        Ok(())
    }

    /// Вставляет метаданные файла в таблицу `file_meta`.
    pub fn upsert_file_meta(&self, meta: &FileMeta) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO file_meta (node_id, size, mtime, git_changes_30d, last_commit, is_test, is_generated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                meta.node_id.to_string(),
                meta.size,
                meta.mtime.map(|dt| dt.to_rfc3339()),
                meta.git_changes_30d,
                meta.last_commit,
                meta.is_test,
                meta.is_generated,
            ],
        )?;
        Ok(())
    }

    /// Находит узел по ID.
    pub fn get_node(&self, id: &uuid::Uuid) -> Result<Option<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, path, name, parent_id, lang, hash, line_start, line_end, signature, doc_summary, tokens_est, rank
             FROM node WHERE id = ?1",
        )?;
        match stmt.query_row(params![id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let id = uuid::Uuid::parse_str(&id_str).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
            let kind_str: String = row.get(1)?;
            let kind = parse_node_kind(&kind_str).map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e)))?;
            let parent_id: Option<String> = row.get(4)?;
            let parent_id = match parent_id {
                Some(s) => Some(uuid::Uuid::parse_str(&s).map_err(|e| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e)))?),
                None => None,
            };
            Ok(Node {
                id,
                kind,
                path: row.get(2)?,
                name: row.get(3)?,
                parent_id,
                lang: row.get(5)?,
                hash: row.get(6)?,
                line_start: row.get(7)?,
                line_end: row.get(8)?,
                signature: row.get(9)?,
                doc_summary: row.get(10)?,
                tokens_est: row.get(11)?,
                rank: row.get(12)?,
            })
        }) {
            Ok(node) => Ok(Some(node)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StoreError::Sql(e)),
        }
    }

    /// Проверяет, существует ли узел с таким ID (без полной десериализации).
    pub fn node_exists(&self, id: &uuid::Uuid) -> Result<bool> {
        let exists: bool = self
            .conn
            .query_row("SELECT COUNT(*) > 0 FROM node WHERE id = ?1", params![id.to_string()], |r| r.get(0))?;
        Ok(exists)
    }

    /// Находит узел по точному пути.
    pub fn find_node_by_path(&self, path: &str) -> Result<Option<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, path, name, parent_id, lang, hash, line_start, line_end, signature, doc_summary, tokens_est, rank
             FROM node WHERE path = ?1",
        )?;
        match stmt.query_row(params![path], |row| {
            let id_str: String = row.get(0)?;
            let id = uuid::Uuid::parse_str(&id_str).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
            let kind_str: String = row.get(1)?;
            let kind = parse_node_kind(&kind_str).map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e)))?;
            let parent_id: Option<String> = row.get(4)?;
            let parent_id = match parent_id {
                Some(s) => Some(uuid::Uuid::parse_str(&s).map_err(|e| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e)))?),
                None => None,
            };
            Ok(Node {
                id,
                kind,
                path: row.get(2)?,
                name: row.get(3)?,
                parent_id,
                lang: row.get(5)?,
                hash: row.get(6)?,
                line_start: row.get(7)?,
                line_end: row.get(8)?,
                signature: row.get(9)?,
                doc_summary: row.get(10)?,
                tokens_est: row.get(11)?,
                rank: row.get(12)?,
            })
        }) {
            Ok(node) => Ok(Some(node)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StoreError::Sql(e)),
        }
    }

    /// Находит узлы по пути (префиксный поиск).
    pub fn find_nodes_by_path_prefix(&self, path_prefix: &str) -> Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, path, name, parent_id, lang, hash, line_start, line_end, signature, doc_summary, tokens_est, rank
             FROM node WHERE path LIKE ?1 ORDER BY path",
        )?;
        let rows = stmt.query_map(params![format!("{}%", path_prefix)], |row| {
            let id_str: String = row.get(0)?;
            let id = uuid::Uuid::parse_str(&id_str).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
            let kind_str: String = row.get(1)?;
            let kind = parse_node_kind(&kind_str).map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e)))?;
            let parent_id: Option<String> = row.get(4)?;
            let parent_id = match parent_id {
                Some(s) => Some(uuid::Uuid::parse_str(&s).map_err(|e| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e)))?),
                None => None,
            };
            Ok(Node {
                id,
                kind,
                path: row.get(2)?,
                name: row.get(3)?,
                parent_id,
                lang: row.get(5)?,
                hash: row.get(6)?,
                line_start: row.get(7)?,
                line_end: row.get(8)?,
                signature: row.get(9)?,
                doc_summary: row.get(10)?,
                tokens_est: row.get(11)?,
                rank: row.get(12)?,
            })
        })?;
        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(row.map_err(StoreError::Sql)?);
        }
        Ok(nodes)
    }

    /// Находит входящие рёбра для узла.
    pub fn incoming_edges(&self, dst_id: &uuid::Uuid) -> Result<Vec<Edge>> {
        let mut stmt = self.conn.prepare(
            "SELECT src_id, dst_id, kind, confidence, weight FROM edge WHERE dst_id = ?1",
        )?;
        let rows = stmt.query_map(params![dst_id.to_string()], parse_edge)?;
        let mut edges = Vec::new();
        for row in rows {
            edges.push(row?);
        }
        Ok(edges)
    }

    /// Находит исходящие рёбра из узла.
    pub fn outgoing_edges(&self, src_id: &uuid::Uuid) -> Result<Vec<Edge>> {
        let mut stmt = self.conn.prepare(
            "SELECT src_id, dst_id, kind, confidence, weight FROM edge WHERE src_id = ?1",
        )?;
        let rows = stmt.query_map(params![src_id.to_string()], parse_edge)?;
        let mut edges = Vec::new();
        for row in rows {
            edges.push(row?);
        }
        Ok(edges)
    }


    /// Находит узел файла (kind = 'file') по пути. У символов путь совпадает с путём файла.
    pub fn find_file_node_by_path(&self, path: &str) -> Result<Option<Node>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {NODE_COLS} FROM node WHERE path = ?1 AND kind = 'file'"
        ))?;
        let mut rows = stmt.query_map(params![path], row_to_node)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// Связи из других файлов в узлы этого файла: (ребро, имя и вид узла-цели).
    /// Нужны, чтобы перепривязать их после переиндексации изменённого файла.
    pub fn incoming_edges_from_other_files(&self, path: &str) -> Result<Vec<(Edge, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.src_id, e.dst_id, e.kind, e.confidence, e.weight, d.name, d.kind
             FROM edge e JOIN node d ON d.id = e.dst_id JOIN node s ON s.id = e.src_id
             WHERE d.path = ?1 AND s.path != ?1",
        )?;
        let rows = stmt.query_map(params![path], |row| {
            Ok((parse_edge(row)?, row.get::<_, String>(5)?, row.get::<_, String>(6)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Удаляет файл и все его символы вместе со связями (перед переиндексацией).
    pub fn delete_nodes_by_path(&self, path: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM edge WHERE src_id IN (SELECT id FROM node WHERE path = ?1)
                                 OR dst_id IN (SELECT id FROM node WHERE path = ?1)",
            params![path],
        )?;
        self.conn.execute(
            "DELETE FROM file_meta WHERE node_id IN (SELECT id FROM node WHERE path = ?1)",
            params![path],
        )?;
        self.conn.execute("DELETE FROM node WHERE path = ?1", params![path])?;
        Ok(())
    }

    /// Все узлы индекса, по убыванию ранга.
    pub fn get_all_nodes(&self) -> Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {NODE_COLS} FROM node ORDER BY rank DESC, path, line_start"
        ))?;
        let rows = stmt.query_map([], row_to_node)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Все рёбра индекса.
    pub fn get_all_edges(&self) -> Result<Vec<Edge>> {
        let mut stmt = self.conn.prepare("SELECT src_id, dst_id, kind, confidence, weight FROM edge")?;
        let rows = stmt.query_map([], parse_edge)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Все узлы заданных видов, по убыванию ранга.
    pub fn nodes_of_kinds(&self, kinds: &[NodeKind]) -> Result<Vec<Node>> {
        let list: Vec<String> = kinds.iter().map(|k| format!("'{}'", k)).collect();
        let sql = format!(
            "SELECT {NODE_COLS} FROM node WHERE kind IN ({}) ORDER BY rank DESC, path, line_start",
            list.join(",")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_node)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Все узлы-файлы индекса.
    pub fn file_nodes(&self) -> Result<Vec<Node>> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {NODE_COLS} FROM node WHERE kind = 'file' ORDER BY path"))?;
        let rows = stmt.query_map([], row_to_node)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Все узлы-символы (всё, кроме каталогов и файлов).
    pub fn symbol_nodes(&self) -> Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {NODE_COLS} FROM node WHERE kind NOT IN ('dir','file') ORDER BY path, line_start"
        ))?;
        let rows = stmt.query_map([], row_to_node)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Все узлы одного файла (сам файл и символы), по порядку строк.
    pub fn nodes_in_file(&self, path: &str) -> Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {NODE_COLS} FROM node WHERE path = ?1 ORDER BY (kind = 'file') DESC, line_start"
        ))?;
        let rows = stmt.query_map(params![path], row_to_node)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Символы с точным именем (без файлов и каталогов), по убыванию ранга.
    pub fn symbols_by_name(&self, name: &str) -> Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {NODE_COLS} FROM node WHERE name = ?1 AND kind NOT IN ('file','dir') ORDER BY rank DESC, path"
        ))?;
        let rows = stmt.query_map(params![name], row_to_node)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Полнотекстовый поиск по имени, сигнатуре, докстрингу и пути.
    /// Запрос превращается в безопасное префиксное выражение FTS5.
    pub fn search_nodes(&self, query: &str, limit: usize) -> Result<Vec<Node>> {
        let terms: Vec<String> = query
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|t| !t.is_empty())
            .map(|t| format!("\"{}\"*", t))
            .collect();
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!(
            "SELECT {} FROM search_fts f JOIN node n ON n.rowid = f.rowid
             WHERE search_fts MATCH ?1 AND n.kind != 'dir'
             ORDER BY bm25(search_fts, 10.0, 3.0, 1.0, 0.5), n.rank DESC LIMIT ?2",
            NODE_COLS_N
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![terms.join(" "), limit as i64], row_to_node)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Пути всех файлов, которые есть в индексе.
    pub fn file_paths(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT path FROM node WHERE kind = 'file'")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Пересчитывает ранги всех узлов: число входящих `imports` и `calls`, нормированное на максимум.
    /// Считает по всему индексу, поэтому результат верен и после частичной переиндексации.
    pub fn recompute_ranks(&self) -> Result<()> {
        self.conn.execute(
            "UPDATE node SET rank = 1.0 * (
                SELECT COUNT(*) FROM edge e WHERE e.dst_id = node.id AND e.kind IN ('imports', 'calls')
             ) / MAX(1, COALESCE((
                SELECT MAX(c) FROM (
                    SELECT COUNT(*) AS c FROM edge WHERE kind IN ('imports', 'calls') GROUP BY dst_id
                )
             ), 1))
             WHERE kind != 'dir'",
            [],
        )?;
        Ok(())
    }

    /// Обновляет ранг узла.
    pub fn set_rank(&self, id: &uuid::Uuid, rank: f64) -> Result<()> {
        self.conn.execute(
            "UPDATE node SET rank = ?1 WHERE id = ?2",
            params![rank, id.to_string()],
        )?;
        Ok(())
    }

    /// Корень проекта и время последней индексации (если проект уже индексировался).
    pub fn project_info(&self) -> Result<Option<(String, Option<String>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT root_path, last_indexed_at FROM project ORDER BY created_at DESC LIMIT 1")?;
        let mut rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// Количество узлов каждого вида.
    pub fn count_by_kind(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT kind, COUNT(*) FROM node GROUP BY kind ORDER BY COUNT(*) DESC")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Количество рёбер каждого вида.
    pub fn count_edges_by_kind(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT kind, COUNT(*) FROM edge GROUP BY kind ORDER BY COUNT(*) DESC")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Записывает вызов инструмента в журнал (раздел 10.3 ТЗ).
    pub fn log_call(&self, session_id: &str, tool: &str, args: &str, tokens_out: usize, node_ids: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO call_log (session_id, tool, args, tokens_out, node_ids) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_id, tool, args, tokens_out as i64, node_ids],
        )?;
        Ok(())
    }

    /// Проверка целостности базы для `atlas doctor`.
    pub fn integrity_check(&self) -> Result<String> {
        let r: String = self.conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        Ok(r)
    }

    /// Начинает транзакцию для пакетной записи (используется индексатором).
    pub fn transaction(&mut self) -> rusqlite::Result<rusqlite::Transaction<'_>> {
        self.conn.transaction()
    }

    /// Находит узел каталога по точному пути (не путать с файлом/символом с тем же путём).
    pub fn find_dir_node_by_path(&self, path: &str) -> Result<Option<Node>> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {NODE_COLS} FROM node WHERE path = ?1 AND kind = 'dir'"))?;
        let mut rows = stmt.query_map(params![path], row_to_node)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// Возвращает узлы по списку идентификаторов (порядок не гарантируется).
    pub fn get_nodes_by_ids(&self, ids: &[uuid::Uuid]) -> Result<Vec<Node>> {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(node) = self.get_node(id)? {
                out.push(node);
            }
        }
        Ok(out)
    }

    /// Сохраняет настройку проекта (таблица `settings`).
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Читает настройку проекта.
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |r| r.get::<_, String>(0))?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// Все настройки проекта.
    pub fn all_settings(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Число файлов в индексе.
    pub fn file_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM node WHERE kind = 'file'", [], |r| r.get(0))?)
    }

    /// Самое позднее время изменения среди файлов индекса (RFC3339, лексикографически сравнимо).
    pub fn max_file_mtime(&self) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT MAX(mtime) FROM file_meta")?;
        let mut rows = stmt.query_map([], |r| r.get::<_, Option<String>>(0))?;
        match rows.next() {
            Some(r) => Ok(r?),
            None => Ok(None),
        }
    }

    /// Файлы, отсортированные по времени изменения (свежие сверху).
    pub fn files_by_mtime(&self, limit: usize) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT n.path, COALESCE(m.mtime, '') FROM node n
             LEFT JOIN file_meta m ON m.node_id = n.id
             WHERE n.kind = 'file' AND m.mtime IS NOT NULL
             ORDER BY m.mtime DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Сводка журнала вызовов: инструмент, число вызовов, суммарные токены ответов.
    pub fn call_log_summary(&self) -> Result<Vec<(String, i64, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT tool, COUNT(*), COALESCE(SUM(tokens_out), 0) FROM call_log GROUP BY tool ORDER BY 3 DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Последние записи журнала: время, инструмент, токены, идентификаторы узлов.
    pub fn recent_calls(&self, limit: usize) -> Result<Vec<(String, String, i64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT time, tool, tokens_out, COALESCE(node_ids, '') FROM call_log ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Удаляет каталоги, у которых не осталось дочерних узлов.
    pub fn clear_empty_dirs(&self) -> Result<()> {
        // Повторяем, пока есть что удалять: пустой родитель может стать пустым после удаления ребёнка
        loop {
            let removed = self.conn.execute(
                "DELETE FROM node WHERE kind = 'dir'
                 AND id NOT IN (SELECT DISTINCT parent_id FROM node WHERE parent_id IS NOT NULL)",
                [],
            )?;
            if removed == 0 {
                break;
            }
        }
        Ok(())
    }

    /// Полная очистка индекса перед повторной индексацией с нуля.
    pub fn clear_index(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM edge; DELETE FROM file_meta; DELETE FROM node; DELETE FROM search_fts;",
        )?;
        Ok(())
    }
}

fn parse_node_kind(s: &str) -> Result<NodeKind> {
    match s {
        "dir" => Ok(NodeKind::Dir),
        "file" => Ok(NodeKind::File),
        "module" => Ok(NodeKind::Module),
        "class" => Ok(NodeKind::Class),
        "interface" => Ok(NodeKind::Interface),
        "function" => Ok(NodeKind::Function),
        "method" => Ok(NodeKind::Method),
        "constant" => Ok(NodeKind::Constant),
        "type" => Ok(NodeKind::Type),
        "test" => Ok(NodeKind::Test),
        "doc" => Ok(NodeKind::Doc),
        "config" => Ok(NodeKind::Config),
        other => Err(StoreError::Serialization(format!(
            "unknown node kind: {}",
            other
        ))),
    }
}

fn parse_edge(row: &rusqlite::Row<'_>) -> rusqlite::Result<Edge> {
    let src_id_str: String = row.get(0)?;
    let src_id = uuid::Uuid::parse_str(&src_id_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
    let dst_id_str: String = row.get(1)?;
    let dst_id = uuid::Uuid::parse_str(&dst_id_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e)))?;
    let kind_str: String = row.get(2)?;
    let kind = match kind_str.as_str() {
        "contains" => EdgeKind::Contains,
        "imports" => EdgeKind::Imports,
        "calls" => EdgeKind::Calls,
        "extends" => EdgeKind::Extends,
        "tests" => EdgeKind::Tests,
        "mentions" => EdgeKind::Mentions,
        "co_changes" => EdgeKind::CoChanges,
        other => return Err(rusqlite::Error::InvalidColumnType(2, other.to_string(), rusqlite::types::Type::Text)),
    };
    let conf_str: String = row.get(3)?;
    let confidence = match conf_str.as_str() {
        "certain" => Confidence::Certain,
        "likely" => Confidence::Likely,
        "guess" => Confidence::Guess,
        other => return Err(rusqlite::Error::InvalidColumnType(3, other.to_string(), rusqlite::types::Type::Text)),
    };
    Ok(Edge {
        src_id,
        dst_id,
        kind,
        confidence,
        weight: row.get(4)?,
    })
}

use crate::schema::Confidence;

const NODE_COLS: &str = "id, kind, path, name, parent_id, lang, hash, line_start, line_end, signature, doc_summary, tokens_est, rank";
const NODE_COLS_N: &str = "n.id, n.kind, n.path, n.name, n.parent_id, n.lang, n.hash, n.line_start, n.line_end, n.signature, n.doc_summary, n.tokens_est, n.rank";

fn row_to_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<Node> {
    let conv = |i: usize, e: Box<dyn std::error::Error + Send + Sync>| {
        rusqlite::Error::FromSqlConversionFailure(i, rusqlite::types::Type::Text, e)
    };
    let id_str: String = row.get(0)?;
    let id = uuid::Uuid::parse_str(&id_str).map_err(|e| conv(0, Box::new(e)))?;
    let kind_str: String = row.get(1)?;
    let kind = parse_node_kind(&kind_str).map_err(|e| conv(1, Box::new(e)))?;
    let parent: Option<String> = row.get(4)?;
    let parent_id = match parent {
        Some(s) => Some(uuid::Uuid::parse_str(&s).map_err(|e| conv(4, Box::new(e)))?),
        None => None,
    };
    Ok(Node {
        id,
        kind,
        path: row.get(2)?,
        name: row.get(3)?,
        parent_id,
        lang: row.get(5)?,
        hash: row.get(6)?,
        line_start: row.get(7)?,
        line_end: row.get(8)?,
        signature: row.get(9)?,
        doc_summary: row.get(10)?,
        tokens_est: row.get(11)?,
        rank: row.get(12)?,
    })
}

/// Читает файл проекта построчно с проверкой границ проекта (раздел 10.1 ТЗ).
/// Возвращает строки файла; ошибка, если путь вне корня или файл не найден.
pub fn read_project_lines(root: &str, rel_path: &str) -> anyhow::Result<Vec<String>> {
    let root = std::fs::canonicalize(root)
        .map_err(|e| anyhow::anyhow!("корень проекта недоступен: {e}"))?;
    let full: std::path::PathBuf = root.join(rel_path);
    let canonical = std::fs::canonicalize(&full)
        .map_err(|_| anyhow::anyhow!("файл «{rel_path}» не найден на диске"))?;
    if !canonical.starts_with(&root) {
        anyhow::bail!("путь «{rel_path}» вне проекта");
    }
    let content = std::fs::read_to_string(&canonical)
        .map_err(|e| anyhow::anyhow!("не удалось прочитать «{rel_path}»: {e}"))?;
    Ok(content.lines().map(str::to_string).collect())
}
