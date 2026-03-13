use crate::parser::language::{
    Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility,
};
use tree_sitter::Language as TsLanguage;

pub struct SwiftLanguage;

impl SwiftLanguage {
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
            if child.kind() == "simple_identifier" || child.kind() == "identifier" {
                return Self::node_text(&child, source).to_string();
            }
        }
        String::new()
    }

    /// Swift visibility comes from modifier nodes: `public`, `private`, `internal`, `open`.
    /// The default visibility is `internal` — treated as Private here.
    fn extract_visibility(node: &tree_sitter::Node, source: &[u8]) -> Visibility {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind == "modifiers" || kind == "visibility_modifier" || kind == "modifier" {
                let text = Self::node_text(&child, source);
                if text.contains("public") || text.contains("open") {
                    return Visibility::Public;
                }
            }
            // Some grammars surface modifiers as direct children with keyword kinds
            if kind == "public" || kind == "open" {
                return Visibility::Public;
            }
        }
        Visibility::Private
    }

    fn extract_fn_body(node: &tree_sitter::Node, source: &[u8]) -> String {
        if let Some(body_node) = node.child_by_field_name("body") {
            let text = &source[body_node.start_byte()..body_node.end_byte()];
            return String::from_utf8_lossy(text).into_owned();
        }
        // Fall back to looking for a code_block child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "code_block" || child.kind() == "function_body" {
                let text = &source[child.start_byte()..child.end_byte()];
                return String::from_utf8_lossy(text).into_owned();
            }
        }
        String::new()
    }

    fn extract_fn_signature(node: &tree_sitter::Node, source: &[u8]) -> String {
        let full_text = Self::node_text(node, source);
        // Signature is everything before the body block
        if let Some(body_node) = node.child_by_field_name("body") {
            let body_start = body_node.start_byte() - node.start_byte();
            return full_text[..body_start].trim().to_string();
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "code_block" || child.kind() == "function_body" {
                let body_start = child.start_byte() - node.start_byte();
                return full_text[..body_start].trim().to_string();
            }
        }
        full_text.lines().next().unwrap_or("").trim().to_string()
    }

    /// Determine the SymbolKind for a `class_declaration` node by inspecting the
    /// `declaration_kind` field, which contains tokens like `class`, `struct`, `enum`,
    /// `actor`, `extension`.
    fn class_declaration_kind(node: &tree_sitter::Node, source: &[u8]) -> SymbolKind {
        if let Some(kind_node) = node.child_by_field_name("declaration_kind") {
            let kind_text = Self::node_text(&kind_node, source);
            return match kind_text {
                "struct" => SymbolKind::Struct,
                "enum" => SymbolKind::Enum,
                _ => SymbolKind::Class,
            };
        }
        SymbolKind::Class
    }

    /// Extract the import path from an `import_declaration` node.
    /// e.g. `import Foundation` → source="Foundation", names=["Foundation"]
    fn extract_import(node: &tree_sitter::Node, source: &[u8]) -> Option<Import> {
        let text = Self::node_text(node, source);
        // Strip leading "import" keyword and optional kind (e.g., "import class Foundation.NSString")
        let stripped = text.trim_start_matches("import").trim();
        // Drop optional import kind (class, struct, enum, func, var, let, typealias, protocol)
        let import_kinds = [
            "class",
            "struct",
            "enum",
            "func",
            "var",
            "let",
            "typealias",
            "protocol",
            "actor",
        ];
        let module_part = {
            let mut s = stripped;
            for kw in &import_kinds {
                if let Some(rest) = s.strip_prefix(kw) {
                    if rest.starts_with(|c: char| c.is_whitespace()) {
                        s = rest.trim();
                        break;
                    }
                }
            }
            s
        };

        if module_part.is_empty() {
            return None;
        }

        let name = module_part
            .rsplit('.')
            .next()
            .unwrap_or(module_part)
            .to_string();
        Some(Import {
            source: module_part.to_string(),
            names: vec![name],
        })
    }
}

