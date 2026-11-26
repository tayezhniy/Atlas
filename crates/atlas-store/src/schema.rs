use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Типы узлов графа (раздел 5.2 ТЗ)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Dir,
    File,
    Module,
    Class,
    Interface,
    Function,
    Method,
    Constant,
    Type,
    Test,
    Doc,
    Config,
}

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeKind::Dir => write!(f, "dir"),
            NodeKind::File => write!(f, "file"),
            NodeKind::Module => write!(f, "module"),
            NodeKind::Class => write!(f, "class"),
            NodeKind::Interface => write!(f, "interface"),
            NodeKind::Function => write!(f, "function"),
            NodeKind::Method => write!(f, "method"),
            NodeKind::Constant => write!(f, "constant"),
            NodeKind::Type => write!(f, "type"),
            NodeKind::Test => write!(f, "test"),
            NodeKind::Doc => write!(f, "doc"),
            NodeKind::Config => write!(f, "config"),
        }
    }
}

/// Типы рёбер графа (раздел 5.3 ТЗ)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeKind {
    Contains,
    Imports,
    Calls,
    Extends,
    Tests,
    Mentions,
    CoChanges,
}

impl std::fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeKind::Contains => write!(f, "contains"),
            EdgeKind::Imports => write!(f, "imports"),
            EdgeKind::Calls => write!(f, "calls"),
            EdgeKind::Extends => write!(f, "extends"),
            EdgeKind::Tests => write!(f, "tests"),
            EdgeKind::Mentions => write!(f, "mentions"),
            EdgeKind::CoChanges => write!(f, "co_changes"),
        }
    }
}

/// Уровень уверенности связи (раздел 4.5 ТЗ)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Confidence {
    Certain,
    Likely,
    Guess,
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Confidence::Certain => write!(f, "certain"),
            Confidence::Likely => write!(f, "likely"),
            Confidence::Guess => write!(f, "guess"),
        }
    }
}

/// Узел графа (таблица `node`, раздел 5.1 ТЗ)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid,
    pub kind: NodeKind,
    pub path: String,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub lang: Option<String>,
    pub hash: Option<String>,
    pub line_start: Option<i32>,
    pub line_end: Option<i32>,
    pub signature: Option<String>,
    pub doc_summary: Option<String>,
    pub tokens_est: i32,
    pub rank: f64,
}

/// Ребро между узлами (таблица `edge`, раздел 5.1 ТЗ)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub src_id: Uuid,
    pub dst_id: Uuid,
    pub kind: EdgeKind,
    pub confidence: Confidence,
    pub weight: f64,
}

/// Метаданные файла (таблица `file_meta`, раздел 5.1 ТЗ)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMeta {
    pub node_id: Uuid,
    pub size: i64,
    pub mtime: Option<DateTime<Utc>>,
    pub git_changes_30d: i32,
    pub last_commit: Option<String>,
    pub is_test: bool,
    pub is_generated: bool,
}

impl Default for FileMeta {
    fn default() -> Self {
        Self {
            node_id: Uuid::nil(),
            size: 0,
            mtime: None,
            git_changes_30d: 0,
            last_commit: None,
            is_test: false,
            is_generated: false,
        }
    }
}
