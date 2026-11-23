use crate::parser::{
    Call, Import, ParsedFile, Symbol, SymbolKind, Visibility,
};
use anyhow::{Context, Result};
use std::path::Path;
use tree_sitter::{Parser, Tree};

/// Реальный парсер на основе tree-sitter (раздел 4.2 ТЗ).
pub struct TreeSitterParser {
    parser: Parser,
}

impl TreeSitterParser {
    pub fn new() -> Result<Self> {
        let parser = Parser::new();
        Ok(Self { parser })
    }

    /// Определяет язык по расширению файла.
    pub fn detect_language(path: &str) -> Option<&'static str> {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())?;
        match ext {
            "rs" => Some("rust"),
            "py" => Some("python"),
            "ts" | "tsx" => Some("typescript"),
            "js" | "jsx" => Some("typescript"), // tree-sitter-typescript парсит и JS, и TS
            _ => None,
        }
    }

    /// Устанавливает язык парсера по имени.
    fn set_language(&mut self, lang_name: &str) -> Result<()> {
        let lang = match lang_name {
            "rust" => tree_sitter_rust::language(),
            "python" => tree_sitter_python::language(),
            "typescript" => tree_sitter_typescript::language_typescript(),
            other => anyhow::bail!("unsupported language: {}", other),
        };
        self.parser
            .set_language(&lang)
            .context(format!("failed to set language: {}", lang_name))?;
        Ok(())
    }

    /// Парсит файл и возвращает символы, импорты и вызовы.
    pub fn parse_file(&mut self, path: &str, content: &str) -> Result<ParsedFile> {
        let lang_name = Self::detect_language(path)
            .ok_or_else(|| anyhow::anyhow!("unsupported language for file: {}", path))?;

        self.set_language(lang_name)?;

        let tree = self
            .parser
            .parse(content, None)
            .context("failed to parse source code")?;

        let root_node = tree.root_node();
        let source = content.as_bytes();

        let symbols = extract_symbols(&root_node, source, lang_name)?;
        let imports = extract_imports(&root_node, source, lang_name)?;
        let calls = extract_calls(&root_node, source, lang_name)?;

        Ok(ParsedFile {
            path: path.to_string(),
            language: lang_name.to_string(),
            symbols,
            imports,
            calls,
        })
    }
}

impl Default for TreeSitterParser {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

// ---------------------------------------------------------------------------
// Рекурсивный обход дерева (tree-sitter 0.22: child_count + child(i))
// ---------------------------------------------------------------------------

fn for_each_child<F>(node: &tree_sitter::Node, mut f: F)
where
    F: FnMut(tree_sitter::Node),
{
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            f(child);
        }
    }
}

// ---------------------------------------------------------------------------
// Извлечение символов
// ---------------------------------------------------------------------------

fn extract_symbols(
    node: &tree_sitter::Node,
    source: &[u8],
    lang: &str,
) -> Result<Vec<Symbol>> {
    match lang {
        "rust" => extract_symbols_rust(node, source),
        "python" => extract_symbols_python(node, source),
        "typescript" => extract_symbols_ts_js(node, source),
        _ => Ok(vec![]),
    }
}

fn extract_symbols_rust(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    collect_symbols_by_type(
        node,
        source,
        &[
            "function_item",
            "struct_item",
            "enum_item",
            "trait_item",
            "impl_item",
            "const_item",
            "type_item",
            "mod_item",
        ],
        &mut symbols,
        "rust",
    )?;
    Ok(symbols)
}

fn extract_symbols_python(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    collect_symbols_by_type(
        node,
        source,
        &[
            "function_definition",
            "class_definition",
            "async_function_definition",
        ],
        &mut symbols,
        "python",
    )?;
    Ok(symbols)
}

fn extract_symbols_ts_js(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    collect_symbols_by_type(
        node,
        source,
        &[
            "function_declaration",
            "class_declaration",
            "method_definition",
            "lexical_declaration",
            "variable_declarator",
            "arrow_function",
            "function_expression",
        ],
        &mut symbols,
        "typescript",
    )?;
    Ok(symbols)
}

