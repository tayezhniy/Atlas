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
        conn.pragma_update(None, "journal_mode", &"WAL")?;
        conn.pragma_update(None, "synchronous", &"NORMAL")?;
        conn.pragma_update(None, "foreign_keys", &"ON")?;

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
    pub fn upsert_edge(&self, edge: &Edge) -> Result<()> {
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
            nodes.push(row.map_err(|e| StoreError::Sql(e))?);
        }
        Ok(nodes)
    }

    /// Находит входящие рёбра для узла.
    pub fn incoming_edges(&self, dst_id: &uuid::Uuid) -> Result<Vec<Edge>> {
        let mut stmt = self.conn.prepare(
            "SELECT src_id, dst_id, kind, confidence, weight FROM edge WHERE dst_id = ?1",
        )?;
        let rows = stmt.query_map(params![dst_id.to_string()], |row| parse_edge(row))?;
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
        let rows = stmt.query_map(params![src_id.to_string()], |row| parse_edge(row))?;
        let mut edges = Vec::new();
        for row in rows {
            edges.push(row?);
        }
        Ok(edges)
    }

    /// Начинает транзакцию для пакетной записи (используется индексатором).
    pub fn transaction(&mut self) -> rusqlite::Result<rusqlite::Transaction> {
        self.conn.transaction()
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