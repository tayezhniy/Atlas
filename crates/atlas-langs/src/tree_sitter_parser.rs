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
            "js" | "jsx" => Some("typescript"),
            "go" => Some("go"),
            "java" => Some("java"),
            "cs" => Some("c_sharp"),
            "c" | "h" => Some("c"),
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Some("cpp"),
            "dart" => Some("dart"),
            "swift" => Some("swift"),
            "kt" | "kts" => Some("kotlin"),
            "lua" => Some("lua"),
            "zig" => Some("zig"),
            "ex" | "exs" => Some("elixir"),
            "hs" => Some("haskell"),
            "ml" | "mli" => Some("ocaml"),
            "prisma" => Some("prisma"),
            "graphql" | "gql" => Some("graphql"),
            "sql" => Some("sql"),
            _ => None,
        }
    }

    /// Устанавливает язык парсера по имени.
    fn set_language(&mut self, lang_name: &str) -> Result<()> {
        let lang = match lang_name {
            "rust" => tree_sitter_rust::LANGUAGE.into(),
            "python" => tree_sitter_python::LANGUAGE.into(),
            "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "go" => tree_sitter_go::LANGUAGE.into(),
            "java" => tree_sitter_java::LANGUAGE.into(),
            "c_sharp" => tree_sitter_c_sharp::LANGUAGE.into(),
            "c" => tree_sitter_c::LANGUAGE.into(),
            "cpp" => tree_sitter_cpp::LANGUAGE.into(),
            "dart" => tree_sitter_dart::LANGUAGE.into(),
            "swift" => tree_sitter_swift::LANGUAGE.into(),
            "kotlin" => tree_sitter_kt::LANGUAGE.into(),
            "lua" => tree_sitter_lua::LANGUAGE.into(),
            "zig" => tree_sitter_zig::LANGUAGE.into(),
            "elixir" => tree_sitter_elixir::LANGUAGE.into(),
            "haskell" => tree_sitter_haskell::LANGUAGE.into(),
            "ocaml" => tree_sitter_ocaml::LANGUAGE_OCAML.into(),
            "prisma" => tree_sitter_prisma::LANGUAGE.into(),
            "graphql" => tree_sitter_graphql::LANGUAGE.into(),
            "sql" => tree_sitter_snowflake::LANGUAGE.into(),
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
        // JSX разбирает только грамматика TSX, а `.mli` — своя грамматика интерфейсов
        match Path::new(path).extension().and_then(|e| e.to_str()) {
            Some("tsx" | "jsx" | "js") => self
                .parser
                .set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())?,
            Some("mli") => self
                .parser
                .set_language(&tree_sitter_ocaml::LANGUAGE_OCAML_INTERFACE.into())?,
            _ => {}
        }

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
        "go" => extract_symbols_go(node, source),
        "java" => extract_symbols_java(node, source),
        "c_sharp" => extract_symbols_csharp(node, source),
        "c" | "cpp" | "dart" | "kotlin" | "elixir" | "haskell" | "ocaml" | "graphql" | "sql" => {
            Ok(crate::extra_symbols::extract(node, source, lang))
        }
        "swift" => extract_symbols_swift(node, source),
        "lua" => extract_symbols_lua(node, source),
        "zig" => extract_symbols_zig(node, source),
        "prisma" => extract_symbols_prisma(node, source),
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

fn extract_symbols_go(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    // Functions and methods
    collect_symbols_by_type(
        node,
        source,
        &["function_declaration", "method_declaration"],
        &mut symbols,
        "go",
    )?;
    // Type declarations (structs, interfaces, type aliases)
    collect_go_type_declarations(node, source, &mut symbols)?;
    // Constants and variables
    collect_symbols_by_type(
        node,
        source,
        &["const_declaration", "var_declaration"],
        &mut symbols,
        "go",
    )?;
    Ok(symbols)
}

/// Собирает type_declaration, но только для struct/interface типов.
fn collect_go_type_declarations(
    node: &tree_sitter::Node,
    source: &[u8],
    out: &mut Vec<Symbol>,
) -> Result<()> {
    if node.kind() == "type_declaration" {
        // Check if it's a struct or interface type
        for i in 0..node.named_child_count() {
            if let Some(spec) = node.named_child(i) {
                if spec.kind() == "type_spec" {
                    // Look for type_identifier (name) and struct_type/interface_type
                    let mut name = None;
                    let mut is_struct_or_interface = false;
                    for j in 0..spec.named_child_count() {
                        if let Some(child) = spec.named_child(j) {
                            if child.kind() == "type_identifier" {
                                name = node_text(&child, source);
                            } else if matches!(child.kind(), "struct_type" | "interface_type") {
                                is_struct_or_interface = true;
                            }
                        }
                    }
                    if let Some(name) = name.filter(|n| !n.is_empty()) {
                        if is_struct_or_interface {
                            let range = spec.range();
                            out.push(Symbol {
                                name,
                                kind: SymbolKind::Class, // struct/interface -> Class
                                line_start: range.start_point.row + 1,
                                line_end: range.end_point.row + 1,
                                signature: extract_signature(&spec, source),
                                doc: extract_doc_comment(&spec, source),
                                visibility: extract_visibility(&spec, source, "go"),
                            });
                        }
                    }
                }
            }
        }
    }
    for_each_child(node, |child| {
        let _ = collect_go_type_declarations(&child, source, out);
    });
    Ok(())
}

fn extract_symbols_java(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    collect_symbols_by_type(
        node,
        source,
        &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "method_declaration",
            "constructor_declaration",
            "field_declaration",
            "constant_declaration",
        ],
        &mut symbols,
        "java",
    )?;
    Ok(symbols)
}

