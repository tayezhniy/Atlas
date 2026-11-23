/// Инструменты MCP (раздел 8.2 ТЗ): overview, search, outline, read, neighbors, context_for, status.
use anyhow::Result;

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

/// Реестр инструментов.
pub struct ToolRegistry {}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {}
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_tools() {
        let registry = ToolRegistry::new();
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 7);
    }
}