use atlas_core::KnowledgeGraph;
use atlas_store::Store;
use anyhow::Result;

/// Конфигурация MCP-сервера (раздел 8.1 ТЗ).
#[derive(Debug, Clone)]
pub struct McpConfig {
    pub transport: Transport,
    pub token: Option<String>,
    pub budget_default: usize,
}

#[derive(Debug, Clone)]
pub enum Transport {
    Stdio,
    Http { port: u16 },
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            transport: Transport::Stdio,
            token: None,
            budget_default: 2000,
        }
    }
}

/// MCP-сервер Atlas (раздел 8 ТЗ).
pub struct McpServer {
    config: McpConfig,
    store: Store,
    graph: KnowledgeGraph,
}

impl McpServer {
    pub fn new(config: McpConfig, store: Store, graph: KnowledgeGraph) -> Self {
        Self {
            config,
            store,
            graph,
        }
    }

    pub fn run(self) -> Result<()> {
        match self.config.transport {
            Transport::Stdio => self.run_stdio(),
            Transport::Http { port } => self.run_http(port),
        }
    }

    fn run_stdio(self) -> Result<()> {
        tracing::info!("MCP-сервер запущен в режиме stdio");
        // TODO: интеграция с mpc crate
        Ok(())
    }

    fn run_http(self, _port: u16) -> Result<()> {
        tracing::info!("MCP-сервер запущен в режиме HTTP на 127.0.0.1");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = McpConfig::default();
        match config.transport {
            Transport::Stdio => {}
            _ => panic!("expected stdio"),
        }
    }
}