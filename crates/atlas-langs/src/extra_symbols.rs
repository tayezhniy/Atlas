//! Символы языков, у грамматик которых имя объявления лежит не в поле `name`
//! прямого узла: C, C++, Dart, Kotlin, Elixir, Haskell, OCaml, GraphQL, SQL.

use crate::parser::{Symbol, SymbolKind, Visibility};
use crate::tree_sitter_parser::{extract_doc_comment, extract_signature, extract_visibility};
use tree_sitter::Node;

/// Что делать с узлом: объявление (если есть) и нужно ли идти внутрь.
struct Found {
    symbol: Option<(String, SymbolKind)>,
    descend: bool,
}

const NOTHING: Found = Found {
    symbol: None,
    descend: true,
};

fn found(name: Option<String>, kind: SymbolKind, descend: bool) -> Found {
    Found {
        symbol: name.map(|n| (n, kind)),
        descend,
    }
}

pub(crate) fn extract(root: &Node, source: &[u8], lang: &str) -> Vec<Symbol> {
    let mut out: Vec<Symbol> = Vec::new();
    // Обход без рекурсии: глубокие деревья не переполняют стек
    let mut stack = vec![*root];
    while let Some(node) = stack.pop() {
        let hit = match lang {
            "c" | "cpp" => c_family(&node, source),
            "dart" => dart(&node, source),
            "kotlin" => kotlin(&node, source),
            "elixir" => elixir(&node, source),
            "haskell" => haskell(&node, source),
            "ocaml" => ocaml(&node, source),
            "graphql" => graphql(&node, source),
            "sql" => sql(&node, source),
            _ => NOTHING,
        };
        if let Some((name, kind)) = hit.symbol {
            let line_start = node.start_position().row + 1;
            let line_end = node.end_position().row + 1;
            // Haskell: сигнатура и ветки одной функции — один символ
            match out.last_mut() {
                Some(prev)
                    if lang == "haskell"
                        && prev.name == name
                        && prev.line_end + 1 >= line_start =>
                {
                    prev.line_end = line_end;
                }
                _ => {
                    let visibility = match lang {
                        "dart" if name.starts_with('_') => Visibility::Private,
                        "dart" => Visibility::Public,
                        _ => extract_visibility(&node, source, lang),
                    };
                    out.push(Symbol {
                        name,
                        kind,
                        line_start,
                        line_end,
                        signature: extract_signature(&node, source),
                        doc: extract_doc_comment(&node, source),
                        visibility,
                    });
                }
            }
        }
        if hit.descend {
            for i in (0..node.named_child_count()).rev() {
                if let Some(child) = node.named_child(i) {
                    stack.push(child);
                }
            }
        }
    }
    out
}

fn text(node: Node, source: &[u8]) -> Option<String> {
    let t = node.utf8_text(source).ok()?.trim();
    (!t.is_empty()).then(|| t.to_string())
}

fn child_of_kind<'a>(node: &Node<'a>, kinds: &[&str]) -> Option<Node<'a>> {
    (0..node.named_child_count())
        .filter_map(|i| node.named_child(i))
        .find(|c| kinds.contains(&c.kind()))
}

fn child_text(node: &Node, source: &[u8], kinds: &[&str]) -> Option<String> {
    text(child_of_kind(node, kinds)?, source)
}

fn field_text(node: &Node, source: &[u8], field: &str) -> Option<String> {
    text(node.child_by_field_name(field)?, source)
}

fn parent_is(node: &Node, kind: &str) -> bool {
    node.parent().is_some_and(|p| p.kind() == kind)
}

/// Идёт по цепочке деклараторов C/C++ до имени. Возвращает имя,
/// признак «это функция» и признак «имя с квалификатором `A::`».
fn c_declarator(mut node: Node, source: &[u8]) -> Option<(String, bool, bool)> {
    let (mut function, mut qualified) = (false, false);
    loop {
        match node.kind() {
            "identifier" | "field_identifier" | "type_identifier" | "destructor_name"
            | "operator_name" => {
                return Some((text(node, source)?, function, qualified));
            }
            "qualified_identifier" => {
                qualified = true;
                node = node.child_by_field_name("name")?;
            }
            "template_function" => node = node.child_by_field_name("name")?,
            kind => {
                function |= kind == "function_declarator";
                // у reference_declarator и parenthesized_declarator поля нет
                node = node
                    .child_by_field_name("declarator")
                    .or_else(|| node.named_child(0))?;
            }
        }
    }
}

