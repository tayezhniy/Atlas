mod extra_symbols;
pub mod parser;
pub mod queries;
pub mod tree_sitter_parser;

// Реэкспорт
pub use parser::{LanguageParser, ParsedFile, Symbol, Import, Call, Heritage, SymbolKind, Visibility};
pub use tree_sitter_parser::TreeSitterParser;