/// Рекурсивно собирает узлы заданных типов и извлекает из них символы.
fn collect_symbols_by_type(
    node: &tree_sitter::Node,
    source: &[u8],
    types: &[&str],
    out: &mut Vec<Symbol>,
    lang: &str,
) -> Result<()> {
    if types.contains(&node.kind()) {
        if let Some(sym) = node_to_symbol(node, source, lang) {
            out.push(sym);
        }
    }
    for_each_child(node, |child| {
        let _ = collect_symbols_by_type(&child, source, types, out, lang);
    });
    Ok(())
}

/// Преобразует tree-sitter узел в Symbol.
fn node_to_symbol(node: &tree_sitter::Node, source: &[u8], lang: &str) -> Option<Symbol> {
    let name = extract_name(node, source, lang)?;
    let kind = map_kind(node.kind(), lang);
    let range = node.range();
    let signature = extract_signature(node, source);
    let doc = extract_doc_comment(node, source);
    let visibility = extract_visibility(node, source, lang);

    Some(Symbol {
        name,
        kind,
        line_start: range.start_point.row + 1,
        line_end: range.end_point.row + 1,
        signature,
        doc,
        visibility,
    })
}

/// Извлекает имя символа из узла (первый идентификатор или NamedChild).
fn extract_name(node: &tree_sitter::Node, source: &[u8], lang: &str) -> Option<String> {
    // Простой подход: ищем identifier среди named children
    find_child_by_kind(node, "identifier")
        .and_then(|n| node_text(&n, source))
        .or_else(|| {
            // В TypeScript/JavaScript имя может быть в поле "name"
            if lang == "typescript" {
                find_child_by_kind(node, "name").and_then(|name_node| {
                    find_child_by_kind(&name_node, "identifier")
                        .and_then(|n| node_text(&n, source))
                })
            } else {
                None
            }
        })
}

/// Находит первого named child с заданным типом.
fn find_child_by_kind<'a>(node: &'a tree_sitter::Node, kind: &str) -> Option<tree_sitter::Node<'a>> {
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            if child.kind() == kind {
                return Some(child);
            }
        }
    }
    None
}

/// Извлекает текст узла из исходника.
fn node_text(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    let range = node.range();
    let start = utf8_byte_index(source, range.start_point.row, range.start_point.column);
    let end = utf8_byte_index(source, range.end_point.row, range.end_point.column);
    if start >= end || end > source.len() {
        return None;
    }
    String::from_utf8(source[start..end].to_vec()).ok()
}

/// Примерный расчёт байтового индекса по строке/колонке.
fn utf8_byte_index(source: &[u8], row: usize, column: usize) -> usize {
    let mut bytes = 0;
    let mut cur_row = 0;
    let mut cur_col = 0;
    for (i, &b) in source.iter().enumerate() {
        if cur_row == row && cur_col == column {
            return i;
        }
        if b == b'\n' {
            cur_row += 1;
            cur_col = 0;
        } else {
            cur_col += 1;
        }
        bytes = i + 1;
    }
    bytes
}

/// Извлекает сигнатуру символа (первая строка узла).
fn extract_signature(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    let range = node.range();
    let start = utf8_byte_index(source, range.start_point.row, range.start_point.column);
    let end_of_line = source[start..]
        .iter()
        .position(|&b| b == b'\n')
        .unwrap_or(source.len() - start);
    let end = start + end_of_line;
    if start < end && end <= source.len() {
        String::from_utf8(source[start..end].to_vec())
            .ok()
            .filter(|s| !s.trim().is_empty())
    } else {
        None
    }
}

/// Извлекает doc-комментарий (комментарий сразу перед узлом).
fn extract_doc_comment(_node: &tree_sitter::Node, _source: &[u8]) -> Option<String> {
    // tree-sitter не всегда включает комментарии в дерево в зависимости от грамматики;
    // оставим пустым для MVP
    None
}

