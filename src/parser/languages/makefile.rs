use crate::parser::language::{
    Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility,
};
use tree_sitter::Language as TsLanguage;

pub struct MakefileLanguage;

impl MakefileLanguage {
    fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
        node.utf8_text(source).unwrap_or("")
    }

    fn first_line(node: &tree_sitter::Node, source: &[u8]) -> String {
        let text = Self::node_text(node, source);
        text.lines().next().unwrap_or("").trim().to_string()
    }

    /// Extract the target name(s) from a rule node.
    /// The target is the part before the colon.
    fn extract_target_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "targets" {
                return Self::node_text(&child, source).trim().to_string();
            }
        }
        // Fallback: parse from the first line (text before colon)
        let first = Self::first_line(node, source);
        if let Some(colon_idx) = first.find(':') {
            let target = first[..colon_idx].trim();
            if !target.is_empty() {
                return target.to_string();
            }
        }
        String::new()
    }

    /// Extract the variable name from a variable_assignment node.
    fn extract_variable_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "word" {
                return Self::node_text(&child, source).to_string();
            }
        }
        String::new()
    }

    /// Extract include path from an include directive.
    fn extract_include_path(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        let text = Self::node_text(node, source).trim().to_string();
        // Strip the "include" or "-include" prefix
        let after = text
            .strip_prefix("-include")
            .or_else(|| text.strip_prefix("include"))
            .unwrap_or("")
            .trim();
        if after.is_empty() {
            None
        } else {
            Some(after.to_string())
        }
    }
}

