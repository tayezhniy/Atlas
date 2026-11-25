pub mod support;
pub mod walker;
pub mod watcher;
pub mod incremental;
pub mod indexer;
pub mod lsp_client;

// Реэкспорт
pub use walker::{ProjectWalker, WalkOptions, WalkedFile};
pub use watcher::{FileWatcher, WatchEvent};
pub use indexer::{ProjectIndexer, IndexStats};
pub use incremental::IncrementalUpdater;