/// Извлекает видимость символа.
fn extract_visibility(_node: &tree_sitter::Node, _source: &[u8], lang: &str) -> Visibility {
    let _ = lang;
    Visibility::Public
}

/// Маппинг типа узла tree-sitter в SymbolKind.
fn map_kind(kind: &str, _lang: &str) -> SymbolKind {
    match kind {
        "function_item" | "function_definition" | "function_declaration"
        | "async_function_definition" | "arrow_function" | "function_expression" => {
            SymbolKind::Function
        }
        "struct_item" | "class_definition" | "class_declaration" => SymbolKind::Class,
        "enum_item" => SymbolKind::Type,
        "trait_item" => SymbolKind::Interface,
        "impl_item" => SymbolKind::Class,
        "const_item" | "lexical_declaration" | "variable_declarator" => SymbolKind::Constant,
        "type_item" => SymbolKind::Type,
        "method_definition" => SymbolKind::Method,
        _ => SymbolKind::Function,
    }
}

// ---------------------------------------------------------------------------
// Извлечение импортов
// ---------------------------------------------------------------------------

fn extract_imports(
    node: &tree_sitter::Node,
    source: &[u8],
    lang: &str,
) -> Result<Vec<Import>> {
    match lang {
        "rust" => extract_imports_rust(node, source),
        "python" => extract_imports_python(node, source),
        "typescript" => extract_imports_ts_js(node, source),
        _ => Ok(vec![]),
    }
}

fn extract_imports_rust(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    collect_nodes_by_type(node, source, &["use_declaration"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let target = extract_rust_import_target(&text);
        imports.push(Import {
            from: "self".to_string(),
            to: target,
            line: n.range().start_point.row + 1,
        });
    })?;
    Ok(imports)
}

fn extract_imports_python(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    collect_nodes_by_type(
        node,
        source,
        &["import_statement", "import_from_statement"],
        &mut |n, src| {
            let text = node_text(n, src).unwrap_or_default();
            let target = extract_python_import_target(&text);
            imports.push(Import {
                from: "self".to_string(),
                to: target,
                line: n.range().start_point.row + 1,
            });
        },
    )?;
    Ok(imports)
}

fn extract_imports_ts_js(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    collect_nodes_by_type(node, source, &["import_statement"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let target = extract_ts_import_target(&text);
        imports.push(Import {
            from: "self".to_string(),
            to: target,
            line: n.range().start_point.row + 1,
        });
    })?;
    Ok(imports)
}

fn extract_rust_import_target(text: &str) -> String {
    // use foo::bar::baz; -> foo::bar::baz
    text.trim_start_matches("use")
        .trim_end_matches(';')
        .trim()
        .to_string()
}

fn extract_python_import_target(text: &str) -> String {
    let t = text.trim();
    if t.starts_with("import ") {
        t.trim_start_matches("import ").trim().to_string()
    } else if t.starts_with("from ") {
        let parts: Vec<&str> = t.splitn(2, " import ").collect();
        parts
            .get(0)
            .map(|s| s.trim_start_matches("from ").trim().to_string())
            .unwrap_or_default()
    } else {
        t.to_string()
    }
}

fn extract_ts_import_target(text: &str) -> String {
    let t = text.trim();
    if let Some(from_idx) = t.find("from") {
        let after = t[from_idx..]
            .trim_start_matches("from")
            .trim()
            .trim_start_matches('"')
            .trim_start_matches('\'')
            .trim_end_matches('"')
            .trim_end_matches('\'');
        after.to_string()
    } else {
        t.to_string()
    }
}