fn c_family(node: &Node, source: &[u8]) -> Found {
    let declarator = || c_declarator(node.child_by_field_name("declarator")?, source);
    let member = parent_is(node, "field_declaration_list");
    let callable = |qualified: bool| {
        if member || qualified {
            SymbolKind::Method
        } else {
            SymbolKind::Function
        }
    };
    match node.kind() {
        // внутрь тела не идём: локальные объявления не символы файла
        "function_definition" => match declarator() {
            Some((name, _, qualified)) => found(Some(name), callable(qualified), false),
            None => NOTHING,
        },
        "declaration" | "field_declaration" => match declarator() {
            Some((name, true, qualified)) => found(Some(name), callable(qualified), false),
            _ => NOTHING,
        },
        "type_definition" => found(declarator().map(|d| d.0), SymbolKind::Type, true),
        "class_specifier" | "struct_specifier" | "union_specifier" | "enum_specifier"
            if node.child_by_field_name("body").is_some() =>
        {
            let kind = match node.kind() {
                "class_specifier" | "struct_specifier" => SymbolKind::Class,
                _ => SymbolKind::Type,
            };
            found(field_text(node, source, "name"), kind, true)
        }
        "namespace_definition" => found(field_text(node, source, "name"), SymbolKind::Class, true),
        _ => NOTHING,
    }
}

/// Первое поле `name` внутри сигнатуры Dart.
fn dart_signature_name(node: Node, source: &[u8]) -> Option<String> {
    if node.kind().ends_with("_signature") {
        if let Some(name) = field_text(&node, source, "name") {
            return Some(name);
        }
    }
    (0..node.named_child_count())
        .filter_map(|i| node.named_child(i))
        .filter(|c| c.kind().ends_with("signature") || c.kind() == "declaration")
        .find_map(|c| dart_signature_name(c, source))
}

fn dart(node: &Node, source: &[u8]) -> Found {
    let member = parent_is(node, "member_declaration") || parent_is(node, "class_body");
    let callable = if member {
        SymbolKind::Method
    } else {
        SymbolKind::Function
    };
    match node.kind() {
        "function_declaration" | "method_declaration" | "declaration" => {
            found(dart_signature_name(*node, source), callable, false)
        }
        "class_declaration" | "mixin_declaration" | "extension_declaration" => {
            found(field_text(node, source, "name"), SymbolKind::Class, true)
        }
        "enum_declaration" => found(field_text(node, source, "name"), SymbolKind::Type, false),
        "type_alias" => found(
            child_text(node, source, &["type_identifier", "identifier"]),
            SymbolKind::Type,
            false,
        ),
        _ => NOTHING,
    }
}

fn kotlin(node: &Node, source: &[u8]) -> Found {
    match node.kind() {
        "function_declaration" => {
            let kind = if parent_is(node, "class_body") {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            };
            found(
                child_text(node, source, &["simple_identifier"]),
                kind,
                false,
            )
        }
        "class_declaration" | "object_declaration" => {
            let interface = (0..node.child_count())
                .any(|i| node.child(i).is_some_and(|c| c.kind() == "interface"));
            let kind = if interface {
                SymbolKind::Interface
            } else {
                SymbolKind::Class
            };
            found(child_text(node, source, &["type_identifier"]), kind, true)
        }
        "type_alias" => found(
            child_text(node, source, &["type_identifier"]),
            SymbolKind::Type,
            false,
        ),
        "property_declaration" => {
            let name = child_of_kind(node, &["variable_declaration"])
                .and_then(|v| child_text(&v, source, &["simple_identifier"]));
            found(name, SymbolKind::Constant, false)
        }
        _ => NOTHING,
    }
}

