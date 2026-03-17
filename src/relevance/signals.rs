use crate::index::CodebaseIndex;
use crate::relevance::SignalResult;

/// Tokenizes query + file path segments, computes Jaccard similarity.
pub fn path_similarity(query: &str, file_path: &str) -> SignalResult {
    // Stub — will be implemented in Task 3
    let _ = (query, file_path);
    SignalResult {
        name: "path_similarity",
        score: 0.0,
        detail: "stub".to_string(),
    }
}

/// Fuzzy match query terms against function/struct/class names in file.
pub fn symbol_match(query: &str, file_path: &str, index: &CodebaseIndex) -> SignalResult {
    // Stub — will be implemented in Task 3
    let _ = (query, file_path, index);
    SignalResult {
        name: "symbol_match",
        score: 0.0,
        detail: "stub".to_string(),
    }
}

/// Boost if file imports/is imported by high-scoring files.
pub fn import_proximity(file_path: &str, index: &CodebaseIndex) -> SignalResult {
    // Stub — will be implemented in Task 3
    let _ = (file_path, index);
    SignalResult {
        name: "import_proximity",
        score: 0.0,
        detail: "stub".to_string(),
    }
}

/// Lightweight TF of query terms in file content.
pub fn term_frequency(query: &str, file_path: &str, index: &CodebaseIndex) -> SignalResult {
    // Stub — will be implemented in Task 3
    let _ = (query, file_path, index);
    SignalResult {
        name: "term_frequency",
        score: 0.0,
        detail: "stub".to_string(),
    }
}
