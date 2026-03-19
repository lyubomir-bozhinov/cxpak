use crate::parser::language::{LanguageSupport, ParseResult, Symbol, SymbolKind, Visibility};
use tree_sitter::Language as TsLanguage;

pub struct SqlLanguage;

impl SqlLanguage {
    fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
        node.utf8_text(source).unwrap_or("")
    }

    fn first_line(node: &tree_sitter::Node, source: &[u8]) -> String {
        let text = Self::node_text(node, source);
        text.lines().next().unwrap_or("").trim().to_string()
    }

    /// Extract the first `object_reference` child's text as the name.
    /// Used for CREATE TABLE, CREATE VIEW, CREATE FUNCTION, CREATE TRIGGER, ALTER TABLE.
    fn extract_object_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "object_reference" {
                return Self::node_text(&child, source).to_string();
            }
        }
        String::new()
    }

    /// Extract the first direct `identifier` child's text as the name.
    /// Used for CREATE INDEX where the index name is a bare identifier, not object_reference.
    fn extract_identifier_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return Self::node_text(&child, source).to_string();
            }
        }
        String::new()
    }

    fn push_symbol(
        symbols: &mut Vec<Symbol>,
        name: String,
        kind: SymbolKind,
        signature: String,
        body: String,
        start_line: usize,
        end_line: usize,
    ) {
        if !name.is_empty() {
            symbols.push(Symbol {
                name,
                kind,
                visibility: Visibility::Public,
                signature,
                body,
                start_line,
                end_line,
            });
        }
    }

    fn process_node(node: &tree_sitter::Node, source: &[u8], symbols: &mut Vec<Symbol>) {
        match node.kind() {
            "create_table" => {
                let name = Self::extract_object_name(node, source);
                let signature = Self::first_line(node, source);
                let body = Self::node_text(node, source).to_string();
                let start_line = node.start_position().row + 1;
                let end_line = node.end_position().row + 1;
                Self::push_symbol(
                    symbols,
                    name,
                    SymbolKind::Struct,
                    signature,
                    body,
                    start_line,
                    end_line,
                );
            }

            "create_view" => {
                let name = Self::extract_object_name(node, source);
                let signature = Self::first_line(node, source);
                let body = Self::node_text(node, source).to_string();
                let start_line = node.start_position().row + 1;
                let end_line = node.end_position().row + 1;
                Self::push_symbol(
                    symbols,
                    name,
                    SymbolKind::TypeAlias,
                    signature,
                    body,
                    start_line,
                    end_line,
                );
            }

            "create_index" => {
                let name = Self::extract_identifier_name(node, source);
                let signature = Self::first_line(node, source);
                let body = Self::node_text(node, source).to_string();
                let start_line = node.start_position().row + 1;
                let end_line = node.end_position().row + 1;
                Self::push_symbol(
                    symbols,
                    name,
                    SymbolKind::Constant,
                    signature,
                    body,
                    start_line,
                    end_line,
                );
            }

            "create_function" => {
                let name = Self::extract_object_name(node, source);
                let signature = Self::first_line(node, source);
                let body = Self::node_text(node, source).to_string();
                let start_line = node.start_position().row + 1;
                let end_line = node.end_position().row + 1;
                Self::push_symbol(
                    symbols,
                    name,
                    SymbolKind::Function,
                    signature,
                    body,
                    start_line,
                    end_line,
                );
            }

            "create_trigger" => {
                let name = Self::extract_object_name(node, source);
                let signature = Self::first_line(node, source);
                let body = Self::node_text(node, source).to_string();
                let start_line = node.start_position().row + 1;
                let end_line = node.end_position().row + 1;
                Self::push_symbol(
                    symbols,
                    name,
                    SymbolKind::Function,
                    signature,
                    body,
                    start_line,
                    end_line,
                );
            }

            "alter_table" => {
                let name = Self::extract_object_name(node, source);
                let signature = Self::first_line(node, source);
                let body = Self::node_text(node, source).to_string();
                let start_line = node.start_position().row + 1;
                let end_line = node.end_position().row + 1;
                Self::push_symbol(
                    symbols,
                    name,
                    SymbolKind::Struct,
                    signature,
                    body,
                    start_line,
                    end_line,
                );
            }

            // Drill through statement wrappers
            "statement" | "block" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    Self::process_node(&child, source, symbols);
                }
            }

            _ => {}
        }
    }
}

