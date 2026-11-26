//! LSP (Language Server Protocol) клиент для динамического разрешения вызовов.
//!
//! Дополняет статический анализ tree-sitter семантической информацией от языковых серверов:
//! - call hierarchy (входящие/исходящие вызовы)
//! - go to definition
//! - find references
//! - hover information

use anyhow::{Context, Result};
use lsp_types::{
    CallHierarchyClientCapabilities, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams, ClientCapabilities,
    DidOpenTextDocumentParams, HoverClientCapabilities, HoverParams, InitializeParams,
    InitializedParams, Location, Position, ReferenceClientCapabilities, ReferenceParams,
    TextDocumentClientCapabilities, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, WorkDoneProgressParams, WorkspaceClientCapabilities,
    WorkspaceFolder,
};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tracing::{debug, info};
use url::Url;

// ---------------------------------------------------------------------------
// LSP-клиент
// ---------------------------------------------------------------------------

const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Сообщение LSP: заголовок с длиной тела, пустая строка, тело.
fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{body}", body.len())
}

async fn read_message<R: tokio::io::AsyncBufRead + Unpin>(reader: &mut R) -> Option<JsonValue> {
    let mut length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).await.ok()? == 0 {
            return None;
        }
        let header = header.trim();
        if header.is_empty() {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length.filter(|n| *n <= 32 * 1024 * 1024)?];
    reader.read_exact(&mut body).await.ok()?;
    serde_json::from_slice(&body).ok()
}

/// Клиент для общения с LSP-сервером через stdio JSON-RPC.
pub struct LspClient {
    process: Arc<Mutex<Option<Child>>>,
    writer: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    request_id: Arc<Mutex<u64>>,
    /// Pending requests waiting for responses.
    pending: Arc<RwLock<HashMap<u64, oneshot::Sender<JsonValue>>>>,
    /// Channel for reader task to send parsed responses.
    response_tx: mpsc::UnboundedSender<(u64, JsonValue)>,
    response_rx: Arc<Mutex<mpsc::UnboundedReceiver<(u64, JsonValue)>>>,
    root_uri: Url,
    capabilities: Arc<RwLock<Option<lsp_types::ServerCapabilities>>>,
    _reader_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl LspClient {
    /// Создаёт новый LSP-клиент для указанного корня проекта.
    pub async fn new(project_root: &Path) -> Result<Self> {
        let root_uri = Url::from_file_path(project_root.canonicalize()?)
            .map_err(|_| anyhow::anyhow!("invalid project root path"))?;

        let (response_tx, response_rx) = mpsc::unbounded_channel();

        Ok(Self {
            process: Arc::new(Mutex::new(None)),
            writer: Arc::new(Mutex::new(None)),
            request_id: Arc::new(Mutex::new(1)),
            pending: Arc::new(RwLock::new(HashMap::new())),
            response_tx,
            response_rx: Arc::new(Mutex::new(response_rx)),
            root_uri,
            capabilities: Arc::new(RwLock::new(None)),
            _reader_handle: Arc::new(Mutex::new(None)),
        })
    }

    /// Запускает LSP-сервер для языка (если не запущен).
    pub async fn ensure_started(&self, config: &LspServerConfig) -> Result<()> {
        let mut proc_guard = self.process.lock().await;
        if proc_guard.is_some() {
            return Ok(());
        }

        info!(
            "starting LSP server: {} {}",
            config.command,
            config.args.join(" ")
        );

        let mut cmd = Command::new(config.command);
        cmd.args(config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(
                self.root_uri
                    .to_file_path()
                    .unwrap_or_else(|_| PathBuf::from(".")),
            );

        let mut child = cmd.spawn().context("failed to spawn LSP server")?;

        let stdin = child.stdin.take().context("no stdin")?;
        let stdout = child.stdout.take().context("no stdout")?;
        let stderr = child.stderr.take().context("no stderr")?;

        // Логируем stderr в фоне
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                debug!("[LSP stderr] {}", line);
            }
        });

