use crate::budget::counter::TokenCounter;
use crate::cache::{CacheEntry, FileCache};
use crate::parser::language::ParseResult;
use crate::parser::LanguageRegistry;
use crate::scanner::ScannedFile;
use std::collections::HashMap;
use std::path::Path;

/// Get the mtime of a file as seconds since UNIX epoch, or 0 on failure.
fn file_mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Parse all `files` using tree-sitter, with a persistent disk cache stored
/// under `<repo_root>/.cxpak/cache/`.
///
/// For each file the function:
/// 1. Checks whether a valid cache entry exists (matching mtime + size).
/// 2. On a cache hit, reuses the stored `ParseResult`.
/// 3. On a cache miss, parses the file with tree-sitter and records the result.
/// 4. Saves the updated cache back to disk.
///
/// Returns a `HashMap` mapping `relative_path → ParseResult` for every file
/// that could be parsed.
pub fn parse_with_cache(
    files: &[ScannedFile],
    repo_root: &Path,
    counter: &TokenCounter,
    verbose: bool,
) -> HashMap<String, ParseResult> {
    if verbose {
        eprintln!("cxpak: parsing with tree-sitter");
    }

    let cache_dir = repo_root.join(".cxpak").join("cache");
    let existing_cache = FileCache::load(&cache_dir);
    let cache_map = existing_cache.as_map();

    let registry = LanguageRegistry::new();
    let mut parse_results: HashMap<String, ParseResult> = HashMap::new();
    let mut new_cache_entries: Vec<CacheEntry> = Vec::new();

    for file in files {
        let mtime = file_mtime(&file.absolute_path);
        let size_bytes = file.size_bytes;

        // Check for a valid cache hit.
        let cached_parse = if let Some(entry) = cache_map.get(file.relative_path.as_str()) {
            if entry.mtime == mtime && entry.size_bytes == size_bytes {
                Some((entry.parse_result.clone(), entry.token_count))
            } else {
                None
            }
        } else {
            None
        };

        let parse_result = if let Some((pr, _token_count)) = cached_parse {
            pr
        } else {
            // Cache miss — parse with tree-sitter.
            let mut result = None;
            if let Some(lang_name) = &file.language {
                if let Some(lang) = registry.get(lang_name) {
                    let source = std::fs::read_to_string(&file.absolute_path).unwrap_or_default();
                    let mut parser = tree_sitter::Parser::new();
                    if parser.set_language(&lang.ts_language()).is_ok() {
                        if let Some(tree) = parser.parse(&source, None) {
                            result = Some(lang.extract(&source, &tree));
                        }
                    }
                }
            }
            result
        };

        if let Some(ref pr) = parse_result {
            parse_results.insert(file.relative_path.clone(), pr.clone());
        }

        // Preserve the cached token_count; it will be updated by the caller
        // after indexing.
        let token_count = cache_map
            .get(file.relative_path.as_str())
            .map(|e| e.token_count)
            .unwrap_or(0);

        new_cache_entries.push(CacheEntry {
            relative_path: file.relative_path.clone(),
            mtime,
            size_bytes,
            language: file.language.clone(),
            token_count,
            parse_result,
        });
    }

    if verbose {
        eprintln!("cxpak: parsed {} files", parse_results.len());
    }

    // Persist the updated cache.  Token counts are not yet finalised (the
    // caller will update them after building the index), but we at least
    // preserve the previously-cached values so repeated invocations benefit
    // from caching.
    let mut new_cache = FileCache::new();
    // Re-compute a rough token count for new entries from the parse result so
    // that a subsequent call without indexing still gets a useful estimate.
    for entry in new_cache_entries {
        let token_count = if entry.token_count == 0 {
            entry
                .parse_result
                .as_ref()
                .map(|pr| {
                    let text: String = pr
                        .symbols
                        .iter()
                        .map(|s| s.signature.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    counter.count(&text)
                })
                .unwrap_or(0)
        } else {
            entry.token_count
        };
        new_cache.entries.push(CacheEntry {
            token_count,
            ..entry
        });
    }

    if let Err(e) = new_cache.save(&cache_dir) {
        if verbose {
            eprintln!("cxpak: warning: failed to save cache: {e}");
        }
    }

    parse_results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::counter::TokenCounter;
    use std::fs;

    /// Create a minimal git repo with one Rust source file and return its root.
    fn make_test_repo(tmp: &tempfile::TempDir, source: &str) -> std::path::PathBuf {
        let root = tmp.path().to_path_buf();
        // Initialise a git repo so Scanner accepts the directory.
        std::process::Command::new("git")
            .args(["init", root.to_str().unwrap()])
            .output()
            .expect("git init");
        let src_dir = root.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        let file = src_dir.join("lib.rs");
        fs::write(&file, source).unwrap();
        // Stage the file so it is git-tracked.
        std::process::Command::new("git")
            .args(["-C", root.to_str().unwrap(), "add", "src/lib.rs"])
            .output()
            .expect("git add");
        root
    }

    fn scan_files(root: &Path) -> Vec<ScannedFile> {
        crate::scanner::Scanner::new(root)
            .expect("scanner")
            .scan()
            .expect("scan")
    }

    // ------------------------------------------------------------------
    // test_parse_with_cache_creates_cache
    // ------------------------------------------------------------------
    #[test]
    fn test_parse_with_cache_creates_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_test_repo(&tmp, "pub fn hello() {}");
        let counter = TokenCounter::new();

        let files = scan_files(&root);
        assert!(!files.is_empty(), "expected at least one scanned file");

        parse_with_cache(&files, &root, &counter, false);

        let cache_file = root.join(".cxpak").join("cache").join("cache.json");
        assert!(
            cache_file.exists(),
            "cache.json should have been created at {cache_file:?}"
        );
    }

    // ------------------------------------------------------------------
    // test_parse_with_cache_returns_parse_results
    // ------------------------------------------------------------------
    #[test]
    fn test_parse_with_cache_returns_parse_results() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_test_repo(&tmp, "pub fn hello() {}\npub fn world() {}");
        let counter = TokenCounter::new();

        let files = scan_files(&root);
        let results = parse_with_cache(&files, &root, &counter, false);

        // At least one parseable Rust file should appear in the map.
        assert!(
            !results.is_empty(),
            "expected non-empty parse results, got empty map"
        );
        // The result should contain symbols.
        let any_has_symbols = results.values().any(|pr| !pr.symbols.is_empty());
        assert!(any_has_symbols, "expected at least one symbol to be parsed");
    }

    // ------------------------------------------------------------------
    // test_parse_with_cache_cache_hit
    // ------------------------------------------------------------------
    #[test]
    fn test_parse_with_cache_cache_hit() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_test_repo(&tmp, "pub fn cached() {}");
        let counter = TokenCounter::new();

        let files = scan_files(&root);

        // First call — populates the cache.
        let results_first = parse_with_cache(&files, &root, &counter, false);

        // Verify cache exists.
        let cache_file = root.join(".cxpak").join("cache").join("cache.json");
        assert!(cache_file.exists());

        // Read the cache JSON before the second call.
        let cache_before = fs::read_to_string(&cache_file).unwrap();

        // Second call — should hit the cache.
        let results_second = parse_with_cache(&files, &root, &counter, false);

        // Both calls should return the same symbol names.
        let symbols_first: Vec<String> = results_first
            .values()
            .flat_map(|pr| pr.symbols.iter().map(|s| s.name.clone()))
            .collect();
        let symbols_second: Vec<String> = results_second
            .values()
            .flat_map(|pr| pr.symbols.iter().map(|s| s.name.clone()))
            .collect();
        assert_eq!(
            symbols_first, symbols_second,
            "cache hit should return identical results"
        );

        // The cache content should not have changed (same mtime/size).
        let cache_after = fs::read_to_string(&cache_file).unwrap();
        // Both should be valid JSON with the same entries; compare by
        // deserialising to avoid whitespace/ordering differences.
        let before: serde_json::Value = serde_json::from_str(&cache_before).unwrap();
        let after: serde_json::Value = serde_json::from_str(&cache_after).unwrap();
        assert_eq!(before, after, "cache should not change on a cache hit");
    }

    // ------------------------------------------------------------------
    // test_parse_with_cache_invalidates_on_change
    // ------------------------------------------------------------------
    #[test]
    fn test_parse_with_cache_invalidates_on_change() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_test_repo(&tmp, "pub fn original() {}");
        let counter = TokenCounter::new();

        let files = scan_files(&root);
        let results_first = parse_with_cache(&files, &root, &counter, false);

        let first_symbols: Vec<String> = results_first
            .values()
            .flat_map(|pr| pr.symbols.iter().map(|s| s.name.clone()))
            .collect();
        assert!(
            first_symbols.iter().any(|n| n == "original"),
            "expected symbol 'original' in first parse, got: {first_symbols:?}"
        );

        // Modify the file so its mtime and/or size changes.
        let file_path = root.join("src").join("lib.rs");
        // Sleep briefly to ensure mtime differs on filesystems with 1-second
        // granularity.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        fs::write(&file_path, "pub fn renamed() {}").unwrap();

        // Re-scan so ScannedFile reflects new size.
        let files_updated = scan_files(&root);
        let results_second = parse_with_cache(&files_updated, &root, &counter, false);

        let second_symbols: Vec<String> = results_second
            .values()
            .flat_map(|pr| pr.symbols.iter().map(|s| s.name.clone()))
            .collect();

        assert!(
            second_symbols.iter().any(|n| n == "renamed"),
            "expected symbol 'renamed' after file change, got: {second_symbols:?}"
        );
        assert!(
            !second_symbols.iter().any(|n| n == "original"),
            "stale symbol 'original' should not appear after file change"
        );
    }
}