impl LanguageSupport for SqlLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_sequel::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "sql"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let source_bytes = source.as_bytes();
        let root = tree.root_node();

        let mut symbols: Vec<Symbol> = Vec::new();

        let mut cursor = root.walk();
        for node in root.children(&mut cursor) {
            Self::process_node(&node, source_bytes, &mut symbols);
        }

        ParseResult {
            symbols,
            imports: Vec::new(),
            exports: Vec::new(),
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
            .set_language(&tree_sitter_sequel::LANGUAGE.into())
            .expect("failed to set language");
        parser
    }

    fn parse_and_extract(source: &str) -> ParseResult {
        let mut parser = make_parser();
        let tree = parser.parse(source, None).expect("parse failed");
        let lang = SqlLanguage;
        lang.extract(source, &tree)
    }

    #[test]
    fn test_extract_create_table() {
        let source = r#"CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT UNIQUE
);"#;
        let result = parse_and_extract(source);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "users");
        assert_eq!(sym.kind, SymbolKind::Struct);
        assert_eq!(sym.visibility, Visibility::Public);
        assert!(
            sym.signature.contains("CREATE TABLE"),
            "signature: {}",
            sym.signature
        );
        assert_eq!(sym.start_line, 1);
        assert!(sym.end_line >= 4);
    }

    #[test]
    fn test_extract_create_view() {
        let source = "CREATE VIEW active_users AS SELECT * FROM users WHERE active = 1;";
        let result = parse_and_extract(source);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "active_users");
        assert_eq!(sym.kind, SymbolKind::TypeAlias);
        assert_eq!(sym.visibility, Visibility::Public);
        assert!(
            sym.signature.contains("CREATE VIEW"),
            "signature: {}",
            sym.signature
        );
    }

    #[test]
    fn test_extract_create_index() {
        let source = "CREATE INDEX idx_users_email ON users (email);";
        let result = parse_and_extract(source);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "idx_users_email");
        assert_eq!(sym.kind, SymbolKind::Constant);
        assert_eq!(sym.visibility, Visibility::Public);
        assert!(
            sym.signature.contains("CREATE INDEX"),
            "signature: {}",
            sym.signature
        );
    }

    #[test]
    fn test_extract_create_function() {
        let source = r#"CREATE FUNCTION get_user(user_id INT) RETURNS TEXT AS $$
BEGIN
    RETURN (SELECT name FROM users WHERE id = user_id);
END;
$$ LANGUAGE plpgsql;"#;
        let result = parse_and_extract(source);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "get_user");
        assert_eq!(sym.kind, SymbolKind::Function);
        assert_eq!(sym.visibility, Visibility::Public);
        assert!(
            sym.signature.contains("CREATE FUNCTION"),
            "signature: {}",
            sym.signature
        );
    }

    #[test]
    fn test_extract_alter_table() {
        let source = "ALTER TABLE users ADD COLUMN age INTEGER;";
        let result = parse_and_extract(source);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "users");
        assert_eq!(sym.kind, SymbolKind::Struct);
        assert_eq!(sym.visibility, Visibility::Public);
        assert!(
            sym.signature.contains("ALTER TABLE"),
            "signature: {}",
            sym.signature
        );
    }

    #[test]
    fn test_empty_source() {
        let result = parse_and_extract("");

        assert!(result.symbols.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.exports.is_empty());
    }

    #[test]
    fn test_complex_schema() {
        let source = r#"CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT UNIQUE
);

CREATE TABLE orders (
    id INTEGER PRIMARY KEY,
    user_id INTEGER REFERENCES users(id),
    total DECIMAL(10, 2)
);

CREATE VIEW order_summary AS
SELECT u.name, COUNT(o.id) as order_count
FROM users u
JOIN orders o ON u.id = o.user_id
GROUP BY u.name;

CREATE INDEX idx_orders_user ON orders (user_id);

ALTER TABLE users ADD COLUMN created_at TIMESTAMP;

CREATE FUNCTION total_orders(uid INT) RETURNS INT AS $$
BEGIN
    RETURN (SELECT COUNT(*) FROM orders WHERE user_id = uid);
END;
$$ LANGUAGE plpgsql;