        let writer = stdin;

        *proc_guard = Some(child);
        *self.writer.lock().await = Some(writer);

        // Запускаем фоновую задачу для чтения stdout
        self.spawn_reader_task(stdout).await;

        // Инициализация
        self.initialize(config).await?;
        self.initialized().await?;

        Ok(())
    }

    /// Запускает фоновую задачу для чтения ответов от LSP-сервера.
    async fn spawn_reader_task(&self, stdout: tokio::process::ChildStdout) {
        let response_tx = self.response_tx.clone();
        let mut reader = BufReader::new(stdout);

        tokio::spawn(async move {
            while let Some(value) = read_message(&mut reader).await {
                if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
                    if response_tx.send((id, value)).is_err() {
                        break;
                    }
                }
            }
            debug!("LSP reader task ended");
        });

        // Start response dispatcher to route responses to pending requests
        self.start_response_dispatcher().await;
    }

    /// Отправляет initialize запрос.
    async fn initialize(&self, config: &LspServerConfig) -> Result<()> {
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri: Some(self.root_uri.clone()),
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: self.root_uri.clone(),
                name: config.language.to_string(),
            }]),
            capabilities: ClientCapabilities {
                text_document: Some(TextDocumentClientCapabilities {
                    call_hierarchy: Some(CallHierarchyClientCapabilities {
                        dynamic_registration: Some(false),
                    }),
                    hover: Some(HoverClientCapabilities {
                        dynamic_registration: Some(false),
                        content_format: Some(vec![lsp_types::MarkupKind::Markdown]),
                    }),
                    references: Some(ReferenceClientCapabilities {
                        dynamic_registration: Some(false),
                    }),
                    ..Default::default()
                }),
                workspace: Some(WorkspaceClientCapabilities {
                    workspace_folders: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
            initialization_options: None,
            trace: Some(lsp_types::TraceValue::Off),
            ..Default::default()
        };

        let result: lsp_types::InitializeResult = self.request("initialize", params).await?;
        *self.capabilities.write().await = Some(result.capabilities);
        Ok(())
    }

    /// Отправляет initialized уведомление.
    async fn initialized(&self) -> Result<()> {
        self.notify("initialized", InitializedParams {}).await
    }

    /// Открывает документ в LSP-сервере.
    pub async fn did_open(&self, path: &Path, content: &str) -> Result<()> {
        let uri = Url::from_file_path(path.canonicalize()?)
            .map_err(|_| anyhow::anyhow!("invalid file path"))?;

        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: Self::language_id_from_path(path),
                version: 0,
                text: content.to_string(),
            },
        };

        self.notify("textDocument/didOpen", params).await
    }

    /// Получает call hierarchy incoming calls (кто вызывает этот символ).
    pub async fn incoming_calls(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<CallInfo>> {
        let uri = Url::from_file_path(path.canonicalize()?)
            .map_err(|_| anyhow::anyhow!("invalid file path"))?;

        let prepare_params = CallHierarchyPrepareParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        let items: Vec<CallHierarchyItem> = self
            .request("textDocument/prepareCallHierarchy", prepare_params)
            .await?;
        if items.is_empty() {
            return Ok(vec![]);
        }

        let mut calls = Vec::new();
        for item in items {
            let params = CallHierarchyIncomingCallsParams {
                item: item.clone(),
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            };

            let incoming: Vec<lsp_types::CallHierarchyIncomingCall> =
                self.request("callHierarchy/incomingCalls", params).await?;
            for inc in incoming {
                let from = inc.from;
                let from_uri = from.uri.to_file_path().ok();
                calls.push(CallInfo {
                    caller: from.name,
                    callee: item.name.clone(),
                    caller_path: from_uri
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    callee_path: item
                        .uri
                        .to_file_path()
                        .ok()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    caller_line: from.selection_range.start.line,
                    callee_line: item.range.start.line,
                    kind: CallKind::CalledBy,
                });
            }
        }

        Ok(calls)
    }

    /// Получает call hierarchy outgoing calls (что вызывает этот символ).
    pub async fn outgoing_calls(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<CallInfo>> {
        let uri = Url::from_file_path(path.canonicalize()?)
            .map_err(|_| anyhow::anyhow!("invalid file path"))?;

        let prepare_params = CallHierarchyPrepareParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        let items: Vec<CallHierarchyItem> = self
            .request("textDocument/prepareCallHierarchy", prepare_params)
            .await?;
        if items.is_empty() {
            return Ok(vec![]);
        }

        let mut calls = Vec::new();
        for item in items {
            let params = CallHierarchyOutgoingCallsParams {
                item: item.clone(),
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            };

            let outgoing: Vec<lsp_types::CallHierarchyOutgoingCall> =
                self.request("callHierarchy/outgoingCalls", params).await?;
            for out in outgoing {
                let to = out.to;
                let to_uri = to.uri.to_file_path().ok();
                calls.push(CallInfo {
                    caller: item.name.clone(),
                    callee: to.name,
                    caller_path: item
                        .uri
                        .to_file_path()
                        .ok()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    callee_path: to_uri
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    caller_line: item.range.start.line,
                    callee_line: to.range.start.line,
                    kind: CallKind::Calls,
                });
            }
        }

        Ok(calls)
    }

    /// Получает определение символа (go to definition).
    pub async fn goto_definition(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<Location>> {
        let uri = Url::from_file_path(path.canonicalize()?)
            .map_err(|_| anyhow::anyhow!("invalid file path"))?;

        let params = TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position { line, character },
        };

        let result: lsp_types::GotoDefinitionResponse =
            self.request("textDocument/definition", params).await?;
        Ok(match result {
            lsp_types::GotoDefinitionResponse::Scalar(loc) => vec![loc],
            lsp_types::GotoDefinitionResponse::Array(locs) => locs,
            lsp_types::GotoDefinitionResponse::Link(links) => links
                .into_iter()
                .map(|l| Location {
                    uri: l.target_uri,
                    range: l.target_selection_range,
                })
                .collect(),
        })
    }

    /// Находит все референсы к символу.
    pub async fn find_references(
        &self,
        path: &Path,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> Result<Vec<Location>> {
        let uri = Url::from_file_path(path.canonicalize()?)
            .map_err(|_| anyhow::anyhow!("invalid file path"))?;

        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            context: lsp_types::ReferenceContext {
                include_declaration,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: Default::default(),
        };

        let result: Vec<Location> = self.request("textDocument/references", params).await?;
        Ok(result)
    }

    /// Получает hover информацию.
    pub async fn hover(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Option<lsp_types::Hover>> {
        let uri = Url::from_file_path(path.canonicalize()?)
            .map_err(|_| anyhow::anyhow!("invalid file path"))?;

        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        self.request("textDocument/hover", params).await
    }

    /// Отправляет JSON-RPC запрос с ответом.
    async fn request<P: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R> {
        let id = {
            let mut id_guard = self.request_id.lock().await;
            let id = *id_guard;
            *id_guard += 1;
            id
        };

        let (tx, rx) = oneshot::channel::<JsonValue>();
        self.pending.write().await.insert(id, tx);

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let mut writer = self.writer.lock().await;
        if let Some(w) = writer.as_mut() {
            let msg = frame(&serde_json::to_string(&request)?);
            w.write_all(msg.as_bytes()).await?;
            w.flush().await?;
        } else {
            self.pending.write().await.remove(&id);
            return Err(anyhow::anyhow!("LSP server not started"));
        }
        drop(writer);

        // Wait for response
        // Без таймаута молчащий или упавший сервер подвешивает всю индексацию
        let response = match tokio::time::timeout(REQUEST_TIMEOUT, rx).await {
            Ok(received) => received.context("LSP request cancelled")?,
            Err(_) => {
                self.pending.write().await.remove(&id);
                anyhow::bail!(
                    "LSP-сервер не ответил на {method} за {} с",
                    REQUEST_TIMEOUT.as_secs()
                );
            }
        };

        if let Some(error) = response.get("error") {
            return Err(anyhow::anyhow!("LSP error: {}", error));
        }

        let result = response
            .get("result")
            .context("no result in LSP response")?;
        Ok(serde_json::from_value(result.clone())?)
    }

    /// Background task to dispatch responses from mpsc channel to pending oneshot senders.
    async fn start_response_dispatcher(&self) {
        let pending = self.pending.clone();
        let response_rx = self.response_rx.clone();

        tokio::spawn(async move {
            let mut rx = response_rx.lock().await;
            while let Some((id, value)) = rx.recv().await {
                if let Some(tx) = pending.write().await.remove(&id) {
                    let _ = tx.send(value);
                }
            }
        });
    }

    /// Отправляет JSON-RPC уведомление (без ответа).
    async fn notify<P: serde::Serialize>(&self, method: &str, params: P) -> Result<()> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        let mut writer = self.writer.lock().await;
        if let Some(w) = writer.as_mut() {
            let msg = frame(&serde_json::to_string(&notification)?);
            w.write_all(msg.as_bytes()).await?;
            w.flush().await?;
        }
        Ok(())
    }

    /// Определяет language_id по пути файла.
    fn language_id_from_path(path: &Path) -> String {
        match path.extension().and_then(|e| e.to_str()) {
            Some("rs") => "rust",
            Some("py") => "python",
            Some("ts") | Some("tsx") => "typescript",
            Some("js") | Some("jsx") => "javascript",
            Some("go") => "go",
            Some("java") => "java",
            Some("cs") => "csharp",
            Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") | Some("h") => "cpp",
            Some("dart") => "dart",
            Some("swift") => "swift",
            Some("kt") | Some("kts") => "kotlin",
            Some("lua") => "lua",
            Some("zig") => "zig",
            Some("ex") | Some("exs") => "elixir",
            Some("hs") => "haskell",
            Some("ml") | Some("mli") => "ocaml",
            Some("prisma") => "prisma",
            Some("graphql") | Some("gql") => "graphql",
            Some("sql") => "sql",
            _ => "plaintext",
        }
        .to_string()
    }

    /// Завершает работу LSP-сервера.
    pub async fn shutdown(&self) -> Result<()> {
        self.notify("shutdown", ()).await?;
        self.notify("exit", ()).await?;

        if let Some(mut child) = self.process.lock().await.take() {
            let _ = child.kill().await;
        }
        Ok(())
    }
}

/// Информация о вызове для call hierarchy
#[derive(Debug, Clone)]
pub struct CallInfo {
    pub caller: String,      // имя вызывающего
    pub callee: String,      // имя вызываемого
    pub caller_path: String, // путь к файлу вызывающего
    pub callee_path: String, // путь к файлу вызываемого
    pub caller_line: u32,    // строка вызова
    pub callee_line: u32,    // строка определения
    pub kind: CallKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallKind {
    Calls,    // A вызывает B
    CalledBy, // B вызывается из A
}

/// Конфигурация LSP-сервера для языка
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    pub language: &'static str,
    pub extensions: &'static [&'static str],
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub root_markers: &'static [&'static str],
}

