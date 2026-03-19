use crate::parser::language::{
    Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility,
};
use tree_sitter::Language as TsLanguage;

pub struct PrismaLanguage;

impl PrismaLanguage {
    fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
        node.utf8_text(source).unwrap_or("")
    }

    fn first_line(node: &tree_sitter::Node, source: &[u8]) -> String {
        let text = Self::node_text(node, source);
        text.lines().next().unwrap_or("").trim().to_string()
    }

    /// Extract the identifier name from a declaration node.
    fn extract_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return Self::node_text(&child, source).to_string();
            }
        }
        String::new()
    }
}

impl LanguageSupport for PrismaLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_prisma_io::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "prisma"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let mut symbols: Vec<Symbol> = Vec::new();
        let imports: Vec<Import> = Vec::new();
        let exports: Vec<Export> = Vec::new();

        let mut cursor = root.walk();

        for node in root.children(&mut cursor) {
            let kind = match node.kind() {
                "model_declaration" => Some(SymbolKind::Struct),
                "enum_declaration" => Some(SymbolKind::Enum),
                "datasource_declaration" => Some(SymbolKind::Block),
                "generator_declaration" => Some(SymbolKind::Block),
                "type_declaration" => Some(SymbolKind::TypeAlias),
                _ => None,
            };

            if let Some(symbol_kind) = kind {
                let name = Self::extract_name(&node, source_bytes);
                if !name.is_empty() {
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    symbols.push(Symbol {
                        name,
                        kind: symbol_kind,
                        visibility: Visibility::Public,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
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
            .set_language(&tree_sitter_prisma_io::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_model() {
        let source = r#"model User {
  id    Int     @id @default(autoincrement())
  email String  @unique
  name  String?
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PrismaLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "User");
        assert_eq!(sym.kind, SymbolKind::Struct);
        assert_eq!(sym.visibility, Visibility::Public);
        assert!(sym.signature.contains("model User"));
        assert!(sym.body.contains("email"));
        assert_eq!(sym.start_line, 1);
        assert!(sym.end_line >= 5);
    }

    #[test]
    fn test_extract_enum() {
        let source = r#"enum Role {
  USER
  ADMIN
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PrismaLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "Role");
        assert_eq!(sym.kind, SymbolKind::Enum);
        assert_eq!(sym.visibility, Visibility::Public);
        assert!(sym.signature.contains("enum Role"));
    }

    #[test]
    fn test_extract_datasource() {
        let source = r#"datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PrismaLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "db");
        assert_eq!(sym.kind, SymbolKind::Block);
        assert_eq!(sym.visibility, Visibility::Public);
        assert!(sym.signature.contains("datasource db"));
    }

    #[test]
    fn test_extract_generator() {
        let source = r#"generator client {
  provider = "prisma-client-js"
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PrismaLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "client");
        assert_eq!(sym.kind, SymbolKind::Block);
        assert!(sym.signature.contains("generator client"));
    }

    #[test]
    fn test_extract_type_alias() {
        let source = r#"type Address {
  street String
  city   String
  zip    String
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PrismaLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "Address");
        assert_eq!(sym.kind, SymbolKind::TypeAlias);
        assert!(sym.signature.contains("type Address"));
    }

    #[test]
    fn test_empty_source() {
        let source = "";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = PrismaLanguage;
        let result = lang.extract(source, &tree);
        assert!(result.symbols.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.exports.is_empty());
    }

    #[test]
    fn test_complex_schema() {
        let source = r#"datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

generator client {
  provider = "prisma-client-js"
}

model User {
  id    Int     @id @default(autoincrement())
  email String  @unique
  name  String?
  posts Post[]
  role  Role    @default(USER)
}

model Post {
  id        Int     @id @default(autoincrement())
  title     String
  content   String?
  published Boolean @default(false)
  author    User    @relation(fields: [authorId], references: [id])
  authorId  Int
}

enum Role {
  USER
  ADMIN
}

type Address {
  street String
  city   String
  zip    String
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PrismaLanguage;
        let result = lang.extract(source, &tree);

        // datasource (Block) + generator (Block) + 2 models (Struct) + 1 enum + 1 type alias = 6
        assert_eq!(
            result.symbols.len(),
            6,
            "expected 6 symbols, got: {:?}",
            result
                .symbols
                .iter()
                .map(|s| (&s.name, &s.kind))
                .collect::<Vec<_>>()
        );

        let blocks: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Block)
            .collect();
        assert_eq!(blocks.len(), 2);

        let structs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Struct)
            .collect();
        assert_eq!(structs.len(), 2);

        let enums: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Enum)
            .collect();
        assert_eq!(enums.len(), 1);

        let type_aliases: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::TypeAlias)
            .collect();
        assert_eq!(type_aliases.len(), 1);
    }

    #[test]
    fn test_model_with_relations() {
        let source = r#"model Post {
  id        Int     @id @default(autoincrement())
  title     String
  author    User    @relation(fields: [authorId], references: [id])
  authorId  Int
  tags      Tag[]   @relation("PostTags")
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PrismaLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "Post");
        assert_eq!(sym.kind, SymbolKind::Struct);
        // Fields are NOT extracted as separate symbols
        assert!(sym.body.contains("@relation"));
    }

    #[test]
    fn test_multiple_models() {
        let source = r#"model User {
  id   Int    @id
  name String
}

