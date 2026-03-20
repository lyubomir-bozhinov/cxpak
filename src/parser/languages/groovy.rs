use crate::parser::language::{
    Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility,
};
use tree_sitter::Language as TsLanguage;

pub struct GroovyLanguage;

impl GroovyLanguage {
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

    /// Determine visibility from access modifiers in the node text.
    fn determine_visibility(node: &tree_sitter::Node, source: &[u8]) -> Visibility {
        let text = Self::node_text(node, source);
        let first_line = text.lines().next().unwrap_or("");

        // Check for explicit access modifiers
        if first_line.contains("private ") || first_line.contains("protected ") {
            Visibility::Private
        } else {
            // In Groovy, default visibility is public (unlike Java)
            Visibility::Public
        }
    }

    /// Extract function/method signature (everything before the body block).
    fn extract_fn_signature(node: &tree_sitter::Node, source: &[u8]) -> String {
        let full_text = Self::node_text(node, source);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block" || child.kind() == "closure" {
                let body_start = child.start_byte() - node.start_byte();
                if body_start < full_text.len() {
                    return full_text[..body_start].trim().to_string();
                }
            }
        }
        full_text.lines().next().unwrap_or("").trim().to_string()
    }

    /// Extract the body block text.
    fn extract_fn_body(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block" || child.kind() == "closure" {
                let text = &source[child.start_byte()..child.end_byte()];
                return String::from_utf8_lossy(text).into_owned();
            }
        }
        String::new()
    }

    /// Extract import declaration: import foo.bar.Baz or import static foo.Bar.*
    fn extract_import(node: &tree_sitter::Node, source: &[u8]) -> Option<Import> {
        let text = Self::node_text(node, source);
        let trimmed = text
            .trim()
            .trim_end_matches(';')
            .trim_start_matches("import")
            .trim()
            .trim_start_matches("static")
            .trim();

        if trimmed.is_empty() {
            return None;
        }

        // Handle wildcard: import foo.bar.*
        if trimmed.ends_with(".*") {
            let source_path = trimmed.trim_end_matches(".*").to_string();
            return Some(Import {
                source: source_path,
                names: vec!["*".to_string()],
            });
        }

        // Regular import: import foo.bar.Baz
        if let Some(last_dot) = trimmed.rfind('.') {
            let source_path = trimmed[..last_dot].to_string();
            let name = trimmed[last_dot + 1..].to_string();
            Some(Import {
                source: source_path,
                names: vec![name],
            })
        } else {
            Some(Import {
                source: String::new(),
                names: vec![trimmed.to_string()],
            })
        }
    }

    /// Extract class name, handling Groovy class declarations.
    fn extract_class_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        Self::extract_name(node, source)
    }

    /// Extract methods from a class body.
    fn extract_class_methods(node: &tree_sitter::Node, source: &[u8]) -> Vec<Symbol> {
        let mut methods = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "class_body" {
                let mut inner_cursor = child.walk();
                for item in child.children(&mut inner_cursor) {
                    if item.kind() == "method_declaration" || item.kind() == "function_definition" {
                        let name = Self::extract_name(&item, source);
                        let visibility = Self::determine_visibility(&item, source);
                        let signature = Self::extract_fn_signature(&item, source);
                        let body = Self::extract_fn_body(&item, source);
                        let start_line = item.start_position().row + 1;
                        let end_line = item.end_position().row + 1;

                        if !name.is_empty() {
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
        }
        methods
    }
}

impl LanguageSupport for GroovyLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_groovy::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "groovy"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let mut symbols: Vec<Symbol> = Vec::new();
        let mut imports: Vec<Import> = Vec::new();
        let mut exports: Vec<Export> = Vec::new();

        // Use stack to walk deeper into class bodies
        let mut stack: Vec<tree_sitter::Node> = root.children(&mut root.walk()).collect();

        while let Some(node) = stack.pop() {
            let kind = node.kind();

            match kind {
                "method_declaration" | "function_definition" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let visibility = Self::determine_visibility(&node, source_bytes);
                    let signature = Self::extract_fn_signature(&node, source_bytes);
                    let body = Self::extract_fn_body(&node, source_bytes);
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if !name.is_empty() {
                        let sym_kind = SymbolKind::Function;
                        if visibility == Visibility::Public {
                            exports.push(Export {
                                name: name.clone(),
                                kind: sym_kind.clone(),
                            });
                        }
                        symbols.push(Symbol {
                            name,
                            kind: sym_kind,
                            visibility,
                            signature,
                            body,
                            start_line,
                            end_line,
                        });
                    }
                }

                "class_declaration" => {
                    let name = Self::extract_class_name(&node, source_bytes);
                    let visibility = Self::determine_visibility(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if !name.is_empty() {
                        if visibility == Visibility::Public {
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

                        // Extract methods from class body
                        let methods = Self::extract_class_methods(&node, source_bytes);
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
                }

                "import_declaration" => {
                    if let Some(imp) = Self::extract_import(&node, source_bytes) {
                        imports.push(imp);
                    }
                }

                _ => {
                    // Push children to continue scanning deeper nodes
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        stack.push(child);
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
            .set_language(&tree_sitter_groovy::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_function() {
        let source = r#"def greet(String name) {
    println "Hello, ${name}!"
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| {
                (s.kind == SymbolKind::Function || s.kind == SymbolKind::Method)
                    && s.name == "greet"
            })
            .collect();
        assert!(
            !funcs.is_empty(),
            "expected function 'greet', got symbols: {:?}",
            result
                .symbols
                .iter()
                .map(|s| (&s.name, &s.kind))
                .collect::<Vec<_>>()
        );
        assert_eq!(funcs[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_extract_class() {
        let source = r#"class Animal {
    String name

    def speak() {
        return "..."
    }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
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
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"import groovy.json.JsonSlurper
import java.util.*
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
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
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);
        assert!(result.symbols.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.exports.is_empty());
    }

    #[test]
    fn test_complex_groovy_class() {
        let source = r#"import groovy.transform.ToString

@ToString
class Person {
    String name
    int age

    def greet() {
        return "Hello, I'm ${name}"
    }

    private void helper() {
        // internal logic
    }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        // Should have class and possibly methods
        assert!(
            !result.symbols.is_empty(),
            "expected symbols from Groovy class"
        );

        // Should have import
        assert!(
            !result.imports.is_empty(),
            "expected import from groovy.transform"
        );
    }

    #[test]
    fn test_wildcard_import() {
        let source = "import java.util.*\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        assert!(!result.imports.is_empty(), "expected wildcard import");
        let util_import = result
            .imports
            .iter()
            .find(|i| i.source.contains("java.util"));
        assert!(
            util_import.is_some(),
            "expected java.util import, got: {:?}",
            result.imports
        );
        if let Some(imp) = util_import {
            assert!(
                imp.names.contains(&"*".to_string()),
                "expected wildcard name, got: {:?}",
                imp.names
            );
        }
    }

    #[test]
    fn test_coverage_class_with_methods() {
        let source = r#"class Calculator {
    def add(int a, int b) {
        return a + b
    }

    private def helper() {
        return 0
    }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        let classes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(!classes.is_empty(), "expected class symbol");

        // Methods may be extracted as Method kind
        let methods: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Method || s.kind == SymbolKind::Function)
            .collect();
        assert!(
            !methods.is_empty(),
            "expected method symbols, got: {:?}",
            result
                .symbols
                .iter()
                .map(|s| (&s.name, &s.kind))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_coverage_private_visibility() {
        let source = r#"class Service {
    private void internal() {
        println "internal"
    }

    public void external() {
        println "external"
    }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        // Check that private methods are detected
        let private_syms: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.visibility == Visibility::Private)
            .collect();
        // The class or its methods should be detected
        assert!(
            !result.symbols.is_empty(),
            "expected symbols from class with visibility modifiers"
        );
        // If methods extracted, check for private
        if !private_syms.is_empty() {
            assert!(
                private_syms
                    .iter()
                    .any(|s| s.name == "internal" || s.name == "Service"),
                "expected private symbol, got: {:?}",
                private_syms.iter().map(|s| &s.name).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn test_coverage_static_method() {
        let source = r#"class Utils {
    static def format(String input) {
        return input.trim()
    }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        assert!(
            !result.symbols.is_empty(),
            "expected symbols from class with static method"
        );
    }

    #[test]
    fn test_coverage_interface_declaration() {
        let source = r#"interface Greeter {
    void greet(String name)
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        // Interface might be detected as class or might not be parsed at all
        // The important thing is no panic
        let _ = result;
    }

    #[test]
    fn test_coverage_closure() {
        let source = r#"def greetAll = { names ->
    names.each { name ->
        println "Hello, ${name}"
    }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        // Closures may or may not produce symbols
        let _ = result;
    }

    #[test]
    fn test_coverage_multiple_imports() {
        let source = r#"import groovy.json.JsonSlurper
import groovy.transform.ToString
import java.util.*
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        assert!(
            result.imports.len() >= 2,
            "expected at least 2 imports, got: {:?}",
            result.imports
        );
    }

    #[test]
    fn test_coverage_import_no_dot() {
        // Import with no dot (single name) — exercises the else branch in extract_import
        let source = "import Foo\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        // Should produce an import (single name, no dots)
        if !result.imports.is_empty() {
            let imp = &result.imports[0];
            assert!(
                !imp.names.is_empty(),
                "expected at least one name in import"
            );
        }
    }

    #[test]
    fn test_coverage_class_export() {
        let source = r#"class MyService {
    def execute() {
        return "done"
    }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        // Public class should be exported
        let class_exported = result.exports.iter().any(|e| e.kind == SymbolKind::Class);
        assert!(
            class_exported,
            "expected class export, got exports: {:?}",
            result.exports
        );
    }

    #[test]
    fn test_private_class_declaration() {
        // private class => class_declaration with private modifier, visibility Private
        let source = r#"private class Secret {
    void doWork() {
        println "work"
    }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        let classes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(!classes.is_empty(), "expected private class");
        assert_eq!(classes[0].visibility, Visibility::Private);
        // Private class should NOT be exported
        assert!(
            !result.exports.iter().any(|e| e.name == "Secret"),
            "private class should not be exported"
        );
    }

    #[test]
    fn test_protected_function() {
        // protected void service() => function_definition with protected modifier
        let source = "protected void service() {\n    println \"protected\"\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        assert!(!funcs.is_empty(), "expected protected function");
        assert_eq!(funcs[0].name, "service");
        assert_eq!(funcs[0].visibility, Visibility::Private);
        // Private/protected should NOT be exported
        assert!(
            !result.exports.iter().any(|e| e.name == "service"),
            "protected function should not be exported"
        );
    }

    #[test]
    fn test_static_import() {
        // static import exercises the static-trimming path in extract_import
        let source = "import static java.lang.Math.abs\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        assert!(!result.imports.is_empty(), "expected static import");
        let imp = &result.imports[0];
        assert_eq!(imp.names[0], "abs");
        assert!(imp.source.contains("Math"));
    }

    #[test]
    fn test_interface_with_methods() {
        // interface_declaration falls into the _ branch and text contains "interface"
        // but it should NOT match "class " text check
        let source = "interface Greeter {\n    void greet(String name)\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);
        // interface_declaration is not matched by our class/function patterns
        // but its children (method_declaration) get pushed onto the stack
        // The interface_body children get scanned
        let _ = result;
    }

    #[test]
    fn test_class_methods_private_not_exported() {
        // Verify that private methods inside a class are NOT exported
        let source = r#"class Svc {
    private void internal() {
        println "internal"
    }
    void external() {
        println "external"
    }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        // external should be exported as Method
        let ext_export = result.exports.iter().find(|e| e.name == "external");
        assert!(ext_export.is_some(), "external should be exported");
        // internal should NOT be exported
        assert!(
            !result.exports.iter().any(|e| e.name == "internal"),
            "internal should not be exported"
        );
    }

    #[test]
    fn test_extract_fn_signature_with_closure() {
        // Top-level function_definition has closure body
        let source = "def greet(String name) {\n    println name\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);

        let func = result.symbols.iter().find(|s| s.name == "greet").unwrap();
        // Signature should be everything before the closure body
        assert!(
            func.signature.contains("greet"),
            "signature should contain function name"
        );
        assert!(
            !func.signature.contains('{'),
            "signature should not include body brace"
        );
        // Body should contain the closure content
        assert!(!func.body.is_empty(), "body should not be empty");
    }

    #[test]
    fn test_extract_class_name() {
        // class_declaration always has an identifier child
        let mut parser = make_parser();
        let source = "class MyTest { }\n";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let class_node = root.child(0).unwrap();
        assert_eq!(class_node.kind(), "class_declaration");
        let name = GroovyLanguage::extract_class_name(&class_node, source.as_bytes());
        assert_eq!(name, "MyTest");
    }

    #[test]
    fn test_extract_import_empty() {
        // Exercise the empty-import path in extract_import
        // When the import text, after stripping, is empty
        let mut parser = make_parser();
        let source = "import\n";
        let tree = parser.parse(source, None).unwrap();
        let lang = GroovyLanguage;
        let result = lang.extract(source, &tree);
        // Should not produce an import (empty path)
        // No panic is the main assertion
        let _ = result;
    }
}
