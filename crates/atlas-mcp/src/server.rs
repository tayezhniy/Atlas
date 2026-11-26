use crate::tools::{Tool, ToolContext, ToolRegistry};
use anyhow::{bail, Result};
use atlas_store::Store;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};

/// Версия протокола, которую сервер называет, если клиент свою не прислал.
const DEFAULT_PROTOCOL: &str = "2024-11-05";

/// Конфигурация MCP-сервера (раздел 8.1 ТЗ).
#[derive(Debug, Clone)]
pub struct McpConfig {
    pub transport: Transport,
    pub token: Option<String>,
    pub budget_default: usize,
    /// Бюджеты токенов на инструмент (из настроек проекта).
    pub tool_budgets: HashMap<String, usize>,
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
            // 0 = без общего потолка: у каждого инструмента свой бюджет по умолчанию
            budget_default: 0,
            tool_budgets: HashMap::new(),
        }
    }
}

/// MCP-сервер Atlas (раздел 8 ТЗ): JSON-RPC 2.0, по одному сообщению в строке.
pub struct McpServer {
    config: McpConfig,
    registry: ToolRegistry,
    project_root: String,
    session_id: String,
}

impl McpServer {
    pub fn new(config: McpConfig, store: Store, project_root: String) -> Self {
        // Загружаем настройки проекта
        let mut tool_budgets = config.tool_budgets.clone();

        if let Ok(settings) = store.all_settings() {
            for (key, value) in settings {
                if key == "budgets" {
                    if let Ok(budgets) = serde_json::from_str::<HashMap<String, usize>>(&value) {
                        tool_budgets.extend(budgets);
                    }
                }
            }
        }

        Self {
            config: McpConfig {
                tool_budgets,
                ..config
            },
            registry: ToolRegistry::new(store),
            project_root,
            session_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn run(self) -> Result<()> {
        match self.config.transport.clone() {
            Transport::Stdio => self.run_stdio(),
            Transport::Http { .. } => {
                bail!("HTTP-транспорт ещё не реализован (раздел 8.1 ТЗ); используйте stdio")
            }
        }
    }

    fn run_stdio(mut self) -> Result<()> {
        tracing::info!("MCP-сервер запущен (stdio), проект: {}", self.project_root);
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout().lock();
        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(reply) = self.handle_message(&line) {
                writeln!(stdout, "{reply}")?;
                stdout.flush()?;
            }
        }
        tracing::info!("stdin закрыт, сервер остановлен");
        Ok(())
    }

    /// Обрабатывает одно входящее сообщение. Для уведомлений ответа нет.
    pub fn handle_message(&mut self, line: &str) -> Option<String> {
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                return Some(
                    error_reply(Value::Null, -32700, &format!("некорректный JSON: {e}"))
                        .to_string(),
                );
            }
        };
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        // Нет id — уведомление (например, notifications/initialized): отвечать нельзя
        let id = id?;

        let reply = match method {
            "initialize" => {
                let version = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or(DEFAULT_PROTOCOL);
                ok_reply(
                    id,
                    json!({
                        "protocolVersion": version,
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "atlas", "version": env!("CARGO_PKG_VERSION")},
                        "instructions": "Atlas — карта проекта. Начните с overview, затем search/outline, код читайте через read или context_for."
                    }),
                )
            }
            "ping" => ok_reply(id, json!({})),
            "tools/list" => {
                let tools: Vec<Value> = self
                    .registry
                    .list_tools()
                    .iter()
                    .map(|t| {
                        json!({
                            "name": t.name(),
                            "description": t.description(),
                            "inputSchema": t.input_schema(),
                        })
                    })
                    .collect();
                ok_reply(id, json!({"tools": tools}))
            }
            "tools/call" => self.call_tool(id, &params),
            // Структурированные данные для интерфейса, не для нейросети
            "atlas/ui" => {
                let method = params.get("method").and_then(Value::as_str).unwrap_or("");
                let args = params.get("args").cloned().unwrap_or_else(|| json!({}));
                match self.registry.ui(&self.project_root, method, &args) {
                    Ok(result) => ok_reply(id, result),
                    Err(e) => error_reply(id, -32000, &e.to_string()),
                }
            }
            other => error_reply(id, -32601, &format!("метод не поддерживается: {other}")),
        };
        Some(reply.to_string())
    }

    fn call_tool(&mut self, id: Value, params: &Value) -> Value {
        let name = params.get("name").and_then(Value::as_str).unwrap_or("");
        let Some(tool) = Tool::from_name(name) else {
            return error_reply(id, -32602, &format!("неизвестный инструмент: {name}"));
        };
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let ctx = ToolContext::new(
            self.session_id.clone(),
            tool.clone(),
            self.config.budget_default,
            self.project_root.clone(),
            self.config.tool_budgets.clone(),
        )
        .with_args(args);

        // Ошибки инструмента — не ошибки протокола: модель должна их увидеть и поправить вызов
        match self.registry.execute(&tool, &ctx) {
            Ok(result) => ok_reply(
                id,
                json!({"content": [{"type": "text", "text": result.text}], "isError": false}),
            ),
            Err(e) => ok_reply(
                id,
                json!({"content": [{"type": "text", "text": e.to_string()}], "isError": true}),
            ),
        }
    }
}

