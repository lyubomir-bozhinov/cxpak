use crate::parser::language::{
    Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility,
};
use tree_sitter::Language as TsLanguage;

pub struct RubyLanguage;

impl RubyLanguage {
    fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
        node.utf8_text(source).unwrap_or("")
    }

    fn first_line(node: &tree_sitter::Node, source: &[u8]) -> String {
        let text = Self::node_text(node, source);
        text.lines().next().unwrap_or("").trim().to_string()
    }

    /// Extract the name from a node by finding an `identifier` child.
    fn extract_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "constant" {
                return Self::node_text(&child, source).to_string();
            }
        }
        String::new()
    }

    /// Ruby `require` / `require_relative` calls appear as `call` nodes.
    /// We look for `call` nodes whose `method` field text is "require" or "require_relative".
    fn extract_require(node: &tree_sitter::Node, source: &[u8]) -> Option<Import> {
        // Verify that the method name is require or require_relative
        let method_name = node
            .child_by_field_name("method")
            .map(|n| Self::node_text(&n, source))
            .unwrap_or("");

        if method_name != "require" && method_name != "require_relative" {
            return None;
        }

        // The arguments node contains the string literal
        let args = node.child_by_field_name("arguments")?;
        let mut cursor = args.walk();
        for child in args.children(&mut cursor) {
            let text = Self::node_text(&child, source);
            let trimmed = text
                .trim_matches('"')
                .trim_matches('\'')
                .trim_start_matches('(')
                .trim_end_matches(')')
                .trim_matches('"')
                .trim_matches('\'');
            if !trimmed.is_empty() && child.kind() != "," {
                let source_path = trimmed.to_string();
                let name = source_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&source_path)
                    .to_string();
                return Some(Import {
                    source: source_path,
                    names: vec![name],
                });
            }
        }
        None
    }

    fn extract_fn_body(node: &tree_sitter::Node, source: &[u8]) -> String {
        // Ruby method body is in a "body_statement" child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "body_statement" || child.kind() == "do_block" {
                let text = &source[child.start_byte()..child.end_byte()];
                return String::from_utf8_lossy(text).into_owned();
            }
        }
        String::new()
    }
}

impl LanguageSupport for RubyLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_ruby::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "ruby"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let mut symbols: Vec<Symbol> = Vec::new();
        let mut imports: Vec<Import> = Vec::new();
        let mut exports: Vec<Export> = Vec::new();

        // Walk top-level and all descendants via a stack. We recurse into
        // class/module bodies to find nested methods.
        let mut stack: Vec<tree_sitter::Node> = root.children(&mut root.walk()).collect();

        while let Some(node) = stack.pop() {
            match node.kind() {
                "method" => {
                    let name = Self::extract_name(&node, source_bytes);
                    // All methods discovered at top-level are public by default in Ruby.
                    // We do not track private/protected access modifiers in this simple pass.
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::extract_fn_body(&node, source_bytes);
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    exports.push(Export {
                        name: name.clone(),
                        kind: SymbolKind::Method,
                    });
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Method,
                        visibility: Visibility::Public,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "singleton_method" => {
                    // def self.method_name — always public
                    let name = Self::extract_name(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::extract_fn_body(&node, source_bytes);
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    exports.push(Export {
                        name: name.clone(),
                        kind: SymbolKind::Method,
                    });
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Method,
                        visibility: Visibility::Public,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "class" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

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

                    // Recurse into class body
                    if let Some(body_node) = node.child_by_field_name("body") {
                        let mut cursor = body_node.walk();
                        for child in body_node.children(&mut cursor) {
                            stack.push(child);
                        }
                    }
                }

                "module" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    exports.push(Export {
                        name: name.clone(),
                        kind: SymbolKind::Trait,
                    });
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Trait,
                        visibility: Visibility::Public,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });

                    // Recurse into module body
                    if let Some(body_node) = node.child_by_field_name("body") {
                        let mut cursor = body_node.walk();
                        for child in body_node.children(&mut cursor) {
                            stack.push(child);
                        }
                    }
                }

                "call" => {
                    if let Some(imp) = Self::extract_require(&node, source_bytes) {
                        imports.push(imp);
                    }
                }

                _ => {}
            }
        }

        ParseResult {
            symbols,
            imports,
            exports,
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
            .set_language(&tree_sitter_ruby::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_public_method() {
        let source = r#"def greet(name)
  puts "Hello, #{name}!"
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = RubyLanguage;
        let result = lang.extract(source, &tree);

        let methods: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Method)
            .collect();
        assert!(!methods.is_empty(), "expected method symbol");
        assert_eq!(methods[0].name, "greet");
        assert_eq!(methods[0].visibility, Visibility::Public);

        let exported: Vec<_> = result
            .exports
            .iter()
            .filter(|e| e.name == "greet")
            .collect();
        assert!(!exported.is_empty(), "greet should be exported");
    }

    #[test]
    fn test_extract_class() {
        let source = r#"class Animal
  def speak
    "..."
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = RubyLanguage;
        let result = lang.extract(source, &tree);

        let classes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(!classes.is_empty(), "expected class symbol");
        assert_eq!(classes[0].name, "Animal");
        assert_eq!(classes[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_extract_module() {
        let source = r#"module Greetable
  def greet
    "hello"
  end
end
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = RubyLanguage;
        let result = lang.extract(source, &tree);

        let modules: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Trait)
            .collect();
        assert!(!modules.is_empty(), "expected module symbol");
        assert_eq!(modules[0].name, "Greetable");
    }

    #[test]
    fn test_extract_require_import() {
        let source = r#"require 'json'
require_relative 'helper'
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = RubyLanguage;
        let result = lang.extract(source, &tree);

        assert!(
            !result.imports.is_empty(),
            "expected at least one import, got {:?}",
            result.imports
        );
        let sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(
            sources.contains(&"json") || sources.iter().any(|s| s.contains("json")),
            "expected json import, got: {:?}",
            sources
        );
    }
}