/// Реестр конфигураций LSP-серверов.
pub struct LspRegistry;

impl LspRegistry {
    /// Возвращает конфигурацию для языка.
    pub fn for_language(lang: &str) -> Option<LspServerConfig> {
        match lang {
            "rust" => Some(LspServerConfig {
                language: "rust",
                extensions: &["rs"],
                command: "rust-analyzer",
                args: &[],
                root_markers: &["Cargo.toml", "rust-project.json"],
            }),
            "typescript" | "javascript" => Some(LspServerConfig {
                language: "typescript",
                extensions: &["ts", "tsx", "js", "jsx"],
                command: "typescript-language-server",
                args: &["--stdio"],
                root_markers: &["package.json", "tsconfig.json"],
            }),
            "python" => Some(LspServerConfig {
                language: "python",
                extensions: &["py"],
                command: "pyright-langserver",
                args: &["--stdio"],
                root_markers: &["pyproject.toml", "setup.py", "requirements.txt"],
            }),
            "go" => Some(LspServerConfig {
                language: "go",
                extensions: &["go"],
                command: "gopls",
                args: &["-rpc.trace"],
                root_markers: &["go.mod", "go.work"],
            }),
            "java" => Some(LspServerConfig {
                language: "java",
                extensions: &["java"],
                command: "jdtls",
                args: &[],
                root_markers: &["pom.xml", "build.gradle", "gradlew"],
            }),
            "csharp" => Some(LspServerConfig {
                language: "csharp",
                extensions: &["cs"],
                command: "csharp-ls",
                args: &[],
                root_markers: &["*.csproj", "*.sln"],
            }),
            "cpp" => Some(LspServerConfig {
                language: "cpp",
                extensions: &["cpp", "cc", "cxx", "hpp", "h"],
                command: "clangd",
                args: &["--background-index", "--clang-tidy"],
                root_markers: &["compile_commands.json", "CMakeLists.txt"],
            }),
            "dart" => Some(LspServerConfig {
                language: "dart",
                extensions: &["dart"],
                command: "dart",
                args: &["language-server"],
                root_markers: &["pubspec.yaml"],
            }),
            "swift" => Some(LspServerConfig {
                language: "swift",
                extensions: &["swift"],
                command: "sourcekit-lsp",
                args: &[],
                root_markers: &["Package.swift", "*.xcodeproj"],
            }),
            "kotlin" => Some(LspServerConfig {
                language: "kotlin",
                extensions: &["kt", "kts"],
                command: "kotlin-language-server",
                args: &[],
                root_markers: &["build.gradle.kts", "pom.xml"],
            }),
            _ => None,
        }
    }

