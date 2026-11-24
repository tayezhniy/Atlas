-- Начальная схема индекса Atlas (раздел 5.1 ТЗ)
-- Таблица project: метаданные проекта
CREATE TABLE IF NOT EXISTS project (
    id TEXT PRIMARY KEY,
    root_path TEXT NOT NULL UNIQUE,
    schema_version INTEGER NOT NULL DEFAULT 1,
    parser_version INTEGER NOT NULL DEFAULT 1,
    last_indexed_at TEXT,
    name TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Таблица node: все узлы графа
CREATE TABLE IF NOT EXISTS node (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    path TEXT NOT NULL,
    name TEXT NOT NULL,
    parent_id TEXT,
    lang TEXT,
    hash TEXT,
    line_start INTEGER,
    line_end INTEGER,
    signature TEXT,
    doc_summary TEXT,
    tokens_est INTEGER NOT NULL DEFAULT 0,
    rank REAL NOT NULL DEFAULT 0.0,
    FOREIGN KEY (parent_id) REFERENCES node(id)
);

CREATE INDEX IF NOT EXISTS idx_node_path ON node(path);
CREATE INDEX IF NOT EXISTS idx_node_parent_id ON node(parent_id);
CREATE INDEX IF NOT EXISTS idx_node_lang ON node(lang);
CREATE INDEX IF NOT EXISTS idx_node_kind ON node(kind);

-- Таблица edge: связи между узлами
CREATE TABLE IF NOT EXISTS edge (
    src_id TEXT NOT NULL,
    dst_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    confidence TEXT NOT NULL,
    weight REAL NOT NULL DEFAULT 1.0,
    PRIMARY KEY (src_id, dst_id, kind),
    FOREIGN KEY (src_id) REFERENCES node(id),
    FOREIGN KEY (dst_id) REFERENCES node(id)
);

CREATE INDEX IF NOT EXISTS idx_edge_src ON edge(src_id);
CREATE INDEX IF NOT EXISTS idx_edge_dst ON edge(dst_id);
CREATE INDEX IF NOT EXISTS idx_edge_kind ON edge(kind);

-- Таблица file_meta: данные о файле
CREATE TABLE IF NOT EXISTS file_meta (
    node_id TEXT PRIMARY KEY,
    size INTEGER NOT NULL DEFAULT 0,
    mtime TEXT,
    git_changes_30d INTEGER NOT NULL DEFAULT 0,
    last_commit TEXT,
    is_test INTEGER NOT NULL DEFAULT 0,
    is_generated INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (node_id) REFERENCES node(id)
);

-- Таблица summary: кэш текстовых описаний (опционально, раздел 5.1)
CREATE TABLE IF NOT EXISTS summary (
    node_id TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    text TEXT NOT NULL,
    model TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (node_id) REFERENCES node(id)
);

-- Таблица search_fts: полнотекстовый индекс (FTS5).
-- Самостоятельная таблица (не external content): у node нет колонки doc, а rowid
-- берётся из node.rowid, поэтому поиск соединяется с node по rowid.
CREATE VIRTUAL TABLE IF NOT EXISTS search_fts USING fts5(
    name,
    signature,
    doc,
    path
);

-- Таблица call_log: журнал запросов нейросети (раздел 5.1)
CREATE TABLE IF NOT EXISTS call_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    time TEXT NOT NULL DEFAULT (datetime('now')),
    tool TEXT NOT NULL,
    args TEXT,
    tokens_out INTEGER NOT NULL DEFAULT 0,
    node_ids TEXT
);

CREATE INDEX IF NOT EXISTS idx_call_log_session ON call_log(session_id);
CREATE INDEX IF NOT EXISTS idx_call_log_time ON call_log(time);
CREATE INDEX IF NOT EXISTS idx_call_log_tool ON call_log(tool);

-- Таблица settings: настройки проекта (раздел 5.1)
CREATE TABLE IF NOT EXISTS settings (
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (key)
);

-- Триггеры для поддержки FTS при вставке/обновлении/удалении узлов
CREATE TRIGGER IF NOT EXISTS node_ai AFTER INSERT ON node BEGIN
    INSERT INTO search_fts (rowid, name, signature, doc, path)
    VALUES (new.rowid, new.name, COALESCE(new.signature, ''), COALESCE(new.doc_summary, ''), new.path);
END;

CREATE TRIGGER IF NOT EXISTS node_ad AFTER DELETE ON node BEGIN
    DELETE FROM search_fts WHERE rowid = old.rowid;
END;

CREATE TRIGGER IF NOT EXISTS node_au AFTER UPDATE ON node BEGIN
    DELETE FROM search_fts WHERE rowid = old.rowid;
    INSERT INTO search_fts (rowid, name, signature, doc, path)
    VALUES (new.rowid, new.name, COALESCE(new.signature, ''), COALESCE(new.doc_summary, ''), new.path);
END;