fn extract_symbols_csharp(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    collect_symbols_by_type(
        node,
        source,
        &[
            "class_declaration",
            "interface_declaration",
            "struct_declaration",
            "enum_declaration",
            "method_declaration",
            "constructor_declaration",
            "property_declaration",
            "field_declaration",
            "delegate_declaration",
        ],
        &mut symbols,
        "c_sharp",
    )?;
    Ok(symbols)
}

// ===== Новые языки =====

fn extract_symbols_swift(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    collect_symbols_by_type(
        node,
        source,
        &[
            "function_declaration",
            "method_declaration",
            "class_declaration",
            "struct_declaration",
            "enum_declaration",
            "protocol_declaration",
            "extension_declaration",
            "typealias_declaration",
            "variable_declaration",
        ],
        &mut symbols,
        "swift",
    )?;
    Ok(symbols)
}

fn extract_symbols_lua(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    collect_symbols_by_type(
        node,
        source,
        &[
            "function_declaration",
            "local_function",
            "function_definition",
            "table_constructor",
        ],
        &mut symbols,
        "lua",
    )?;
    Ok(symbols)
}

fn extract_symbols_zig(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    collect_symbols_by_type(
        node,
        source,
        &[
            "function_declaration",
            "struct_declaration",
            "union_declaration",
            "enum_declaration",
            "const_declaration",
            "var_declaration",
        ],
        &mut symbols,
        "zig",
    )?;
    Ok(symbols)
}

