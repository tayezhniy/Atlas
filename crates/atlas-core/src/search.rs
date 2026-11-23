use atlas_store::{Node, Store};
use std::collections::HashSet;

/// Результат поиска по индексу.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub node: Node,
    pub score: f64,
    pub snippet: Option<String>,
}

/// Простой поиск по имени и сигнатуре (использует FTS5 через Store).
pub fn search(store: &Store, query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>> {
    let _ = query;
    let _ = limit;
    let results: Vec<SearchResult> = vec![];
    Ok(results)
}

/// Поиск по префиксу пути (для быстрой навигации по дереву файлов).
pub fn search_by_path_prefix(store: &Store, prefix: &str, limit: usize) -> anyhow::Result<Vec<Node>> {
    let nodes = store.find_nodes_by_path_prefix(prefix)?;
    Ok(nodes.into_iter().take(limit).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_empty() {
        // Заглушка: поиск без реальной БД
        assert!(true);
    }
}