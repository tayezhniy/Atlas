use atlas_store::{Node, NodeKind, Store};

/// Результат поиска по индексу.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub node: Node,
    pub score: f64,
    pub snippet: Option<String>,
}

/// Поиск по индексу: FTS5 по имени/сигнатуре/докстрингу/пути с надбавкой за ранг.
pub fn search(store: &Store, query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>> {
    let found = store.search_nodes(query, limit)?;
    let mut results: Vec<SearchResult> = found
        .into_iter()
        .map(|node| {
            let kind_bonus = match node.kind {
                NodeKind::File => 0.05,
                NodeKind::Class | NodeKind::Interface => 0.1,
                NodeKind::Function | NodeKind::Method => 0.08,
                NodeKind::Test => -0.1,
                _ => 0.0,
            };
            let snippet = node
                .doc_summary
                .clone()
                .or_else(|| node.signature.clone())
                .map(|s| s.lines().next().unwrap_or("").trim().to_string())
                .filter(|s| !s.is_empty());
            SearchResult {
                score: node.rank + kind_bonus,
                node,
                snippet,
            }
        })
        .collect();
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.node.path.cmp(&b.node.path))
    });
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
    use atlas_store::schema::{Node, NodeKind};
    use uuid::Uuid;

    fn node(kind: NodeKind, path: &str, name: &str, rank: f64) -> Node {
        Node {
            id: Uuid::new_v4(),
            kind,
            path: path.into(),
            name: name.into(),
            parent_id: None,
            lang: Some("rust".into()),
            hash: None,
            line_start: Some(1),
            line_end: Some(2),
            signature: Some(format!("fn {name}()")),
            doc_summary: Some(format!("документация {name}")),
            tokens_est: 3,
            rank,
        }
    }

    #[test]
    fn search_orders_by_rank_and_finds_prefixes() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("i.db")).unwrap();
        store
            .upsert_node(&node(NodeKind::Function, "a.rs", "helper", 0.1))
            .unwrap();
        store
            .upsert_node(&node(NodeKind::Function, "b.rs", "helper_two", 0.9))
            .unwrap();
        let found = search(&store, "hel", 10).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].node.name, "helper_two");
        assert!(found[0].snippet.is_some());
    }
}
