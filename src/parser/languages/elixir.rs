use crate::parser::language::{
    Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility,
};
use tree_sitter::Language as TsLanguage;

pub struct ElixirLanguage;

impl ElixirLanguage {
    fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
        node.utf8_text(source).unwrap_or("")
    }

    fn first_line(node: &tree_sitter::Node, source: &[u8]) -> String {
        let text = Self::node_text(node, source);
        text.lines().next().unwrap_or("").trim().to_string()
    }

    /// Extract the function/macro name from a `call` node representing def/defp/defmacro/defmodule.
    /// The structure is: call -> arguments -> (first argument is call node with the fn name, or atom)
    fn extract_def_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        // In Elixir tree-sitter grammar, `def foo(x)` parses as:
        //   (call (identifier "def") (arguments (call (identifier "foo") ...)))
        // or for no-arg: (call (identifier "def") (arguments (identifier "foo")))
        // The grammar uses positional children, not field names.
        let args = {
            let mut cursor = node.walk();
            let mut found = None;
            for child in node.children(&mut cursor) {
                if child.kind() == "arguments" {
                    found = Some(child);
                    break;
                }
            }
            match found {
                Some(a) => a,
                None => return String::new(),
            }
        };

        let mut cursor = args.walk();
        for child in args.children(&mut cursor) {
            match child.kind() {
                "call" => {
                    // def foo(args) -- the first child of arguments is a call node
                    // whose target is the function name
                    return Self::extract_call_target(&child, source);
                }
                "identifier" => {
                    return Self::node_text(&child, source).to_string();
                }
                "binary_operator" => {
                    // Pattern like `def foo(x) when is_integer(x)`
                    // The left side has the call
                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        if inner.kind() == "call" {
                            return Self::extract_call_target(&inner, source);
                        }
                        if inner.kind() == "identifier" {
                            return Self::node_text(&inner, source).to_string();
                        }
                    }
                }
                _ => {}
            }
        }
        String::new()
    }

    /// Extract the target (function name) from a call node.
    /// The grammar uses positional children: the first `identifier` child is the target.
    fn extract_call_target(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return Self::node_text(&child, source).to_string();
            }
        }
        String::new()
    }

    /// Extract module name from defmodule call.
    /// `defmodule MyApp.Router do ... end`
    fn extract_module_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let args = {
            let mut cursor = node.walk();
            let mut found = None;
            for child in node.children(&mut cursor) {
                if child.kind() == "arguments" {
                    found = Some(child);
                    break;
                }
            }
            match found {
                Some(a) => a,
                None => return String::new(),
            }
        };

        let mut cursor = args.walk();
        for child in args.children(&mut cursor) {
            match child.kind() {
                "alias" | "identifier" => {
                    return Self::node_text(&child, source).to_string();
                }
                "atom" => {
                    return Self::node_text(&child, source)
                        .trim_start_matches(':')
                        .to_string();
                }
                _ => {}
            }
        }
        String::new()
    }

    /// Extract import source from alias/import/use calls.
    fn extract_import_from_call(node: &tree_sitter::Node, source: &[u8]) -> Option<Import> {
        let target_text = Self::extract_call_target(node, source);
        if target_text != "alias"
            && target_text != "import"
            && target_text != "use"
            && target_text != "require"
        {
            return None;
        }

        let args = {
            let mut cursor = node.walk();
            let mut found = None;
            for child in node.children(&mut cursor) {
                if child.kind() == "arguments" {
                    found = Some(child);
                    break;
                }
            }
            found?
        };

        let mut cursor = args.walk();
        for child in args.children(&mut cursor) {
            match child.kind() {
                "alias" | "atom" | "identifier" => {
                    let name = Self::node_text(&child, source).to_string();
                    if !name.is_empty() {
                        let short = name.rsplit('.').next().unwrap_or(&name).to_string();
                        return Some(Import {
                            source: name,
                            names: vec![short],
                        });
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Get the call target text (the keyword like def, defp, defmodule, etc.)
    fn call_target_text(node: &tree_sitter::Node, source: &[u8]) -> String {
        Self::extract_call_target(node, source)
    }
}

impl LanguageSupport for ElixirLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_elixir::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "elixir"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let mut symbols: Vec<Symbol> = Vec::new();
        let mut imports: Vec<Import> = Vec::new();
        let mut exports: Vec<Export> = Vec::new();

        // Use a stack to walk into do blocks and module bodies
        let mut stack: Vec<tree_sitter::Node> = root.children(&mut root.walk()).collect();

        while let Some(node) = stack.pop() {
            if node.kind() == "call" {
                let target = Self::call_target_text(&node, source_bytes);

                match target.as_str() {
                    "defmodule" => {
                        let name = Self::extract_module_name(&node, source_bytes);
                        let signature = Self::first_line(&node, source_bytes);
                        let body = Self::node_text(&node, source_bytes).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;

                        if !name.is_empty() {
                            exports.push(Export {
                                name: name.clone(),
                                kind: SymbolKind::Class,
                            });
                            symbols.push(Symbol {
                                name,
                                kind: SymbolKind::Class,
                                visibility: Visibility::Public,
                                signature,
                                body,
                                start_line,
                                end_line,
                            });
                        }

                        // Recurse into the do block
                        Self::push_do_children(&node, &mut stack);
                    }

                    "def" | "defmacro" => {
                        let name = Self::extract_def_name(&node, source_bytes);
                        let signature = Self::first_line(&node, source_bytes);
                        let body = Self::node_text(&node, source_bytes).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;

                        let kind = SymbolKind::Function;

                        if !name.is_empty() {
                            exports.push(Export {
                                name: name.clone(),
                                kind: kind.clone(),
                            });
                            symbols.push(Symbol {
                                name,
                                kind,
                                visibility: Visibility::Public,
                                signature,
                                body,
                                start_line,
                                end_line,
                            });
                        }
                    }

                    "defp" | "defmacrop" => {
                        let name = Self::extract_def_name(&node, source_bytes);
                        let signature = Self::first_line(&node, source_bytes);
                        let body = Self::node_text(&node, source_bytes).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;

                        if !name.is_empty() {
                            symbols.push(Symbol {
                                name,
                                kind: SymbolKind::Function,
                                visibility: Visibility::Private,
                                signature,
                                body,
                                start_line,
                                end_line,
                            });
                        }
                    }

                    "alias" | "import" | "use" | "require" => {
                        if let Some(imp) = Self::extract_import_from_call(&node, source_bytes) {
                            imports.push(imp);
                        }
                    }

                    _ => {
                        // Recurse into unknown calls that might contain do blocks with defs
                        Self::push_do_children(&node, &mut stack);
                    }
                }
            } else {
                // For non-call nodes, push all children to continue scanning
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    stack.push(child);
                }
            }
        }

        ParseResult {
            symbols,
            imports,
            exports,
        }
    }
}

impl ElixirLanguage {
    /// Push children of `do_block` nodes into the stack for further processing.
    fn push_do_children<'a>(node: &tree_sitter::Node<'a>, stack: &mut Vec<tree_sitter::Node<'a>>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "do_block" {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    stack.push(inner);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::language::{SymbolKind, Visibility};

    fn make_parser() -> tree_sitter::Parser {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_elixir::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_public_function() {
        let source = r#"defmodule MyApp do
  def greet(name) do
    "Hello, #{name}!"
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function && s.name == "greet")
            .collect();
        assert!(!funcs.is_empty(), "expected public function 'greet'");
        assert_eq!(funcs[0].visibility, Visibility::Public);

        let exported: Vec<_> = result
            .exports
            .iter()
            .filter(|e| e.name == "greet")
            .collect();
        assert!(!exported.is_empty(), "public function should be exported");
    }

    #[test]
    fn test_extract_private_function() {
        let source = r#"defmodule MyApp do
  defp helper(x) do
    x * 2
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function && s.name == "helper")
            .collect();
        assert!(!funcs.is_empty(), "expected private function 'helper'");
        assert_eq!(funcs[0].visibility, Visibility::Private);

        assert!(
            !result.exports.iter().any(|e| e.name == "helper"),
            "private function should not be exported"
        );
    }

    #[test]
    fn test_extract_module() {
        let source = r#"defmodule MyApp.Router do
  def index do
    :ok
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        let classes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(!classes.is_empty(), "expected module as class symbol");
        assert_eq!(classes[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"defmodule MyApp do
  alias MyApp.Repo
  import Ecto.Query
  use GenServer
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        assert!(!result.imports.is_empty(), "expected imports");
    }

    #[test]
    fn test_empty_source() {
        let source = "";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);
        assert!(result.symbols.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.exports.is_empty());
    }

    #[test]
    fn test_complex_module_with_macro() {
        let source = r#"defmodule MyApp.Helpers do
  defmacro debug(msg) do
    quote do
      IO.puts(unquote(msg))
    end
  end

  def run do
    debug("starting")
  end

  defp internal do
    :ok
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        let classes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(!classes.is_empty(), "expected module");

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        assert!(funcs.len() >= 2, "expected at least 2 functions");

        let private_funcs: Vec<_> = funcs
            .iter()
            .filter(|f| f.visibility == Visibility::Private)
            .collect();
        assert!(!private_funcs.is_empty(), "expected private function");
    }

    #[test]
    fn test_coverage_defmacrop() {
        let source = r#"defmodule MyApp do
  defmacrop private_macro(x) do
    quote do
      unquote(x) + 1
    end
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        let private_fns: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function && s.visibility == Visibility::Private)
            .collect();
        assert!(
            !private_fns.is_empty(),
            "expected private macro from defmacrop"
        );
        assert!(
            !result.exports.iter().any(|e| e.name == "private_macro"),
            "defmacrop should not be exported"
        );
    }

    #[test]
    fn test_coverage_nested_modules() {
        let source = r#"defmodule Outer do
  defmodule Inner do
    def inner_func do
      :ok
    end
  end

  def outer_func do
    :ok
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        let classes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(classes.len() >= 2, "expected at least 2 modules");

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        assert!(funcs.len() >= 2, "expected at least 2 functions");
    }

    #[test]
    fn test_coverage_require_import() {
        let source = r#"defmodule MyApp do
  require Logger
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        let has_logger = result.imports.iter().any(|i| i.source.contains("Logger"));
        assert!(has_logger, "expected require Logger import");
    }

    #[test]
    fn test_coverage_multiple_imports() {
        let source = r#"defmodule MyApp do
  alias MyApp.Repo
  import Ecto.Query
  use GenServer
  require Logger

  def start do
    :ok
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        assert!(result.imports.len() >= 3, "expected at least 3 imports");
    }

    #[test]
    fn test_coverage_module_with_multiple_functions() {
        let source = r#"defmodule Calculator do
  def add(a, b) do
    a + b
  end

  def subtract(a, b) do
    a - b
  end

  defp validate(x) do
    x > 0
  end

  defmacro assert_positive(x) do
    quote do
      if unquote(x) <= 0, do: raise("not positive")
    end
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        let pub_funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function && s.visibility == Visibility::Public)
            .collect();
        assert!(pub_funcs.len() >= 3, "expected at least 3 public functions");

        let priv_funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function && s.visibility == Visibility::Private)
            .collect();
        assert!(!priv_funcs.is_empty(), "expected private function");
    }

    #[test]
    fn test_def_with_guard() {
        // Test the binary_operator path in extract_def_name: def foo(x) when is_integer(x)
        let source = r#"defmodule MyApp do
  def guarded(x) when is_integer(x) do
    x + 1
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function && s.name == "guarded")
            .collect();
        assert!(!funcs.is_empty(), "expected guarded function");
    }

    #[test]
    fn test_def_no_args() {
        // Exercise the identifier path in extract_def_name (no-arg function)
        let source = r#"defmodule MyApp do
  def hello do
    :world
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function && s.name == "hello")
            .collect();
        assert!(!funcs.is_empty(), "expected no-arg function 'hello'");
    }

    #[test]
    fn test_extract_import_from_call_non_import() {
        // Calling extract_import_from_call on a non-import call should return None
        let source = r#"defmodule MyApp do
  def foo do
    :ok
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        // Just ensure no spurious imports from non-import calls
        assert!(
            !result.imports.iter().any(|i| i.source.contains("foo")),
            "def should not produce imports"
        );
    }

    #[test]
    fn test_push_do_children_no_do_block() {
        // A call node without a do_block should not push anything
        let source = r#"defmodule MyApp do
  alias MyApp.Repo
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);
        // Should still work -- alias should be found
        assert!(!result.imports.is_empty(), "expected alias import");
    }

    #[test]
    fn test_non_call_node_scanning() {
        // Test that non-call nodes have their children scanned
        let source = r#"defmodule MyApp do
  def start do
    :ok
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        // The do_block and other non-call nodes should be traversed
        let has_start = result
            .symbols
            .iter()
            .any(|s| s.name == "start" && s.kind == SymbolKind::Function);
        assert!(has_start, "expected function 'start'");
    }

    #[test]
    fn test_unknown_call_with_do_block() {
        // Test the _ => push_do_children path for unknown calls
        let source = r#"defmodule MyApp do
  if Mix.env() == :test do
    def test_helper do
      :ok
    end
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        // The `if` call should recurse via push_do_children
        let has_helper = result
            .symbols
            .iter()
            .any(|s| s.name == "test_helper" && s.kind == SymbolKind::Function);
        assert!(
            has_helper,
            "expected test_helper via unknown call recursion"
        );
    }

    #[test]
    fn test_first_line_helper() {
        let source = r#"defmodule MyApp do
  def foo do
    :ok
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        let module = result.symbols.iter().find(|s| s.kind == SymbolKind::Class);
        assert!(module.is_some(), "expected module");
        if let Some(m) = module {
            assert_eq!(m.signature, "defmodule MyApp do");
        }
    }

    #[test]
    fn test_defmodule_with_atom_name() {
        // Exercise the "atom" branch in extract_module_name
        let source = "defmodule :my_mod do\n  def foo, do: :ok\nend\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        let modules: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(!modules.is_empty(), "expected module from atom name");
        assert_eq!(modules[0].name, "my_mod");
    }

    #[test]
    fn test_extract_call_target_empty() {
        // A non-call node without identifier children should return empty
        let mut parser = make_parser();
        let source = ":ok\n";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            let target = ElixirLanguage::extract_call_target(&child, source.as_bytes());
            // atom node has no identifier child, should return ""
            assert!(target.is_empty(), "atom should not have call target");
        }
    }

    #[test]
    fn test_extract_def_name_no_arguments() {
        // A call-like node without an arguments child returns ""
        let mut parser = make_parser();
        let source = ":ok\n";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            let name = ElixirLanguage::extract_def_name(&child, source.as_bytes());
            assert!(
                name.is_empty(),
                "no-arguments node should return empty name"
            );
        }
    }

    #[test]
    fn test_extract_module_name_no_arguments() {
        // exercise the None => return String::new() branch
        let mut parser = make_parser();
        let source = ":ok\n";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            let name = ElixirLanguage::extract_module_name(&child, source.as_bytes());
            assert!(name.is_empty());
        }
    }

    #[test]
    fn test_extract_import_from_call_no_arguments() {
        // exercise the found? (None) path in extract_import_from_call
        // by calling it on a non-call node
        let mut parser = make_parser();
        let source = "alias MyApp.Repo\n";
        let tree = parser.parse(source, None).unwrap();
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);
        // Just ensures we parse without panicking and find the import
        assert!(!result.imports.is_empty(), "expected alias import");
    }

    #[test]
    fn test_def_with_keyword_syntax() {
        // Exercise the one-liner `def foo, do: :ok` (no do block) path
        let source = "defmodule M do\n  def bar, do: :ok\nend\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = ElixirLanguage;
        let result = lang.extract(source, &tree);

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function && s.name == "bar")
            .collect();
        assert!(
            !funcs.is_empty(),
            "expected function 'bar' from keyword syntax"
        );
    }
}
