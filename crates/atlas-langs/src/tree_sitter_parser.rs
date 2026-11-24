use crate::parser::{
    Call, Heritage, Import, ParsedFile, Symbol, SymbolKind, Visibility,
};
use anyhow::{Context, Result};
use std::path::Path;
use tree_sitter::Parser;

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
        let extends = extract_extends(&root_node, source, lang_name)?;

        Ok(ParsedFile {
            path: path.to_string(),
            language: lang_name.to_string(),
            symbols,
            imports,
            calls,
            extends,
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
            "interface_declaration",
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

/// Извлекает имя символа: поле `name`, затем первый подходящий именованный ребёнок.
fn extract_name(node: &tree_sitter::Node, source: &[u8], _lang: &str) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name("name") {
        if let Some(text) = node_text(&name_node, source) {
            let text = text.trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            if matches!(
                child.kind(),
                "identifier" | "type_identifier" | "property_identifier" | "field_identifier"
            ) {
                if let Some(text) = node_text(&child, source) {
                    let text = text.trim().to_string();
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }
    }
    None
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

/// Извлекает doc-комментарий: docstring Python или комментарии непосредственно перед узлом.
fn extract_doc_comment(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    if let Some(doc) = python_docstring(node, source) {
        return Some(doc);
    }

    let mut parts: Vec<String> = Vec::new();
    let mut expected_row = node.range().start_point.row;
    let mut prev = node.prev_named_sibling();
    while let Some(p) = prev {
        let kind = p.kind();
        let is_comment = kind.contains("comment");
        if !is_comment {
            break;
        }
        // Комментарий должен примыкать к узлу без пустых строк между ними
        if p.range().end_point.row + 1 != expected_row {
            break;
        }
        parts.push(node_text(&p, source).unwrap_or_default());
        expected_row = p.range().start_point.row;
        prev = p.prev_named_sibling();
    }
    if parts.is_empty() {
        return None;
    }
    parts.reverse();
    let cleaned = clean_doc_text(&parts.join("\n"));
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// Первая строка тела Python-функции/класса, если это строковый литерал.
fn python_docstring(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    if !matches!(node.kind(), "function_definition" | "class_definition" | "async_function_definition") {
        return None;
    }
    let body = node.child_by_field_name("body")?;
    let first = body.named_child(0)?;
    if first.kind() != "expression_statement" {
        return None;
    }
    let literal = first.named_child(0)?;
    if literal.kind() != "string" {
        return None;
    }
    let raw = node_text(&literal, source)?;
    let cleaned = clean_doc_text(&strip_string_quotes(&raw));
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn strip_string_quotes(raw: &str) -> String {
    let t = raw.trim();
    for q in ["\"\"\"", "'''"] {
        if let Some(inner) = t.strip_prefix(q).and_then(|s| s.strip_suffix(q)) {
            return inner.to_string();
        }
    }
    for q in ['"', '\''] {
        if let Some(inner) = t.strip_prefix(q).and_then(|s| s.strip_suffix(q)) {
            return inner.to_string();
        }
    }
    t.to_string()
}

/// Убирает маркеры комментариев и выравнивает отступы.
fn clean_doc_text(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let mut l = line.trim();
        for prefix in ["///", "//!", "//", "/**", "/*", "*/", "*", "#"] {
            if let Some(rest) = l.strip_prefix(prefix) {
                l = rest.trim_start();
                break;
            }
        }
        if l.is_empty() && out.last().map(|s| s.is_empty()).unwrap_or(true) {
            continue;
        }
        out.push(l.to_string());
    }
    while out.last().map(|s| s.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.join("\n").trim().to_string()
}

/// Извлекает видимость символа.
fn extract_visibility(node: &tree_sitter::Node, source: &[u8], lang: &str) -> Visibility {
    let first_line = extract_signature(node, source).unwrap_or_default();
    match lang {
        "rust" => {
            let before_name = first_line
                .split(&extract_name(node, source, lang).unwrap_or_default())
                .next()
                .unwrap_or("");
            if before_name.split_whitespace().any(|w| w == "pub" || w.starts_with("pub(")) {
                Visibility::Public
            } else {
                Visibility::Private
            }
        }
        "typescript" => {
            let t = first_line.trim_start();
            if t.contains(" private ") || t.starts_with("private ") {
                Visibility::Private
            } else if t.contains(" protected ") || t.starts_with("protected ") {
                Visibility::Protected
            } else if t.starts_with("export ") || t.contains(" export ") {
                Visibility::Public
            } else {
                Visibility::Package
            }
        }
        "python" => {
            let name = extract_name(node, source, lang).unwrap_or_default();
            if name.starts_with("__") && !name.ends_with("__") {
                Visibility::Private
            } else if name.starts_with('_') && !name.starts_with("__") {
                Visibility::Protected
            } else {
                Visibility::Public
            }
        }
        _ => Visibility::Unknown,
    }
}

/// Маппинг типа узла tree-sitter в SymbolKind.
fn map_kind(kind: &str, _lang: &str) -> SymbolKind {
    match kind {
        "function_item" | "function_definition" | "function_declaration"
        | "async_function_definition" | "arrow_function" | "function_expression" => {
            SymbolKind::Function
        }
        "struct_item" | "class_definition" | "class_declaration" => SymbolKind::Class,
        "interface_declaration" => SymbolKind::Interface,
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
        for target in rust_use_targets(&text) {
            imports.push(Import {
                from: "self".to_string(),
                to: target,
                line: n.range().start_point.row + 1,
            });
        }
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
            for target in python_import_targets(&text) {
                imports.push(Import {
                    from: "self".to_string(),
                    to: target,
                    line: n.range().start_point.row + 1,
                });
            }
        },
    )?;
    Ok(imports)
}

fn extract_imports_ts_js(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    collect_nodes_by_type(
        node,
        source,
        &["import_statement", "export_statement"],
        &mut |n, src| {
            let text = node_text(n, src).unwrap_or_default();
            if let Some(target) = ts_module_specifier(&text) {
                imports.push(Import {
                    from: "self".to_string(),
                    to: target,
                    line: n.range().start_point.row + 1,
                });
            }
        },
    )?;
    Ok(imports)
}

/// `use crate::a::{b, c as d};` -> ["crate::a::b", "crate::a::c"].
fn rust_use_targets(text: &str) -> Vec<String> {
    let mut t = text.trim();
    if let Some(rest) = t.strip_prefix("pub ") {
        t = rest.trim_start();
    }
    let t = t.strip_prefix("use ").unwrap_or(t).trim().trim_end_matches(';').trim();
    expand_rust_use(t)
}

fn expand_rust_use(t: &str) -> Vec<String> {
    let t = t.trim();
    if t.is_empty() {
        return vec![];
    }
    if let Some(open) = t.find('{') {
        if let Some(close) = t.rfind('}') {
            let prefix = t[..open].trim_end_matches("::").trim();
            let inner = &t[open + 1..close];
            let mut out = Vec::new();
            for part in split_top_level(inner, ',') {
                let p = part.trim();
                if p.is_empty() {
                    continue;
                }
                for e in expand_rust_use(p) {
                    out.push(if prefix.is_empty() {
                        e
                    } else {
                        format!("{prefix}::{e}")
                    });
                }
            }
            return out;
        }
    }
    let base = t.split(" as ").next().unwrap_or(t).trim();
    if base.is_empty() || base == "self" || base == "*" {
        vec![]
    } else {
        vec![base.to_string()]
    }
}

/// `import a, b as c` -> ["a", "b"]; `from .pkg import x` -> [".pkg"].
fn python_import_targets(text: &str) -> Vec<String> {
    let t = text.trim();
    if let Some(rest) = t.strip_prefix("import ") {
        return split_top_level(rest, ',')
            .into_iter()
            .filter_map(|part| {
                let name = part.split(" as ").next().unwrap_or("").trim();
                (!name.is_empty()).then(|| name.to_string())
            })
            .collect();
    }
    if let Some(rest) = t.strip_prefix("from ") {
        if let Some((module, _)) = rest.split_once(" import ") {
            let module = module.trim();
            if !module.is_empty() {
                return vec![module.to_string()];
            }
        }
    }
    vec![]
}

/// Достаёт строковый литерал модуля из `import ... from "./x"` или `import "./x"`.
fn ts_module_specifier(text: &str) -> Option<String> {
    let t = text.trim();
    if !(t.starts_with("import ") || t.starts_with("export ")) {
        return None;
    }
    // Ищем последний строковый литерал в кавычках
    let (start, quote) = t
        .char_indices()
        .find(|(_, c)| *c == '\'' || *c == '"')
        .map(|(i, c)| (i, c))?;
    let rest = &t[start + quote.len_utf8()..];
    let end = rest.find(quote)?;
    let spec = &rest[..end];
    (!spec.is_empty()).then(|| spec.to_string())
}

/// Делит строку по разделителю, не заходя внутрь фигурных скобок.
fn split_top_level(text: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for c in text.chars() {
        match c {
            '{' => {
                depth += 1;
                current.push(c);
            }
            '}' => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            _ if c == sep && depth == 0 => {
                out.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
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

// ---------------------------------------------------------------------------
// Извлечение наследования
// ---------------------------------------------------------------------------

fn extract_extends(
    node: &tree_sitter::Node,
    source: &[u8],
    lang: &str,
) -> Result<Vec<Heritage>> {
    let mut out = Vec::new();
    match lang {
        "rust" => {
            collect_nodes_by_type(node, source, &["impl_item"], &mut |n, src| {
                let (Some(trait_node), Some(type_node)) =
                    (n.child_by_field_name("trait"), n.child_by_field_name("type"))
                else {
                    return;
                };
                let child = last_type_segment(&node_text(&type_node, src).unwrap_or_default());
                let parent = last_type_segment(&node_text(&trait_node, src).unwrap_or_default());
                if !child.is_empty() && !parent.is_empty() {
                    out.push(Heritage {
                        child,
                        parent,
                        line: n.range().start_point.row + 1,
                    });
                }
            })?;
        }
        "python" => {
            collect_nodes_by_type(node, source, &["class_definition"], &mut |n, src| {
                let Some(name_node) = n.child_by_field_name("name") else {
                    return;
                };
                let child = last_type_segment(&node_text(&name_node, src).unwrap_or_default());
                let Some(bases) = n.child_by_field_name("superclasses") else {
                    return;
                };
                for i in 0..bases.named_child_count() {
                    if let Some(base) = bases.named_child(i) {
                        let parent = last_type_segment(&node_text(&base, src).unwrap_or_default());
                        if !parent.is_empty() {
                            out.push(Heritage {
                                child: child.clone(),
                                parent,
                                line: n.range().start_point.row + 1,
                            });
                        }
                    }
                }
            })?;
        }
        "typescript" => {
            collect_nodes_by_type(
                node,
                source,
                &["class_declaration", "interface_declaration"],
                &mut |n, src| {
                    let Some(name_node) = n.child_by_field_name("name") else {
                        return;
                    };
                    let child = last_type_segment(&node_text(&name_node, src).unwrap_or_default());
                    let text = node_text(n, src).unwrap_or_default();
                    let header = text.split('{').next().unwrap_or(&text);
                    for (keyword, rest) in [("extends", header), ("implements", header)] {
                        let Some(pos) = rest.find(keyword) else {
                            continue;
                        };
                        let after = &rest[pos + keyword.len()..];
                        let after = after.split("implements").next().unwrap_or(after);
                        for part in after.split(',') {
                            let parent = last_type_segment(part);
                            if !parent.is_empty() {
                                out.push(Heritage {
                                    child: child.clone(),
                                    parent,
                                    line: n.range().start_point.row + 1,
                                });
                            }
                        }
                    }
                },
            )?;
        }
        _ => {}
    }
    Ok(out)
}

/// Последний сегмент типа без обобщений: `crate::a::B<T>` -> `B`.
fn last_type_segment(text: &str) -> String {
    let t = text.trim();
    let t = t.split('<').next().unwrap_or(t).trim();
    let seg = t
        .rsplit(|c: char| c == '.' || c == ':')
        .next()
        .unwrap_or(t)
        .trim();
    seg.trim_matches(|c: char| !(c.is_alphanumeric() || c == '_'))
        .to_string()
}

/// Выражение вызова до открывающей скобки аргументов: `obj.method(x.y)` -> `obj.method`.
fn callee_head(text: &str) -> &str {
    text.split('(').next().unwrap_or(text).trim().trim_end_matches('!')
}

fn last_segment<'a>(head: &'a str, separators: &[&str]) -> &'a str {
    let mut cut = 0;
    for sep in separators {
        if let Some(pos) = head.rfind(sep) {
            cut = cut.max(pos + sep.len());
        }
    }
    head[cut..].trim()
}

fn extract_rust_call_callee(text: &str) -> String {
    last_segment(callee_head(text), &["::", "."]).to_string()
}

fn extract_python_call_callee(text: &str) -> String {
    last_segment(callee_head(text), &["."]).to_string()
}

fn extract_ts_call_callee(text: &str) -> String {
    last_segment(callee_head(text), &["."]).to_string()
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