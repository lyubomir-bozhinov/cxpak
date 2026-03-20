use crate::parser::language::{
    Export, Import, LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility,
};
use tree_sitter::Language as TsLanguage;

pub struct GraphqlLanguage;

impl GraphqlLanguage {
    fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
        node.utf8_text(source).unwrap_or("")
    }

    fn first_line(node: &tree_sitter::Node, source: &[u8]) -> String {
        let text = Self::node_text(node, source);
        text.lines().next().unwrap_or("").trim().to_string()
    }

    /// Extract the name from a node by looking for a `name` child.
    fn extract_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "name" {
                return Self::node_text(&child, source).to_string();
            }
        }
        String::new()
    }

    /// Detect the operation type from an operation_definition node.
    fn detect_operation_type(node: &tree_sitter::Node, source: &[u8]) -> SymbolKind {
        // Look for an operation_type child node
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "operation_type" {
                let text = Self::node_text(&child, source);
                return match text {
                    "mutation" => SymbolKind::Mutation,
                    "subscription" => SymbolKind::Query,
                    _ => SymbolKind::Query, // "query" or default
                };
            }
        }
        // Default: anonymous query
        SymbolKind::Query
    }
}

impl LanguageSupport for GraphqlLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_graphql::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "graphql"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let mut symbols: Vec<Symbol> = Vec::new();
        let imports: Vec<Import> = Vec::new();
        let exports: Vec<Export> = Vec::new();

        // tree-sitter-graphql wraps everything:
        // document -> definition -> type_system_definition/executable_definition -> actual node.
        let mut stack: Vec<tree_sitter::Node> = Vec::new();
        {
            let mut cursor = root.walk();
            for child in root.children(&mut cursor) {
                stack.push(child);
            }
        }

        while let Some(node) = stack.pop() {
            let kind_str = node.kind();
            match kind_str {
                // Wrapper nodes -- drill into children
                "document"
                | "definition"
                | "type_system_definition"
                | "type_system_extension"
                | "executable_definition"
                | "type_extension"
                | "type_definition" => {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        stack.push(child);
                    }
                }

                "object_type_definition" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if !name.is_empty() {
                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Type,
                            visibility: Visibility::Public,
                            signature,
                            body,
                            start_line,
                            end_line,
                        });
                    }
                }

                "operation_definition" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let kind = Self::detect_operation_type(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    symbols.push(Symbol {
                        name: if name.is_empty() {
                            "anonymous_operation".to_string()
                        } else {
                            name
                        },
                        kind,
                        visibility: Visibility::Public,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "enum_type_definition" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    symbols.push(Symbol {
                        name: if name.is_empty() {
                            "anonymous_enum".to_string()
                        } else {
                            name
                        },
                        kind: SymbolKind::Enum,
                        visibility: Visibility::Public,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "interface_type_definition" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    symbols.push(Symbol {
                        name: if name.is_empty() {
                            "anonymous_interface".to_string()
                        } else {
                            name
                        },
                        kind: SymbolKind::Interface,
                        visibility: Visibility::Public,
                        signature,
                        body,
                        start_line,
                        end_line,
                    });
                }

                "scalar_type_definition"
                | "union_type_definition"
                | "input_object_type_definition" => {
                    let name = Self::extract_name(&node, source_bytes);
                    let signature = Self::first_line(&node, source_bytes);
                    let body = Self::node_text(&node, source_bytes).to_string();
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;

                    if !name.is_empty() {
                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Type,
                            visibility: Visibility::Public,
                            signature,
                            body,
                            start_line,
                            end_line,
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
            .set_language(&tree_sitter_graphql::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    #[test]
    fn test_extract_type_definitions() {
        let source = r#"type User {
  id: ID!
  name: String!
  email: String
}

type Post {
  id: ID!
  title: String!
  author: User!
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);

        let types: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Type)
            .collect();
        assert!(types.len() >= 2, "expected at least 2 types (User, Post)");
        assert_eq!(types[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_extract_operations() {
        let source = r#"query GetUser($id: ID!) {
  user(id: $id) {
    name
    email
  }
}

mutation CreateUser($input: CreateUserInput!) {
  createUser(input: $input) {
    id
    name
  }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);

        let queries: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Query)
            .collect();
        assert!(!queries.is_empty(), "expected query operation");

        let mutations: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Mutation)
            .collect();
        assert!(!mutations.is_empty(), "expected mutation operation");
    }

    #[test]
    fn test_empty_source() {
        let source = "";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).unwrap();
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);
        assert!(result.symbols.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.exports.is_empty());
    }

    #[test]
    fn test_complex_schema() {
        let source = r#"type Query {
  users: [User!]!
  user(id: ID!): User
}

type Mutation {
  createUser(input: CreateUserInput!): User!
  deleteUser(id: ID!): Boolean!
}

enum Role {
  ADMIN
  USER
  GUEST
}

interface Node {
  id: ID!
}

type User implements Node {
  id: ID!
  name: String!
  role: Role!
}

input CreateUserInput {
  name: String!
  email: String!
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);

        assert!(result.symbols.len() >= 4, "expected multiple symbols");

        let enums: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Enum)
            .collect();
        assert!(!enums.is_empty(), "expected enum type");
    }

    #[test]
    fn test_coverage_enum_type() {
        let source = r#"enum Status {
  ACTIVE
  INACTIVE
  PENDING
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);

        let enums: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Enum)
            .collect();
        assert!(!enums.is_empty(), "expected enum symbol");
        assert_eq!(enums[0].name, "Status");
        assert_eq!(enums[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_coverage_interface_type() {
        let source = r#"interface Node {
  id: ID!
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);

        let interfaces: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Interface)
            .collect();
        assert!(!interfaces.is_empty(), "expected interface symbol");
        assert_eq!(interfaces[0].name, "Node");
    }

    #[test]
    fn test_coverage_input_type() {
        let source = r#"input CreateUserInput {
  name: String!
  email: String!
  role: String
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);

        let inputs: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Type && s.name == "CreateUserInput")
            .collect();
        assert!(!inputs.is_empty(), "expected input type symbol");
    }

    #[test]
    fn test_coverage_scalar_type() {
        let source = "scalar DateTime\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);

        let scalars: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.name == "DateTime")
            .collect();
        assert!(!scalars.is_empty(), "expected scalar type symbol");
    }

    #[test]
    fn test_coverage_union_type() {
        let source = "union SearchResult = User | Post | Comment\n";
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);

        let unions: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.name == "SearchResult")
            .collect();
        assert!(!unions.is_empty(), "expected union type symbol");
    }

    #[test]
    fn test_coverage_subscription() {
        let source = r#"subscription OnMessageAdded {
  messageAdded {
    id
    content
  }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);

        assert!(!result.symbols.is_empty(), "expected subscription symbol");
        let sub = result.symbols.iter().find(|s| s.name == "OnMessageAdded");
        assert!(sub.is_some(), "expected OnMessageAdded symbol");
        // Subscriptions map to Query kind
        if let Some(s) = sub {
            assert_eq!(s.kind, SymbolKind::Query);
        }
    }

    #[test]
    fn test_coverage_mutation_standalone() {
        let source = r#"mutation DeleteUser($id: ID!) {
  deleteUser(id: $id) {
    success
  }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);

        let mutations: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Mutation)
            .collect();
        assert!(!mutations.is_empty(), "expected mutation symbol");
        assert_eq!(mutations[0].name, "DeleteUser");
    }

    #[test]
    fn test_coverage_fragment() {
        let source = r#"fragment UserFields on User {
  id
  name
  email
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GraphqlLanguage;
        let _result = lang.extract(source, &tree);
        // Fragments are not extracted as symbols -- just ensure no panic
    }

    #[test]
    fn test_coverage_anonymous_query() {
        let source = r#"query {
  users {
    id
    name
  }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);

        assert!(
            !result.symbols.is_empty(),
            "expected anonymous query symbol"
        );
        let anon = result
            .symbols
            .iter()
            .find(|s| s.name == "anonymous_operation");
        assert!(anon.is_some(), "expected anonymous_operation name");
    }

    #[test]
    fn test_coverage_detect_operation_type_mutation() {
        let source = r#"mutation CreatePost($input: PostInput!) {
  createPost(input: $input) {
    id
  }
}
"#;
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);

        let has_mutation = result
            .symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Mutation);
        assert!(has_mutation, "expected Mutation kind");
    }

    #[test]
    fn test_extract_name_no_name_child() {
        // Test extract_name on a node with no name child
        let mut parser = make_parser();
        let source = "scalar DateTime\n";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        // Root is "document" -- extract_name on it should return ""
        let name = GraphqlLanguage::extract_name(&root, source.as_bytes());
        assert!(name.is_empty(), "document node has no name child");
    }

    #[test]
    fn test_detect_operation_type_default() {
        // Test detect_operation_type when there is no operation_type child
        let mut parser = make_parser();
        // A bare selection set { users { id } } doesn't have an operation_type
        let source = "{ users { id } }\n";
        let tree = parser.parse(source, None).unwrap();
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);
        // Should default to Query
        if !result.symbols.is_empty() {
            assert_eq!(result.symbols[0].kind, SymbolKind::Query);
        }
    }

    #[test]
    fn test_first_line_helper() {
        let mut parser = make_parser();
        let source = "type User {\n  id: ID!\n  name: String!\n}\n";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        // Walk down to the object_type_definition
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if n.kind() == "object_type_definition" {
                let fl = GraphqlLanguage::first_line(&n, source.as_bytes());
                assert_eq!(fl, "type User {");
                return;
            }
            let mut c = n.walk();
            for child in n.children(&mut c) {
                stack.push(child);
            }
        }
        panic!("did not find object_type_definition node");
    }

    #[test]
    fn test_object_type_empty_name() {
        // An object type definition whose extract_name returns empty
        // should not produce a symbol
        let mut parser = make_parser();
        // tree-sitter-graphql might parse this oddly, but it exercises the empty-name path
        let source = "type {\n}\n";
        let tree = parser.parse(source, None).unwrap();
        let lang = GraphqlLanguage;
        let result = lang.extract(source, &tree);
        // If name is empty, the symbol should not be added
        let unnamed = result
            .symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Type && s.name.is_empty());
        assert!(!unnamed, "empty-name type should not be added");
    }
}
