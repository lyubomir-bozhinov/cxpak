use crate::parser::language::{
    Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility,
};
use tree_sitter::Language as TsLanguage;

pub struct ScalaLanguage;

impl ScalaLanguage {
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

    /// Scala visibility: private/protected modifiers mean private, else public.
    fn has_private_modifier(node: &tree_sitter::Node, source: &[u8]) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" || child.kind() == "access_modifier" {
                let text = Self::node_text(&child, source);
                if text.contains("private") || text.contains("protected") {
                    return true;
                }
            }
        }
        false
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

    /// Extract import declaration.
    fn extract_import(node: &tree_sitter::Node, source: &[u8]) -> Option<Import> {
        let text = Self::node_text(node, source).trim().to_string();
        // e.g., "import scala.collection.mutable.ListBuffer"
        // e.g., "import scala.collection.mutable.{ListBuffer, ArrayBuffer}"
        let trimmed = text.trim_start_matches("import").trim().to_string();

        if trimmed.is_empty() {
            return None;
        }

        // Handle grouped imports: import scala.collection.{List, Map}
        if let Some(brace_start) = trimmed.find('{') {
            let base = trimmed[..brace_start].trim_end_matches('.').to_string();
            let names_str = &trimmed[brace_start + 1..];
            let names_str = names_str.trim_end_matches('}');
            let names: Vec<String> = names_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            Some(Import {
                source: base,
                names,
            })
        } else {
            // Single import
            let name = trimmed.rsplit('.').next().unwrap_or(&trimmed).to_string();
            Some(Import {
                source: trimmed,
                names: vec![name],
            })
        }
    }
}

