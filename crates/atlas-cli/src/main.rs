use anyhow::{bail, Context, Result};
use atlas_core::filter::{filter_output, FilterOptions};
use atlas_indexer::{FileWatcher, IncrementalUpdater, ProjectIndexer, WatchEvent};
use atlas_mcp::{McpConfig, McpServer, Transport};
use atlas_store::Store;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "atlas")]
#[command(about = "Atlas — навигатор по проекту для нейросетей", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Индексировать проект
    Index {
        /// Путь к корню проекта
        #[arg(value_name = "PROJECT_PATH")]
        path: String,
    },
    /// Запустить MCP-сервер
    Mcp {
        /// Корень проекта (по умолчанию текущий каталог)
        #[arg(long, value_name = "PROJECT_PATH")]
        project: Option<String>,
        /// Транспорт: stdio (http пока не реализован)
        #[arg(long, default_value = "stdio")]
        transport: String,
    },
    /// Запуск команды с фильтром вывода (раздел 9.1 ТЗ)
    Run {
        /// Команда для выполнения
        #[arg(value_name = "COMMAND")]
        command: String,
    },
    /// Наблюдать за изменениями и переиндексировать по ходу дела (раздел 4.3 ТЗ)
    Watch {
        /// Корень проекта (по умолчанию текущий каталог)
        #[arg(long, value_name = "PROJECT_PATH")]
        path: Option<String>,
    },
    /// Проверка состояния индекса
    Doctor {
        /// Корень проекта (по умолчанию текущий каталог)
        #[arg(long, value_name = "PROJECT_PATH")]
        project: Option<String>,
    },
}

