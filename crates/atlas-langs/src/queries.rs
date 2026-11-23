/// Запросы tree-sitter (.scm) для каждого языка.
/// Загружаются как данные (плагинная модель, раздел 4.2 ТЗ).
pub struct LanguageQueries {
    pub symbols_query: Option<String>,
    pub imports_query: Option<String>,
    pub calls_query: Option<String>,
}

impl LanguageQueries {
    pub fn new() -> Self {
        Self {
            symbols_query: None,
            imports_query: None,
            calls_query: None,
        }
    }
}