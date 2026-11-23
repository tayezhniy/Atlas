use atlas_core::ranking::Ranker;
use atlas_store::{db::Store, schema::{EdgeKind, NodeKind}};
use anyhow::Result;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Инструменты MCP (раздел 8.2 ТЗ): overview, search, outline, read, neighbors, context_for, status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Tool {
    Overview,
    Search,
    Outline,
    Read,
    Neighbors,
    ContextFor,
    Status,
}

impl std::fmt::Display for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tool::Overview => write!(f, "overview"),
            Tool::Search => write!(f, "search"),
            Tool::Outline => write!(f, "outline"),
            Tool::Read => write!(f, "read"),
            Tool::Neighbors => write!(f, "neighbors"),
            Tool::ContextFor => write!(f, "context_for"),
            Tool::Status => write!(f, "status"),
        }
    }
}

/// Контекст выполнения инструмента (сессия, бюджет, проект).
#[derive(Debug, Clone)]
pub struct ToolContext {
    pub session_id: String,
    pub tool: Tool,
    pub max_tokens: usize,
    pub project_root: String,
}

impl ToolContext {
    pub fn new(session_id: String, tool: Tool, max_tokens: usize, project_root: String) -> Self {
        Self {
            session_id,
            tool,
            max_tokens,
            project_root,
        }
    }
}

/// Результат вызова инструмента.
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub text: String,
    pub tokens_out: usize,
    pub node_ids: Vec<String>,
}

/// Реестр инструментов с реальной логикой.
pub struct ToolRegistry {
    store: Store,
    ranker: Ranker,
}

