use clap::{Parser, Subcommand};
use anyhow::Result;
use tracing_subscriber::fmt;

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
        /// Транспорт: stdio или http
        #[arg(long, default_value = "stdio")]
        transport: String,
    },
    /// Запуск команды с фильтром вывода (раздел 9.1 ТЗ)
    Run {
        /// Команда для выполнения
        #[arg(value_name = "COMMAND")]
        command: String,
    },
    /// Проверка состояния приложения
    Doctor,
}

fn main() -> Result<()> {
    fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Index { path } => {
            println!("Индексация проекта: {}", path);
            // TODO: atlas-indexer
        }
        Commands::Mcp { transport } => {
            println!("Запуск MCP-сервера (транспорт: {})", transport);
            // TODO: atlas-mcp
        }
        Commands::Run { command } => {
            println!("Выполнение: {}", command);
            // TODO: фильтр вывода команд (раздел 9.1)
        }
        Commands::Doctor => {
            println!("Проверка состояния Atlas...");
            // TODO: диагностика
        }
    }

    Ok(())
}