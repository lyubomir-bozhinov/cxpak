use crate::parser::language::{
    Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility,
};
use tree_sitter::Language as TsLanguage;

pub struct KotlinLanguage;

impl KotlinLanguage {
    fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
        node.utf8_text(source).unwrap_or("")
    }

    fn first_line(node: &tree_sitter::Node, source: &[u8]) -> String {
        let text = Self::node_text(node, source);
        text.lines().next().unwrap_or("").trim().to_string()
    }

    fn extract_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        if let Some(name_node) = node.child_by_field_name("name") {
            return Self::node_text(&name_node, source).to_string();
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "simple_identifier" {
                return Self::node_text(&child, source).to_string();
            }
        }
        String::new()
    }

    /// Kotlin default visibility is public. Look for a `modifiers` node containing
    /// `private`, `protected`, or `internal` to override.
    fn extract_visibility(node: &tree_sitter::Node, source: &[u8]) -> Visibility {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let text = Self::node_text(&child, source);
                if text.contains("private")
                    || text.contains("protected")
                    || text.contains("internal")
                {
                    return Visibility::Private;
                }
            }
        }
        Visibility::Public
    }

    fn extract_fn_body(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "function_body" || child.kind() == "block" {
                let text = &source[child.start_byte()..child.end_byte()];
                return String::from_utf8_lossy(text).into_owned();
            }
        }
        String::new()
    }

    fn extract_fn_signature(node: &tree_sitter::Node, source: &[u8]) -> String {
        let full_text = Self::node_text(node, source);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "function_body" || child.kind() == "block" {
                let body_start = child.start_byte() - node.start_byte();
                return full_text[..body_start].trim().to_string();
            }
        }
        full_text.lines().next().unwrap_or("").trim().to_string()
    }

    /// Extract import path from an `import` node.
    /// e.g. `import kotlin.collections.List`
    fn extract_import(node: &tree_sitter::Node, source: &[u8]) -> Option<Import> {
        let text = Self::node_text(node, source);
        let inner = text
            .trim_start_matches("import")
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

impl LanguageSupport for KotlinLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_kotlin_ng::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "kotlin"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let mut symbols: Vec<Symbol> = Vec::new();
        let mut imports: Vec<Import> = Vec::new();
        let mut exports: Vec<Export> = Vec::new();

        let mut stack: Vec<tree_sitter::Node> = root.children(&mut root.walk()).collect();

        while let Some(node) = stack.pop() {
            match node.kind() {
                "import" => {
                    if let Some(imp) = Self::extract_import(&node, source_bytes) {
                        imports.push(imp);
                    }
                }

                "function_declaration" => {
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

                    // Recurse into class body
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() == "class_body" || child.kind() == "enum_class_body" {
                            let mut inner = child.walk();
                            for grandchild in child.children(&mut inner) {
                                stack.push(grandchild);
                            }
                        }
                    }
                }

                "object_declaration" => {
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
            .set_language(&tree_sitter_kotlin_ng::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_function_public_by_default() {
        let source = r#"fun greet(name: String): String {
    return "Hello, $name!"
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = KotlinLanguage;
        let result = lang.extract(source, &tree);

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        assert!(!funcs.is_empty(), "expected function symbol");
        assert_eq!(funcs[0].name, "greet");
        assert_eq!(funcs[0].visibility, Visibility::Public);

        let exported: Vec<_> = result
            .exports
            .iter()
            .filter(|e| e.name == "greet")
            .collect();
        assert!(!exported.is_empty(), "greet should be exported");
    }

    #[test]
    fn test_extract_class() {
        let source = r#"class Animal(val name: String) {
    fun speak(): String = "..."
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = KotlinLanguage;
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
    fn test_extract_import() {
        let source = r#"import kotlin.collections.List
import java.util.HashMap
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = KotlinLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(
            result.imports.len(),
            2,
            "expected 2 imports, got {:?}",
            result.imports
        );
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
}
