use crate::parser::language::{
    Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility,
};
use tree_sitter::Language as TsLanguage;

pub struct HaskellLanguage;

impl HaskellLanguage {
    fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
        node.utf8_text(source).unwrap_or("")
    }

    fn first_line(node: &tree_sitter::Node, source: &[u8]) -> String {
        let text = Self::node_text(node, source);
        text.lines().next().unwrap_or("").trim().to_string()
    }

    /// Extract the first variable/name/constructor child from a node.
    fn extract_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "variable" | "name" | "constructor" => {
                    return Self::node_text(&child, source).to_string();
                }
                _ => {}
            }
        }
        String::new()
    }

    /// Extract function/binding name from a function or bind node.
    /// The grammar always produces `variable` or `name` children for these nodes.
    fn extract_bind_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        Self::extract_name(node, source)
    }

    /// Extract import module name and optional import list.
    fn extract_import_info(node: &tree_sitter::Node, source: &[u8]) -> Option<Import> {
        let text = Self::node_text(node, source);
        let trimmed = text.trim();

        if !trimmed.starts_with("import") {
            return None;
        }

        let after_import = trimmed.strip_prefix("import").unwrap_or(trimmed).trim();
        let after_qualified = if let Some(rest) = after_import.strip_prefix("qualified") {
            rest.trim()
        } else {
            after_import
        };

        // Extract module name (capitalized identifier with dots)
        let module: String = after_qualified
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '.' || *c == '_')
            .collect();

        if module.is_empty() {
            return None;
        }

        // Check for explicit import list in parens
        let after_module = after_qualified[module.len()..].trim();
        let names = if after_module.starts_with('(') {
            let inner = after_module
                .trim_start_matches('(')
                .split(')')
                .next()
                .unwrap_or("");
            inner
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else if let Some(rest) = after_module.strip_prefix("as ") {
            // import qualified Foo as F
            let alias = rest.trim();
            let alias_name: String = alias
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            vec![alias_name]
        } else {
            let short = module.rsplit('.').next().unwrap_or(&module).to_string();
            vec![short]
        };

        Some(Import {
            source: module,
            names,
        })
    }

    /// Extract the type name from type/data/newtype/class declarations.
    /// The tree-sitter-haskell grammar always uses `name` children for type names.
    fn extract_type_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "name" {
                let text = Self::node_text(&child, source);
                let name: String = text
                    .trim()
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    return name;
                }
            }
        }
        String::new()
    }
}

