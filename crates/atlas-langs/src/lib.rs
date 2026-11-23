pub mod parser;
pub mod queries;

// Реэкспорт
pub use parser::{LanguageParser, ParsedFile, Symbol, Import, Call};