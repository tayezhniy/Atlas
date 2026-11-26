mod extra_symbols;
pub mod parser;
pub mod queries;
pub mod tree_sitter_parser;

// Реэкспорт
pub use parser::{
    Call, Heritage, Import, LanguageParser, ParsedFile, Symbol, SymbolKind, Visibility,
};
pub use tree_sitter_parser::TreeSitterParser;
