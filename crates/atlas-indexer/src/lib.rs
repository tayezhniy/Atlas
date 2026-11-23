pub mod walker;
pub mod watcher;
pub mod incremental;
pub mod indexer;

// Реэкспорт
pub use walker::{ProjectWalker, WalkOptions, WalkedFile};
pub use watcher::{FileWatcher, WatchEvent};
pub use indexer::{ProjectIndexer, IndexStats};