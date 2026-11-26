use tree_sitter::{Parser, Language};

fn print_tree(node: &tree_sitter::Node, source: &str, depth: usize) {
    let indent = "  ".repeat(depth);
    let text = if node.end_byte() - node.start_byte() < 100 {
        source[node.start_byte()..node.end_byte()].replace('\n', "\\n")
    } else {
        format!("(length: {})", node.end_byte() - node.start_byte())
    };
    println!("{}{} '{}'", indent, node.kind(), text);
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            println!("{}  child {}: kind={}, field={:?}", indent, i, child.kind(), node.field_name_for_child(i as u32));
            print_tree(&child, source, depth + 1);
        }
    }
}

fn test_language(name: &str, lang: Language, code: &str) {
    println!("\n=== {} ===", name);
    let mut parser = Parser::new();
    parser.set_language(&lang).unwrap();
    let tree = parser.parse(code, None).unwrap();
    print_tree(&tree.root_node(), code, 0);
}

#[test]
fn test_ocaml_calls_and_include() {
    let ocaml_code = r#"
include Base
open Core

module MyModule = struct
  let func x = x + 1
end

let () = 
  MyModule.func 5
  helper x 2
"#;
    test_language("OCaml", tree_sitter_ocaml::LANGUAGE_OCAML.into(), ocaml_code);
}

#[test]
fn test_elixir_pipe_and_alias() {
    let elixir_code = r#"
defmodule MyModule do
  import OtherModule
  alias My.{C, D}
  require Logger

  defmacro m(a), do: a
  defp hid(x), do: x
  def run(x) do
    x |> pipe
    OtherModule.func()
    SomeModule.other()
    Logger.info("test")
  end
end
"#;
    test_language("Elixir", tree_sitter_elixir::LANGUAGE.into(), elixir_code);
}

#[test]
fn test_haskell_infix_and_dup() {
    let haskell_code = r#"
module Main where
import Data.List

main :: IO ()
main = do
  let result = a + b * 2
  print $ Map.lookup 1 Map.empty
  print $ fromMaybe "default" Nothing
  helper 1 2 3
"#;
    test_language("Haskell", tree_sitter_haskell::LANGUAGE.into(), haskell_code);
}

#[test]
fn test_lua_require() {
    let lua_code = r#"
local mod = require("mymodule")
require "other"
local x = mod.func()
"#;
    test_language("Lua", tree_sitter_lua::LANGUAGE.into(), lua_code);
}

#[test]
fn test_zig_import() {
    let zig_code = r#"
const std = @import("std");
const other = @import("root").Other;
fn main() void {
    std.debug.print("Hello\n", .{});
    helper();
}
"#;
    test_language("Zig", tree_sitter_zig::LANGUAGE.into(), zig_code);
}

#[test]
fn test_sql_calls() {
    let sql_code = r#"
SELECT count(*), sum(x) FROM users;
SELECT max(id) FROM orders;
"#;
    test_language("SQL", tree_sitter_snowflake::LANGUAGE.into(), sql_code);
}
