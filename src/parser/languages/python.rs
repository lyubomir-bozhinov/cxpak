use crate::parser::language::{Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility};
use tree_sitter::Language as TsLanguage;

pub struct PythonLanguage;

impl PythonLanguage {
    fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
        node.utf8_text(source).unwrap_or("")
    }

    fn first_line(node: &tree_sitter::Node, source: &[u8]) -> String {
        let text = Self::node_text(node, source);
        text.lines().next().unwrap_or("").trim().to_string()
    }

    fn extract_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return Self::node_text(&child, source).to_string();
            }
        }
        String::new()
    }

    /// Python visibility: names starting with `_` are private.
    fn is_public(name: &str) -> bool {
        !name.starts_with('_')
    }

    /// Extract the function signature (first line: `def name(...):`)
    fn extract_fn_signature(node: &tree_sitter::Node, source: &[u8]) -> String {
        Self::first_line(node, source)
    }

    /// Extract the body text (everything after the first line / colon).
    fn extract_fn_body(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block" {
                let text = &source[child.start_byte()..child.end_byte()];
                return String::from_utf8_lossy(text).into_owned();
            }
        }
        String::new()
    }

    fn extract_import(node: &tree_sitter::Node, source: &[u8]) -> Option<Import> {
        let text = Self::node_text(node, source);
        match node.kind() {
            "import_statement" => {
                // "import os" / "import os, sys"
                let inner = text.trim_start_matches("import").trim();
                let names: Vec<String> = inner
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                Some(Import {
                    source: String::new(),
                    names,
                })
            }
            "import_from_statement" => {
                // "from os.path import join, exists"
                // "from . import something"
                let after_from = text.trim_start_matches("from").trim();
                if let Some(import_idx) = after_from.find(" import ") {
                    let module = after_from[..import_idx].trim().to_string();
                    let names_str = after_from[import_idx + 8..].trim();
                    let names: Vec<String> = if names_str.starts_with('(') {
                        names_str
                            .trim_matches(|c| c == '(' || c == ')')
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    } else {
                        names_str
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    };
                    Some(Import {
                        source: module,
                        names,
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn extract_methods(node: &tree_sitter::Node, source: &[u8]) -> Vec<Symbol> {
        let mut methods = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block" {
                let mut inner_cursor = child.walk();
                for item in child.children(&mut inner_cursor) {
                    if item.kind() == "function_definition" {
                        let name = Self::extract_name(&item, source);
                        let is_pub = Self::is_public(&name);
                        let visibility = if is_pub { Visibility::Public } else { Visibility::Private };
                        let signature = Self::extract_fn_signature(&item, source);
                        let body = Self::extract_fn_body(&item, source);
                        let start_line = item.start_position().row + 1;
                        let end_line = item.end_position().row + 1;
                        methods.push(Symbol {
                            name,
                            kind: SymbolKind::Method,
                            visibility,
                            signature,
                            body,
                            start_line,
                            end_line,
                        });
                    }
                }
            }
        }
        methods
    }
}

impl LanguageSupport for PythonLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_python::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "python"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let mut symbols: Vec<Symbol> = Vec::new();
        let mut imports: Vec<Import> = Vec::new();
        let mut exports: Vec<Export> = Vec::new();

        let mut cursor = root.walk();

        for node in root.children(&mut cursor) {
            match node.kind() {
                "function_definition" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let is_pub = Self::is_public(&name);
                    let visibility = if is_pub { Visibility::Public } else { Visibility::Private };
                    let signature = Self::extract_fn_signature(&node, source_bytes);
                    let body = Self::extract_fn_body(&node, source_bytes);
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if is_pub {
                        exports.push(Export {
                            name: name.clone(),
                            kind: SymbolKind::Function,
                        });
                    }

                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Function,
                        visibility,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "class_definition" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let is_pub = Self::is_public(&name);
                    let visibility = if is_pub { Visibility::Public } else { Visibility::Private };
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if is_pub {
                        exports.push(Export {
                            name: name.clone(),
                            kind: SymbolKind::Class,
                        });
                    }

                    symbols.push(Symbol {
                        name: name.clone(),
                        kind: SymbolKind::Class,
                        visibility,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });

                    // Extract methods from class body
                    let methods = Self::extract_methods(&node, source_bytes);
                    for method in &methods {
                        if method.visibility == Visibility::Public {
                            exports.push(Export {
                                name: method.name.clone(),
                                kind: SymbolKind::Method,
                            });
                        }
                    }
                    symbols.extend(methods);
                }

                "import_statement" | "import_from_statement" => {
                    if let Some(import) = Self::extract_import(&node, source_bytes) {
                        imports.push(import);
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
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_public_function() {
        let source = r#"def greet(name):
    return f"Hello, {name}!"
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PythonLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "greet");
        assert_eq!(sym.kind, SymbolKind::Function);
        assert_eq!(sym.visibility, Visibility::Public);
        assert!(sym.signature.contains("def greet"), "signature: {}", sym.signature);

        assert_eq!(result.exports.len(), 1);
        assert_eq!(result.exports[0].name, "greet");
    }

    #[test]
    fn test_extract_private_function() {
        let source = r#"def _helper(x):
    return x * 2
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PythonLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "_helper");
        assert_eq!(sym.visibility, Visibility::Private);
        assert!(result.exports.is_empty(), "private function should not be exported");
    }

    #[test]
    fn test_extract_class() {
        let source = r#"class Animal:
    def __init__(self, name):
        self.name = name

    def speak(self):
        pass
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PythonLanguage;
        let result = lang.extract(source, &tree);

        let classes: Vec<_> = result.symbols.iter().filter(|s| s.kind == SymbolKind::Class).collect();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].name, "Animal");
        assert_eq!(classes[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"import os
from pathlib import Path, PurePath
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PythonLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.imports.len(), 2);
        assert!(result.imports[0].names.contains(&"os".to_string()));
        let second = &result.imports[1];
        assert_eq!(second.source, "pathlib");
        assert!(second.names.contains(&"Path".to_string()));
        assert!(second.names.contains(&"PurePath".to_string()));
    }
}
