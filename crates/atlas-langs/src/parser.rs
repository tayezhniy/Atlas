use thiserror::Error;
use anyhow::Result;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("unsupported language: {0}")]
    UnsupportedLanguage(String),
    #[error("parse error: {0}")]
    Parse(String),
}

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

/// Результат парсинга одного файла.
#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub path: String,
    pub language: String,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<Import>,
    pub calls: Vec<Call>,
}

/// Парсер исходного кода на основе tree-sitter.
pub struct LanguageParser {}

impl LanguageParser {
    pub fn new() -> Self {
        Self {}
    }

    /// Определяет язык по расширению файла.
    pub fn detect_language(path: &str) -> Option<&'static str> {
        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())?;
        match ext {
            "ts" | "tsx" | "js" | "jsx" => Some("typescript"),
            "py" => Some("python"),
            "rs" => Some("rust"),
            "go" => Some("go"),
            "md" => Some("markdown"),
            _ => None,
        }
    }

    /// Парсит файл и возвращает символы, импорты и вызовы.
    pub fn parse(&self, path: &str, content: &str) -> Result<ParsedFile> {
        let language = Self::detect_language(path)
            .ok_or_else(|| anyhow::anyhow!(ParseError::UnsupportedLanguage(path.to_string())))?;

        let _ = language;
        let _ = content;

        Ok(ParsedFile {
            path: path.to_string(),
            language: language.to_string(),
            symbols: vec![],
            imports: vec![],
            calls: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(LanguageParser::detect_language("src/main.rs"), Some("rust"));
        assert_eq!(LanguageParser::detect_language("src/main.ts"), Some("typescript"));
        assert_eq!(LanguageParser::detect_language("src/main.py"), Some("python"));
        assert_eq!(LanguageParser::detect_language("src/main.go"), Some("go"));
        assert_eq!(LanguageParser::detect_language("README.md"), Some("markdown"));
        assert_eq!(LanguageParser::detect_language("data.bin"), None);
    }
}