fn extract_symbols_prisma(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    collect_symbols_by_type(
        node,
        source,
        &[
            "model_declaration",
            "enum_declaration",
            "type_declaration",
        ],
        &mut symbols,
        "prisma",
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
pub(crate) fn extract_signature(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
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
pub(crate) fn extract_doc_comment(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
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
pub(crate) fn extract_visibility(node: &tree_sitter::Node, source: &[u8], lang: &str) -> Visibility {
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
        "go" => {
            let name = extract_name(node, source, lang).unwrap_or_default();
            if name.chars().next().is_some_and(char::is_uppercase) {
                Visibility::Public
            } else {
                Visibility::Private
            }
        }
        "java" => {
            let t = first_line.trim_start();
            if t.contains(" private ") || t.starts_with("private ") {
                Visibility::Private
            } else if t.contains(" protected ") || t.starts_with("protected ") {
                Visibility::Protected
            } else if t.contains(" public ") || t.starts_with("public ") {
                Visibility::Public
            } else {
                Visibility::Package
            }
        }
        "c_sharp" => {
            let t = first_line.trim_start();
            if t.contains(" private ") || t.starts_with("private ") {
                Visibility::Private
            } else if t.contains(" protected ") || t.starts_with("protected ") {
                Visibility::Protected
            } else if t.contains(" public ") || t.starts_with("public ") {
                Visibility::Public
            } else if t.contains(" internal ") || t.starts_with("internal ") {
                Visibility::Package
            } else {
                Visibility::Private
            }
        }
        "c" | "cpp" => {
            // C/C++: no explicit visibility in function/class definition
            // Could check for static (file-local) or in header vs source
            Visibility::Public
        }
        "dart" => {
            let name = extract_name(node, source, lang).unwrap_or_default();
            if name.starts_with('_') {
                Visibility::Private
            } else {
                Visibility::Public
            }
        }
        "swift" => {
            let t = first_line.trim_start();
            if t.contains(" private ") || t.starts_with("private ") {
                Visibility::Private
            } else if t.contains(" fileprivate ") || t.starts_with("fileprivate ") {
                Visibility::Package
            } else if t.contains(" internal ") || t.starts_with("internal ") {
                Visibility::Package
            } else if t.contains(" public ") || t.starts_with("public ") {
                Visibility::Public
            } else if t.contains(" open ") || t.starts_with("open ") {
                Visibility::Public
            } else {
                Visibility::Package // default is internal
            }
        }
        "kotlin" => {
            let t = first_line.trim_start();
            if t.contains(" private ") || t.starts_with("private ") {
                Visibility::Private
            } else if t.contains(" protected ") || t.starts_with("protected ") {
                Visibility::Protected
            } else if t.contains(" internal ") || t.starts_with("internal ") {
                Visibility::Package
            } else if t.contains(" public ") || t.starts_with("public ") {
                Visibility::Public
            } else {
                Visibility::Public // default is public
            }
        }
        "lua" => {
            // Lua: local = private, global = public
            if first_line.contains("local ") {
                Visibility::Private
            } else {
                Visibility::Public
            }
        }
        "zig" => {
            // Zig: pub = public, default = private
            if first_line.contains("pub ") {
                Visibility::Public
            } else {
                Visibility::Private
            }
        }
        "elixir" => {
            // Elixir: defp = private, def = public
            if first_line.contains("defp ") {
                Visibility::Private
            } else if first_line.contains("def ") {
                Visibility::Public
            } else {
                Visibility::Package
            }
        }
        "haskell" => {
            // Haskell: no explicit visibility, but module exports control it
            Visibility::Public
        }
        "ocaml" => {
            // OCaml: no explicit visibility in signature
            Visibility::Public
        }
        "prisma" => Visibility::Public,
        "graphql" => Visibility::Public,
        "sql" => Visibility::Public,
        _ => Visibility::Unknown,
    }
}

/// Маппинг типа узла tree-sitter в SymbolKind.
fn map_kind(kind: &str, _lang: &str) -> SymbolKind {
    match kind {
        // Functions
        "function_item" | "function_definition" | "function_declaration"
        | "async_function_definition" | "arrow_function" | "function_expression"
        | "constructor_declaration" | "method_declaration" | "function_method_declaration"
        | "function_signature" | "local_function" => SymbolKind::Function,
        
        // Classes/Structs
        "struct_item" | "class_definition" | "class_declaration" 
        | "struct_declaration" | "impl_item" | "class_specifier"
        | "struct_specifier" | "object_declaration" | "data_class_declaration"
        | "mixin_declaration" | "extension_declaration" | "protocol_declaration"
        | "module_declaration" => SymbolKind::Class,
        
        // Interfaces
        "interface_declaration" | "trait_item" | "interface_type_definition" => SymbolKind::Interface,
        
        // Enums/Types
        "enum_item" | "type_item" | "enum_declaration" | "type_declaration"
        | "enum_specifier" | "union_specifier" | "union_declaration"
        | "type_alias_declaration" | "typealias_declaration" | "type_alias"
        | "newtype_declaration" | "data_declaration" => SymbolKind::Type,
        
        // Methods
        "method_definition" | "method_invocation" => SymbolKind::Method,
        
        // Constants/Fields
        "const_item" | "lexical_declaration" | "variable_declarator" 
        | "const_declaration" | "field_declaration" | "constant_declaration" 
        | "property_declaration" | "var_declaration" | "const_spec"
        | "top_level_variable_declaration" | "let_binding" => SymbolKind::Constant,
        
        // C# specific
        "delegate_declaration" => SymbolKind::Type,
        
        // GraphQL specific
        "object_type_definition" => SymbolKind::Class,
        "field_definition" => SymbolKind::Field,
        "directive_definition" => SymbolKind::Constant,
        
        // Prisma specific
        "model_declaration" => SymbolKind::Class,
        
        // SQL specific
        "create_table_statement" => SymbolKind::Class,
        "create_view_statement" => SymbolKind::Class,
        "create_function_statement" => SymbolKind::Function,
        "create_procedure_statement" => SymbolKind::Function,
        
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
        "go" => extract_imports_go(node, source),
        "java" => extract_imports_java(node, source),
        "c_sharp" => extract_imports_csharp(node, source),
        "c" | "cpp" => extract_imports_c_cpp(node, source),
        "dart" => extract_imports_dart(node, source),
        "swift" => extract_imports_swift(node, source),
        "kotlin" => extract_imports_kotlin(node, source),
        "lua" => extract_imports_lua(node, source),
        "zig" => extract_imports_zig(node, source),
        "elixir" => extract_imports_elixir(node, source),
        "haskell" => extract_imports_haskell(node, source),
        "ocaml" => extract_imports_ocaml(node, source),
        "prisma" => extract_imports_prisma(node, source),
        "graphql" => extract_imports_graphql(node, source),
        "sql" => extract_imports_sql(node, source),
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

fn extract_imports_go(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    collect_nodes_by_type(node, source, &["import_declaration"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        for target in go_import_targets(&text) {
            imports.push(Import {
                from: "self".to_string(),
                to: target,
                line: n.range().start_point.row + 1,
            });
        }
    })?;
    Ok(imports)
}

fn extract_imports_java(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    collect_nodes_by_type(node, source, &["import_declaration"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        for target in java_import_targets(&text) {
            imports.push(Import {
                from: "self".to_string(),
                to: target,
                line: n.range().start_point.row + 1,
            });
        }
    })?;
    Ok(imports)
}

fn extract_imports_csharp(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    collect_nodes_by_type(node, source, &["using_directive"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        if let Some(target) = csharp_using_target(&text) {
            imports.push(Import {
                from: "self".to_string(),
                to: target,
                line: n.range().start_point.row + 1,
            });
        }
    })?;
    Ok(imports)
}

/// `import ( "a" "b" )` or `import "a"` -> ["a", "b"]
fn go_import_targets(text: &str) -> Vec<String> {
    let t = text.trim().strip_prefix("import").unwrap_or(text).trim();
    let t = t.trim().trim_matches(|c: char| c == '(' || c == ')').trim();
    t.split_whitespace()
        .filter_map(|s| s.trim_matches('"').to_string().into())
        .filter(|s: &String| !s.is_empty())
        .collect()
}

/// `import java.util.*;` or `import java.util.List;` -> ["java.util.*", "java.util.List"]
fn java_import_targets(text: &str) -> Vec<String> {
    let t = text.trim().strip_prefix("import").unwrap_or(text).trim().trim_end_matches(';').trim();
    if t.starts_with("static ") {
        return vec![];
    }
    vec![t.to_string()]
}

/// `using System;` or `using System.Collections.Generic;` -> ["System", "System.Collections.Generic"]
fn csharp_using_target(text: &str) -> Option<String> {
    let t = text.trim().strip_prefix("using").unwrap_or(text).trim().trim_end_matches(';').trim();
    if t.starts_with("static ") {
        return None;
    }
    (!t.is_empty()).then_some(t.to_string())
}

// ===== Новые языки: импорты =====

fn extract_imports_c_cpp(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    collect_nodes_by_type(node, source, &["preproc_include"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        // #include <stdio.h> or #include "local.h"
        if let Some(path) = text.split(&['<', '"'][..]).nth(1).and_then(|s| s.split(&['>', '"'][..]).next()) {
            if !path.is_empty() {
                imports.push(Import {
                    from: "self".to_string(),
                    to: path.to_string(),
                    line: n.range().start_point.row + 1,
                });
            }
        }
    })?;
    Ok(imports)
}

fn extract_imports_dart(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    // Dart: imports are `import_or_export` -> `library_import` -> `import_specification` -> `configurable_uri` -> `uri` -> `string_literal`
    collect_nodes_by_type(node, source, &["import_or_export"], &mut |n, src| {
        // Find the string literal inside the import
        fn find_string_literal(node: &tree_sitter::Node, src: &[u8]) -> Option<String> {
            if node.kind() == "string_literal" {
                let text = node_text(node, src).unwrap_or_default();
                // Remove quotes
                return Some(text.trim_matches('\'').trim_matches('"').to_string());
            }
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if let Some(found) = find_string_literal(&child, src) {
                        return Some(found);
                    }
                }
            }
            None
        }
        
        if let Some(path) = find_string_literal(n, src) {
            if !path.is_empty() {
                imports.push(Import {
                    from: "self".to_string(),
                    to: path,
                    line: n.range().start_point.row + 1,
                });
            }
        }
    })?;
    Ok(imports)
}

fn extract_imports_swift(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    collect_nodes_by_type(node, source, &["import_declaration"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        // import Foundation
        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.len() >= 2 {
            imports.push(Import {
                from: "self".to_string(),
                to: parts[1].to_string(),
                line: n.range().start_point.row + 1,
            });
        }
    })?;
    Ok(imports)
}

fn extract_imports_kotlin(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    collect_nodes_by_type(node, source, &["import_header"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        // import com.example.Foo
        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.len() >= 2 {
            imports.push(Import {
                from: "self".to_string(),
                to: parts[1].to_string(),
                line: n.range().start_point.row + 1,
            });
        }
    })?;
    Ok(imports)
}

fn extract_imports_lua(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    collect_nodes_by_type(node, source, &["require_call"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        // require "module" or require("module")
        if let Some(path) = text.split(&['"', '(', ')'][..]).find(|s| !s.trim().is_empty() && !s.contains("require")) {
            imports.push(Import {
                from: "self".to_string(),
                to: path.trim().to_string(),
                line: n.range().start_point.row + 1,
            });
        }
    })?;
    Ok(imports)
}

fn extract_imports_zig(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    collect_nodes_by_type(node, source, &["import_declaration"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        // @import("std") or @import("root").xxx
        if let Some(path) = text.split(&['"', '(', ')'][..]).nth(1) {
            if !path.is_empty() {
                imports.push(Import {
                    from: "self".to_string(),
                    to: path.to_string(),
                    line: n.range().start_point.row + 1,
                });
            }
        }
    })?;
    Ok(imports)
}

fn extract_imports_elixir(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    // Elixir: import/alias/require are `call` nodes with identifier as first child
    collect_nodes_by_type(node, source, &["call"], &mut |n, src| {
        // Check if this is an import/alias/require call
        if let Some(id_node) = n.child(0) {
            if id_node.kind() == "identifier" {
                let keyword = node_text(&id_node, src).unwrap_or_default();
                if matches!(keyword.as_str(), "import" | "alias" | "require") {
                    // Get the module name from arguments
                    if let Some(args_node) = n.child(1) {
                        if args_node.kind() == "arguments" {
                            // Get the first child of arguments which should be the module name
                            if let Some(module_node) = args_node.child(0) {
                                let module = node_text(&module_node, src).unwrap_or_default();
                                if !module.is_empty() {
                                    imports.push(Import {
                                        from: "self".to_string(),
                                        to: module,
                                        line: n.range().start_point.row + 1,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    })?;
    Ok(imports)
}

fn extract_imports_haskell(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    // Haskell: imports are `import` nodes inside `imports` node
    collect_nodes_by_type(node, source, &["import"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        // import Data.List
        // import qualified Data.Map as Map
        // import Data.Maybe (fromMaybe)
        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.len() >= 2 {
            // Skip "import" and "qualified" if present
            let module_idx = if parts[1] == "qualified" { 2 } else { 1 };
            if module_idx < parts.len() {
                let module = parts[module_idx].trim_end_matches('.').trim_end_matches('(');
                if !module.is_empty() {
                    imports.push(Import {
                        from: "self".to_string(),
                        to: module.to_string(),
                        line: n.range().start_point.row + 1,
                    });
                }
            }
        }
    })?;
    Ok(imports)
}

fn extract_imports_ocaml(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    // OCaml: imports are `open_module` nodes (open Base, open Core)
    // open_module children: open (keyword), module_path (field "module")
    // module_path child: module_name
    collect_nodes_by_type(node, source, &["open_module"], &mut |n, src| {
        // Try field name first
        let module_path = n.child_by_field_name("module");
        // Fallback: second child (index 1) is module_path
        let module_path = module_path.or_else(|| n.child(1));
        if let Some(module_path) = module_path {
            // module_name is first child of module_path
            if let Some(module_name) = module_path.child(0) {
                let module = node_text(&module_name, src).unwrap_or_default();
                if !module.is_empty() {
                    imports.push(Import {
                        from: "self".to_string(),
                        to: module,
                        line: n.range().start_point.row + 1,
                    });
                }
            }
        }
    })?;
    Ok(imports)
}

fn extract_imports_prisma(_node: &tree_sitter::Node, _source: &[u8]) -> Result<Vec<Import>> {
    let imports = Vec::new();
    // Prisma uses generator blocks and datasource, not traditional imports
    // Could parse generator and datasource blocks
    Ok(imports)
}

fn extract_imports_graphql(_node: &tree_sitter::Node, _source: &[u8]) -> Result<Vec<Import>> {
    let imports = Vec::new();
    // GraphQL doesn't have traditional imports, but has schema extensions
    Ok(imports)
}

fn extract_imports_sql(_node: &tree_sitter::Node, _source: &[u8]) -> Result<Vec<Import>> {
    let imports = Vec::new();
    // SQL doesn't have imports in the traditional sense
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
        .find(|(_, c)| *c == '\'' || *c == '"')?;
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
        "go" => extract_calls_go(node, source),
        "java" => extract_calls_java(node, source),
        "c_sharp" => extract_calls_csharp(node, source),
        "c" | "cpp" => extract_calls_c_cpp(node, source),
        "dart" => extract_calls_dart(node, source),
        "swift" => extract_calls_swift(node, source),
        "kotlin" => extract_calls_kotlin(node, source),
        "lua" => extract_calls_lua(node, source),
        "zig" => extract_calls_zig(node, source),
        "elixir" => extract_calls_elixir(node, source),
        "haskell" => extract_calls_haskell(node, source),
        "ocaml" => extract_calls_ocaml(node, source),
        "prisma" => extract_calls_prisma(node, source),
        "graphql" => extract_calls_graphql(node, source),
        "sql" => extract_calls_sql(node, source),
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

fn extract_calls_go(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    collect_nodes_by_type(node, source, &["call_expression"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_go_call_callee(&text);
        calls.push(Call {
            caller: "unknown".to_string(),
            callee,
            line: n.range().start_point.row + 1,
        });
    })?;
    Ok(calls)
}

fn extract_calls_java(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    collect_nodes_by_type(node, source, &["method_invocation"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_java_call_callee(&text);
        calls.push(Call {
            caller: "unknown".to_string(),
            callee,
            line: n.range().start_point.row + 1,
        });
    })?;
    Ok(calls)
}

fn extract_calls_csharp(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    collect_nodes_by_type(node, source, &["invocation_expression"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_csharp_call_callee(&text);
        calls.push(Call {
            caller: "unknown".to_string(),
            callee,
            line: n.range().start_point.row + 1,
        });
    })?;
    Ok(calls)
}

/// Go: `pkg.Func(args)` -> `pkg.Func`
fn extract_go_call_callee(text: &str) -> String {
    last_segment(callee_head(text), &["."]).to_string()
}

/// Java: `obj.method(args)` -> `method`
fn extract_java_call_callee(text: &str) -> String {
    last_segment(callee_head(text), &["."]).to_string()
}

/// C#: `obj.Method(args)` -> `Method`
fn extract_csharp_call_callee(text: &str) -> String {
    last_segment(callee_head(text), &["."]).to_string()
}

// ===== Новые языки: вызовы =====

fn extract_calls_c_cpp(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    collect_nodes_by_type(node, source, &["call_expression"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_c_cpp_call_callee(&text);
        calls.push(Call {
            caller: "unknown".to_string(),
            callee,
            line: n.range().start_point.row + 1,
        });
    })?;
    Ok(calls)
}

fn extract_calls_dart(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    // Dart uses `call_expression` for both function calls and method calls
    // member_expression is used for method access (obj.method)
    collect_nodes_by_type(node, source, &["call_expression"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_dart_call_callee(&text);
        if !callee.is_empty() {
            calls.push(Call {
                caller: "unknown".to_string(),
                callee,
                line: n.range().start_point.row + 1,
            });
        }
    })?;
    Ok(calls)
}

fn extract_calls_swift(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    collect_nodes_by_type(node, source, &["call_expression", "function_call_expression"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_swift_call_callee(&text);
        calls.push(Call {
            caller: "unknown".to_string(),
            callee,
            line: n.range().start_point.row + 1,
        });
    })?;
    Ok(calls)
}

fn extract_calls_kotlin(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    collect_nodes_by_type(node, source, &["call_expression"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_kotlin_call_callee(&text);
        calls.push(Call {
            caller: "unknown".to_string(),
            callee,
            line: n.range().start_point.row + 1,
        });
    })?;
    Ok(calls)
}

fn extract_calls_lua(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    collect_nodes_by_type(node, source, &["function_call"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_lua_call_callee(&text);
        calls.push(Call {
            caller: "unknown".to_string(),
            callee,
            line: n.range().start_point.row + 1,
        });
    })?;
    Ok(calls)
}

fn extract_calls_zig(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    collect_nodes_by_type(node, source, &["call_expression"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_zig_call_callee(&text);
        calls.push(Call {
            caller: "unknown".to_string(),
            callee,
            line: n.range().start_point.row + 1,
        });
    })?;
    Ok(calls)
}

fn extract_calls_elixir(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    // Elixir: calls are `call` nodes. Filter out import/alias/require.
    // Method calls have a `dot` child, bare calls don't.
    collect_nodes_by_type(node, source, &["call"], &mut |n, src| {
        // Skip import/alias/require calls (they have identifier as first child)
        if let Some(first_child) = n.child(0) {
            if first_child.kind() == "identifier" {
                let keyword = node_text(&first_child, src).unwrap_or_default();
                if matches!(keyword.as_str(), "import" | "alias" | "require" | "def" | "defmodule" | "defp") {
                    return;
                }
            }
        }
        // Check if this is a method call (has dot child) or bare function call
        let has_dot = (0..n.child_count()).any(|i| n.child(i).map(|c| c.kind() == "dot").unwrap_or(false));
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_elixir_call_callee(&text);
        if !callee.is_empty() && (has_dot || text.contains('(')) {
            calls.push(Call {
                caller: "unknown".to_string(),
                callee,
                line: n.range().start_point.row + 1,
            });
        }
    })?;
    Ok(calls)
}

fn extract_calls_haskell(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    // Haskell: calls are `apply`, `infix`, `application` nodes
    // `apply` - function application: func arg
    // `infix` - infix operator: a `op` b or func $ arg
    // `application` - general application
    collect_nodes_by_type(node, source, &["apply", "infix"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_haskell_call_callee(&text);
        if !callee.is_empty() {
            calls.push(Call {
                caller: "unknown".to_string(),
                callee,
                line: n.range().start_point.row + 1,
            });
        }
    })?;
    Ok(calls)
}

fn extract_calls_ocaml(node: &tree_sitter::Node, source: &[u8]) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    // OCaml: calls are `application_expression` nodes
    collect_nodes_by_type(node, source, &["application_expression"], &mut |n, src| {
        let text = node_text(n, src).unwrap_or_default();
        let callee = extract_ocaml_call_callee(&text);
        if !callee.is_empty() {
            calls.push(Call {
                caller: "unknown".to_string(),
                callee,
                line: n.range().start_point.row + 1,
            });
        }
    })?;
    Ok(calls)
}

fn extract_calls_prisma(_node: &tree_sitter::Node, _source: &[u8]) -> Result<Vec<Call>> {
    // Prisma doesn't have traditional function calls
    Ok(vec![])
}

fn extract_calls_graphql(_node: &tree_sitter::Node, _source: &[u8]) -> Result<Vec<Call>> {
    // GraphQL doesn't have traditional function calls
    Ok(vec![])
}

fn extract_calls_sql(_node: &tree_sitter::Node, _source: &[u8]) -> Result<Vec<Call>> {
    // SQL doesn't have traditional function calls
    Ok(vec![])
}

/// C: `func(args)` -> `func`; C++: `obj.method(args)` -> `method`
fn extract_c_cpp_call_callee(text: &str) -> String {
    last_segment(callee_head(text), &["::", ".", "->"]).to_string()
}

/// Dart: `obj.method(args)` -> `method`
fn extract_dart_call_callee(text: &str) -> String {
    last_segment(callee_head(text), &["."]).to_string()
}

/// Swift: `obj.method(args)` -> `method`
fn extract_swift_call_callee(text: &str) -> String {
    last_segment(callee_head(text), &["."]).to_string()
}

/// Kotlin: `obj.method(args)` -> `method`
fn extract_kotlin_call_callee(text: &str) -> String {
    last_segment(callee_head(text), &["."]).to_string()
}

/// Lua: `obj:method(args)` or `func(args)` -> `method` or `func`
fn extract_lua_call_callee(text: &str) -> String {
    last_segment(callee_head(text), &[":", "."]).to_string()
}

/// Zig: `obj.method(args)` -> `method`
fn extract_zig_call_callee(text: &str) -> String {
    last_segment(callee_head(text), &["."]).to_string()
}

/// Elixir: `Module.function(args)` -> `function`
fn extract_elixir_call_callee(text: &str) -> String {
    last_segment(callee_head(text), &["."]).to_string()
}

/// Haskell: `func args` -> `func`
fn extract_haskell_call_callee(text: &str) -> String {
    let head = callee_head(text);
    head.split_whitespace().next().unwrap_or(head).to_string()
}

/// OCaml: `Module.func args` -> `func`
fn extract_ocaml_call_callee(text: &str) -> String {
    last_segment(callee_head(text), &["."]).to_string()
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
        "go" => {
            // Go uses interfaces implicitly; no explicit extends in AST
            // Could add struct embedding detection here if needed
        }
        "java" => {
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
                    for (keyword, rest) in [("extends", text.as_str()), ("implements", text.as_str())] {
                        let Some(pos) = rest.find(keyword) else {
                            continue;
                        };
                        let after = &rest[pos + keyword.len()..];
                        // Stop at next keyword or brace
                        let after = after.split(&['{', 'e', 'i'][..]).next().unwrap_or(after);
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
        "c_sharp" => {
            collect_nodes_by_type(
                node,
                source,
                &["class_declaration", "interface_declaration", "struct_declaration"],
                &mut |n, src| {
                    let Some(name_node) = n.child_by_field_name("name") else {
                        return;
                    };
                    let child = last_type_segment(&node_text(&name_node, src).unwrap_or_default());
                    let text = node_text(n, src).unwrap_or_default();
                    // C# uses : for inheritance
                    if let Some(colon_pos) = text.find(':') {
                        let after = &text[colon_pos + 1..];
                        let after = after.split('{').next().unwrap_or(after);
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
        "c" | "cpp" => {
            collect_nodes_by_type(
                node,
                source,
                &["class_specifier", "struct_specifier"],
                &mut |n, src| {
                    let text = node_text(n, src).unwrap_or_default();
                    if let Some(colon_pos) = text.find(':') {
                        let after = &text[colon_pos + 1..];
                        let after = after.split('{').next().unwrap_or(after);
                        let child = text.split_whitespace().nth(1).unwrap_or("").split(':').next().unwrap_or("");
                        let child = last_type_segment(child);
                        for part in after.split(',') {
                            let parent = last_type_segment(part);
                            if !parent.is_empty() && !child.is_empty() {
                                out.push(Heritage {
                                    child: child.to_string(),
                                    parent,
                                    line: n.range().start_point.row + 1,
                                });
                            }
                        }
                    }
                },
            )?;
        }
        "dart" => {
            collect_nodes_by_type(node, source, &["class_declaration", "mixin_declaration"], &mut |n, src| {
                let text = node_text(n, src).unwrap_or_default();
                let child = text.split_whitespace().nth(1).unwrap_or("");
                let child = last_type_segment(child);
                let text_clone = text.clone();
                for (keyword, rest) in [("extends", text), ("implements", text_clone.clone()), ("with", text_clone)] {
                    let Some(pos) = rest.find(keyword) else { continue; };
                    let after = &rest[pos + keyword.len()..];
                    let after = after.split(&['{', 'e', 'i', 'w'][..]).next().unwrap_or(after);
                    for part in after.split(',') {
                        let parent = last_type_segment(part);
                        if !parent.is_empty() && !child.is_empty() {
                            out.push(Heritage { child: child.to_string(), parent, line: n.range().start_point.row + 1 });
                        }
                    }
                }
            })?;
        }
        "swift" => {
            collect_nodes_by_type(node, source, &["class_declaration", "struct_declaration", "enum_declaration", "protocol_declaration"], &mut |n, src| {
                let text = node_text(n, src).unwrap_or_default();
                if let Some(colon_pos) = text.find(':') {
                    let after = &text[colon_pos + 1..];
                    let after = after.split('{').next().unwrap_or(after);
                    let child = text.split_whitespace().nth(1).unwrap_or("");
                    let child = last_type_segment(child);
                    for part in after.split(',') {
                        let parent = last_type_segment(part);
                        if !parent.is_empty() && !child.is_empty() {
                            out.push(Heritage { child: child.to_string(), parent, line: n.range().start_point.row + 1 });
                        }
                    }
                }
            })?;
        }
        "kotlin" => {
            collect_nodes_by_type(node, source, &["class_declaration", "interface_declaration"], &mut |n, src| {
                let text = node_text(n, src).unwrap_or_default();
                if let Some(colon_pos) = text.find(':') {
                    let after = &text[colon_pos + 1..];
                    let after = after.split('{').next().unwrap_or(after);
                    let child = text.split_whitespace().nth(1).unwrap_or("");
                    let child = last_type_segment(child);
                    for part in after.split(',') {
                        let parent = last_type_segment(part);
                        if !parent.is_empty() && !child.is_empty() {
                            out.push(Heritage { child: child.to_string(), parent, line: n.range().start_point.row + 1 });
                        }
                    }
                }
            })?;
        }
        "zig" => {
            // Zig uses comptime and @TypeOf for polymorphism, no traditional inheritance
        }
        "elixir" => {
            // Elixir uses behaviours and protocols, no traditional inheritance
        }
        "haskell" => {
            // Haskell uses typeclasses, not traditional inheritance
        }
        "ocaml" => {
            // OCaml uses modules and module types, no traditional inheritance
        }
        "prisma" => {
            collect_nodes_by_type(node, source, &["model_declaration"], &mut |n, src| {
                let text = node_text(n, src).unwrap_or_default();
                if let Some(extends_pos) = text.find("extends") {
                    let after = &text[extends_pos + 7..];
                    let child = text.split_whitespace().nth(1).unwrap_or("");
                    let child = last_type_segment(child);
                    for part in after.split_whitespace() {
                        let parent = last_type_segment(part);
                        if !parent.is_empty() && !child.is_empty() {
                            out.push(Heritage { child: child.to_string(), parent, line: n.range().start_point.row + 1 });
                        }
                    }
                }
            })?;
        }
        "graphql" => {
            collect_nodes_by_type(node, source, &["object_type_definition", "interface_type_definition"], &mut |n, src| {
                let text = node_text(n, src).unwrap_or_default();
                if let Some(impl_pos) = text.find("implements") {
                    let after = &text[impl_pos + 10..];
                    let after = after.split('{').next().unwrap_or(after);
                    let child = text.split_whitespace().nth(1).unwrap_or("");
                    let child = last_type_segment(child);
                    for part in after.split('&') {
                        let parent = last_type_segment(part);
                        if !parent.is_empty() && !child.is_empty() {
                            out.push(Heritage { child: child.to_string(), parent, line: n.range().start_point.row + 1 });
                        }
                    }
                }
            })?;
        }
        "sql" => {
            // SQL doesn't have traditional inheritance
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
        .rsplit(&['.', ':'][..])
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