use crate::parser::language::{
    Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility,
};
use tree_sitter::Language as TsLanguage;

pub struct YamlLanguage;

impl YamlLanguage {
    fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
        node.utf8_text(source).unwrap_or("")
    }

    fn first_line(node: &tree_sitter::Node, source: &[u8]) -> String {
        let text = Self::node_text(node, source);
        text.lines().next().unwrap_or("").trim().to_string()
    }

    fn full_text(node: &tree_sitter::Node, source: &[u8]) -> String {
        Self::node_text(node, source).to_string()
    }

    /// Extract the key text from a block_mapping_pair node.
    /// The grammar always wraps keys in `flow_node` children.
    fn extract_key_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "flow_node" {
                let text = Self::node_text(&child, source).trim().to_string();
                if !text.is_empty() {
                    return text.trim_matches('"').trim_matches('\'').to_string();
                }
            }
        }
        String::new()
    }

    /// Check if a block_mapping_pair's value is itself a block_mapping (nested map).
    fn has_nested_mapping(node: &tree_sitter::Node) -> bool {
        let mut cursor = node.walk();
        let mut seen_key = false;
        for child in node.children(&mut cursor) {
            if seen_key {
                // The value part
                if child.kind() == "block_node" {
                    // Check if it contains a block_mapping
                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        if inner.kind() == "block_mapping" {
                            return true;
                        }
                    }
                }
            }
            if child.kind() == "flow_node" && !seen_key {
                seen_key = true;
            }
        }
        false
    }

    /// Recursively extract top-level mapping pairs from a block_mapping node.
    fn extract_mapping_pairs(
        node: &tree_sitter::Node,
        source: &[u8],
        symbols: &mut Vec<Symbol>,
        top_level: bool,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block_mapping_pair" {
                let name = Self::extract_key_name(&child, source);
                if name.is_empty() {
                    continue;
                }

                let start_line = child.start_position().row + 1;
                let end_line = child.end_position().row + 1;
                let is_nested = Self::has_nested_mapping(&child);

                let kind = if top_level && is_nested {
                    SymbolKind::Block
                } else {
                    SymbolKind::Key
                };

                symbols.push(Symbol {
                    name,
                    kind,
                    visibility: Visibility::Public,
                    signature: Self::first_line(&child, source),
                    body: Self::full_text(&child, source),
                    start_line,
                    end_line,
                });
            }
        }
    }
}

impl LanguageSupport for YamlLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_yaml::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "yaml"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let mut symbols: Vec<Symbol> = Vec::new();
        let imports: Vec<Import> = Vec::new();
        let exports: Vec<Export> = Vec::new();

        // Walk through documents (YAML can have multiple documents separated by ---)
        let mut root_cursor = root.walk();
        for doc_node in root.children(&mut root_cursor) {
            // Look for block_mapping nodes inside document nodes
            let mut doc_cursor = doc_node.walk();
            for child in doc_node.children(&mut doc_cursor) {
                if child.kind() == "block_node" || child.kind() == "block_mapping" {
                    // If block_node, look for block_mapping inside
                    if child.kind() == "block_node" {
                        let mut inner_cursor = child.walk();
                        for inner in child.children(&mut inner_cursor) {
                            if inner.kind() == "block_mapping" {
                                Self::extract_mapping_pairs(
                                    &inner,
                                    source_bytes,
                                    &mut symbols,
                                    true,
                                );
                            }
                        }
                    } else {
                        Self::extract_mapping_pairs(&child, source_bytes, &mut symbols, true);
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::language::{SymbolKind, Visibility};

    fn make_parser() -> tree_sitter::Parser {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_yaml::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_top_level_keys() {
        let source = r#"name: my-project
version: 1.0.0
description: A test project
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = YamlLanguage;
        let result = lang.extract(source, &tree);

        assert!(
            !result.symbols.is_empty(),
            "expected symbols from YAML, got none"
        );
        let key_names: Vec<_> = result.symbols.iter().map(|s| &s.name).collect();
        assert!(
            key_names.iter().any(|n| n.contains("name")),
            "expected 'name' key, got: {:?}",
            key_names
        );
    }

    #[test]
    fn test_extract_nested_mapping() {
        let source = r#"database:
  host: localhost
  port: 5432
server:
  port: 8080
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = YamlLanguage;
        let result = lang.extract(source, &tree);

        assert!(
            !result.symbols.is_empty(),
            "expected symbols from nested YAML"
        );
    }

    #[test]
    fn test_empty_source() {
        let source = "";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = YamlLanguage;
        let result = lang.extract(source, &tree);
        assert!(result.symbols.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.exports.is_empty());
    }

    #[test]
    fn test_complex_yaml() {
        let source = r#"apiVersion: v1
kind: Deployment
metadata:
  name: my-app
  labels:
    app: my-app
spec:
  replicas: 3
  template:
    spec:
      containers:
        - name: app
          image: my-app:latest
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = YamlLanguage;
        let result = lang.extract(source, &tree);

        assert!(
            !result.symbols.is_empty(),
            "expected symbols from complex YAML"
        );
    }

    #[test]
    fn test_symbol_kinds() {
        let source = "key: value\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = YamlLanguage;
        let result = lang.extract(source, &tree);

        assert!(!result.symbols.is_empty(), "expected at least one symbol");
        // Keys should be Key or Block
        assert!(
            result
                .symbols
                .iter()
                .all(|s| s.kind == SymbolKind::Key || s.kind == SymbolKind::Block),
            "all YAML symbols should be Key or Block"
        );
        assert!(
            result
                .symbols
                .iter()
                .all(|s| s.visibility == Visibility::Public),
            "all YAML symbols should be public"
        );
    }

    #[test]
    fn test_multi_document() {
        // Tests the second document path (doc_node.kind() == "block_node" at line 162)
        let source = "a: 1\n---\nb: 2\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = YamlLanguage;
        let result = lang.extract(source, &tree);

        let names: Vec<_> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"a"), "expected key 'a', got: {:?}", names);
        assert!(names.contains(&"b"), "expected key 'b', got: {:?}", names);
    }

    #[test]
    fn test_quoted_keys() {
        let source = "\"quoted_key\": value1\n'single_quoted': value2\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = YamlLanguage;
        let result = lang.extract(source, &tree);

        assert!(
            !result.symbols.is_empty(),
            "expected symbols from quoted keys"
        );
    }

    #[test]
    fn test_nested_block_kind() {
        // Nested mapping should produce Block kind at top level
        let source = "parent:\n  child: value\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = YamlLanguage;
        let result = lang.extract(source, &tree);

        assert!(!result.symbols.is_empty(), "expected symbols");
        let parent = result.symbols.iter().find(|s| s.name == "parent");
        assert!(parent.is_some(), "expected 'parent' symbol");
        assert_eq!(
            parent.unwrap().kind,
            SymbolKind::Block,
            "top-level nested mapping should be Block"
        );
    }

    #[test]
    fn test_flat_key_kind() {
        // Flat key:value should produce Key kind
        let source = "simple: value\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = YamlLanguage;
        let result = lang.extract(source, &tree);

        assert!(!result.symbols.is_empty(), "expected symbols");
        assert_eq!(
            result.symbols[0].kind,
            SymbolKind::Key,
            "flat key should be Key kind"
        );
    }

    #[test]
    fn test_no_imports_exports() {
        let source = "a: 1\nb: 2\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = YamlLanguage;
        let result = lang.extract(source, &tree);
        assert!(result.imports.is_empty(), "yaml should have no imports");
        assert!(result.exports.is_empty(), "yaml should have no exports");
    }
}
