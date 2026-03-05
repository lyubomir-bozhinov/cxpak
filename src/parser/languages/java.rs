use crate::parser::language::{
    Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility,
};
use tree_sitter::Language as TsLanguage;

pub struct JavaLanguage;

impl JavaLanguage {
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

    fn extract_visibility(node: &tree_sitter::Node, source: &[u8]) -> Visibility {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let text = Self::node_text(&child, source);
                if text.contains("public") {
                    return Visibility::Public;
                }
                return Visibility::Private;
            }
        }
        // Default in Java is package-private (treat as Private for our purposes)
        Visibility::Private
    }

    fn extract_fn_signature(node: &tree_sitter::Node, source: &[u8]) -> String {
        let full_text = Self::node_text(node, source);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block" {
                let body_start = child.start_byte() - node.start_byte();
                return full_text[..body_start].trim().to_string();
            }
        }
        full_text.lines().next().unwrap_or("").trim().to_string()
    }

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
        // "import java.util.List;"
        // "import java.util.*;"
        let text = Self::node_text(node, source);
        let inner = text
            .trim_start_matches("import")
            .trim()
            .trim_start_matches("static")
            .trim()
            .trim_end_matches(';')
            .trim();

        if inner.is_empty() {
            return None;
        }

        if inner.ends_with(".*") {
            let source_path = inner.trim_end_matches(".*").to_string();
            return Some(Import {
                source: source_path,
                names: vec!["*".to_string()],
            });
        }

        if let Some(sep) = inner.rfind('.') {
            let source_path = inner[..sep].to_string();
            let name = inner[sep + 1..].to_string();
            Some(Import {
                source: source_path,
                names: vec![name],
            })
        } else {
            Some(Import {
                source: String::new(),
                names: vec![inner.to_string()],
            })
        }
    }
}

impl LanguageSupport for JavaLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_java::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "java"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let mut symbols: Vec<Symbol> = Vec::new();
        let mut imports: Vec<Import> = Vec::new();
        let mut exports: Vec<Export> = Vec::new();

        // Walk all descendants to find top-level and nested declarations
        let mut stack: Vec<tree_sitter::Node> = root.children(&mut root.walk()).collect();

        while let Some(node) = stack.pop() {
            match node.kind() {
                "import_declaration" => {
                    if let Some(import) = Self::extract_import(&node, source_bytes) {
                        imports.push(import);
                    }
                }

                "class_declaration" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let visibility = Self::extract_visibility(&node, source_bytes);
                    let is_pub = visibility == Visibility::Public;
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
                        name,
                        kind: SymbolKind::Class,
                        visibility,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });

                    // Recurse into class body to find methods
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        stack.push(child);
                    }
                }

                "interface_declaration" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let visibility = Self::extract_visibility(&node, source_bytes);
                    let is_pub = visibility == Visibility::Public;
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if is_pub {
                        exports.push(Export {
                            name: name.clone(),
                            kind: SymbolKind::Interface,
                        });
                    }
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Interface,
                        visibility,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "enum_declaration" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let visibility = Self::extract_visibility(&node, source_bytes);
                    let is_pub = visibility == Visibility::Public;
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if is_pub {
                        exports.push(Export {
                            name: name.clone(),
                            kind: SymbolKind::Enum,
                        });
                    }
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Enum,
                        visibility,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "method_declaration" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let visibility = Self::extract_visibility(&node, source_bytes);
                    let is_pub = visibility == Visibility::Public;
                    let signature = Self::extract_fn_signature(&node, source_bytes);
                    let body = Self::extract_fn_body(&node, source_bytes);
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if is_pub {
                        exports.push(Export {
                            name: name.clone(),
                            kind: SymbolKind::Method,
                        });
                    }
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Method,
                        visibility,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "constructor_declaration" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let visibility = Self::extract_visibility(&node, source_bytes);
                    let is_pub = visibility == Visibility::Public;
                    let signature = Self::extract_fn_signature(&node, source_bytes);
                    let body = Self::extract_fn_body(&node, source_bytes);
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if is_pub {
                        exports.push(Export {
                            name: name.clone(),
                            kind: SymbolKind::Method,
                        });
                    }
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Method,
                        visibility,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                // Recurse into class/interface bodies to find methods
                "class_body" | "interface_body" | "enum_body" => {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        stack.push(child);
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
            .set_language(&tree_sitter_java::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_public_class() {
        let source = r#"public class HelloWorld {
    public void greet() {
        System.out.println("Hello");
    }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = JavaLanguage;
        let result = lang.extract(source, &tree);

        let classes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(!classes.is_empty(), "expected class symbol");
        assert_eq!(classes[0].name, "HelloWorld");
        assert_eq!(classes[0].visibility, Visibility::Public);

        let exported: Vec<_> = result
            .exports
            .iter()
            .filter(|e| e.name == "HelloWorld")
            .collect();
        assert!(!exported.is_empty(), "HelloWorld should be exported");
    }

    #[test]
    fn test_extract_import() {
        let source = r#"import java.util.List;
import java.util.HashMap;
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = JavaLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.imports.len(), 2);
        let names: Vec<&str> = result
            .imports
            .iter()
            .flat_map(|i| i.names.iter().map(|n| n.as_str()))
            .collect();
        assert!(
            names.contains(&"List"),
            "expected List import, got: {:?}",
            names
        );
        assert!(
            names.contains(&"HashMap"),
            "expected HashMap import, got: {:?}",
            names
        );
    }

    #[test]
    fn test_extract_method_visibility() {
        let source = r#"public class Foo {
    public void publicMethod() {}
    private void privateMethod() {}
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = JavaLanguage;
        let result = lang.extract(source, &tree);

        let methods: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Method)
            .collect();
        assert_eq!(methods.len(), 2);

        let pub_method = methods
            .iter()
            .find(|m| m.name == "publicMethod")
            .expect("publicMethod not found");
        assert_eq!(pub_method.visibility, Visibility::Public);

        let priv_method = methods
            .iter()
            .find(|m| m.name == "privateMethod")
            .expect("privateMethod not found");
        assert_eq!(priv_method.visibility, Visibility::Private);
    }
}
