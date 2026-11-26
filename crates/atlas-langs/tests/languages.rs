//! Для каждого заявленного языка: какие символы находятся в небольшом примере.

use atlas_langs::LanguageParser;

fn names(path: &str, code: &str) -> Vec<String> {
    let parsed = LanguageParser::new().parse(path, code).unwrap();
    parsed.symbols.into_iter().map(|s| s.name).collect()
}

fn check(path: &str, code: &str, expected: &[&str]) {
    assert_eq!(names(path, code), expected, "{path}");
}

fn contains(path: &str, code: &str, expected: &[&str]) {
    let found = names(path, code);
    for name in expected {
        assert!(found.iter().any(|f| f == name), "{path}: нет {name} в {found:?}");
    }
}

#[test]
fn c_and_cpp() {
    check(
        "a.c",
        "struct point { int x; };\nstatic int *add(int a) {\n  struct point p;\n  return 0;\n}\ntypedef struct { int a; } pair_t;\nint proto(void);\n",
        &["point", "add", "pair_t", "proto"],
    );
    check(
        "a.cpp",
        "namespace n {\nclass Box : public Base {\n public:\n  void open() { run(); }\n  void shut();\n  int& at(int i);\n};\n}\nvoid Box::shut() {}\ntemplate <class T> T id(T v) { return v; }\n",
        &["n", "Box", "open", "shut", "at", "shut", "id"],
    );
}

#[test]
fn dart_and_kotlin() {
    check(
        "a.dart",
        "class Box {\n  Box();\n  void open() { run(); }\n  int get size => 1;\n}\nint add(int a) => a;\nenum E { a }\nmixin M {}\n",
        &["Box", "Box", "open", "size", "add", "E", "M"],
    );
    check(
        "a.kt",
        "class Box {\n  fun open(): Int { val local = 1; return local }\n  val size = 1\n}\nfun add(a: Int) = a\nobject O\ninterface I { fun f() }\n",
        &["Box", "open", "size", "add", "O", "I", "f"],
    );
}

#[test]
fn elixir_haskell_ocaml() {
    check(
        "a.ex",
        "defmodule A.Box do\n  alias A.B\n  def open(x) when x > 0 do\n    run(x)\n  end\n  defp hid, do: 1\n  defmacro m(a), do: a\n  def noargs do\n    1\n  end\nend\n",
        &["A.Box", "open", "hid", "m", "noargs"],
    );
    check(
        "a.hs",
        "module A where\nimport Data.List\ndata Box = Box Int\nnewtype W = W Int\nclass C a where\n  m :: a -> a\nadd :: Int -> Int\nadd 0 = 0\nadd x = x\nconstant = 1\n",
        &["Box", "W", "C", "m", "add", "constant"],
    );
    check(
        "a.ml",
        "open List\ntype box = { x : int }\nlet add a b = let c = a in c + b\nlet v = 1\nmodule M = struct let f y = y end\nlet () = ()\n",
        &["box", "add", "v", "M", "f"],
    );
    check("a.mli", "type t\nval make : int -> t\n", &["t", "make"]);
}

#[test]
fn graphql_and_sql() {
    check(
        "a.graphql",
        "type Query {\n  box(id: ID): Int\n}\ninterface Node { id: ID }\nenum E { A }\ninput I { a: Int }\nscalar Date\nunion U = A | B\nquery GetBox { box }\nfragment F on Query { box }\n",
        &["Query", "box", "Node", "id", "E", "I", "Date", "U", "GetBox", "F"],
    );
    check(
        "a.sql",
        "CREATE TABLE s.users (id INT);\nCREATE OR REPLACE VIEW v AS SELECT * FROM users;\nCREATE FUNCTION f() RETURNS INT AS $$ 1 $$;\nCREATE PROCEDURE p() RETURNS INT LANGUAGE SQL AS $$ select 1 $$;\n",
        &["s.users", "v", "f", "p"],
    );
}

#[test]
fn jsx_files() {
    for path in ["a.tsx", "a.jsx", "a.js"] {
        contains(path, "function Hello() { return <div/> }\n", &["Hello"]);
    }
}

#[test]
fn other_languages() {
    contains("a.rs", "pub fn hello() {}\n", &["hello"]);
    contains("a.py", "def hello():\n    pass\n", &["hello"]);
    contains("a.ts", "export function hello() {}\n", &["hello"]);
    contains("a.go", "package a\nfunc Hello() {}\nvar Ответ = 1\n", &["Hello"]);
    contains("A.java", "class Hello { void run() {} }\n", &["Hello", "run"]);
    contains("a.cs", "class Hello { void Run() {} }\n", &["Hello", "Run"]);
    contains("a.swift", "func hello() {}\n", &["hello"]);
    contains("a.lua", "function hello() end\n", &["hello"]);
    contains("a.zig", "pub fn hello() void {}\n", &["hello"]);
    contains("a.prisma", "model Hello {\n  id Int @id\n}\n", &["Hello"]);
}