impl LanguageSupport for ScalaLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_scala::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "scala"
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
                "function_definition" | "val_definition" | "var_definition" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let is_private = Self::has_private_modifier(&node, source_bytes);
                    let visibility = if is_private {
                        Visibility::Private
                    } else {
                        Visibility::Public
                    };
                    let signature = Self::extract_fn_signature(&node, source_bytes);
                    let body = Self::extract_fn_body(&node, source_bytes);
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    let kind = if node.kind() == "function_definition" {
                        SymbolKind::Function
                    } else {
                        SymbolKind::Variable
                    };

                    if !is_private {
                        exports.push(Export {
                            name: name.clone(),
                            kind: kind.clone(),
                        });
                    }
                    symbols.push(Symbol {
                        name,
                        kind,
                        visibility,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "class_definition" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let is_private = Self::has_private_modifier(&node, source_bytes);
                    let visibility = if is_private {
                        Visibility::Private
                    } else {
                        Visibility::Public
                    };
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if !is_private {
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

                "object_definition" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let is_private = Self::has_private_modifier(&node, source_bytes);
                    let visibility = if is_private {
                        Visibility::Private
                    } else {
                        Visibility::Public
                    };
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if !is_private {
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

                "trait_definition" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let is_private = Self::has_private_modifier(&node, source_bytes);
                    let visibility = if is_private {
                        Visibility::Private
                    } else {
                        Visibility::Public
                    };
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if !is_private {
                        exports.push(Export {
                            name: name.clone(),
                            kind: SymbolKind::Trait,
                        });
                    }
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Trait,
                        visibility,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "import_declaration" => {
                    if let Some(imp) = Self::extract_import(&node, source_bytes) {
                        imports.push(imp);
                    }
                }

                "package_clause" => {
                    // Skip package declarations
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
            .set_language(&tree_sitter_scala::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_function() {
        let source = r#"def greet(name: String): String = {
  s"Hello, $name!"
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        assert!(
            !funcs.is_empty(),
            "expected function symbol, got: {:?}",
            result
                .symbols
                .iter()
                .map(|s| (&s.name, &s.kind))
                .collect::<Vec<_>>()
        );
        assert_eq!(funcs[0].name, "greet");
        assert_eq!(funcs[0].visibility, Visibility::Public);

        assert!(
            result.exports.iter().any(|e| e.name == "greet"),
            "greet should be exported"
        );
    }

    #[test]
    fn test_extract_class() {
        let source = r#"class Point(val x: Double, val y: Double) {
  def distance(): Double = {
    math.sqrt(x * x + y * y)
  }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        let classes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(!classes.is_empty(), "expected class symbol");
        assert_eq!(classes[0].name, "Point");
        assert_eq!(classes[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_extract_object() {
        let source = r#"object Main {
  def main(args: Array[String]): Unit = {
    println("Hello")
  }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        let objects: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class && s.name == "Main")
            .collect();
        assert!(
            !objects.is_empty(),
            "expected object symbol (mapped to Class)"
        );
    }

    #[test]
    fn test_extract_trait() {
        let source = r#"trait Greeter {
  def greet(name: String): String
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        let traits: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Trait)
            .collect();
        assert!(!traits.is_empty(), "expected trait symbol");
        assert_eq!(traits[0].name, "Greeter");
    }

    #[test]
    fn test_extract_imports() {
        let source = "import scala.collection.mutable.ListBuffer\nimport scala.io.Source\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        assert!(
            !result.imports.is_empty(),
            "expected imports, got: {:?}",
            result.imports
        );
    }

    #[test]
    fn test_empty_source() {
        let source = "";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);
        assert!(result.symbols.is_empty());
    }

    #[test]
    fn test_val_definition() {
        let source = "val myVal = 42\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        let vals: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Variable)
            .collect();
        assert!(!vals.is_empty(), "expected val as Variable symbol");
        assert_eq!(vals[0].name, "myVal");
        assert_eq!(vals[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_var_definition() {
        let source = "var myVar = \"hello\"\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        let vars: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Variable)
            .collect();
        assert!(!vars.is_empty(), "expected var as Variable symbol");
        assert_eq!(vars[0].name, "myVar");
    }

    #[test]
    fn test_private_function() {
        let source = "private def secret(): Int = {\n  0\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        assert!(!funcs.is_empty(), "expected private function");
        assert_eq!(funcs[0].visibility, Visibility::Private);
        assert!(
            !result.exports.iter().any(|e| e.name == "secret"),
            "private function should not be exported"
        );
    }

    #[test]
    fn test_private_class() {
        let source = "private class Hidden\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        let classes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(!classes.is_empty(), "expected private class");
        assert_eq!(classes[0].visibility, Visibility::Private);
        assert!(
            !result.exports.iter().any(|e| e.name == "Hidden"),
            "private class should not be exported"
        );
    }

    #[test]
    fn test_grouped_import() {
        let source = "import scala.collection.mutable.{ListBuffer, ArrayBuffer}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        assert!(!result.imports.is_empty(), "expected grouped import");
        let imp = &result.imports[0];
        assert!(imp.source.contains("scala.collection.mutable"));
        assert!(imp.names.len() >= 2, "expected multiple import names");
    }

    #[test]
    fn test_package_clause_ignored() {
        let source = "package com.example\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        assert!(
            result.symbols.is_empty(),
            "package clause should not produce symbols"
        );
    }

    #[test]
    fn test_function_no_body() {
        // Abstract function without a block body
        let source = "def greet(name: String): String\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        if !funcs.is_empty() {
            assert!(
                funcs[0].body.is_empty(),
                "abstract function should have empty body"
            );
        }
    }

    #[test]
    fn test_private_object() {
        let source = "private object Internal\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        let objs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class && s.name == "Internal")
            .collect();
        if !objs.is_empty() {
            assert_eq!(objs[0].visibility, Visibility::Private);
        }
    }

    #[test]
    fn test_private_trait() {
        let source = "private trait Secret {\n  def x(): Int\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        let traits: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Trait)
            .collect();
        if !traits.is_empty() {
            assert_eq!(traits[0].visibility, Visibility::Private);
        }
    }

    #[test]
    fn test_complex_scala() {
        let source = r#"import scala.collection.mutable.ListBuffer

trait Serializable {
  def serialize(): String
}

class User(val name: String, val age: Int) extends Serializable {
  def serialize(): String = {
    s"$name,$age"
  }
}

object UserFactory {
  def create(name: String, age: Int): User = {
    new User(name, age)
  }
}

def helper(): Unit = {
  println("helper")
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = ScalaLanguage;
        let result = lang.extract(source, &tree);

        let traits: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Trait)
            .collect();
        assert!(!traits.is_empty(), "expected trait");

        let classes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(
            classes.len() >= 2,
            "expected class and object, got: {:?}",
            classes.iter().map(|c| &c.name).collect::<Vec<_>>()
        );

        assert!(!result.imports.is_empty(), "expected imports");
    }
}