    /// Возвращает все доступные конфигурации.
    pub fn all() -> Vec<LspServerConfig> {
        vec![
            Self::for_language("rust"),
            Self::for_language("typescript"),
            Self::for_language("python"),
            Self::for_language("go"),
            Self::for_language("java"),
            Self::for_language("csharp"),
            Self::for_language("cpp"),
            Self::for_language("dart"),
            Self::for_language("swift"),
            Self::for_language("kotlin"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_registry() {
        assert!(LspRegistry::for_language("rust").is_some());
        assert!(LspRegistry::for_language("typescript").is_some());
        assert!(LspRegistry::for_language("python").is_some());
        assert!(LspRegistry::for_language("go").is_some());
        assert!(LspRegistry::for_language("java").is_some());
        assert!(LspRegistry::for_language("unknown").is_none());
    }

    #[tokio::test]
    async fn messages_use_content_length_framing() {
        let data = format!("{}{}", frame(r#"{"id":1}"#), frame(r#"{"id":2}"#));
        let mut reader = BufReader::new(data.as_bytes());
        assert_eq!(read_message(&mut reader).await.unwrap()["id"], 1);
        assert_eq!(read_message(&mut reader).await.unwrap()["id"], 2);
        assert!(read_message(&mut reader).await.is_none());
    }

    #[test]
    fn test_language_id() {
        assert_eq!(
            LspClient::language_id_from_path(Path::new("test.rs")),
            "rust"
        );
        assert_eq!(
            LspClient::language_id_from_path(Path::new("test.py")),
            "python"
        );
        assert_eq!(
            LspClient::language_id_from_path(Path::new("test.ts")),
            "typescript"
        );
        assert_eq!(LspClient::language_id_from_path(Path::new("test.go")), "go");
    }
}