/// Рекурсивно собирает узлы заданных типов и вызывает колбэк.
fn collect_nodes_by_type<F>(
    node: &tree_sitter::Node,
    source: &[u8],
    types: &[&str],
    cb: &mut F,
) -> Result<()>
where
    F: FnMut(&tree_sitter::Node, &[u8]),
{
    if types.contains(&node.kind()) {
        cb(node, source);
    }
    for_each_child(node, |child| {
        let _ = collect_nodes_by_type(&child, source, types, cb);
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Извлечение вызовов
// ---------------------------------------------------------------------------

fn extract_calls(
    node: &tree_sitter::Node,
    source: &[u8],
    lang: &str,
) -> Result<Vec<Call>> {
    match lang {
        "rust" => extract_calls_rust(node, source),
        "python" => extract_calls_python(node, source),
        "typescript" => extract_calls_ts_js(node, source),
        _ => Ok(vec![]),
    }
}

fn extract_calls_rust(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    collect_nodes_by_type(node, source, &["call_expression"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_rust_call_callee(&text);
        calls.push(Call {
            caller: "unknown".to_string(),
            callee,
            line: n.range().start_point.row + 1,
        });
    })?;
    Ok(calls)
}

fn extract_calls_python(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    collect_nodes_by_type(node, source, &["call"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_python_call_callee(&text);
        calls.push(Call {
            caller: "unknown".to_string(),
            callee,
            line: n.range().start_point.row + 1,
        });
    })?;
    Ok(calls)
}

fn extract_calls_ts_js(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    collect_nodes_by_type(node, source, &["call_expression"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_ts_call_callee(&text);
        calls.push(Call {
            caller: "unknown".to_string(),
            callee,
            line: n.range().start_point.row + 1,
        });
    })?;
    Ok(calls)
}

fn extract_rust_call_callee(text: &str) -> String {
    let t = text.trim_end_matches(';').trim_end_matches(')').trim();
    let parts: Vec<&str> = t.split("::").collect();
    parts
        .last()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| t.to_string())
}

fn extract_python_call_callee(text: &str) -> String {
    let t = text.trim_end_matches(')').trim();
    let parts: Vec<&str> = t.split('.').collect();
    parts
        .last()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| t.to_string())
}

fn extract_ts_call_callee(text: &str) -> String {
    let t = text.trim_end_matches(';').trim_end_matches(')').trim();
    let parts: Vec<&str> = t.split('.').collect();
    parts
        .last()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(TreeSitterParser::detect_language("src/main.rs"), Some("rust"));
        assert_eq!(TreeSitterParser::detect_language("src/main.py"), Some("python"));
        assert_eq!(TreeSitterParser::detect_language("src/main.ts"), Some("typescript"));
        assert_eq!(TreeSitterParser::detect_language("src/main.js"), Some("typescript"));
        assert_eq!(TreeSitterParser::detect_language("README.md"), None);
    }

    #[test]
    fn test_parse_rust() {
        let mut parser = TreeSitterParser::new().unwrap();
        let code = "pub fn foo(x: i32) -> i32 { x + 1 }\nuse std::collections::HashMap;\nfn bar() { foo(42); }\n";
        let result = parser.parse_file("src/lib.rs", code).unwrap();
        assert!(!result.symbols.is_empty(), "должны быть символы");
        assert!(!result.imports.is_empty(), "должны быть импорты");
        assert!(!result.calls.is_empty(), "должны быть вызовы");
        assert_eq!(result.language, "rust");
    }

    #[test]
    fn test_parse_python() {
        let mut parser = TreeSitterParser::new().unwrap();
        let code = "def foo(x):\n    return x + 1\n\nimport os\n\nbar(1)\n";
        let result = parser.parse_file("src/main.py", code).unwrap();
        assert!(!result.symbols.is_empty());
        assert_eq!(result.language, "python");
    }

    #[test]
    fn test_parse_typescript() {
        let mut parser = TreeSitterParser::new().unwrap();
        let code = "export function foo(x: number): number { return x + 1; }\nimport { bar } from './bar';\nbaz(1);\n";
        let result = parser.parse_file("src/main.ts", code).unwrap();
        assert!(!result.symbols.is_empty());
        assert_eq!(result.language, "typescript");
    }
}