use crate::parser::language::{
    Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility,
};
use tree_sitter::Language as TsLanguage;

pub struct TypeScriptLanguage;

impl TypeScriptLanguage {
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
            if child.kind() == "identifier"
                || child.kind() == "type_identifier"
                || child.kind() == "property_identifier"
            {
                return Self::node_text(&child, source).to_string();
            }
        }
        String::new()
    }

    /// Determine visibility: exported items are Public.
    fn is_exported(node: &tree_sitter::Node, _source: &[u8]) -> bool {
        // Check for "export" keyword as a sibling or parent context.
        // In the tree-sitter TS grammar, exported declarations appear inside
        // an `export_statement` node, so we look at the parent.
        if let Some(parent) = node.parent() {
            if parent.kind() == "export_statement" {
                return true;
            }
        }
        // Also handle direct children markers
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "export" {
                return true;
            }
        }
        false
    }

    fn extract_fn_signature(node: &tree_sitter::Node, source: &[u8]) -> String {
        let full_text = Self::node_text(node, source);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "statement_block" {
                let body_start = child.start_byte() - node.start_byte();
                return full_text[..body_start].trim().to_string();
            }
        }
        full_text.lines().next().unwrap_or("").trim().to_string()
    }

    fn extract_fn_body(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "statement_block" {
                let text = &source[child.start_byte()..child.end_byte()];
                return String::from_utf8_lossy(text).into_owned();
            }
        }
        String::new()
    }

    fn extract_import(node: &tree_sitter::Node, source: &[u8]) -> Option<Import> {
        // import_statement: "import" import_clause "from" string
        // import_clause can be: namespace_import, named_imports, identifier, ...
        let text = Self::node_text(node, source);
        // Extract the module path (after "from")
        let source_path = if let Some(from_idx) = text.rfind(" from ") {
            text[from_idx + 6..]
                .trim()
                .trim_matches(|c| c == '\'' || c == '"' || c == ';')
                .to_string()
        } else {
            String::new()
        };

        // Extract names: look for { Name, Name } pattern
        let names = if let Some(brace_start) = text.find('{') {
            if let Some(brace_end) = text.find('}') {
                text[brace_start + 1..brace_end]
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            } else {
                vec![]
            }
        } else if text.contains("* as ") {
            vec!["*".to_string()]
        } else {
            // "import defaultExport from 'module'"
            let after_import = text.trim_start_matches("import").trim();
            let name = after_import
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            if name.is_empty() || name == "from" {
                vec![]
            } else {
                vec![name]
            }
        };

        if source_path.is_empty() && names.is_empty() {
            None
        } else {
            Some(Import {
                source: source_path,
                names,
            })
        }
    }
}

impl LanguageSupport for TypeScriptLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }

    fn name(&self) -> &str {
        "typescript"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let mut symbols: Vec<Symbol> = Vec::new();
        let mut imports: Vec<Import> = Vec::new();
        let mut exports: Vec<Export> = Vec::new();

        // Walk all nodes in the tree to catch top-level and export-wrapped declarations
        let mut stack: Vec<tree_sitter::Node> = root.children(&mut root.walk()).collect();

        while let Some(node) = stack.pop() {
            match node.kind() {
                "import_statement" => {
                    if let Some(import) = Self::extract_import(&node, source_bytes) {
                        imports.push(import);
                    }
                }

                "export_statement" => {
                    // Push children so we handle the exported declaration
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        stack.push(child);
                    }
                }

                "function_declaration" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let is_pub = Self::is_exported(&node, source_bytes);
                    let visibility = if is_pub {
                        Visibility::Public
                    } else {
                        Visibility::Private
                    };
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
                    let is_pub = Self::is_exported(&node, source_bytes);
                    let visibility = if is_pub {
                        Visibility::Public
                    } else {
                        Visibility::Private
                    };
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

                "interface_declaration" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let is_pub = Self::is_exported(&node, source_bytes);
                    let visibility = if is_pub {
                        Visibility::Public
                    } else {
                        Visibility::Private
                    };
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

                "type_alias_declaration" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let is_pub = Self::is_exported(&node, source_bytes);
                    let visibility = if is_pub {
                        Visibility::Public
                    } else {
                        Visibility::Private
                    };
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if is_pub {
                        exports.push(Export {
                            name: name.clone(),
                            kind: SymbolKind::TypeAlias,
                        });
                    }
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::TypeAlias,
                        visibility,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "enum_declaration" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let is_pub = Self::is_exported(&node, source_bytes);
                    let visibility = if is_pub {
                        Visibility::Public
                    } else {
                        Visibility::Private
                    };
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
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_exported_function() {
        let source = r#"export function greet(name: string): string {
    return `Hello, ${name}!`;
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = TypeScriptLanguage;
        let result = lang.extract(source, &tree);

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        assert!(!funcs.is_empty(), "expected at least one function symbol");
        let sym = funcs[0];
        assert_eq!(sym.name, "greet");
        assert_eq!(sym.visibility, Visibility::Public);

        let exported: Vec<_> = result
            .exports
            .iter()
            .filter(|e| e.name == "greet")
            .collect();
        assert!(!exported.is_empty(), "greet should be exported");
    }

    #[test]
    fn test_extract_interface() {
        let source = r#"export interface Animal {
    name: string;
    speak(): void;
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = TypeScriptLanguage;
        let result = lang.extract(source, &tree);

        let interfaces: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Interface)
            .collect();
        assert!(!interfaces.is_empty(), "expected interface symbol");
        assert_eq!(interfaces[0].name, "Animal");
        assert_eq!(interfaces[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_extract_import() {
        let source = r#"import { readFile, writeFile } from 'fs';
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = TypeScriptLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.imports.len(), 1);
        let imp = &result.imports[0];
        assert_eq!(imp.source, "fs");
        assert!(imp.names.contains(&"readFile".to_string()));
        assert!(imp.names.contains(&"writeFile".to_string()));
    }

    #[test]
    fn test_extract_type_alias() {
        let source = r#"export type UserId = string;
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = TypeScriptLanguage;
        let result = lang.extract(source, &tree);

        let aliases: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::TypeAlias)
            .collect();
        assert!(!aliases.is_empty(), "expected type alias symbol");
        assert_eq!(aliases[0].name, "UserId");
    }
}