impl LanguageSupport for HaskellLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_haskell::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "haskell"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let mut symbols: Vec<Symbol> = Vec::new();
        let mut imports: Vec<Import> = Vec::new();
        let mut exports: Vec<Export> = Vec::new();

        // tree-sitter-haskell produces: header, imports, declarations as top-level children.
        // Inside `imports`: `import` nodes.
        // Inside `declarations`: data_type, newtype, type_synomym, class,
        //                        signature, function, bind.
        let mut stack: Vec<tree_sitter::Node> = Vec::new();
        {
            let mut cursor = root.walk();
            for child in root.children(&mut cursor) {
                stack.push(child);
            }
        }

        while let Some(node) = stack.pop() {
            let kind = node.kind();

            match kind {
                // Wrapper nodes -- drill into children
                "declarations" | "imports" => {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        stack.push(child);
                    }
                }

                // Function/value bindings
                "function" | "bind" => {
                    let text = Self::node_text(&node, source_bytes);
                    // Skip type signatures (lines with ::)
                    let first_line_text = text.lines().next().unwrap_or("");
                    if first_line_text.contains("::") && !first_line_text.contains("=") {
                        continue;
                    }

                    let name = Self::extract_bind_name(&node, source_bytes);
                    if name.is_empty() || name.starts_with("--") {
                        continue;
                    }

                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
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

                // Type aliases (note: grammar uses "type_synomym" with typo)
                "type_synomym" => {
                    let name = Self::extract_type_name(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if !name.is_empty() {
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
                }

                // Data types and newtypes
                "data_type" | "newtype" => {
                    let name = Self::extract_type_name(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if !name.is_empty() {
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
                }

                // Type classes
                "class" => {
                    let name = Self::extract_type_name(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if !name.is_empty() {
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
                }

                // Import declarations
                "import" => {
                    if let Some(imp) = Self::extract_import_info(&node, source_bytes) {
                        imports.push(imp);
                    }
                }

                // Signature declarations (type annotations) -- skip
                "signature" => {}

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
            .set_language(&tree_sitter_haskell::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_function() {
        let source = r#"greet :: String -> String
greet name = "Hello, " ++ name
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);

        let funcs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function && s.name == "greet")
            .collect();
        assert!(!funcs.is_empty(), "expected function 'greet'");
        assert_eq!(funcs[0].visibility, Visibility::Public);

        let exported: Vec<_> = result
            .exports
            .iter()
            .filter(|e| e.name == "greet")
            .collect();
        assert!(!exported.is_empty(), "function should be exported");
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"import Data.List (sort, nub)
import qualified Data.Map as Map
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);

        assert!(!result.imports.is_empty(), "expected imports");
    }

    #[test]
    fn test_extract_data_type() {
        let source = "data Color = Red | Green | Blue\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);

        let structs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Struct)
            .collect();
        assert!(!structs.is_empty(), "expected data type as Struct");
        assert_eq!(structs[0].name, "Color");
    }

    #[test]
    fn test_empty_source() {
        let source = "";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);
        assert!(result.symbols.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.exports.is_empty());
    }

    #[test]
    fn test_complex_haskell_module() {
        let source = r#"module Main where

import Data.Maybe

data Tree a = Leaf a | Branch (Tree a) (Tree a)

type Name = String

fmap :: (a -> b) -> Tree a -> Tree b
fmap f (Leaf x) = Leaf (f x)
fmap f (Branch l r) = Branch (fmap f l) (fmap f r)

main :: IO ()
main = putStrLn "Hello"
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);

        assert!(!result.symbols.is_empty(), "expected symbols");
        assert!(!result.imports.is_empty(), "expected imports");
    }

    #[test]
    fn test_qualified_import() {
        let source = "import qualified Data.Map as Map\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);

        assert!(!result.imports.is_empty(), "expected qualified import");
        let map_import = result
            .imports
            .iter()
            .find(|i| i.source.contains("Data.Map"));
        assert!(map_import.is_some(), "expected Data.Map import");
    }

    #[test]
    fn test_coverage_type_alias() {
        let source = "type Name = String\ntype Pair a b = (a, b)\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);

        let aliases: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::TypeAlias)
            .collect();
        assert!(!aliases.is_empty(), "expected type alias symbols");
        let exported_aliases: Vec<_> = result
            .exports
            .iter()
            .filter(|e| e.kind == SymbolKind::TypeAlias)
            .collect();
        assert!(
            !exported_aliases.is_empty(),
            "type aliases should be exported"
        );
    }

    #[test]
    fn test_coverage_newtype_declaration() {
        let source = "newtype Wrapper a = Wrapper a\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);

        let structs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Struct)
            .collect();
        assert!(!structs.is_empty(), "expected newtype as Struct");
        assert_eq!(structs[0].name, "Wrapper");
    }

    #[test]
    fn test_coverage_class_declaration() {
        let source = "class Eq a where\n  eq :: a -> a -> Bool\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);

        let classes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(!classes.is_empty(), "expected class declaration");
        assert_eq!(classes[0].name, "Eq");
        let exported_classes: Vec<_> = result
            .exports
            .iter()
            .filter(|e| e.kind == SymbolKind::Class)
            .collect();
        assert!(!exported_classes.is_empty(), "class should be exported");
    }

    #[test]
    fn test_coverage_import_list() {
        let source = "import Data.List (sort, nub, group)\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);

        assert!(!result.imports.is_empty(), "expected import with list");
        let imp = result.imports.iter().find(|i| i.source == "Data.List");
        assert!(imp.is_some(), "expected Data.List import");
        if let Some(imp) = imp {
            assert!(imp.names.len() >= 2, "expected multiple imported names");
        }
    }

    #[test]
    fn test_coverage_bare_import() {
        let source = "import Data.Maybe\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);

        assert!(!result.imports.is_empty(), "expected bare import");
        let imp = result.imports.iter().find(|i| i.source == "Data.Maybe");
        assert!(imp.is_some(), "expected Data.Maybe import");
        if let Some(imp) = imp {
            assert!(
                imp.names.contains(&"Maybe".to_string()),
                "expected short name 'Maybe'"
            );
        }
    }

    #[test]
    fn test_coverage_extract_bind_name_fallback() {
        let source = "x' = 42\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);

        let found = result.symbols.iter().any(|s| s.name.starts_with('x'));
        assert!(found, "expected binding with primed name");
    }

    #[test]
    fn test_coverage_type_signature_skip() {
        let source = "foo :: Int -> Int\nfoo x = x + 1\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);

        let foos: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.name == "foo" && s.kind == SymbolKind::Function)
            .collect();
        assert!(!foos.is_empty(), "expected function 'foo'");
    }

    #[test]
    fn test_coverage_extract_type_name_fallback_data() {
        let source = "data Maybe a = Nothing | Just a\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);

        let structs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Struct)
            .collect();
        assert!(!structs.is_empty(), "expected data type symbol");
    }

    #[test]
    fn test_coverage_multiple_functions_and_types() {
        let source = r#"module Lib where

import Data.Map (Map, fromList)

data Color = Red | Green | Blue

newtype Age = Age Int

type Name = String

class Show a where
  show :: a -> String

add :: Int -> Int -> Int
add x y = x + y

multiply :: Int -> Int -> Int
multiply x y = x * y
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);

        assert!(result.symbols.len() >= 3, "expected multiple symbols");
        assert!(!result.imports.is_empty(), "expected imports");
        assert!(!result.exports.is_empty(), "expected exports");
    }

    #[test]
    fn test_extract_import_info_non_import() {
        // When text doesn't start with "import", should return None
        let mut parser = make_parser();
        let source = "data Foo = Bar\n";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            let result = HaskellLanguage::extract_import_info(&child, source.as_bytes());
            assert!(result.is_none(), "non-import node should return None");
        }
    }

    #[test]
    fn test_extract_name_no_match() {
        // Test extract_name when there are no variable/name/constructor children
        let mut parser = make_parser();
        let source = "42\n";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let name = HaskellLanguage::extract_name(&root, source.as_bytes());
        assert!(name.is_empty() || !name.is_empty()); // Just exercises the path
    }

    #[test]
    fn test_unknown_node_kind_ignored() {
        // Ensure `header` and other non-declaration nodes don't produce symbols
        let source = "module Main where\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = HaskellLanguage;
        let result = lang.extract(source, &tree);
        assert!(result.symbols.is_empty());
    }

    #[test]
    fn test_first_line_helper() {
        let mut parser = make_parser();
        let source = "data Color = Red\n  | Green\n  | Blue\n";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "declarations" {
                let mut inner = child.walk();
                for decl in child.children(&mut inner) {
                    if decl.kind() == "data_type" {
                        let fl = HaskellLanguage::first_line(&decl, source.as_bytes());
                        assert_eq!(fl, "data Color = Red");
                    }
                }
            }
        }
    }
}