fn ok_reply(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error_reply(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;

    fn server() -> (tempfile::TempDir, McpServer) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("i.db")).unwrap();
        let s = McpServer::new(
            McpConfig::default(),
            store,
            dir.path().to_string_lossy().to_string(),
        );
        (dir, s)
    }

    fn call(s: &mut McpServer, msg: Value) -> Value {
        serde_json::from_str(&s.handle_message(&msg.to_string()).expect("ожидался ответ")).unwrap()
    }

    #[test]
    fn test_config_default() {
        match McpConfig::default().transport {
            Transport::Stdio => {}
            _ => panic!("expected stdio"),
        }
    }

    #[test]
    fn initialize_echoes_protocol_and_lists_tools() {
        let (_d, mut s) = server();
        let r = call(
            &mut s,
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}),
        );
        assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(r["result"]["serverInfo"]["name"], "atlas");
        let r = call(
            &mut s,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        );
        let names: Vec<&str> = r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "overview",
                "search",
                "outline",
                "read",
                "neighbors",
                "context_for",
                "status"
            ]
        );
    }

    #[test]
    fn notifications_get_no_reply() {
        let (_d, mut s) = server();
        assert!(s
            .handle_message(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none());
    }

    #[test]
    fn tool_call_and_tool_error() {
        let (_d, mut s) = server();
        let ok = call(
            &mut s,
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"status","arguments":{}}}),
        );
        assert_eq!(ok["result"]["isError"], false);
        assert!(ok["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Состояние индекса"));
        let bad = call(
            &mut s,
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"read","arguments":{"symbol":"nope"}}}),
        );
        assert_eq!(bad["result"]["isError"], true);
    }

    #[test]
    fn closed_paths_setting_hides_file_contents() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_string_lossy().to_string();
        std::fs::create_dir(dir.path().join("private")).unwrap();
        std::fs::write(
            dir.path().join("private/p.rs"),
            "pub fn f(x: HiddenType) {}\n",
        )
        .unwrap();
        let store = Store::open(dir.path().join("test.db")).unwrap();
        let node = atlas_store::schema::Node {
            id: uuid::Uuid::new_v4(),
            kind: atlas_store::schema::NodeKind::Function,
            path: "private/p.rs".into(),
            name: "f".into(),
            parent_id: None,
            lang: Some("rust".into()),
            hash: None,
            line_start: Some(1),
            line_end: Some(1),
            signature: Some("pub fn f(x: HiddenType)".into()),
            doc_summary: Some("HiddenDoc".into()),
            tokens_est: 5,
            rank: 0.0,
        };
        store.upsert_node(&node).unwrap();
        store
            .set_setting("closedPaths", r#"["private/**"]"#)
            .unwrap();
        let mut registry = ToolRegistry::new(store);

        let args = [
            (Tool::Read, json!({"path": "private/p.rs"})),
            (Tool::Read, json!({"symbol": "f", "mode": "signature"})),
            (Tool::Outline, json!({"path": "private/p.rs"})),
            (Tool::Search, json!({"query": "f"})),
            (Tool::ContextFor, json!({"symbol": "f"})),
        ];
        for (tool, args) in args {
            let ctx = crate::tools::ToolContext::new(
                "s".into(),
                tool.clone(),
                0,
                root.clone(),
                HashMap::new(),
            )
            .with_args(args);
            let text = match registry.execute(&tool, &ctx) {
                Ok(result) => result.text,
                Err(e) => e.to_string(),
            };
            assert!(!text.contains("Hidden"), "{}: {text}", tool.name());
        }
        for method in ["read", "outline", "neighbors"] {
            let args = json!({"target": "private/p.rs::f", "path": "private/p.rs"});
            let text = match registry.ui(&root, method, &args) {
                Ok(v) => v.to_string(),
                Err(e) => e.to_string(),
            };
            assert!(!text.contains("Hidden"), "{method}: {text}");
        }
    }

    #[test]
    fn ui_map_is_not_logged_as_ai_call() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("test.db")).unwrap();
        let registry = ToolRegistry::new(store);
        let map = registry.ui("/test/project", "map", &json!({})).unwrap();
        assert!(map["text"].as_str().unwrap().contains("Карта проекта"));
        let status = registry.ui("/test/project", "status", &json!({})).unwrap();
        assert_eq!(status["calls"]["recent"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn ui_method_returns_structured_data() {
        let (_d, mut s) = server();
        let r = call(
            &mut s,
            json!({"jsonrpc":"2.0","id":7,"method":"atlas/ui","params":{"method":"overview"}}),
        );
        assert!(
            r["result"]["top"].is_array() && r["result"]["edges"].is_array(),
            "{r}"
        );
        let r = call(
            &mut s,
            json!({"jsonrpc":"2.0","id":8,"method":"atlas/ui","params":{"method":"search","args":{"query":"x"}}}),
        );
        assert!(r["result"]["items"].is_array(), "{r}");
        let r = call(
            &mut s,
            json!({"jsonrpc":"2.0","id":9,"method":"atlas/ui","params":{"method":"nope"}}),
        );
        assert_eq!(r["error"]["code"], -32000);
    }

    #[test]
    fn protocol_errors() {
        let (_d, mut s) = server();
        let r = call(&mut s, json!({"jsonrpc":"2.0","id":5,"method":"nope"}));
        assert_eq!(r["error"]["code"], -32601);
        let r = call(
            &mut s,
            json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"zzz"}}),
        );
        assert_eq!(r["error"]["code"], -32602);
        let r: Value = serde_json::from_str(&s.handle_message("{не json").unwrap()).unwrap();
        assert_eq!(r["error"]["code"], -32700);
    }
}