fn main() -> Result<()> {
    // Журнал только в stderr: в режиме `mcp` stdout занят протоколом
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Index { path } => {
            let root = project_root(Some(&path))?;
            let mut store = open_store(&root)?;
            let stats = ProjectIndexer::new(&mut store, &root.to_string_lossy()).index_project()?;
            println!("Проект: {}", root.display());
            println!(
                "Файлов: найдено {}, проиндексировано {} (остальные не изменились), ошибок {}",
                stats.files_scanned, stats.files_indexed, stats.errors
            );
            println!(
                "Символов {}, импортов {}, вызовов {} (связей {} из них, не разрешено {})",
                stats.symbols_extracted,
                stats.imports_extracted,
                stats.calls_extracted,
                stats.calls_linked,
                stats.calls_unresolved
            );
            println!("Индекс: {}", db_path(&root).display());
        }
        Commands::Mcp { project, transport } => {
            let root = project_root(project.as_deref())?;
            let transport = match transport.as_str() {
                "stdio" => Transport::Stdio,
                "http" => bail!("HTTP-транспорт ещё не реализован; используйте --transport stdio"),
                other => bail!("неизвестный транспорт «{other}»: допустим stdio"),
            };
            let mut store = open_store(&root)?;
            // Клиент не обязан помнить про `atlas index`: первый запуск строит индекс сам
            if store.project_info()?.and_then(|p| p.1).is_none() {
                tracing::info!("индекса ещё нет, индексирую {}", root.display());
                ProjectIndexer::new(&mut store, &root.to_string_lossy()).index_project()?;
            }
            let config = McpConfig { transport, ..McpConfig::default() };
            McpServer::new(config, store, root.to_string_lossy().to_string()).run()?;
        }
        Commands::Run { command } => {
            let failed = cmd_run(&command)?;
            if failed {
                std::process::exit(1);
            }
        }
        Commands::Watch { path } => {
            let root = project_root(path.as_deref())?;
            return cmd_watch(&root);
        }
        Commands::Doctor { project } => {
            let root = project_root(project.as_deref())?;
            let failed = doctor(&root)?;
            if failed {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

/// Абсолютный корень проекта: путь из аргумента или текущий каталог.
fn project_root(arg: Option<&str>) -> Result<PathBuf> {
    let raw = match arg {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir()?,
    };
    let root = raw
        .canonicalize()
        .with_context(|| format!("каталог проекта не найден: {}", raw.display()))?;
    if !root.is_dir() {
        bail!("{} — не каталог", root.display());
    }
    Ok(root)
}

/// База индекса лежит внутри проекта, в скрытом `.atlas/`: обход её не видит (раздел 5 ТЗ).
fn db_path(root: &Path) -> PathBuf {
    root.join(".atlas").join("index.db")
}

fn open_store(root: &Path) -> Result<Store> {
    let path = db_path(root);
    std::fs::create_dir_all(path.parent().unwrap())?;
    Ok(Store::open(&path)?)
}

/// Запускает команду с фильтром вывода (раздел 9.1 ТЗ).
/// Оставляет ошибки, предупреждения и контекст вокруг них, схлопывает повторы,
/// убирает ANSI-коды и шум из каталогов зависимостей.
fn cmd_run(command: &str) -> Result<bool> {
    println!("🏃 Выполнение: {command}");
    use std::process::Command;
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let raw = format!("{stdout}\n{stderr}");
    let filtered = filter_output(&raw, &FilterOptions::default());
    if !filtered.text.trim().is_empty() {
        println!("{}", filtered.text);
    }

    let failed = !output.status.success();
    if failed {
        println!("❌ Команда завершилась с ошибкой (код: {:?})", output.status.code());
    } else {
        println!("✅ Команда выполнена успешно");
    }
    Ok(failed)
}

/// Печатает отчёт о состоянии и возвращает true, если есть критичная проблема.
fn doctor(root: &Path) -> Result<bool> {
    let mut failed = false;
    println!("Atlas doctor: {}", root.display());

    let path = db_path(root);
    if !path.exists() {
        println!("[FAIL] индекса нет: {} — выполните `atlas index {}`", path.display(), root.display());
        return Ok(true);
    }
    println!("[ OK ] файл индекса: {}", path.display());

    let store = match Store::open(&path) {
        Ok(s) => s,
        Err(e) => {
            println!("[FAIL] база не открывается: {e} — удалите .atlas/ и переиндексируйте");
            return Ok(true);
        }
    };
    match store.integrity_check() {
        Ok(r) if r == "ok" => println!("[ OK ] целостность базы"),
        Ok(r) => {
            println!("[FAIL] целостность базы: {r}");
            failed = true;
        }
        Err(e) => {
            println!("[FAIL] проверка целостности: {e}");
            failed = true;
        }
    }

    match store.project_info()? {
        Some((indexed_root, Some(when))) => {
            println!("[ OK ] проект проиндексирован: {when}");
            if Path::new(&indexed_root) != root {
                println!("[WARN] индекс построен для другого пути: {indexed_root}");
            }
        }
        _ => {
            println!("[FAIL] проект не проиндексирован — выполните `atlas index`");
            failed = true;
        }
    }

    let nodes = store.count_by_kind()?;
    let total: i64 = nodes.iter().map(|(_, n)| n).sum();
    let parts: Vec<String> = nodes.iter().map(|(k, n)| format!("{k} {n}")).collect();
    println!("[ OK ] узлов: {total} ({})", parts.join(", "));
    let edges = store.count_edges_by_kind()?;
    if edges.is_empty() {
        println!("[WARN] связей нет: граф пуст (проект без поддерживаемых языков или без вызовов и импортов)");
    } else {
        let parts: Vec<String> = edges.iter().map(|(k, n)| format!("{k} {n}")).collect();
        println!("[ OK ] связей: {}", parts.join(", "));
    }
    Ok(failed)
}

/// Наблюдает за изменениями файлов и переиндексирует их по ходу дела (раздел 4.3 ТЗ).
/// Работает до прерывания (Ctrl+C).
fn cmd_watch(root: &Path) -> Result<()> {
    use std::time::Duration;

    let root_str = root.to_string_lossy().to_string();
    let mut store = open_store(root)?;

    // Первичная индексация, если индекса ещё нет
    if store.project_info()?.and_then(|p| p.1).is_none() {
        tracing::info!("индекса ещё нет, строю сначала");
        ProjectIndexer::new(&mut store, &root_str).index_project()?;
    }

    let mut watcher = FileWatcher::new()?;
    watcher.watch(root)?;
    println!("👀 Наблюдение за {} (Ctrl+C для остановки)...", root.display());

    loop {
        match watcher.next_event() {
            Some(WatchEvent::Modified(p))
            | Some(WatchEvent::Created(p)) => {
                if let Ok(rel) = p.strip_prefix(root) {
                    let rel_str = rel.to_string_lossy().to_string();
                    if rel_str.starts_with(".atlas/") || rel_str.starts_with(".git/") {
                        continue;
                    }
                    if let Ok(content) = std::fs::read_to_string(&p) {
                        let mut upd = IncrementalUpdater::new(&mut store, &root_str);
                        if upd.update_file(&rel_str, &content)? {
                            println!("🔄 Переиндексирован: {}", rel_str);
                        }
                    }
                }
            }
            Some(WatchEvent::Deleted(p)) => {
                if let Ok(rel) = p.strip_prefix(root) {
                    let rel_str = rel.to_string_lossy().to_string();
                    store.delete_nodes_by_path(&rel_str)?;
                    store.clear_empty_dirs()?;
                    atlas_core::ranking::recompute_ranks(&store)?;
                    println!("🗑️  Удалён из индекса: {}", rel_str);
                }
            }
            Some(WatchEvent::Renamed(from, to)) => {
                if let Ok(rel_from) = from.strip_prefix(root) {
                    let rel_from_str = rel_from.to_string_lossy().to_string();
                    store.delete_nodes_by_path(&rel_from_str)?;
                }
                if let Ok(rel_to) = to.strip_prefix(root) {
                    if let Ok(content) = std::fs::read_to_string(&to) {
                        let mut upd = IncrementalUpdater::new(&mut store, &root_str);
                        if upd.update_file(rel_to.to_string_lossy().as_ref(), &content)? {
                            println!("🔄 Переиндексирован (переименован): {}", rel_to.to_string_lossy());
                        }
                    }
                }
                store.clear_empty_dirs()?;
                atlas_core::ranking::recompute_ranks(&store)?;
            }
            None => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}
