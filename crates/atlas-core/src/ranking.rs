use atlas_store::{EdgeKind, Node, NodeKind};
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