use crate::parser::language::{
    Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility,
};
use tree_sitter::Language as TsLanguage;

pub struct CppLanguage;

impl CppLanguage {
    fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
        node.utf8_text(source).unwrap_or("")
    }

    fn first_line(node: &tree_sitter::Node, source: &[u8]) -> String {
        let text = Self::node_text(node, source);
        text.lines().next().unwrap_or("").trim().to_string()
    }

    fn find_fn_identifier(node: &tree_sitter::Node, source: &[u8], depth: usize) -> String {
        if depth > 6 {
            return String::new();
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "identifier" | "field_identifier" => {
                    return Self::node_text(&child, source).to_string();
                }
                "qualified_identifier" => {
                    // last segment of a::b::c
                    let text = Self::node_text(&child, source);
                    return text.split("::").last().unwrap_or("").to_string();
                }
                "function_declarator"
                | "pointer_declarator"
                | "reference_declarator"
                | "abstract_pointer_declarator" => {
                    let name = Self::find_fn_identifier(&child, source, depth + 1);
                    if !name.is_empty() {
                        return name;
                    }
                }
                _ => {}
            }
        }
        String::new()
    }

    fn extract_tag_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_identifier" || child.kind() == "identifier" {
                return Self::node_text(&child, source).to_string();
            }
        }
        String::new()
    }

    fn extract_typedef_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut last_name = String::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "type_identifier" | "identifier" | "primitive_type" => {
                    last_name = Self::node_text(&child, source).to_string();
                }
                _ => {}
            }
        }
        last_name
    }

    fn extract_fn_signature(node: &tree_sitter::Node, source: &[u8]) -> String {
        let full_text = Self::node_text(node, source);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "compound_statement" {
                let body_start = child.start_byte() - node.start_byte();
                return full_text[..body_start].trim().to_string();
            }
        }
        full_text.lines().next().unwrap_or("").trim().to_string()
    }

    fn extract_fn_body(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "compound_statement" {
                let text = &source[child.start_byte()..child.end_byte()];
                return String::from_utf8_lossy(text).into_owned();
            }
        }
        String::new()
    }

    fn extract_include(node: &tree_sitter::Node, source: &[u8]) -> Option<Import> {
        let text = Self::node_text(node, source);
        let path = text
            .trim_start_matches("#include")
            .trim()
            .trim_matches(|c| c == '<' || c == '>' || c == '"')
            .to_string();

        if path.is_empty() {
            None
        } else {
            Some(Import {
                source: path.clone(),
                names: vec![path],
            })
        }
    }

    fn extract_namespace_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "namespace_identifier" {
                return Self::node_text(&child, source).to_string();
            }
        }
        String::new()
    }
}

impl LanguageSupport for CppLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_cpp::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "cpp"
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
                "preproc_include" => {
                    if let Some(import) = Self::extract_include(&node, source_bytes) {
                        imports.push(import);
                    }
                }

                "function_definition" => {
                    let name = Self::find_fn_identifier(&node, source_bytes, 0);
                    if name.is_empty() {
                        continue;
                    }
                    let signature = Self::extract_fn_signature(&node, source_bytes);
                    let body = Self::extract_fn_body(&node, source_bytes);
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    exports.push(Export {
                        name: name.clone(),
                        kind: SymbolKind::Function,
                    });
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Function,
                        visibility: Visibility::Public,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "struct_specifier" => {
                    let name = Self::extract_tag_name(&node, source_bytes);
                    if name.is_empty() {
                        continue;
                    }
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    exports.push(Export {
                        name: name.clone(),
                        kind: SymbolKind::Struct,
                    });
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Struct,
                        visibility: Visibility::Public,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "class_specifier" => {
                    let name = Self::extract_tag_name(&node, source_bytes);
                    if name.is_empty() {
                        continue;
                    }
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
                }

                "enum_specifier" => {
                    let name = Self::extract_tag_name(&node, source_bytes);
                    if name.is_empty() {
                        continue;
                    }
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    exports.push(Export {
                        name: name.clone(),
                        kind: SymbolKind::Enum,
                    });
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Enum,
                        visibility: Visibility::Public,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "namespace_definition" => {
                    let name = Self::extract_namespace_name(&node, source_bytes);
                    if name.is_empty() {
                        continue;
                    }
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    // Namespaces are always accessible (treat as public)
                    exports.push(Export {
                        name: name.clone(),
                        kind: SymbolKind::Struct,
                    });
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Struct,
                        visibility: Visibility::Public,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "type_definition" => {
                    let name = Self::extract_typedef_name(&node, source_bytes);
                    if name.is_empty() {
                        continue;
                    }
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    exports.push(Export {
                        name: name.clone(),
                        kind: SymbolKind::TypeAlias,
                    });
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::TypeAlias,
                        visibility: Visibility::Public,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "declaration" => {
                    let mut decl_cursor = node.walk();
                    for child in node.children(&mut decl_cursor) {
                        if child.kind() == "struct_specifier" || child.kind() == "class_specifier" {
                            let kind = if child.kind() == "class_specifier" {
                                SymbolKind::Class
                            } else {
                                SymbolKind::Struct
                            };
                            let name = Self::extract_tag_name(&child, source_bytes);
                            if !name.is_empty() {
                                let signature = Self::first_line(&child, source_bytes);
                                let body = Self::node_text(&child, source_bytes).to_string();
                                let start_line = child.start_position().row + 1;
                                let end_line = child.end_position().row + 1;
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
            .set_language(&tree_sitter_cpp::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_function() {
        let source = r#"int add(int a, int b) {
    return a + b;
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = CppLanguage;
        let result = lang.extract(source, &tree);

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        assert!(!funcs.is_empty(), "expected function symbol");
        assert_eq!(funcs[0].name, "add");
        assert_eq!(funcs[0].visibility, Visibility::Public);
        assert!(
            funcs[0].signature.contains("int add(int a, int b)"),
            "signature: {}",
            funcs[0].signature
        );
    }

    #[test]
    fn test_extract_include() {
        let source = r#"#include <iostream>
#include <string>
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = CppLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.imports.len(), 2);
        assert!(result.imports.iter().any(|i| i.source == "iostream"));
        assert!(result.imports.iter().any(|i| i.source == "string"));
    }

    #[test]
    fn test_extract_class() {
        let source = r#"class Point {
public:
    double x;
    double y;
};
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = CppLanguage;
        let result = lang.extract(source, &tree);

        let classes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(
            !classes.is_empty(),
            "expected class symbol, got: {:?}",
            result
                .symbols
                .iter()
                .map(|s| (&s.name, &s.kind))
                .collect::<Vec<_>>()
        );
        assert_eq!(classes[0].name, "Point");
    }

    #[test]
    fn test_extract_typedef() {
        let source = r#"typedef unsigned int uint32_t;
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = CppLanguage;
        let result = lang.extract(source, &tree);

        let typedefs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::TypeAlias)
            .collect();
        assert!(!typedefs.is_empty(), "expected typedef symbol");
        assert_eq!(typedefs[0].name, "uint32_t");
    }
}