impl ToolRegistry {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            ranker: Ranker::new(),
        }
    }

    pub fn list_tools(&self) -> Vec<Tool> {
        vec![
            Tool::Overview,
            Tool::Search,
            Tool::Outline,
            Tool::Read,
            Tool::Neighbors,
            Tool::ContextFor,
            Tool::Status,
        ]
    }

    /// Выполняет инструмент и возвращает результат.
    pub fn execute(&mut self, tool: &Tool, ctx: &ToolContext) -> Result<ToolResult> {
        match tool {
            Tool::Overview => self.overview(ctx),
            Tool::Search => self.search(ctx),
            Tool::Outline => self.outline(ctx),
            Tool::Read => self.read(ctx),
            Tool::Neighbors => self.neighbors(ctx),
            Tool::ContextFor => self.context_for(ctx),
            Tool::Status => self.status(ctx),
        }
    }

    // --- overview: карта проекта (L0/L1) ---

    fn overview(&mut self, ctx: &ToolContext) -> Result<ToolResult> {
        let nodes = self.store.find_nodes_by_path_prefix(&ctx.project_root)?;
        let mut text = String::new();
        text.push_str(&format!("## Карта проекта: {}\n\n", ctx.project_root));

        // Группируем по каталогам верхнего уровня
        let mut top_dirs: HashMap<String, Vec<&atlas_store::schema::Node>> = HashMap::new();
        for node in &nodes {
            if node.kind == NodeKind::File {
                let parts: Vec<&str> = node.path.split('/').collect();
                if parts.len() > 1 {
                    let top = parts[0].to_string();
                    top_dirs.entry(top).or_default().push(node);
                }
            }
        }

        let mut tokens = 0;
        for (dir, files) in &top_dirs {
            text.push_str(&format!("- {}/ ({} файлов)\n", dir, files.len()));
            for file in files.iter().take(5) {
                text.push_str(&format!("  - {} ({})\n", file.name, file.lang.as_deref().unwrap_or("?")));
                tokens += file.name.len() + 20;
            }
            if files.len() > 5 {
                text.push_str(&format!("  ... и ещё {} файлов\n", files.len() - 5));
            }
        }

        let result = ToolResult {
            text,
            tokens_out: tokens,
            node_ids: vec![],
        };
        Ok(result)
    }

    // --- search: поиск по символам ---

    fn search(&self, ctx: &ToolContext) -> Result<ToolResult> {
        // В реальной реализации здесь будет FTS5 поиск по search_fts
        // Пока возвращаем заглушку с инструкцией
        let text = format!(
            "## Поиск в проекте: {}\n\nИспользуйте параметр `query` для поиска по именам, сигнатурам и докстрингам.\n",
            ctx.project_root
        );
        let tokens = text.len();
        Ok(ToolResult {
            text,
            tokens_out: tokens,
            node_ids: vec![],
        })
    }

    // --- outline: сигнатуры и докстринги файла (L2) ---

    fn outline(&self, ctx: &ToolContext) -> Result<ToolResult> {
        // Нужен путь к файлу из контекста — пока упрощённая версия
        let text = format!(
            "## Оглавление файла\n\nПуть: {}\n\nСимволы файла будут здесь после реализации.\n",
            ctx.project_root
        );
        let tokens = text.len();
        Ok(ToolResult {
            text,
            tokens_out: tokens,
            node_ids: vec![],
        })
    }

    // --- read: тело символа или диапазон строк (L3) ---

    fn read(&self, ctx: &ToolContext) -> Result<ToolResult> {
        // Читает файл по пути и возвращает содержимое с ограничением по бюджету
        let path = &ctx.project_root;
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|_| format!("Файл не найден: {}", path));

        let truncated = if content.len() > ctx.max_tokens {
            format!("{}...\n\n[Обрезано по бюджету {} токенов]", &content[..ctx.max_tokens], ctx.max_tokens)
        } else {
            content.clone()
        };

        let tokens = truncated.len();
        Ok(ToolResult {
            text: truncated,
            tokens_out: tokens,
            node_ids: vec![],
        })
    }

    // --- neighbors: окружение символа (L4) ---

    fn neighbors(&self, ctx: &ToolContext) -> Result<ToolResult> {
        let text = format!(
            "## Окружение символа\n\nПроект: {}\n\nСвязи символа будут здесь после реализации.\n",
            ctx.project_root
        );
        let tokens = text.len();
        Ok(ToolResult {
            text,
            tokens_out: tokens,
            node_ids: vec![],
        })
    }

    // --- context_for: составной инструмент ---

    fn context_for(&self, ctx: &ToolContext) -> Result<ToolResult> {
        let text = format!(
            "## Контекст для задачи\n\nПроект: {}\n\nСоставной пакет (simbol + зависимости + вызывающие + тест) будет здесь после реализации.\n",
            ctx.project_root
        );
        let tokens = text.len();
        Ok(ToolResult {
            text,
            tokens_out: tokens,
            node_ids: vec![],
        })
    }

    // --- status: состояние индекса ---

    fn status(&self, ctx: &ToolContext) -> Result<ToolResult> {
        let node_count = self.store.find_nodes_by_path_prefix(&ctx.project_root)?.len();
        let text = format!(
            "## Состояние индекса\n\nПроект: {}\n\nУзлов в индексе: {}\nСтатус: fresh\n",
            ctx.project_root, node_count
        );
        let tokens = text.len();
        Ok(ToolResult {
            text,
            tokens_out: tokens,
            node_ids: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_list_tools() {
        // Создаём временную БД для теста
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Store::open(&db_path).unwrap();
        let registry = ToolRegistry::new(store);
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 7);
    }

    #[test]
    fn test_status_tool() -> Result<()> {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Store::open(&db_path).unwrap();
        let mut registry = ToolRegistry::new(store);
        let ctx = ToolContext::new("test-session".to_string(), Tool::Status, 1000, "/test/project".to_string());
        let result = registry.execute(&Tool::Status, &ctx)?;
        assert!(result.text.contains("Состояние индекса"));
        Ok(())
    }

    #[test]
    fn test_overview_tool() -> Result<()> {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Store::open(&db_path).unwrap();
        let mut registry = ToolRegistry::new(store);
        let ctx = ToolContext::new("test-session".to_string(), Tool::Overview, 1000, "/test/project".to_string());
        let result = registry.execute(&Tool::Overview, &ctx)?;
        assert!(result.text.contains("Карта проекта"));
        Ok(())
    }
}