pub mod incremental;
pub mod indexer;
pub mod lsp_client;
pub mod support;
pub mod walker;
pub mod watcher;

// Реэкспорт
pub use incremental::IncrementalUpdater;
pub use indexer::{IndexStats, ProjectIndexer};
pub use walker::{ProjectWalker, WalkOptions, WalkedFile};
pub use watcher::{FileWatcher, WatchEvent};