model Post {
  id    Int    @id
  title String
}

model Comment {
  id   Int    @id
  text String
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PrismaLanguage;
        let result = lang.extract(source, &tree);

        let models: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Struct)
            .collect();
        assert_eq!(models.len(), 3);

        let names: Vec<&str> = models.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"User"));
        assert!(names.contains(&"Post"));
        assert!(names.contains(&"Comment"));
    }

    #[test]
    fn test_no_imports_exports() {
        let source = r#"model User {
  id   Int    @id
  name String
}

enum Role {
  ADMIN
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PrismaLanguage;
        let result = lang.extract(source, &tree);

        assert!(
            result.imports.is_empty(),
            "Prisma should never have imports"
        );
        assert!(
            result.exports.is_empty(),
            "Prisma should never have exports"
        );
    }

    #[test]
    fn test_model_name_extraction() {
        let source = r#"model MyVeryLongModelName {
  id Int @id
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PrismaLanguage;
        let result = lang.extract(source, &tree);

        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "MyVeryLongModelName");
    }

    #[test]
    fn test_all_symbol_kinds() {
        let source = r#"datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

generator client {
  provider = "prisma-client-js"
}

model User {
  id Int @id
}

enum Role {
  ADMIN
}

type Address {
  street String
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = PrismaLanguage;
        let result = lang.extract(source, &tree);

        // Verify each declaration type maps to the correct SymbolKind
        let datasource = result
            .symbols
            .iter()
            .find(|s| s.name == "db")
            .expect("datasource not found");
        assert_eq!(datasource.kind, SymbolKind::Block);

        let generator = result
            .symbols
            .iter()
            .find(|s| s.name == "client")
            .expect("generator not found");
        assert_eq!(generator.kind, SymbolKind::Block);

        let model = result
            .symbols
            .iter()
            .find(|s| s.name == "User")
            .expect("model not found");
        assert_eq!(model.kind, SymbolKind::Struct);

        let enum_sym = result
            .symbols
            .iter()
            .find(|s| s.name == "Role")
            .expect("enum not found");
        assert_eq!(enum_sym.kind, SymbolKind::Enum);

        let type_alias = result
            .symbols
            .iter()
            .find(|s| s.name == "Address")
            .expect("type alias not found");
        assert_eq!(type_alias.kind, SymbolKind::TypeAlias);

        // All should be public
        for sym in &result.symbols {
            assert_eq!(sym.visibility, Visibility::Public);
        }
    }
}