impl LanguageSupport for SwiftLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_swift::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "swift"
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
                "import_declaration" => {
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

                // Swift uses `class_declaration` for class, struct, enum, actor, extension
                "class_declaration" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let visibility = Self::extract_visibility(&node, source_bytes);
                    let is_pub = visibility == Visibility::Public;
                    let kind = Self::class_declaration_kind(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if is_pub {
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

                    // Recurse into the body to find nested declarations
                    if let Some(body_node) = node.child_by_field_name("body") {
                        let mut cursor = body_node.walk();
                        for child in body_node.children(&mut cursor) {
                            stack.push(child);
                        }
                    }
                }

                "protocol_declaration" => {
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
            .set_language(&tree_sitter_swift::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_public_function() {
        let source = r#"public func greet(name: String) -> String {
    return "Hello, \(name)!"
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = SwiftLanguage;
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
    fn test_extract_struct() {
        let source = r#"public struct Point {
    var x: Double
    var y: Double
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);

        let structs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Struct)
            .collect();
        assert!(!structs.is_empty(), "expected struct symbol");
        assert_eq!(structs[0].name, "Point");
        assert_eq!(structs[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_extract_import() {
        let source = r#"import Foundation
import UIKit
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(
            result.imports.len(),
            2,
            "expected 2 imports, got {:?}",
            result.imports
        );
        let sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(
            sources.contains(&"Foundation"),
            "expected Foundation import, got: {:?}",
            sources
        );
        assert!(
            sources.contains(&"UIKit"),
            "expected UIKit import, got: {:?}",
            sources
        );
    }

    #[test]
    fn test_extract_protocol() {
        let source = "public protocol Equatable {\n    func isEqual(to other: Self) -> Bool\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);
        let protocols: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Trait)
            .collect();
        assert!(!protocols.is_empty(), "expected protocol symbol");
        assert_eq!(protocols[0].name, "Equatable");
        assert_eq!(protocols[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_extract_class() {
        let source = "public class Animal {\n    var name: String = \"\"\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
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
    fn test_extract_enum() {
        let source = "public enum Compass {\n    case north\n    case south\n    case east\n    case west\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);
        let enums: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Enum)
            .collect();
        assert!(!enums.is_empty(), "expected enum symbol");
        assert_eq!(enums[0].name, "Compass");
        assert_eq!(enums[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_extract_private_function() {
        let source = "func helper() -> Int {\n    return 42\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);
        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        assert!(!funcs.is_empty());
        assert_eq!(funcs[0].name, "helper");
        assert_eq!(funcs[0].visibility, Visibility::Private);
        assert!(
            result.exports.is_empty(),
            "internal function should not be exported"
        );
    }

    #[test]
    fn test_extract_private_protocol() {
        let source = "protocol InternalProtocol {\n    func doWork()\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);
        let protocols: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Trait)
            .collect();
        assert!(!protocols.is_empty());
        assert_eq!(protocols[0].visibility, Visibility::Private);
    }

    #[test]
    fn test_empty_source() {
        let source = "";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);
        assert!(result.symbols.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.exports.is_empty());
    }

    #[test]
    fn test_extract_function_signature_and_body() {
        let source = "public func add(a: Int, b: Int) -> Int {\n    return a + b\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);
        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        assert!(!funcs.is_empty());
        assert!(!funcs[0].signature.is_empty(), "expected signature");
        assert!(!funcs[0].body.is_empty(), "expected body");
    }

    #[test]
    fn test_extract_multiple_imports() {
        let source = "import Foundation\nimport UIKit\nimport SwiftUI\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);
        assert_eq!(result.imports.len(), 3);
    }

    #[test]
    fn test_extract_open_class() {
        let source = "open class Base {\n    open func override_me() {}\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);
        let classes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(!classes.is_empty());
        assert_eq!(
            classes[0].visibility,
            Visibility::Public,
            "open should be treated as public"
        );
    }

    #[test]
    fn test_extract_import_with_kind() {
        let source = "import class Foundation.NSString\nimport func Darwin.exit\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.imports.len(), 2, "expected 2 imports");
        let sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(
            sources.iter().any(|s| s.contains("Foundation")),
            "expected Foundation import, got: {:?}",
            sources
        );
        assert!(
            sources.iter().any(|s| s.contains("Darwin")),
            "expected Darwin import, got: {:?}",
            sources
        );
    }

    #[test]
    fn test_extract_struct_via_class_declaration() {
        let source = "public struct Point {\n    var x: Int\n    var y: Int\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);

        let structs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Struct)
            .collect();
        assert!(
            !structs.is_empty(),
            "expected struct symbol, got kinds: {:?}",
            result
                .symbols
                .iter()
                .map(|s| (&s.name, &s.kind))
                .collect::<Vec<_>>()
        );
        assert_eq!(structs[0].name, "Point");
    }

    #[test]
    fn test_import_with_kind_qualifier() {
        // "import class Foundation.NSString" should strip the kind qualifier
        let source = "import class Foundation.NSString\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.imports.len(), 1, "expected 1 import");
        // The import kind "class" should be stripped, leaving "Foundation.NSString"
        let imp = &result.imports[0];
        assert!(
            imp.source.contains("Foundation"),
            "expected Foundation in source, got: {}",
            imp.source
        );
    }

    #[test]
    fn test_import_with_func_kind() {
        let source = "import func Darwin.C.isatty\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.imports.len(), 1);
        let imp = &result.imports[0];
        assert!(
            imp.source.contains("Darwin"),
            "expected Darwin in source, got: {}",
            imp.source
        );
    }

    #[test]
    fn test_enum_declaration() {
        let source = "public enum Direction {\n    case north\n    case south\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);

        let enums: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Enum)
            .collect();
        assert!(
            !enums.is_empty(),
            "expected enum symbol, got: {:?}",
            result
                .symbols
                .iter()
                .map(|s| (&s.name, &s.kind))
                .collect::<Vec<_>>()
        );
        assert_eq!(enums[0].name, "Direction");
        assert_eq!(enums[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_function_without_body_field() {
        // A function signature in a protocol has no body — triggers fallback paths
        let source = "protocol P {\n    func doWork(x: Int) -> String\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);

        // The protocol should be extracted even though inner functions have no body
        assert!(!result.symbols.is_empty(), "expected symbols from protocol");
    }

    #[test]
    fn test_protocol_method_no_body() {
        let source = "public protocol Drawable {\n    func draw()\n    func resize(to: Int)\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);

        let protocols: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Trait)
            .collect();
        assert!(!protocols.is_empty());
        assert_eq!(protocols[0].name, "Drawable");
        assert_eq!(protocols[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_empty_import_path() {
        // Covers the empty import path returning None (line 131)
        let source = "import\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);
        // Parser may produce an error node; imports should be empty or ignored
        let _ = result;
    }

    #[test]
    fn test_class_without_declaration_kind_field() {
        // Covers the class_declaration_kind default return (line 96)
        // A plain class should hit the default SymbolKind::Class path
        let source = "class SimpleClass {\n    var x = 0\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);
        let classes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(!classes.is_empty(), "expected class symbol");
        assert_eq!(classes[0].name, "SimpleClass");
    }

    #[test]
    fn test_function_body_via_code_block() {
        // Triggers the code_block fallback in extract_fn_body (lines 57-61)
        let source = "func compute() {\n    let x = 1 + 2\n    print(x)\n}\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = SwiftLanguage;
        let result = lang.extract(source, &tree);
        let fns: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        assert!(!fns.is_empty());
    }
}
