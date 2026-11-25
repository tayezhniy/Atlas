/// Форматирование ответов MCP (раздел 8.4 ТЗ).
/// Плотный текст вместо JSON, псевдонимы для длинных путей (@1, @2).
pub struct ResponseFormatter {}

impl ResponseFormatter {
    pub fn new() -> Self {
        Self {}
    }

    pub fn format_header(project: &str, index_state: &str, tokens: usize) -> String {
        format!("project={} index={} tokens={}", project, index_state, tokens)
    }

    pub fn format_path_alias(alias: &str, path: &str) -> String {
        format!("{} = {}", alias, path)
    }
}

impl Default for ResponseFormatter {
    fn default() -> Self {
        Self::new()
    }
}