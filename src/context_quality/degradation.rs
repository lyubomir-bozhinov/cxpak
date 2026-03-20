// Progressive degradation for context quality

use crate::parser::language::{Symbol, SymbolKind};

pub const MAX_SYMBOL_TOKENS: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DetailLevel {
    Full = 0,
    Trimmed = 1,
    Documented = 2,
    Signature = 3,
    Stub = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileRole {
    Selected,
    Dependency,
}

#[derive(Debug, Clone)]
pub struct DegradedSymbol {
    pub symbol: Symbol,
    pub level: DetailLevel,
    pub rendered: String,
    pub rendered_tokens: usize,
    pub chunk_index: Option<usize>,
    pub chunk_total: Option<usize>,
    pub parent_name: Option<String>,
}

/// Returns the concept priority for a symbol kind (0.0–1.0).
/// Higher values survive degradation longer.
#[allow(unreachable_patterns)] // catch-all for future SymbolKind variants
pub fn concept_priority(kind: &SymbolKind) -> f64 {
    match kind {
        SymbolKind::Function | SymbolKind::Method => 1.00,
        SymbolKind::Struct
        | SymbolKind::Class
        | SymbolKind::Enum
        | SymbolKind::Interface
        | SymbolKind::Trait
        | SymbolKind::Type
        | SymbolKind::TypeAlias => 0.86,
        SymbolKind::Message
        | SymbolKind::Service
        | SymbolKind::Query
        | SymbolKind::Mutation
        | SymbolKind::Table => 0.71,
        SymbolKind::Key
        | SymbolKind::Block
        | SymbolKind::Variable
        | SymbolKind::Target
        | SymbolKind::Rule
        | SymbolKind::Instruction
        | SymbolKind::Selector
        | SymbolKind::Mixin => 0.57,
        SymbolKind::Heading | SymbolKind::Section | SymbolKind::Element => 0.43,
        SymbolKind::Constant => 0.29,
        _ => 0.14, // Imports and any future variants
    }
}

/// Compute the concept priority for a file based on its highest-priority symbol.
pub fn file_concept_priority(symbols: &[Symbol]) -> f64 {
    symbols
        .iter()
        .map(|s| concept_priority(&s.kind))
        .fold(0.0_f64, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::language::Visibility;

    fn make_fn_symbol(name: &str, tokens: usize) -> Symbol {
        Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            visibility: Visibility::Public,
            signature: format!("pub fn {}()", name),
            body: "x ".repeat(tokens),
            start_line: 1,
            end_line: tokens / 10,
        }
    }

    #[test]
    fn test_detail_level_ordering() {
        assert!(DetailLevel::Full < DetailLevel::Trimmed);
        assert!(DetailLevel::Trimmed < DetailLevel::Documented);
        assert!(DetailLevel::Documented < DetailLevel::Signature);
        assert!(DetailLevel::Signature < DetailLevel::Stub);
    }

    #[test]
    fn test_detail_level_equality() {
        assert_eq!(DetailLevel::Full, DetailLevel::Full);
        assert_ne!(DetailLevel::Full, DetailLevel::Stub);
    }

    #[test]
    fn test_file_role_variants() {
        let selected = FileRole::Selected;
        let dep = FileRole::Dependency;
        assert_ne!(selected, dep);
    }

    // --- concept_priority tests ---

    #[test]
    fn test_concept_priority_definitions() {
        assert_eq!(concept_priority(&SymbolKind::Function), 1.00);
        assert_eq!(concept_priority(&SymbolKind::Method), 1.00);
    }

    #[test]
    fn test_concept_priority_structures() {
        assert_eq!(concept_priority(&SymbolKind::Struct), 0.86);
        assert_eq!(concept_priority(&SymbolKind::Class), 0.86);
        assert_eq!(concept_priority(&SymbolKind::Enum), 0.86);
        assert_eq!(concept_priority(&SymbolKind::Interface), 0.86);
        assert_eq!(concept_priority(&SymbolKind::Trait), 0.86);
        assert_eq!(concept_priority(&SymbolKind::Type), 0.86);
        assert_eq!(concept_priority(&SymbolKind::TypeAlias), 0.86);
    }

    #[test]
    fn test_concept_priority_api_surface() {
        assert_eq!(concept_priority(&SymbolKind::Message), 0.71);
        assert_eq!(concept_priority(&SymbolKind::Service), 0.71);
        assert_eq!(concept_priority(&SymbolKind::Query), 0.71);
        assert_eq!(concept_priority(&SymbolKind::Mutation), 0.71);
        assert_eq!(concept_priority(&SymbolKind::Table), 0.71);
    }

    #[test]
    fn test_concept_priority_configuration() {
        assert_eq!(concept_priority(&SymbolKind::Key), 0.57);
        assert_eq!(concept_priority(&SymbolKind::Block), 0.57);
        assert_eq!(concept_priority(&SymbolKind::Variable), 0.57);
        assert_eq!(concept_priority(&SymbolKind::Target), 0.57);
        assert_eq!(concept_priority(&SymbolKind::Rule), 0.57);
        assert_eq!(concept_priority(&SymbolKind::Instruction), 0.57);
        assert_eq!(concept_priority(&SymbolKind::Selector), 0.57);
        assert_eq!(concept_priority(&SymbolKind::Mixin), 0.57);
    }

    #[test]
    fn test_concept_priority_documentation() {
        assert_eq!(concept_priority(&SymbolKind::Heading), 0.43);
        assert_eq!(concept_priority(&SymbolKind::Section), 0.43);
        assert_eq!(concept_priority(&SymbolKind::Element), 0.43);
    }

    #[test]
    fn test_concept_priority_constants() {
        assert_eq!(concept_priority(&SymbolKind::Constant), 0.29);
    }

    #[test]
    fn test_concept_priority_ordering_is_monotonic() {
        assert!(concept_priority(&SymbolKind::Function) > concept_priority(&SymbolKind::Struct));
        assert!(concept_priority(&SymbolKind::Struct) > concept_priority(&SymbolKind::Message));
        assert!(concept_priority(&SymbolKind::Message) > concept_priority(&SymbolKind::Key));
        assert!(concept_priority(&SymbolKind::Key) > concept_priority(&SymbolKind::Heading));
        assert!(concept_priority(&SymbolKind::Heading) > concept_priority(&SymbolKind::Constant));
    }

    #[test]
    fn test_file_concept_priority_max_wins() {
        let symbols = vec![
            make_fn_symbol("f", 10),
            Symbol {
                kind: SymbolKind::Constant,
                ..make_fn_symbol("c", 5)
            },
        ];
        assert_eq!(file_concept_priority(&symbols), 1.00);
    }

    #[test]
    fn test_file_concept_priority_empty() {
        assert_eq!(file_concept_priority(&[]), 0.0);
    }

    #[test]
    fn test_file_concept_priority_single_symbol() {
        let symbols = vec![Symbol {
            kind: SymbolKind::Key,
            ..make_fn_symbol("k", 5)
        }];
        assert_eq!(file_concept_priority(&symbols), 0.57);
    }
}
