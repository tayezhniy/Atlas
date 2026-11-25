
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub database_url: String,
    pub api_port: u16,
    pub max_connections: usize,
    pub redis_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://app.db".into()),
            api_port: std::env::var("API_PORT").unwrap_or("8080").parse().unwrap_or(8080),
            max_connections: std::env::var("MAX_CONN").unwrap_or("100").parse().unwrap_or(100),
            redis_url: std::env::var("REDIS_URL").ok(),
        }
    }
}
