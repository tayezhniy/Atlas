use crate::ranking::Ranker;
use atlas_store::{EdgeKind, NodeKind, Store};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;
use std::collections::HashMap;

/// Лёгкая ссылка на узел в графе в памяти.
#[derive(Debug, Clone)]
pub struct NodeRef {
    pub id: uuid::Uuid,
    pub kind: NodeKind,
    pub path: String,
    pub name: String,
    pub rank: f64,
}

/// Лёгкая ссылка на ребро в графе в памяти.
#[derive(Debug, Clone)]
pub struct EdgeRef {
    pub src_id: uuid::Uuid,
    pub dst_id: uuid::Uuid,
    pub kind: EdgeKind,
    pub weight: f64,
}

/// Граф знаний в памяти, построенный из индекса.
pub struct KnowledgeGraph {
    graph: DiGraph<NodeRef, EdgeRef>,
    node_index_by_id: HashMap<uuid::Uuid, NodeIndex>,
}

impl KnowledgeGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_index_by_id: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node: NodeRef) -> NodeIndex {
        if let Some(&idx) = self.node_index_by_id.get(&node.id) {
            return idx;
        }
        let id = node.id;
        let idx = self.graph.add_node(node);
        self.node_index_by_id.insert(id, idx);
        idx
    }

    pub fn add_edge(&mut self, src_id: uuid::Uuid, dst_id: uuid::Uuid, edge: EdgeRef) {
        let src_idx = self.node_index_by_id.get(&src_id).copied();
        let dst_idx = self.node_index_by_id.get(&dst_id).copied();
        match (src_idx, dst_idx) {
            (Some(s), Some(d)) => {
                self.graph.add_edge(s, d, edge);
            }
            _ => {
                // Пропускаем рёбра к узлам, которых нет в графе
            }
        }
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    pub fn neighbors(&self, id: &uuid::Uuid, direction: Direction) -> Vec<NodeRef> {
        let mut result = Vec::new();
        if let Some(&idx) = self.node_index_by_id.get(id) {
            for neighbor_idx in self.graph.neighbors_directed(idx, direction) {
                result.push(self.graph[neighbor_idx].clone());
            }
        }
        result
    }

    pub fn get_node(&self, id: &uuid::Uuid) -> Option<&NodeRef> {
        self.node_index_by_id
            .get(id)
            .and_then(|&idx| self.graph.node_weight(idx))
    }
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Строит граф в памяти из хранилища, применяя ранжирование.
pub fn build_graph(store: &Store, _ranker: &Ranker) -> anyhow::Result<KnowledgeGraph> {
    let mut graph = KnowledgeGraph::new();

    // Загружаем все узлы индекса и добавляем их в граф
    let nodes = store.get_all_nodes()?;
    for node in &nodes {
        graph.add_node(NodeRef {
            id: node.id,
            kind: node.kind.clone(),
            path: node.path.clone(),
            name: node.name.clone(),
            rank: node.rank,
        });
    }

    // Загружаем рёбра; рёбра к отсутствующим узлам пропускаются внутри add_edge
    let edges = store.get_all_edges()?;
    for edge in &edges {
        graph.add_edge(
            edge.src_id,
            edge.dst_id,
            EdgeRef {
                src_id: edge.src_id,
                dst_id: edge.dst_id,
                kind: edge.kind.clone(),
                weight: edge.weight,
            },
        );
    }

    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_add_node() {
        let mut graph = KnowledgeGraph::new();
        let node = NodeRef {
            id: uuid::Uuid::new_v4(),
            kind: NodeKind::File,
            path: "src/main.rs".to_string(),
            name: "main.rs".to_string(),
            rank: 0.5,
        };
        let idx = graph.add_node(node.clone());
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.get_node(&node.id).unwrap().name, "main.rs");
        assert_eq!(idx.index(), 0);
    }

    #[test]
    fn test_graph_add_edge() {
        let mut graph = KnowledgeGraph::new();
        let n1 = NodeRef {
            id: uuid::Uuid::new_v4(),
            kind: NodeKind::File,
            path: "a.rs".to_string(),
            name: "a.rs".to_string(),
            rank: 0.1,
        };
        let n2 = NodeRef {
            id: uuid::Uuid::new_v4(),
            kind: NodeKind::Function,
            path: "a.rs".to_string(),
            name: "foo".to_string(),
            rank: 0.2,
        };
        let i1 = graph.add_node(n1.clone());
        let i2 = graph.add_node(n2.clone());
        graph.add_edge(
            n1.id,
            n2.id,
            EdgeRef {
                src_id: n1.id,
                dst_id: n2.id,
                kind: EdgeKind::Contains,
                weight: 1.0,
            },
        );
        assert_eq!(graph.edge_count(), 1);
        let _ = (i1, i2);
    }
}