SELECT * FROM users;"#;
        let result = parse_and_extract(source);

        let tables: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Struct)
            .collect();
        // 2 CREATE TABLE + 1 ALTER TABLE = 3 Struct symbols
        assert_eq!(tables.len(), 3, "tables: {:?}", tables);

        let views: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::TypeAlias)
            .collect();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].name, "order_summary");

        let indexes: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Constant)
            .collect();
        assert_eq!(indexes.len(), 1);
        assert_eq!(indexes[0].name, "idx_orders_user");

        let functions: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "total_orders");
    }

    #[test]
    fn test_select_statement_ignored() {
        let source = r#"SELECT u.name, o.total
FROM users u
JOIN orders o ON u.id = o.user_id
WHERE o.total > 100
ORDER BY o.total DESC;"#;
        let result = parse_and_extract(source);

        assert!(
            result.symbols.is_empty(),
            "SELECT should not produce symbols, got: {:?}",
            result.symbols
        );
    }

    #[test]
    fn test_create_trigger() {
        let source = "CREATE TRIGGER user_updated BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION update_timestamp();";
        let result = parse_and_extract(source);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "user_updated");
        assert_eq!(sym.kind, SymbolKind::Function);
        assert_eq!(sym.visibility, Visibility::Public);
        assert!(
            sym.signature.contains("CREATE TRIGGER"),
            "signature: {}",
            sym.signature
        );
    }

    #[test]
    fn test_multiple_tables() {
        let source = r#"CREATE TABLE products (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    price DECIMAL(10, 2)
);

CREATE TABLE categories (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE product_categories (
    product_id INTEGER REFERENCES products(id),
    category_id INTEGER REFERENCES categories(id),
    PRIMARY KEY (product_id, category_id)
);"#;
        let result = parse_and_extract(source);

        let table_names: Vec<&str> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Struct)
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(table_names.len(), 3);
        assert!(table_names.contains(&"products"));
        assert!(table_names.contains(&"categories"));
        assert!(table_names.contains(&"product_categories"));
    }

    #[test]
    fn test_table_with_constraints() {
        let source = r#"CREATE TABLE employees (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    department_id INTEGER REFERENCES departments(id),
    salary DECIMAL(10, 2) CHECK (salary > 0),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);"#;
        let result = parse_and_extract(source);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "employees");
        assert_eq!(sym.kind, SymbolKind::Struct);
        assert!(sym.body.contains("SERIAL PRIMARY KEY"));
        assert!(sym.body.contains("UNIQUE NOT NULL"));
    }

    #[test]
    fn test_no_imports_exports() {
        let source = r#"CREATE TABLE test (id INTEGER PRIMARY KEY);
CREATE VIEW test_view AS SELECT * FROM test;
CREATE INDEX idx_test ON test (id);
ALTER TABLE test ADD COLUMN name TEXT;"#;
        let result = parse_and_extract(source);

        assert!(result.imports.is_empty(), "SQL should never have imports");
        assert!(result.exports.is_empty(), "SQL exports are always empty");
    }

    #[test]
    fn test_unique_index() {
        let source = "CREATE UNIQUE INDEX idx_users_name ON users (name);";
        let result = parse_and_extract(source);

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "idx_users_name");
        assert_eq!(sym.kind, SymbolKind::Constant);
    }

    #[test]
    fn test_extract_object_name_fallback() {
        // When a node has no object_reference child, extract_object_name returns empty
        // This exercises the empty-string path, which push_symbol skips
        let source = "SELECT 1;";
        let result = parse_and_extract(source);
        assert!(result.symbols.is_empty());
    }

    #[test]
    fn test_all_visibility_public() {
        let source = r#"CREATE TABLE t1 (id INT);
CREATE VIEW v1 AS SELECT * FROM t1;
CREATE INDEX i1 ON t1 (id);
ALTER TABLE t1 ADD COLUMN x INT;"#;
        let result = parse_and_extract(source);

        for sym in &result.symbols {
            assert_eq!(
                sym.visibility,
                Visibility::Public,
                "Symbol {} should be Public",
                sym.name
            );
        }
    }

    #[test]
    fn test_insert_update_delete_ignored() {
        let source = r#"INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com');
UPDATE users SET name = 'Bob' WHERE id = 1;
DELETE FROM users WHERE id = 2;"#;
        let result = parse_and_extract(source);

        assert!(
            result.symbols.is_empty(),
            "DML statements should not produce symbols, got: {:?}",
            result.symbols
        );
    }
}
