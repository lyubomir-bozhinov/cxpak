use crate::parser::language::{Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility};
use tree_sitter::Language as TsLanguage;

pub struct CLanguage;

impl CLanguage {
    fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
        node.utf8_text(source).unwrap_or("")
    }

    fn first_line(node: &tree_sitter::Node, source: &[u8]) -> String {
        let text = Self::node_text(node, source);
        text.lines().next().unwrap_or("").trim().to_string()
    }

    /// Extract the function/declarator name from a function_definition node.
    /// C grammar: function_definition -> type declarator compound_statement
    /// The declarator can be a function_declarator containing an identifier.
    fn extract_fn_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        Self::find_fn_identifier(node, source, 0)
    }

    fn find_fn_identifier(node: &tree_sitter::Node, source: &[u8], depth: usize) -> String {
        if depth > 5 {
            return String::new();
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "identifier" => return Self::node_text(&child, source).to_string(),
                "function_declarator" | "pointer_declarator" | "declarator" => {
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

    /// Extract tag name from struct_specifier or enum_specifier.
    fn extract_tag_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_identifier" || child.kind() == "identifier" {
                return Self::node_text(&child, source).to_string();
            }
        }
        String::new()
    }

    /// Extract the name from a type_definition (typedef).
    fn extract_typedef_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        // type_definition: "typedef" type_specifier declarator ";"
        // The last identifier-like child before ";" is the alias name.
        // tree-sitter C may use type_identifier, identifier, or primitive_type.
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
        // preproc_include: "#include" (<path> | "path")
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
}

impl LanguageSupport for CLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_c::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "c"
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
                    let name = Self::extract_fn_name(&node, source_bytes);
                    // C has no access modifiers — all top-level functions are public
                    let signature = Self::extract_fn_signature(&node, source_bytes);
                    let body = Self::extract_fn_body(&node, source_bytes);
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    exports.push(Export { name: name.clone(), kind: SymbolKind::Function });
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

                    exports.push(Export { name: name.clone(), kind: SymbolKind::Struct });
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

                "enum_specifier" => {
                    let name = Self::extract_tag_name(&node, source_bytes);
                    if name.is_empty() {
                        continue;
                    }
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    exports.push(Export { name: name.clone(), kind: SymbolKind::Enum });
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

                "type_definition" => {
                    let name = Self::extract_typedef_name(&node, source_bytes);
                    if name.is_empty() {
                        continue;
                    }
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    exports.push(Export { name: name.clone(), kind: SymbolKind::TypeAlias });
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
                    // Top-level declarations may be struct specifiers embedded in declarations
                    let mut decl_cursor = node.walk();
                    for child in node.children(&mut decl_cursor) {
                        if child.kind() == "struct_specifier" {
                            let name = Self::extract_tag_name(&child, source_bytes);
                            if !name.is_empty() {
                                let signature = Self::first_line(&child, source_bytes);
                                let body = Self::node_text(&child, source_bytes).to_string();
                                let start_line = child.start_position().row + 1;
                                let end_line = child.end_position().row + 1;
                                exports.push(Export { name: name.clone(), kind: SymbolKind::Struct });
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
                        }
                    }
                }

                _ => {}
            }
        }

        ParseResult { symbols, imports, exports }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::language::{SymbolKind, Visibility};

    fn make_parser() -> tree_sitter::Parser {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_c::LANGUAGE.into())
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
        let lang = CLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "add");
        assert_eq!(sym.kind, SymbolKind::Function);
        assert_eq!(sym.visibility, Visibility::Public);
        assert!(sym.signature.contains("int add(int a, int b)"), "signature: {}", sym.signature);

        assert_eq!(result.exports.len(), 1);
        assert_eq!(result.exports[0].name, "add");
    }

    #[test]
    fn test_extract_include() {
        let source = r#"#include <stdio.h>
#include "myheader.h"
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = CLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.imports.len(), 2);
        assert!(result.imports.iter().any(|i| i.source.contains("stdio.h")));
        assert!(result.imports.iter().any(|i| i.source.contains("myheader.h")));
    }

    #[test]
    fn test_extract_typedef() {
        let source = r#"typedef unsigned int uint32_t;
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");

        let lang = CLanguage;
        let result = lang.extract(source, &tree);

        let typedefs: Vec<_> = result.symbols.iter().filter(|s| s.kind == SymbolKind::TypeAlias).collect();
        assert!(!typedefs.is_empty(), "expected typedef symbol");
        assert_eq!(typedefs[0].name, "uint32_t");
    }
}
