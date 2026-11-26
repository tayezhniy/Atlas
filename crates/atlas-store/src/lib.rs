pub mod db;
pub mod error;
pub mod schema;

// Реэкспорт типов для удобства
pub use db::{Store, StoreConnection};
pub use error::StoreError;
pub use schema::{Edge, EdgeKind, FileMeta, Node, NodeKind};
