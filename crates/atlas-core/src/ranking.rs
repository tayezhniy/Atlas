use atlas_store::{EdgeKind, Node, NodeKind, Store};
use std::collections::HashMap;

/// Сигналы ранжирования (раздел 6.2 ТЗ).
#[derive(Debug, Clone)]
pub struct RankingSignal {
    pub pagerank: f64,
    pub git_frequency: f64,
    pub context_boost: f64,
    pub is_public: bool,
    pub penalty: f64,
}

impl Default for RankingSignal {
    fn default() -> Self {
        Self {
            pagerank: 0.0,
            git_frequency: 0.0,
            context_boost: 0.0,
            is_public: false,
            penalty: 0.0,
        }
    }
}

/// Весы сигналов ранжирования (хранятся в настройках проекта).
#[derive(Debug, Clone)]
pub struct RankingWeights {
    pub pagerank: f64,
    pub git_frequency: f64,
    pub context_boost: f64,
    pub is_public: f64,
    pub penalty: f64,
}

impl Default for RankingWeights {
    fn default() -> Self {
        Self {
            pagerank: 0.4,
            git_frequency: 0.2,
            context_boost: 0.2,
            is_public: 0.1,
            penalty: 0.1,
        }
    }
}

/// Ранжировщик узлов графа.
pub struct Ranker {
    weights: RankingWeights,
}

impl Ranker {
    pub fn new() -> Self {
        Self {
            weights: RankingWeights::default(),
        }
    }

    pub fn with_weights(weights: RankingWeights) -> Self {
        Self { weights }
    }

    /// Вычисляет итоговый ранг для узла.
    pub fn rank(&self, node: &Node, signal: &RankingSignal) -> f64 {
        let mut score = 0.0;
        score += self.weights.pagerank * signal.pagerank;
        score += self.weights.git_frequency * signal.git_frequency;
        score += self.weights.context_boost * signal.context_boost;
        if signal.is_public {
            score += self.weights.is_public;
        }
        score -= self.weights.penalty * signal.penalty;

        // Штраф для тестов, сгенерированного кода и устаревших узлов (раздел 6.2, п.5)
        let kind_penalty = match node.kind {
            NodeKind::Test => 0.1,
            NodeKind::Config => 0.05,
            _ => 0.0,
        };
        score -= kind_penalty;

        score.max(0.0)
    }
}

impl Default for Ranker {
    fn default() -> Self {
        Self::new()
    }
}

/// Пересчитывает ранги всех узлов по PageRank на рёбрах `imports` и `calls`.
///
/// Раньше ранг был просто нормированной входящей степенью; PageRank учитывает,
/// насколько важен сам источник ссылки. Тесты и конфиги получают понижение.
pub fn recompute_ranks(store: &Store) -> anyhow::Result<()> {
    const DAMPING: f64 = 0.85;
    const ITERATIONS: usize = 40;
    const EPSILON: f64 = 1e-7;

    let nodes: Vec<Node> = store
        .get_all_nodes()?
        .into_iter()
        .filter(|n| n.kind != NodeKind::Dir)
        .collect();
    if nodes.is_empty() {
        return Ok(());
    }
    let index_of: HashMap<uuid::Uuid, usize> =
        nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();

    // Входящие степени: для каждого dst — список (src, вес)
    let mut incoming: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nodes.len()];
    let mut out_weight: Vec<f64> = vec![0.0; nodes.len()];
    for edge in store.get_all_edges()? {
        if !matches!(edge.kind, EdgeKind::Imports | EdgeKind::Calls) {
            continue;
        }
        let (Some(&src), Some(&dst)) = (index_of.get(&edge.src_id), index_of.get(&edge.dst_id))
        else {
            continue;
        };
        let w = edge.weight.max(0.1);
        incoming[dst].push((src, w));
        out_weight[src] += w;
    }

    let n = nodes.len() as f64;
    let mut rank = vec![1.0 / n; nodes.len()];
    for _ in 0..ITERATIONS {
        let mut next = vec![(1.0 - DAMPING) / n; nodes.len()];
        let mut dangling = 0.0;
        for (i, r) in rank.iter().enumerate() {
            if out_weight[i] <= f64::EPSILON {
                dangling += *r;
            }
        }
        let dangling_share = DAMPING * dangling / n;
        for i in 0..nodes.len() {
            next[i] += dangling_share;
            for &(src, w) in &incoming[i] {
                if out_weight[src] > f64::EPSILON {
                    next[i] += DAMPING * rank[src] * w / out_weight[src];
                }
            }
        }
        let diff: f64 = next
            .iter()
            .zip(rank.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        rank = next;
        if diff < EPSILON {
            break;
        }
    }

    let max = rank.iter().cloned().fold(0.0f64, f64::max).max(f64::EPSILON);
    for (node, value) in nodes.iter().zip(rank.iter()) {
        let penalty = match node.kind {
            NodeKind::Test => 0.6,
            NodeKind::Config => 0.7,
            _ => 1.0,
        };
        store.set_rank(&node.id, (value / max) * penalty)?;
    }
    Ok(())
}