impl LanguageSupport for MakefileLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_make::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "makefile"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let mut symbols: Vec<Symbol> = Vec::new();
        let mut imports: Vec<Import> = Vec::new();
        let exports: Vec<Export> = Vec::new();

        let mut cursor = root.walk();

        for node in root.children(&mut cursor) {
            match node.kind() {
                "rule" => {
                    let name = Self::extract_target_name(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if !name.is_empty() {
                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Target,
                            visibility: Visibility::Public,
                            signature,
                            body,
                            start_line,
                            end_line,
                        });
                    }
                }

                "variable_assignment" => {
                    let name = Self::extract_variable_name(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if !name.is_empty() {
                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Variable,
                            visibility: Visibility::Public,
                            signature,
                            body,
                            start_line,
                            end_line,
                        });
                    }
                }

                "include_directive" => {
                    if let Some(path) = Self::extract_include_path(&node, source_bytes) {
                        let short_name = path.rsplit('/').next().unwrap_or(&path).to_string();
                        imports.push(Import {
                            source: path,
                            names: vec![short_name],
                        });
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
            .set_language(&tree_sitter_make::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_rules() {
        let source = "build:\n\tcargo build\n\ntest:\n\tcargo test\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = MakefileLanguage;
        let result = lang.extract(source, &tree);

        let targets: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Target)
            .collect();
        assert!(
            targets.len() >= 2,
            "expected at least 2 targets, got: {:?}",
            targets.iter().map(|t| &t.name).collect::<Vec<_>>()
        );
        assert_eq!(targets[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_extract_variables() {
        let source = "CC = gcc\nCFLAGS = -Wall -O2\n\nall:\n\t$(CC) $(CFLAGS) main.c\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = MakefileLanguage;
        let result = lang.extract(source, &tree);

        let vars: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Variable)
            .collect();
        assert!(
            vars.len() >= 2,
            "expected at least 2 variables, got: {:?}",
            vars.iter().map(|v| &v.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_empty_source() {
        let source = "";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = MakefileLanguage;
        let result = lang.extract(source, &tree);
        assert!(result.symbols.is_empty());
        assert!(result.imports.is_empty());
    }

    #[test]
    fn test_complex_makefile() {
        let source = "CC = gcc\nCFLAGS = -Wall\nSRC = main.c utils.c\n\n.PHONY: all clean\n\nall: $(SRC)\n\t$(CC) $(CFLAGS) -o app $(SRC)\n\nclean:\n\trm -f app\n\ninstall: all\n\tcp app /usr/local/bin/\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = MakefileLanguage;
        let result = lang.extract(source, &tree);

        let targets: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Target)
            .collect();
        assert!(
            targets.len() >= 2,
            "expected multiple targets, got: {:?}",
            targets.iter().map(|t| &t.name).collect::<Vec<_>>()
        );

        let vars: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Variable)
            .collect();
        assert!(
            vars.len() >= 2,
            "expected multiple variables, got: {:?}",
            vars.iter().map(|v| &v.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_coverage_include_directive() {
        let source = "include config.mk\n\nall:\n\techo done\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = MakefileLanguage;
        let result = lang.extract(source, &tree);

        assert!(
            !result.imports.is_empty(),
            "expected include directive as import, got: {:?}",
            result.imports
        );
        let inc = result.imports.iter().find(|i| {
            i.source.contains("config.mk") || i.names.iter().any(|n| n.contains("config"))
        });
        assert!(inc.is_some(), "expected config.mk include import");
    }

    #[test]
    fn test_coverage_phony_targets() {
        let source = ".PHONY: all clean test\n\nall:\n\techo all\n\nclean:\n\trm -f out\n\ntest:\n\techo test\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = MakefileLanguage;
        let result = lang.extract(source, &tree);

        let targets: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Target)
            .collect();
        assert!(
            targets.len() >= 3,
            "expected at least 3 targets (all, clean, test), got: {:?}",
            targets.iter().map(|t| &t.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_coverage_variable_assignment_types() {
        let source = "CC := gcc\nOPT ?= -O2\nFLAGS += -Wall\nSRC = main.c\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = MakefileLanguage;
        let result = lang.extract(source, &tree);

        let vars: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Variable)
            .collect();
        assert!(
            vars.len() >= 2,
            "expected variables from different assignment types, got: {:?}",
            vars.iter().map(|v| &v.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_coverage_multiple_targets_with_deps() {
        let source =
            "build: src/main.c src/utils.c\n\t$(CC) -o app $^\n\ntest: build\n\t./app --test\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = MakefileLanguage;
        let result = lang.extract(source, &tree);

        let targets: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Target)
            .collect();
        assert!(
            targets.len() >= 2,
            "expected at least 2 targets, got: {:?}",
            targets.iter().map(|t| &t.name).collect::<Vec<_>>()
        );
        // All targets should be public
        for t in &targets {
            assert_eq!(t.visibility, Visibility::Public);
        }
    }

    #[test]
    fn test_dash_include_directive() {
        // -include (optional include) should also be extracted as import
        let source = "-include optional.mk\n\nall:\n\techo done\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = MakefileLanguage;
        let result = lang.extract(source, &tree);

        assert!(
            !result.imports.is_empty(),
            "expected -include directive as import"
        );
        let inc = result
            .imports
            .iter()
            .find(|i| i.source.contains("optional"));
        assert!(inc.is_some(), "expected optional.mk include import");
    }

    #[test]
    fn test_include_with_subdirectory() {
        // include with subdirectory path
        let source = "include config/base.mk\n\nall:\n\techo done\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = MakefileLanguage;
        let result = lang.extract(source, &tree);

        assert!(!result.imports.is_empty(), "expected include import");
        let inc = &result.imports[0];
        assert!(inc.source.contains("config/base.mk"));
        // Short name should be the filename
        assert!(
            inc.names.iter().any(|n| n == "base.mk"),
            "expected short name base.mk, got: {:?}",
            inc.names
        );
    }

    #[test]
    fn test_extract_target_name_fallback() {
        // Exercise the fallback in extract_target_name (colon parsing)
        // Call directly on a non-rule node
        let source = "build:\n\techo build\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        // Call on the root node which doesn't have targets child
        let name = MakefileLanguage::extract_target_name(&root, source.as_bytes());
        // Root has "build:" on first line, so colon fallback should extract "build"
        assert_eq!(name, "build");
    }

    #[test]
    fn test_extract_variable_name_empty() {
        // Exercise extract_variable_name on a node with no word child
        let source = "build:\n\techo build\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        // Call on root which has no word child (its children are rule nodes)
        let name = MakefileLanguage::extract_variable_name(&root, source.as_bytes());
        assert!(
            name.is_empty(),
            "root should have no word child for variable name"
        );
    }

    #[test]
    fn test_extract_include_path_empty() {
        // Exercise extract_include_path returning None
        // Node text that doesn't start with include or -include
        let source = "build:\n\techo build\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let rule = root.child(0).unwrap();
        let result = MakefileLanguage::extract_include_path(&rule, source.as_bytes());
        // "build:" doesn't match include/- include prefix, so after = ""
        assert!(result.is_none(), "non-include node should return None");
    }

    #[test]
    fn test_comment_lines_ignored() {
        // Comments should not produce symbols
        let source = "# This is a comment\nCC = gcc\n\nall:\n\techo done\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = MakefileLanguage;
        let result = lang.extract(source, &tree);

        // Should have variable and target, no comment symbol
        assert!(
            result.symbols.iter().all(|s| !s.name.starts_with('#')),
            "comments should not produce symbols"
        );
    }
}
