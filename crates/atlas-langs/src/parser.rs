use crate::tree_sitter_parser::TreeSitterParser;
use anyhow::Result;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("unsupported language: {0}")]
    UnsupportedLanguage(String),
    #[error("parse error: {0}")]
    Parse(String),
}

use thiserror::Error;

/// Символ, извлечённый из кода (раздел 4.1 ТЗ).
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub line_start: usize,
    pub line_end: usize,
    pub signature: Option<String>,
    pub doc: Option<String>,
    pub visibility: Visibility,
}

/// Вид символа.
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Interface,
    Constant,
    Type,
    Field,
}

/// Видимость символа.
#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Package,
    Unknown,
}

/// Импорт между модулями.
#[derive(Debug, Clone)]
pub struct Import {
    pub from: String,
    pub to: String,
    pub line: usize,
}

/// Вызов между символами.
#[derive(Debug, Clone)]
pub struct Call {
    pub caller: String,
    pub callee: String,
    pub line: usize,
}

/// Наследование: класс-потомок и имя родителя/интерфейса.
#[derive(Debug, Clone)]
pub struct Heritage {
    /// Имя класса/структуры, которая наследует.
    pub child: String,
    /// Имя родителя или реализуемого интерфейса.
    pub parent: String,
    pub line: usize,
}

/// Результат парсинга одного файла.
#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub path: String,
    pub language: String,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<Import>,
    pub calls: Vec<Call>,
    pub extends: Vec<Heritage>,
}

/// Парсер исходного кода на основе tree-sitter (раздел 4.2 ТЗ).
pub struct LanguageParser {
    inner: TreeSitterParser,
}

impl LanguageParser {
    pub fn new() -> Self {
        Self {
            inner: TreeSitterParser::new().unwrap(),
        }
    }

    /// Определяет язык по расширению файла.
    pub fn detect_language(path: &str) -> Option<&'static str> {
        TreeSitterParser::detect_language(path)
    }

    /// Парсит файл и возвращает символы, импорты и вызовы.
    pub fn parse(&mut self, path: &str, content: &str) -> Result<ParsedFile> {
        self.inner.parse_file(path, content)
    }
}

impl Default for LanguageParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(LanguageParser::detect_language("src/main.rs"), Some("rust"));
        assert_eq!(
            LanguageParser::detect_language("src/main.ts"),
            Some("typescript")
        );
        assert_eq!(
            LanguageParser::detect_language("src/main.js"),
            Some("typescript")
        );
        assert_eq!(
            LanguageParser::detect_language("src/main.py"),
            Some("python")
        );
        assert_eq!(LanguageParser::detect_language("data.bin"), None);
    }

    #[test]
    fn test_parse_rust() {
        let mut parser = LanguageParser::new();
        let code = "pub fn foo(x: i32) -> i32 { x + 1 }\nuse std::collections::HashMap;\nfn bar() { foo(42); }\n";
        let result = parser.parse("src/lib.rs", code).unwrap();
        assert!(!result.symbols.is_empty());
        assert!(!result.imports.is_empty());
        assert_eq!(result.language, "rust");
    }

    #[test]
    fn test_parse_python() {
        let mut parser = LanguageParser::new();
        let code = "def foo(x):\n    return x + 1\n\nimport os\n\nbar(1)\n";
        let result = parser.parse("src/main.py", code).unwrap();
        assert!(!result.symbols.is_empty());
        assert_eq!(result.language, "python");
    }
}