fn elixir(node: &Node, source: &[u8]) -> Found {
    if node.kind() != "call" {
        return NOTHING;
    }
    let (kind, descend) = match field_text(node, source, "target").as_deref() {
        Some("defmodule" | "defprotocol" | "defimpl") => (SymbolKind::Class, true),
        Some(
            "def" | "defp" | "defmacro" | "defmacrop" | "defguard" | "defguardp" | "defdelegate",
        ) => (SymbolKind::Function, false),
        _ => return NOTHING,
    };
    let mut head = child_of_kind(node, &["arguments"]).and_then(|a| a.named_child(0));
    // def f(x) when x > 0 → имя слева от when, в вызове `f(x)`
    if let Some(h) = head.filter(|h| h.kind() == "binary_operator") {
        head = h.child_by_field_name("left");
    }
    let name = head.and_then(|h| match h.kind() {
        "call" => field_text(&h, source, "target"),
        "identifier" | "alias" => text(h, source),
        _ => None,
    });
    found(name, kind, descend)
}

fn haskell(node: &Node, source: &[u8]) -> Found {
    let name = || field_text(node, source, "name");
    match node.kind() {
        // `signature` встречается и внутри выражений типов, берём только объявления
        "function" | "signature" if node.child_by_field_name("name").is_some() => {
            found(name(), SymbolKind::Function, false)
        }
        "bind" => found(name(), SymbolKind::Constant, false),
        "data_type" | "newtype" | "type_synonym" | "type_synomym" => {
            found(name(), SymbolKind::Type, false)
        }
        "class" if node.child_by_field_name("name").is_some() => {
            found(name(), SymbolKind::Interface, true)
        }
        _ => NOTHING,
    }
}

fn ocaml(node: &Node, source: &[u8]) -> Found {
    match node.kind() {
        "let_binding" => {
            let pattern = node.child_by_field_name("pattern");
            let name = pattern
                .filter(|p| matches!(p.kind(), "value_name" | "parenthesized_operator"))
                .and_then(|p| text(p, source));
            let function = child_of_kind(node, &["parameter"]).is_some()
                || node
                    .child_by_field_name("body")
                    .is_some_and(|b| matches!(b.kind(), "fun_expression" | "function_expression"));
            let kind = if function {
                SymbolKind::Function
            } else {
                SymbolKind::Constant
            };
            found(name, kind, false)
        }
        "value_specification" => found(
            child_text(node, source, &["value_name"]),
            SymbolKind::Function,
            false,
        ),
        "type_binding" => found(field_text(node, source, "name"), SymbolKind::Type, false),
        "module_binding" => found(
            child_text(node, source, &["module_name"]),
            SymbolKind::Class,
            true,
        ),
        "module_type_definition" => found(
            child_text(node, source, &["module_type_name"]),
            SymbolKind::Interface,
            true,
        ),
        "class_binding" => found(
            child_text(node, source, &["class_name"]),
            SymbolKind::Class,
            true,
        ),
        "method_definition" => found(
            child_text(node, source, &["method_name"]),
            SymbolKind::Method,
            false,
        ),
        _ => NOTHING,
    }
}

fn graphql(node: &Node, source: &[u8]) -> Found {
    let kind = match node.kind() {
        "object_type_definition" | "input_object_type_definition" => SymbolKind::Class,
        "interface_type_definition" => SymbolKind::Interface,
        "enum_type_definition" | "scalar_type_definition" | "union_type_definition" => {
            SymbolKind::Type
        }
        "field_definition" => SymbolKind::Field,
        "directive_definition" => SymbolKind::Constant,
        "operation_definition" | "fragment_definition" => SymbolKind::Function,
        _ => return NOTHING,
    };
    found(
        child_text(node, source, &["name", "fragment_name"]),
        kind,
        true,
    )
}

fn sql(node: &Node, source: &[u8]) -> Found {
    if !node.kind().starts_with("create_") || node.kind() == "create_query" {
        return NOTHING;
    }
    let kind = match node.kind() {
        "create_function" | "create_procedure" => SymbolKind::Function,
        "create_table" | "create_view" | "create_materialized_view" => SymbolKind::Class,
        _ => SymbolKind::Type,
    };
    let name = field_text(node, source, "name")
        .or_else(|| child_text(node, source, &["object_reference"]));
    found(name, kind, false)
}
