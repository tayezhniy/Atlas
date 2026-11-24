use anyhow::{bail, Context, Result};
use atlas_indexer::ProjectIndexer;
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
                "Символов {}, импортов {}, вызовов {}",
                stats.symbols_extracted, stats.imports_extracted, stats.calls_extracted
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
        Commands::Run { command: _ } => {
            bail!("atlas run ещё не реализован (фильтр вывода, раздел 9.1 ТЗ, этап 3)");
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
