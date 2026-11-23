pub mod db;
pub mod schema;
pub mod error;

// Реэкспорт типов для удобства
pub use db::{Store, StoreConnection};
pub use schema::{Node, Edge, FileMeta, NodeKind, EdgeKind};
pub use error::StoreError;