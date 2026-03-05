# cxpak Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Rust CLI that indexes codebases using tree-sitter and produces token-budgeted context bundles for LLMs.

**Architecture:** Pipeline of Scanner → Parser → Index → Budget → Output. Each stage is a module with clear input/output boundaries. The Index is the central data structure. Tree-sitter grammars are compiled in behind Cargo feature flags.

**Tech Stack:** Rust, clap (CLI), tree-sitter (AST), tiktoken-rs (token counting), git2 (git ops), ignore (gitignore), serde/serde_json (JSON), quick-xml (XML)

---

### Task 1: Project Scaffold + Cargo.toml

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `README.md`
- Create: `LICENSE`
- Create: `.gitignore`
- Create: `.cxpakignore.example`

**Step 1: Create Cargo.toml with all dependencies and feature flags**

```toml
[package]
name = "cxpak"
version = "0.1.0"
edition = "2021"
description = "Spends CPU cycles so you don't spend tokens. The LLM gets a briefing packet instead of a flashlight in a dark room."
license = "MIT"
repository = "https://github.com/lyubomir-bozhinov/cxpak"
keywords = ["llm", "context", "tree-sitter", "cli", "ai"]
categories = ["command-line-utilities", "development-tools"]

[dependencies]
clap = { version = "4", features = ["derive"] }
tree-sitter = "0.24"
tree-sitter-rust = { version = "0.23", optional = true }
tree-sitter-typescript = { version = "0.23", optional = true }
tree-sitter-javascript = { version = "0.23", optional = true }
tree-sitter-java = { version = "0.23", optional = true }
tree-sitter-python = { version = "0.23", optional = true }
tree-sitter-go = { version = "0.23", optional = true }
tree-sitter-c = { version = "0.23", optional = true }
tree-sitter-cpp = { version = "0.23", optional = true }
tiktoken-rs = "0.6"
git2 = "0.19"
ignore = "0.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
quick-xml = { version = "0.36", features = ["serialize"] }

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"

[features]
default = ["lang-rust", "lang-typescript", "lang-javascript", "lang-java", "lang-python", "lang-go", "lang-c", "lang-cpp"]
lang-rust = ["dep:tree-sitter-rust"]
lang-typescript = ["dep:tree-sitter-typescript"]
lang-javascript = ["dep:tree-sitter-javascript"]
lang-java = ["dep:tree-sitter-java"]
lang-python = ["dep:tree-sitter-python"]
lang-go = ["dep:tree-sitter-go"]
lang-c = ["dep:tree-sitter-c"]
lang-cpp = ["dep:tree-sitter-cpp"]
```

Note: The exact tree-sitter grammar crate versions may need adjustment. Before implementing, check crates.io for the latest compatible versions of each `tree-sitter-*` grammar crate that work with the chosen `tree-sitter` core version. Some grammars use different versioning schemes. Pin to exact working versions once confirmed.

**Step 2: Create minimal src/main.rs**

```rust
fn main() {
    println!("cxpak - spends CPU cycles so you don't spend tokens");
}
```

**Step 3: Create .gitignore**

```
/target
```

**Step 4: Create README.md**

```markdown
# cxpak

> Spends CPU cycles so you don't spend tokens. The LLM gets a briefing packet instead of a flashlight in a dark room.

A Rust CLI that indexes codebases using tree-sitter and produces token-budgeted context bundles for LLMs.

## Installation

```bash
cargo install cxpak
```

## Usage

```bash
# Structured repo summary within a token budget
cxpak overview --tokens 50k

# Trace from a function/error, pack relevant code paths
cxpak trace --tokens 50k "function_name"
```

## License

MIT
```

**Step 5: Create LICENSE**

Use the MIT license text with `Lyubomir Bozhinov` as the copyright holder and `2026` as the year.

**Step 6: Create .cxpakignore.example**

```
# Additional patterns to exclude from cxpak analysis
# Uses the same syntax as .gitignore

# Generated code
generated/
*.generated.ts

# Vendored dependencies
vendor/

# Large test fixtures
tests/fixtures/large/
```

**Step 7: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

**Step 8: Commit**

```bash
git add Cargo.toml src/main.rs README.md LICENSE .gitignore .cxpakignore.example
git commit -m "scaffold: project setup with dependencies and feature flags"
```

---

### Task 2: CLI Argument Parsing

**Files:**
- Create: `src/cli/mod.rs`
- Modify: `src/main.rs`
- Create: `tests/integration/cli_test.rs`

**Step 1: Write failing test for CLI help output**

Create `tests/integration/cli_test.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_help_output() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("overview"))
        .stdout(predicate::str::contains("trace"))
        .stdout(predicate::str::contains("--tokens"));
}

#[test]
fn test_version_output() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_overview_requires_tokens() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--tokens"));
}

#[test]
fn test_trace_requires_tokens_and_target() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["trace"])
        .assert()
        .failure();
}

#[test]
fn test_tokens_parses_k_suffix() {
    // This will fail until we have a real command implementation,
    // but it validates the parser accepts "50k"
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "50k"])
        .assert(); // just check it doesn't panic on the token parsing
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test cli_test`
Expected: FAIL — no CLI structure exists yet

**Step 3: Create src/cli/mod.rs with clap derive**

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "cxpak",
    about = "Spends CPU cycles so you don't spend tokens",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Structured repo summary within a token budget
    Overview {
        /// Token budget (e.g., 50000 or 50k)
        #[arg(long)]
        tokens: String,

        /// Write output to file instead of stdout
        #[arg(long)]
        out: Option<PathBuf>,

        /// Output format: markdown, xml, json
        #[arg(long, default_value = "markdown")]
        format: OutputFormat,

        /// Show indexing progress on stderr
        #[arg(long)]
        verbose: bool,

        /// Repo path (defaults to current directory)
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Trace from error/function, pack relevant code paths
    Trace {
        /// Token budget (e.g., 50000 or 50k)
        #[arg(long)]
        tokens: String,

        /// Write output to file instead of stdout
        #[arg(long)]
        out: Option<PathBuf>,

        /// Output format: markdown, xml, json
        #[arg(long, default_value = "markdown")]
        format: OutputFormat,

        /// Show indexing progress on stderr
        #[arg(long)]
        verbose: bool,

        /// Pack all matches when target is ambiguous
        #[arg(long)]
        all: bool,

        /// Function name, file:line, or error message to trace
        target: String,
    },
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum OutputFormat {
    Markdown,
    Xml,
    Json,
}

/// Parse token count strings like "50000", "50k", "100K", "1m", "1M"
pub fn parse_token_count(s: &str) -> Result<usize, String> {
    let s = s.trim().to_lowercase();
    if let Some(prefix) = s.strip_suffix('k') {
        prefix
            .parse::<f64>()
            .map(|n| (n * 1_000.0) as usize)
            .map_err(|e| format!("invalid token count: {e}"))
    } else if let Some(prefix) = s.strip_suffix('m') {
        prefix
            .parse::<f64>()
            .map(|n| (n * 1_000_000.0) as usize)
            .map_err(|e| format!("invalid token count: {e}"))
    } else {
        s.parse::<usize>()
            .map_err(|e| format!("invalid token count: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_token_count_plain_number() {
        assert_eq!(parse_token_count("50000").unwrap(), 50000);
    }

    #[test]
    fn test_parse_token_count_k_suffix() {
        assert_eq!(parse_token_count("50k").unwrap(), 50000);
        assert_eq!(parse_token_count("50K").unwrap(), 50000);
        assert_eq!(parse_token_count("100k").unwrap(), 100000);
    }

    #[test]
    fn test_parse_token_count_m_suffix() {
        assert_eq!(parse_token_count("1m").unwrap(), 1000000);
        assert_eq!(parse_token_count("1M").unwrap(), 1000000);
    }

    #[test]
    fn test_parse_token_count_fractional() {
        assert_eq!(parse_token_count("1.5k").unwrap(), 1500);
        assert_eq!(parse_token_count("0.5m").unwrap(), 500000);
    }

    #[test]
    fn test_parse_token_count_invalid() {
        assert!(parse_token_count("abc").is_err());
        assert!(parse_token_count("").is_err());
        assert!(parse_token_count("k").is_err());
    }
}
```

**Step 4: Update src/main.rs to use CLI**

```rust
mod cli;

use clap::Parser;
use cli::{Cli, Commands, parse_token_count};

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Overview {
            tokens, verbose, path, ..
        } => {
            let token_budget = match parse_token_count(tokens) {
                Ok(n) if n == 0 => {
                    eprintln!("Error: --tokens must be greater than 0");
                    std::process::exit(1);
                }
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            if *verbose {
                eprintln!("cxpak: scanning {} with budget of {} tokens", path.display(), token_budget);
            }
            eprintln!("overview command not yet implemented");
        }
        Commands::Trace {
            tokens, target, verbose, ..
        } => {
            let token_budget = match parse_token_count(tokens) {
                Ok(n) if n == 0 => {
                    eprintln!("Error: --tokens must be greater than 0");
                    std::process::exit(1);
                }
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            if *verbose {
                eprintln!("cxpak: tracing '{}' with budget of {} tokens", target, token_budget);
            }
            eprintln!("trace command not yet implemented");
        }
    }
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test`
Expected: All tests PASS

**Step 6: Commit**

```bash
git add src/cli/ src/main.rs tests/
git commit -m "feat: CLI argument parsing with clap"
```

---

### Task 3: Scanner — File Discovery + Ignore Rules

**Files:**
- Create: `src/scanner/mod.rs`
- Create: `src/scanner/defaults.rs`
- Create: `tests/integration/scanner_test.rs`
- Create: `tests/fixtures/simple_repo/` (test fixture)

**Step 1: Create test fixture — a minimal fake repo**

Create the following file structure under `tests/fixtures/simple_repo/`:

```
tests/fixtures/simple_repo/
├── .git/            (empty dir, just a marker)
├── .gitignore       (contains: "target/\n*.log")
├── src/
│   ├── main.rs      (contains: "fn main() {}")
│   └── lib.rs       (contains: "pub fn hello() {}")
├── tests/
│   └── test.rs      (contains: "#[test] fn it_works() {}")
├── README.md        (contains: "# Test Repo")
├── Cargo.toml       (contains: "[package]\nname = \"test\"")
├── target/
│   └── debug/
│       └── binary   (contains: "should be ignored")
└── app.log          (contains: "should be ignored")
```

**Step 2: Write failing tests for scanner**

Create `tests/integration/scanner_test.rs`:

```rust
use std::path::PathBuf;

// We'll import the scanner module once it exists
// For now these tests document expected behavior

#[test]
fn test_scanner_finds_source_files() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/simple_repo");
    let scanner = cxpak::scanner::Scanner::new(&fixture).unwrap();
    let files = scanner.scan().unwrap();

    let paths: Vec<String> = files.iter().map(|f| f.relative_path.clone()).collect();

    assert!(paths.contains(&"src/main.rs".to_string()));
    assert!(paths.contains(&"src/lib.rs".to_string()));
    assert!(paths.contains(&"tests/test.rs".to_string()));
    assert!(paths.contains(&"README.md".to_string()));
    assert!(paths.contains(&"Cargo.toml".to_string()));
}

#[test]
fn test_scanner_respects_gitignore() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/simple_repo");
    let scanner = cxpak::scanner::Scanner::new(&fixture).unwrap();
    let files = scanner.scan().unwrap();

    let paths: Vec<String> = files.iter().map(|f| f.relative_path.clone()).collect();

    assert!(!paths.iter().any(|p| p.contains("target/")));
    assert!(!paths.iter().any(|p| p.ends_with(".log")));
}

#[test]
fn test_scanner_applies_builtin_defaults() {
    // Even without .gitignore, node_modules should be excluded
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/simple_repo");
    let scanner = cxpak::scanner::Scanner::new(&fixture).unwrap();
    let defaults = cxpak::scanner::defaults::BUILTIN_IGNORES;

    assert!(defaults.contains(&"node_modules"));
    assert!(defaults.contains(&"__pycache__"));
    assert!(defaults.contains(&".DS_Store"));
}

#[test]
fn test_scanner_detects_language() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/simple_repo");
    let scanner = cxpak::scanner::Scanner::new(&fixture).unwrap();
    let files = scanner.scan().unwrap();

    let main_rs = files.iter().find(|f| f.relative_path == "src/main.rs").unwrap();
    assert_eq!(main_rs.language, Some("rust".to_string()));

    let readme = files.iter().find(|f| f.relative_path == "README.md").unwrap();
    assert_eq!(readme.language, None);
}
```

**Step 3: Run tests to verify they fail**

Run: `cargo test --test scanner_test`
Expected: FAIL — scanner module doesn't exist

**Step 4: Create src/scanner/defaults.rs**

```rust
/// Built-in ignore patterns applied on top of .gitignore
pub const BUILTIN_IGNORES: &[&str] = &[
    // Dependency directories
    "node_modules",
    "__pycache__",
    ".venv",
    "venv",
    "vendor",
    // Build output
    "target",
    "dist",
    "build",
    "out",
    ".next",
    // IDE and OS
    ".DS_Store",
    ".idea",
    ".vscode",
    "*.swp",
    "*.swo",
    // Lock files
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "Cargo.lock",
    "poetry.lock",
    "Gemfile.lock",
    "go.sum",
    // Binary and media files
    "*.png",
    "*.jpg",
    "*.jpeg",
    "*.gif",
    "*.ico",
    "*.svg",
    "*.woff",
    "*.woff2",
    "*.ttf",
    "*.eot",
    "*.mp3",
    "*.mp4",
    "*.zip",
    "*.tar.gz",
    "*.jar",
    "*.war",
    "*.class",
    "*.o",
    "*.so",
    "*.dylib",
    "*.dll",
    "*.exe",
    "*.wasm",
    "*.pyc",
    // Misc
    ".git",
    ".hg",
    ".svn",
    "*.min.js",
    "*.min.css",
    "*.map",
];
```

**Step 5: Create src/scanner/mod.rs**

```rust
pub mod defaults;

use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// A discovered file in the codebase
#[derive(Debug, Clone)]
pub struct ScannedFile {
    /// Path relative to the repo root
    pub relative_path: String,
    /// Absolute path
    pub absolute_path: PathBuf,
    /// Detected language (None for unknown extensions)
    pub language: Option<String>,
    /// File size in bytes
    pub size_bytes: u64,
}

pub struct Scanner {
    root: PathBuf,
}

impl Scanner {
    pub fn new(root: &Path) -> Result<Self, ScanError> {
        let root = root.canonicalize().map_err(|e| ScanError::InvalidPath {
            path: root.to_path_buf(),
            source: e,
        })?;

        // Check for .git directory
        if !root.join(".git").exists() {
            return Err(ScanError::NotAGitRepo(root));
        }

        Ok(Self { root })
    }

    pub fn scan(&self) -> Result<Vec<ScannedFile>, ScanError> {
        let mut builder = WalkBuilder::new(&self.root);

        // Respect .gitignore
        builder.git_ignore(true);
        builder.git_global(true);
        builder.git_exclude(true);

        // Add .cxpakignore if it exists
        let cxpakignore = self.root.join(".cxpakignore");
        if cxpakignore.exists() {
            builder.add_ignore(&cxpakignore);
        }

        // Add built-in default ignores
        let mut overrides = ignore::overrides::OverrideBuilder::new(&self.root);
        for pattern in defaults::BUILTIN_IGNORES {
            // Negate pattern means "exclude this"
            let glob = format!("!{pattern}");
            overrides.add(&glob).map_err(|e| ScanError::IgnorePattern {
                pattern: pattern.to_string(),
                source: e,
            })?;
        }
        builder.overrides(overrides.build().map_err(|e| ScanError::IgnoreBuild(e))?);

        let mut files = Vec::new();

        for entry in builder.build() {
            let entry = entry.map_err(|e| ScanError::Walk(e))?;

            // Skip directories
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(true) {
                continue;
            }

            let abs_path = entry.path().to_path_buf();
            let rel_path = abs_path
                .strip_prefix(&self.root)
                .unwrap_or(&abs_path)
                .to_string_lossy()
                .to_string();

            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let language = detect_language(&rel_path);

            files.push(ScannedFile {
                relative_path: rel_path,
                absolute_path: abs_path,
                language,
                size_bytes,
            });
        }

        files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(files)
    }
}

fn detect_language(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?;
    match ext {
        "rs" => Some("rust".into()),
        "ts" | "tsx" => Some("typescript".into()),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript".into()),
        "java" => Some("java".into()),
        "py" => Some("python".into()),
        "go" => Some("go".into()),
        "c" | "h" => Some("c".into()),
        "cpp" | "hpp" | "cc" | "hh" | "cxx" => Some("cpp".into()),
        _ => None,
    }
}

#[derive(Debug)]
pub enum ScanError {
    InvalidPath { path: PathBuf, source: std::io::Error },
    NotAGitRepo(PathBuf),
    IgnorePattern { pattern: String, source: ignore::Error },
    IgnoreBuild(ignore::Error),
    Walk(ignore::Error),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanError::InvalidPath { path, source } => {
                write!(f, "invalid path '{}': {}", path.display(), source)
            }
            ScanError::NotAGitRepo(path) => {
                write!(f, "not a git repository: {}", path.display())
            }
            ScanError::IgnorePattern { pattern, source } => {
                write!(f, "invalid ignore pattern '{}': {}", pattern, source)
            }
            ScanError::IgnoreBuild(e) => write!(f, "failed to build ignore rules: {}", e),
            ScanError::Walk(e) => write!(f, "error walking directory: {}", e),
        }
    }
}

impl std::error::Error for ScanError {}
```

**Step 6: Expose scanner module — update src/main.rs**

Add to the top of `src/main.rs`:

```rust
pub mod scanner;
```

Also create `src/lib.rs` to expose modules for integration tests:

```rust
pub mod scanner;
```

**Step 7: Run tests to verify they pass**

Run: `cargo test`
Expected: All tests PASS

**Step 8: Commit**

```bash
git add src/scanner/ src/lib.rs tests/
git commit -m "feat: file scanner with gitignore, built-in defaults, and language detection"
```

---

### Task 4: Token Counter

**Files:**
- Create: `src/budget/mod.rs`
- Create: `src/budget/counter.rs`

**Step 1: Write failing tests for token counter**

In `src/budget/counter.rs`:

```rust
use tiktoken_rs::cl100k_base;

pub struct TokenCounter {
    bpe: tiktoken_rs::CoreBPE,
}

impl TokenCounter {
    pub fn new() -> Self {
        Self {
            bpe: cl100k_base().expect("failed to load cl100k_base tokenizer"),
        }
    }

    /// Count tokens in a string
    pub fn count(&self, text: &str) -> usize {
        self.bpe.encode_with_special_tokens(text).len()
    }

    /// Count tokens in a string, returning 0 for empty input
    pub fn count_or_zero(&self, text: &str) -> usize {
        if text.is_empty() {
            0
        } else {
            self.count(text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_empty_string() {
        let counter = TokenCounter::new();
        assert_eq!(counter.count(""), 0);
    }

    #[test]
    fn test_count_simple_text() {
        let counter = TokenCounter::new();
        let count = counter.count("hello world");
        assert!(count > 0);
        assert!(count < 10); // "hello world" is ~2 tokens
    }

    #[test]
    fn test_count_code() {
        let counter = TokenCounter::new();
        let code = "fn main() {\n    println!(\"Hello, world!\");\n}";
        let count = counter.count(code);
        assert!(count > 0);
        assert!(count < 30);
    }

    #[test]
    fn test_count_or_zero_empty() {
        let counter = TokenCounter::new();
        assert_eq!(counter.count_or_zero(""), 0);
    }
}
```

**Step 2: Create src/budget/mod.rs**

```rust
pub mod counter;
```

**Step 3: Expose module in src/lib.rs**

Add `pub mod budget;` to `src/lib.rs`.

**Step 4: Run tests to verify they pass**

Run: `cargo test budget`
Expected: All tests PASS

**Step 5: Commit**

```bash
git add src/budget/
git commit -m "feat: token counter using tiktoken-rs cl100k_base"
```

---

### Task 5: Language Trait + Registry

**Files:**
- Create: `src/parser/mod.rs`
- Create: `src/parser/language.rs`
- Create: `src/parser/languages/mod.rs`
- Create: `src/parser/languages/rust.rs`

**Step 1: Define the Language trait and symbol types**

Create `src/parser/language.rs`:

```rust
use tree_sitter::Language as TsLanguage;

/// A symbol extracted from source code
#[derive(Debug, Clone, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub visibility: Visibility,
    pub signature: String,
    pub body: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Interface,
    Class,
    Method,
    Constant,
    TypeAlias,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    Public,
    Private,
}

/// An import/dependency reference
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub source: String,
    pub names: Vec<String>,
}

/// An export from a module
#[derive(Debug, Clone, PartialEq)]
pub struct Export {
    pub name: String,
    pub kind: SymbolKind,
}

/// Result of parsing a single file
#[derive(Debug, Clone)]
pub struct ParseResult {
    pub symbols: Vec<Symbol>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
}

/// Trait that each language must implement
pub trait LanguageSupport: Send + Sync {
    /// Return the tree-sitter language
    fn ts_language(&self) -> TsLanguage;

    /// Extract symbols, imports, and exports from parsed source
    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult;

    /// Language name (e.g., "rust", "typescript")
    fn name(&self) -> &str;
}
```

**Step 2: Create the language registry**

Create `src/parser/mod.rs`:

```rust
pub mod language;
pub mod languages;

use language::LanguageSupport;
use std::collections::HashMap;

pub struct LanguageRegistry {
    languages: HashMap<String, Box<dyn LanguageSupport>>,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            languages: HashMap::new(),
        };
        registry.register_defaults();
        registry
    }

    fn register_defaults(&mut self) {
        #[cfg(feature = "lang-rust")]
        self.register(Box::new(languages::rust::RustLanguage));

        // Other languages will be registered as they are implemented
    }

    pub fn register(&mut self, lang: Box<dyn LanguageSupport>) {
        self.languages.insert(lang.name().to_string(), lang);
    }

    pub fn get(&self, name: &str) -> Option<&dyn LanguageSupport> {
        self.languages.get(name).map(|l| l.as_ref())
    }

    pub fn supported_languages(&self) -> Vec<&str> {
        self.languages.keys().map(|k| k.as_str()).collect()
    }
}
```

**Step 3: Implement Rust language support**

Create `src/parser/languages/mod.rs`:

```rust
#[cfg(feature = "lang-rust")]
pub mod rust;
```

Create `src/parser/languages/rust.rs`:

```rust
use crate::parser::language::*;
use tree_sitter::Language as TsLanguage;

pub struct RustLanguage;

impl LanguageSupport for RustLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "rust"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let mut symbols = Vec::new();
        let mut imports = Vec::new();
        let mut exports = Vec::new();

        let root = tree.root_node();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            match child.kind() {
                "function_item" => {
                    if let Some(sym) = self.extract_function(source, &child) {
                        if sym.visibility == Visibility::Public {
                            exports.push(Export {
                                name: sym.name.clone(),
                                kind: sym.kind.clone(),
                            });
                        }
                        symbols.push(sym);
                    }
                }
                "struct_item" => {
                    if let Some(sym) = self.extract_type_def(source, &child, SymbolKind::Struct) {
                        if sym.visibility == Visibility::Public {
                            exports.push(Export {
                                name: sym.name.clone(),
                                kind: sym.kind.clone(),
                            });
                        }
                        symbols.push(sym);
                    }
                }
                "enum_item" => {
                    if let Some(sym) = self.extract_type_def(source, &child, SymbolKind::Enum) {
                        if sym.visibility == Visibility::Public {
                            exports.push(Export {
                                name: sym.name.clone(),
                                kind: sym.kind.clone(),
                            });
                        }
                        symbols.push(sym);
                    }
                }
                "trait_item" => {
                    if let Some(sym) = self.extract_type_def(source, &child, SymbolKind::Trait) {
                        if sym.visibility == Visibility::Public {
                            exports.push(Export {
                                name: sym.name.clone(),
                                kind: sym.kind.clone(),
                            });
                        }
                        symbols.push(sym);
                    }
                }
                "use_declaration" => {
                    if let Some(imp) = self.extract_import(source, &child) {
                        imports.push(imp);
                    }
                }
                "impl_item" => {
                    self.extract_impl_methods(source, &child, &mut symbols, &mut exports);
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

impl RustLanguage {
    fn extract_function(&self, source: &str, node: &tree_sitter::Node) -> Option<Symbol> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let visibility = self.check_visibility(node);
        let body = node.utf8_text(source.as_bytes()).ok()?.to_string();

        // Extract signature (everything before the body block)
        let signature = if let Some(body_node) = node.child_by_field_name("body") {
            source[node.start_byte()..body_node.start_byte()]
                .trim()
                .to_string()
        } else {
            body.clone()
        };

        Some(Symbol {
            name,
            kind: SymbolKind::Function,
            visibility,
            signature,
            body,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
        })
    }

    fn extract_type_def(
        &self,
        source: &str,
        node: &tree_sitter::Node,
        kind: SymbolKind,
    ) -> Option<Symbol> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let visibility = self.check_visibility(node);
        let body = node.utf8_text(source.as_bytes()).ok()?.to_string();

        // For type defs, signature is the first line
        let signature = body.lines().next().unwrap_or("").to_string();

        Some(Symbol {
            name,
            kind,
            visibility,
            signature,
            body,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
        })
    }

    fn extract_import(&self, source: &str, node: &tree_sitter::Node) -> Option<Import> {
        let text = node.utf8_text(source.as_bytes()).ok()?.to_string();
        // Simple extraction: "use std::collections::HashMap;" -> source: "std::collections", names: ["HashMap"]
        let trimmed = text.trim_start_matches("use ").trim_end_matches(';').trim();
        let parts: Vec<&str> = trimmed.rsplitn(2, "::").collect();

        if parts.len() == 2 {
            Some(Import {
                source: parts[1].to_string(),
                names: vec![parts[0].trim_matches('{').trim_matches('}').to_string()],
            })
        } else {
            Some(Import {
                source: trimmed.to_string(),
                names: vec![],
            })
        }
    }

    fn extract_impl_methods(
        &self,
        source: &str,
        node: &tree_sitter::Node,
        symbols: &mut Vec<Symbol>,
        exports: &mut Vec<Export>,
    ) {
        let mut cursor = node.walk();
        if let Some(body) = node.child_by_field_name("body") {
            for child in body.children(&mut cursor) {
                if child.kind() == "function_item" {
                    if let Some(mut sym) = self.extract_function(source, &child) {
                        sym.kind = SymbolKind::Method;
                        if sym.visibility == Visibility::Public {
                            exports.push(Export {
                                name: sym.name.clone(),
                                kind: sym.kind.clone(),
                            });
                        }
                        symbols.push(sym);
                    }
                }
            }
        }
    }

    fn check_visibility(&self, node: &tree_sitter::Node) -> Visibility {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                return Visibility::Public;
            }
        }
        Visibility::Private
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_rust(source: &str) -> ParseResult {
        let lang = RustLanguage;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang.ts_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        lang.extract(source, &tree)
    }

    #[test]
    fn test_extract_public_function() {
        let source = "pub fn hello(name: &str) -> String {\n    format!(\"hello {name}\")\n}";
        let result = parse_rust(source);

        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "hello");
        assert_eq!(result.symbols[0].kind, SymbolKind::Function);
        assert_eq!(result.symbols[0].visibility, Visibility::Public);
        assert!(result.symbols[0].signature.contains("pub fn hello"));

        assert_eq!(result.exports.len(), 1);
        assert_eq!(result.exports[0].name, "hello");
    }

    #[test]
    fn test_extract_private_function() {
        let source = "fn internal() -> bool {\n    true\n}";
        let result = parse_rust(source);

        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].visibility, Visibility::Private);
        assert_eq!(result.exports.len(), 0);
    }

    #[test]
    fn test_extract_struct() {
        let source = "pub struct Config {\n    pub name: String,\n    port: u16,\n}";
        let result = parse_rust(source);

        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "Config");
        assert_eq!(result.symbols[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn test_extract_enum() {
        let source = "pub enum Color {\n    Red,\n    Green,\n    Blue,\n}";
        let result = parse_rust(source);

        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "Color");
        assert_eq!(result.symbols[0].kind, SymbolKind::Enum);
    }

    #[test]
    fn test_extract_trait() {
        let source = "pub trait Drawable {\n    fn draw(&self);\n}";
        let result = parse_rust(source);

        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "Drawable");
        assert_eq!(result.symbols[0].kind, SymbolKind::Trait);
    }

    #[test]
    fn test_extract_use_import() {
        let source = "use std::collections::HashMap;";
        let result = parse_rust(source);

        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "std::collections");
        assert_eq!(result.imports[0].names, vec!["HashMap"]);
    }

    #[test]
    fn test_extract_impl_methods() {
        let source = "struct Foo;\nimpl Foo {\n    pub fn bar(&self) {}\n    fn baz(&self) {}\n}";
        let result = parse_rust(source);

        let methods: Vec<&Symbol> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Method)
            .collect();
        assert_eq!(methods.len(), 2);
        assert_eq!(methods[0].name, "bar");
        assert_eq!(methods[0].visibility, Visibility::Public);
        assert_eq!(methods[1].name, "baz");
        assert_eq!(methods[1].visibility, Visibility::Private);
    }
}
```

**Step 4: Expose parser module in src/lib.rs**

Add `pub mod parser;` to `src/lib.rs`.

**Step 5: Run tests**

Run: `cargo test parser`
Expected: All tests PASS

**Step 6: Commit**

```bash
git add src/parser/
git commit -m "feat: Language trait, registry, and Rust language support"
```

---

### Task 6: Remaining Language Implementations

**Files:**
- Create: `src/parser/languages/typescript.rs`
- Create: `src/parser/languages/javascript.rs`
- Create: `src/parser/languages/java.rs`
- Create: `src/parser/languages/python.rs`
- Create: `src/parser/languages/go.rs`
- Create: `src/parser/languages/c.rs`
- Create: `src/parser/languages/cpp.rs`
- Modify: `src/parser/languages/mod.rs`
- Modify: `src/parser/mod.rs`

Each language follows the same pattern as Rust in Task 5. Implement the `LanguageSupport` trait for each. The key differences per language:

**TypeScript/JavaScript:**
- Functions: `function_declaration`, `arrow_function`, `method_definition`
- Types: `interface_declaration`, `type_alias_declaration`, `class_declaration`, `enum_declaration`
- Imports: `import_statement`
- Exports: `export_statement`
- Visibility: `export` keyword = public

**Java:**
- Functions: `method_declaration`, `constructor_declaration`
- Types: `class_declaration`, `interface_declaration`, `enum_declaration`
- Imports: `import_declaration`
- Visibility: `public`/`private`/`protected` modifiers

**Python:**
- Functions: `function_definition`, `class_definition`
- Imports: `import_statement`, `import_from_statement`
- Visibility: `_prefix` = private convention

**Go:**
- Functions: `function_declaration`, `method_declaration`
- Types: `type_declaration` (struct, interface)
- Imports: `import_declaration`
- Visibility: uppercase first letter = public

**C/C++:**
- Functions: `function_definition`, `declaration`
- Types: `struct_specifier`, `enum_specifier`, `type_definition`
- Imports: `preproc_include`
- C++ adds: `class_specifier`, `namespace_definition`

**Step 1: Implement each language file**

Each file follows this structure:
1. Struct implementing `LanguageSupport`
2. `ts_language()` returns the grammar
3. `extract()` walks the AST and extracts symbols/imports/exports
4. Unit tests with representative code snippets

**Step 2: Register all languages in the registry**

Update `src/parser/mod.rs` `register_defaults()` to include all languages.

**Step 3: Run all tests**

Run: `cargo test parser`
Expected: All tests PASS

**Step 4: Commit each language separately**

```bash
git commit -m "feat: TypeScript language support"
git commit -m "feat: JavaScript language support"
git commit -m "feat: Java language support"
git commit -m "feat: Python language support"
git commit -m "feat: Go language support"
git commit -m "feat: C language support"
git commit -m "feat: C++ language support"
```

---

### Task 7: Index — Central Data Structure

**Files:**
- Create: `src/index/mod.rs`
- Create: `src/index/symbols.rs`
- Create: `src/index/graph.rs`

**Step 1: Write failing test for index building**

In `src/index/mod.rs`:

```rust
pub mod symbols;
pub mod graph;

use crate::parser::language::{ParseResult, Symbol, Import, Export};
use crate::scanner::ScannedFile;
use crate::budget::counter::TokenCounter;
use std::collections::HashMap;

/// Central data structure representing an indexed codebase
#[derive(Debug)]
pub struct CodebaseIndex {
    /// All scanned files
    pub files: Vec<IndexedFile>,
    /// Language breakdown (language -> file count)
    pub language_stats: HashMap<String, LanguageStats>,
    /// Total files scanned
    pub total_files: usize,
    /// Total size in bytes
    pub total_bytes: u64,
    /// Total estimated tokens
    pub total_tokens: usize,
}

#[derive(Debug)]
pub struct IndexedFile {
    pub relative_path: String,
    pub language: Option<String>,
    pub size_bytes: u64,
    pub token_count: usize,
    pub parse_result: Option<ParseResult>,
    pub content: String,
}

#[derive(Debug)]
pub struct LanguageStats {
    pub file_count: usize,
    pub total_bytes: u64,
    pub total_tokens: usize,
}

impl CodebaseIndex {
    pub fn build(
        files: Vec<ScannedFile>,
        parse_results: HashMap<String, ParseResult>,
        counter: &TokenCounter,
    ) -> Self {
        let mut language_stats: HashMap<String, LanguageStats> = HashMap::new();
        let mut indexed_files = Vec::new();
        let mut total_tokens = 0usize;
        let mut total_bytes = 0u64;

        for file in &files {
            let content = std::fs::read_to_string(&file.absolute_path).unwrap_or_default();
            let token_count = counter.count_or_zero(&content);

            total_tokens += token_count;
            total_bytes += file.size_bytes;

            if let Some(lang) = &file.language {
                let stats = language_stats
                    .entry(lang.clone())
                    .or_insert(LanguageStats {
                        file_count: 0,
                        total_bytes: 0,
                        total_tokens: 0,
                    });
                stats.file_count += 1;
                stats.total_bytes += file.size_bytes;
                stats.total_tokens += token_count;
            }

            let parse_result = parse_results.get(&file.relative_path).cloned();

            indexed_files.push(IndexedFile {
                relative_path: file.relative_path.clone(),
                language: file.language.clone(),
                size_bytes: file.size_bytes,
                token_count,
                parse_result,
                content,
            });
        }

        Self {
            total_files: indexed_files.len(),
            total_bytes,
            total_tokens,
            files: indexed_files,
            language_stats,
        }
    }

    /// Get all public symbols across the codebase
    pub fn all_public_symbols(&self) -> Vec<(&str, &Symbol)> {
        self.files
            .iter()
            .filter_map(|f| {
                f.parse_result.as_ref().map(|pr| {
                    pr.symbols
                        .iter()
                        .filter(|s| s.visibility == crate::parser::language::Visibility::Public)
                        .map(move |s| (f.relative_path.as_str(), s))
                })
            })
            .flatten()
            .collect()
    }

    /// Get all imports across the codebase
    pub fn all_imports(&self) -> Vec<(&str, &Import)> {
        self.files
            .iter()
            .filter_map(|f| {
                f.parse_result.as_ref().map(|pr| {
                    pr.imports
                        .iter()
                        .map(move |i| (f.relative_path.as_str(), i))
                })
            })
            .flatten()
            .collect()
    }

    /// Check if a file is a "key file" (README, config, entry point)
    pub fn is_key_file(path: &str) -> bool {
        let lower = path.to_lowercase();
        let filename = lower.rsplit('/').next().unwrap_or(&lower);
        matches!(
            filename,
            "readme.md"
                | "readme"
                | "cargo.toml"
                | "package.json"
                | "pom.xml"
                | "build.gradle"
                | "build.gradle.kts"
                | "go.mod"
                | "pyproject.toml"
                | "setup.py"
                | "setup.cfg"
                | "makefile"
                | "dockerfile"
                | "docker-compose.yml"
                | "docker-compose.yaml"
                | ".env.example"
        ) || lower.ends_with("main.rs")
            || lower.ends_with("main.go")
            || lower.ends_with("main.py")
            || lower.ends_with("main.java")
            || lower.ends_with("app.py")
            || lower.ends_with("index.ts")
            || lower.ends_with("index.js")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_key_file() {
        assert!(CodebaseIndex::is_key_file("README.md"));
        assert!(CodebaseIndex::is_key_file("Cargo.toml"));
        assert!(CodebaseIndex::is_key_file("src/main.rs"));
        assert!(CodebaseIndex::is_key_file("cmd/server/main.go"));
        assert!(CodebaseIndex::is_key_file("Dockerfile"));
        assert!(!CodebaseIndex::is_key_file("src/utils/helper.rs"));
        assert!(!CodebaseIndex::is_key_file("tests/test_foo.py"));
    }
}
```

**Step 2: Create src/index/symbols.rs and src/index/graph.rs as stubs**

`src/index/symbols.rs`:
```rust
// Symbol querying utilities - used by trace command (v2)
```

`src/index/graph.rs`:
```rust
use std::collections::{HashMap, HashSet};

/// Internal dependency graph between files/modules
#[derive(Debug, Default)]
pub struct DependencyGraph {
    /// file path -> set of file paths it imports from
    pub edges: HashMap<String, HashSet<String>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_edge(&mut self, from: &str, to: &str) {
        self.edges
            .entry(from.to_string())
            .or_default()
            .insert(to.to_string());
    }

    /// Files that depend on the given file
    pub fn dependents(&self, path: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|(_, deps)| deps.contains(path))
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Files that the given file depends on
    pub fn dependencies(&self, path: &str) -> Option<&HashSet<String>> {
        self.edges.get(path)
    }
}
```

**Step 3: Expose in src/lib.rs**

Add `pub mod index;` to `src/lib.rs`.

**Step 4: Run tests**

Run: `cargo test index`
Expected: All tests PASS

**Step 5: Commit**

```bash
git add src/index/
git commit -m "feat: codebase index with language stats, key file detection, and dependency graph"
```

---

### Task 8: Budget Allocation + Degradation

**Files:**
- Create: `src/budget/degrader.rs`
- Modify: `src/budget/mod.rs`

**Step 1: Write failing tests for budget allocation**

In `src/budget/mod.rs`:

```rust
pub mod counter;
pub mod degrader;

/// Budget allocation weights per section
#[derive(Debug, Clone)]
pub struct BudgetAllocation {
    pub metadata: usize,
    pub directory_tree: usize,
    pub module_map: usize,
    pub dependency_graph: usize,
    pub key_files: usize,
    pub signatures: usize,
    pub git_context: usize,
}

const METADATA_FIXED: usize = 500;

impl BudgetAllocation {
    /// Allocate token budget across sections using weighted distribution.
    /// If a section uses less than its allocation, surplus flows to remaining sections.
    pub fn allocate(total_budget: usize) -> Self {
        let remaining = total_budget.saturating_sub(METADATA_FIXED);

        Self {
            metadata: METADATA_FIXED,
            directory_tree: (remaining as f64 * 0.05) as usize,
            module_map: (remaining as f64 * 0.20) as usize,
            dependency_graph: (remaining as f64 * 0.15) as usize,
            key_files: (remaining as f64 * 0.20) as usize,
            signatures: (remaining as f64 * 0.30) as usize,
            git_context: (remaining as f64 * 0.10) as usize,
        }
    }

    pub fn total(&self) -> usize {
        self.metadata
            + self.directory_tree
            + self.module_map
            + self.dependency_graph
            + self.key_files
            + self.signatures
            + self.git_context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_50k() {
        let alloc = BudgetAllocation::allocate(50000);
        assert_eq!(alloc.metadata, 500);
        assert!(alloc.total() <= 50000);
        // Signatures should get the largest share
        assert!(alloc.signatures > alloc.module_map);
        assert!(alloc.signatures > alloc.key_files);
    }

    #[test]
    fn test_allocate_tiny_budget() {
        let alloc = BudgetAllocation::allocate(1000);
        assert_eq!(alloc.metadata, 500);
        assert!(alloc.total() <= 1000);
    }

    #[test]
    fn test_allocate_zero() {
        let alloc = BudgetAllocation::allocate(0);
        assert_eq!(alloc.total(), 0);
    }
}
```

**Step 2: Create src/budget/degrader.rs**

```rust
/// Generates an omission marker for cut content
pub fn omission_marker(section: &str, omitted_tokens: usize, min_budget: usize) -> String {
    let display_tokens = if omitted_tokens >= 1000 {
        format!("~{:.1}k", omitted_tokens as f64 / 1000.0)
    } else {
        format!("~{}", omitted_tokens)
    };

    let display_budget = if min_budget >= 1000 {
        format!("{}k+", min_budget / 1000)
    } else {
        format!("{}+", min_budget)
    };

    format!(
        "<!-- {section} omitted: {display_tokens} tokens. Use --tokens {display_budget} to include -->"
    )
}

/// Truncate content to fit within a token budget.
/// Returns (truncated_content, tokens_used, tokens_omitted).
pub fn truncate_to_budget(
    content: &str,
    budget: usize,
    counter: &crate::budget::counter::TokenCounter,
    section_name: &str,
) -> (String, usize, usize) {
    let total_tokens = counter.count(content);

    if total_tokens <= budget {
        return (content.to_string(), total_tokens, 0);
    }

    // Truncate by lines, keeping as many as fit
    let mut lines = Vec::new();
    let mut used = 0;

    for line in content.lines() {
        let line_tokens = counter.count(line) + 1; // +1 for newline
        if used + line_tokens > budget.saturating_sub(50) {
            // Reserve 50 tokens for omission marker
            break;
        }
        lines.push(line);
        used += line_tokens;
    }

    let omitted = total_tokens - used;
    let marker = omission_marker(section_name, omitted, used + omitted + 500);

    let mut truncated = lines.join("\n");
    truncated.push('\n');
    truncated.push_str(&marker);

    (truncated, used, omitted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_omission_marker_small() {
        let marker = omission_marker("git context", 500, 3000);
        assert!(marker.contains("git context"));
        assert!(marker.contains("~500"));
        assert!(marker.contains("3k+"));
    }

    #[test]
    fn test_omission_marker_large() {
        let marker = omission_marker("signatures", 15000, 50000);
        assert!(marker.contains("~15.0k"));
        assert!(marker.contains("50k+"));
    }

    #[test]
    fn test_truncate_fits() {
        let counter = crate::budget::counter::TokenCounter::new();
        let content = "line one\nline two\nline three";
        let (result, used, omitted) = truncate_to_budget(content, 100, &counter, "test");
        assert_eq!(result, content.to_string());
        assert_eq!(omitted, 0);
        assert!(used > 0);
    }

    #[test]
    fn test_truncate_exceeds() {
        let counter = crate::budget::counter::TokenCounter::new();
        // Create content that definitely exceeds 10 tokens
        let content = (0..100)
            .map(|i| format!("this is line number {} with some padding text", i))
            .collect::<Vec<_>>()
            .join("\n");
        let (result, _used, omitted) = truncate_to_budget(&content, 10, &counter, "test section");
        assert!(omitted > 0);
        assert!(result.contains("<!-- test section omitted"));
    }
}
```

**Step 3: Run tests**

Run: `cargo test budget`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add src/budget/
git commit -m "feat: token budget allocation and top-down degradation with omission markers"
```

---

### Task 9: Git Context

**Files:**
- Create: `src/git/mod.rs`

**Step 1: Implement git context extraction**

```rust
use git2::Repository;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct GitContext {
    pub recent_commits: Vec<CommitInfo>,
    pub most_changed_files: Vec<FileChurn>,
    pub contributors: Vec<ContributorInfo>,
}

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

#[derive(Debug, Clone)]
pub struct FileChurn {
    pub path: String,
    pub commit_count: usize,
}

#[derive(Debug, Clone)]
pub struct ContributorInfo {
    pub name: String,
    pub commit_count: usize,
}

pub fn extract_git_context(repo_path: &Path, max_commits: usize) -> Result<GitContext, git2::Error> {
    let repo = Repository::open(repo_path)?;
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    let mut commits = Vec::new();
    let mut file_changes: HashMap<String, usize> = HashMap::new();
    let mut contributor_counts: HashMap<String, usize> = HashMap::new();

    for (i, oid) in revwalk.enumerate() {
        if i >= max_commits {
            break;
        }

        let oid = oid?;
        let commit = repo.find_commit(oid)?;

        let author = commit.author();
        let name = author.name().unwrap_or("unknown").to_string();
        let message = commit.summary().unwrap_or("").to_string();
        let time = commit.time();
        let date = format_git_time(time.seconds());

        commits.push(CommitInfo {
            hash: format!("{:.7}", oid),
            message,
            author: name.clone(),
            date,
        });

        *contributor_counts.entry(name).or_insert(0) += 1;

        // Diff against parent to find changed files
        if let Ok(diff) = diff_commit(&repo, &commit) {
            for delta in diff.deltas() {
                if let Some(path) = delta.new_file().path() {
                    let path_str = path.to_string_lossy().to_string();
                    *file_changes.entry(path_str).or_insert(0) += 1;
                }
            }
        }
    }

    let mut most_changed_files: Vec<FileChurn> = file_changes
        .into_iter()
        .map(|(path, count)| FileChurn {
            path,
            commit_count: count,
        })
        .collect();
    most_changed_files.sort_by(|a, b| b.commit_count.cmp(&a.commit_count));
    most_changed_files.truncate(20);

    let mut contributors: Vec<ContributorInfo> = contributor_counts
        .into_iter()
        .map(|(name, count)| ContributorInfo {
            name,
            commit_count: count,
        })
        .collect();
    contributors.sort_by(|a, b| b.commit_count.cmp(&a.commit_count));

    Ok(GitContext {
        recent_commits: commits,
        most_changed_files,
        contributors,
    })
}

fn diff_commit<'a>(
    repo: &'a Repository,
    commit: &git2::Commit<'a>,
) -> Result<git2::Diff<'a>, git2::Error> {
    let tree = commit.tree()?;
    let parent_tree = commit
        .parent(0)
        .ok()
        .and_then(|p| p.tree().ok());

    repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
}

fn format_git_time(seconds: i64) -> String {
    // Simple ISO-ish date from unix timestamp
    let dt = chrono_lite(seconds);
    dt
}

/// Minimal timestamp formatting without pulling in chrono
fn chrono_lite(unix_seconds: i64) -> String {
    // Use git2's built-in time formatting
    let secs = unix_seconds;
    let days = secs / 86400;
    let years_approx = 1970 + (days / 365);
    let remaining_days = days % 365;
    let months_approx = remaining_days / 30 + 1;
    let day_approx = remaining_days % 30 + 1;
    format!(
        "{:04}-{:02}-{:02}",
        years_approx, months_approx, day_approx
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::process::Command;

    fn create_test_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let path = dir.path();

        Command::new("git").args(["init"]).current_dir(path).output().unwrap();
        Command::new("git").args(["config", "user.email", "test@test.com"]).current_dir(path).output().unwrap();
        Command::new("git").args(["config", "user.name", "Test User"]).current_dir(path).output().unwrap();

        std::fs::write(path.join("file.txt"), "hello").unwrap();
        Command::new("git").args(["add", "."]).current_dir(path).output().unwrap();
        Command::new("git").args(["commit", "-m", "initial commit"]).current_dir(path).output().unwrap();

        std::fs::write(path.join("file.txt"), "hello world").unwrap();
        Command::new("git").args(["add", "."]).current_dir(path).output().unwrap();
        Command::new("git").args(["commit", "-m", "update file"]).current_dir(path).output().unwrap();

        dir
    }

    #[test]
    fn test_extract_git_context() {
        let repo = create_test_repo();
        let ctx = extract_git_context(repo.path(), 20).unwrap();

        assert_eq!(ctx.recent_commits.len(), 2);
        assert_eq!(ctx.recent_commits[0].message, "update file");
        assert_eq!(ctx.recent_commits[1].message, "initial commit");
        assert_eq!(ctx.contributors.len(), 1);
        assert_eq!(ctx.contributors[0].name, "Test User");
        assert!(ctx.most_changed_files.iter().any(|f| f.path == "file.txt"));
    }
}
```

**Step 2: Expose in src/lib.rs**

Add `pub mod git;` to `src/lib.rs`.

**Step 3: Run tests**

Run: `cargo test git`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add src/git/
git commit -m "feat: git context extraction — commits, file churn, contributors"
```

---

### Task 10: Output Renderers

**Files:**
- Create: `src/output/mod.rs`
- Create: `src/output/markdown.rs`
- Create: `src/output/xml.rs`
- Create: `src/output/json.rs`

**Step 1: Define the output data structure**

Create `src/output/mod.rs`:

```rust
pub mod markdown;
pub mod xml;
pub mod json;

use crate::cli::OutputFormat;

/// Sections of the output, each with pre-rendered content and token count
#[derive(Debug, Clone)]
pub struct OutputSections {
    pub metadata: String,
    pub directory_tree: String,
    pub module_map: String,
    pub dependency_graph: String,
    pub key_files: String,
    pub signatures: String,
    pub git_context: String,
}

pub fn render(sections: &OutputSections, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Markdown => markdown::render(sections),
        OutputFormat::Xml => xml::render(sections),
        OutputFormat::Json => json::render(sections),
    }
}
```

**Step 2: Implement markdown renderer**

Create `src/output/markdown.rs`:

```rust
use super::OutputSections;

pub fn render(sections: &OutputSections) -> String {
    let mut out = String::new();

    if !sections.metadata.is_empty() {
        out.push_str("## Project Metadata\n\n");
        out.push_str(&sections.metadata);
        out.push_str("\n\n");
    }

    if !sections.directory_tree.is_empty() {
        out.push_str("## Directory Tree\n\n");
        out.push_str("```\n");
        out.push_str(&sections.directory_tree);
        out.push_str("\n```\n\n");
    }

    if !sections.module_map.is_empty() {
        out.push_str("## Module / Component Map\n\n");
        out.push_str(&sections.module_map);
        out.push_str("\n\n");
    }

    if !sections.dependency_graph.is_empty() {
        out.push_str("## Dependency Graph\n\n");
        out.push_str(&sections.dependency_graph);
        out.push_str("\n\n");
    }

    if !sections.key_files.is_empty() {
        out.push_str("## Key Files\n\n");
        out.push_str(&sections.key_files);
        out.push_str("\n\n");
    }

    if !sections.signatures.is_empty() {
        out.push_str("## Function / Type Signatures\n\n");
        out.push_str(&sections.signatures);
        out.push_str("\n\n");
    }

    if !sections.git_context.is_empty() {
        out.push_str("## Git Context\n\n");
        out.push_str(&sections.git_context);
        out.push_str("\n\n");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_includes_sections() {
        let sections = OutputSections {
            metadata: "Language: Rust (100%)".into(),
            directory_tree: "src/\n  main.rs".into(),
            module_map: String::new(),
            dependency_graph: String::new(),
            key_files: String::new(),
            signatures: String::new(),
            git_context: String::new(),
        };

        let output = render(&sections);
        assert!(output.contains("## Project Metadata"));
        assert!(output.contains("Language: Rust"));
        assert!(output.contains("## Directory Tree"));
        assert!(!output.contains("## Module")); // empty section omitted
    }
}
```

**Step 3: Implement XML renderer**

Create `src/output/xml.rs`:

```rust
use super::OutputSections;

pub fn render(sections: &OutputSections) -> String {
    let mut out = String::from("<cxpak>\n");

    emit_section(&mut out, "metadata", &sections.metadata);
    emit_section(&mut out, "directory-tree", &sections.directory_tree);
    emit_section(&mut out, "module-map", &sections.module_map);
    emit_section(&mut out, "dependency-graph", &sections.dependency_graph);
    emit_section(&mut out, "key-files", &sections.key_files);
    emit_section(&mut out, "signatures", &sections.signatures);
    emit_section(&mut out, "git-context", &sections.git_context);

    out.push_str("</cxpak>\n");
    out
}

fn emit_section(out: &mut String, tag: &str, content: &str) {
    if !content.is_empty() {
        out.push_str(&format!("  <{tag}>\n"));
        for line in content.lines() {
            out.push_str(&format!("    {}\n", escape_xml(line)));
        }
        out.push_str(&format!("  </{tag}>\n"));
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
```

**Step 4: Implement JSON renderer**

Create `src/output/json.rs`:

```rust
use super::OutputSections;
use serde::Serialize;

#[derive(Serialize)]
struct JsonOutput {
    #[serde(skip_serializing_if = "String::is_empty")]
    metadata: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    directory_tree: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    module_map: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    dependency_graph: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    key_files: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    signatures: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    git_context: String,
}

pub fn render(sections: &OutputSections) -> String {
    let output = JsonOutput {
        metadata: sections.metadata.clone(),
        directory_tree: sections.directory_tree.clone(),
        module_map: sections.module_map.clone(),
        dependency_graph: sections.dependency_graph.clone(),
        key_files: sections.key_files.clone(),
        signatures: sections.signatures.clone(),
        git_context: sections.git_context.clone(),
    };

    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".into())
}
```

**Step 5: Expose in src/lib.rs**

Add `pub mod output;` to `src/lib.rs`.

**Step 6: Run tests**

Run: `cargo test output`
Expected: All tests PASS

**Step 7: Commit**

```bash
git add src/output/
git commit -m "feat: output renderers — markdown, XML, JSON"
```

---

### Task 11: Overview Command — Orchestration

**Files:**
- Create: `src/commands/mod.rs`
- Create: `src/commands/overview.rs`
- Create: `src/commands/trace.rs`
- Modify: `src/main.rs`

This is the big wiring task. The overview command orchestrates the full pipeline: scan → parse → index → budget → render → output.

**Step 1: Create src/commands/overview.rs**

```rust
use crate::budget::counter::TokenCounter;
use crate::budget::degrader;
use crate::budget::BudgetAllocation;
use crate::cli::OutputFormat;
use crate::git;
use crate::index::CodebaseIndex;
use crate::output::{self, OutputSections};
use crate::parser::LanguageRegistry;
use crate::scanner::Scanner;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

pub fn run(
    path: &Path,
    token_budget: usize,
    format: &OutputFormat,
    out: Option<&Path>,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let counter = TokenCounter::new();

    // 1. Scan
    if verbose {
        eprintln!("cxpak: scanning {}", path.display());
    }
    let scanner = Scanner::new(path)?;
    let files = scanner.scan()?;
    if verbose {
        eprintln!("cxpak: found {} files", files.len());
    }

    if files.is_empty() {
        return Err("no source files found".into());
    }

    // 2. Parse
    if verbose {
        eprintln!("cxpak: parsing with tree-sitter");
    }
    let registry = LanguageRegistry::new();
    let mut parse_results = HashMap::new();

    for file in &files {
        if let Some(lang_name) = &file.language {
            if let Some(lang) = registry.get(lang_name) {
                let source = std::fs::read_to_string(&file.absolute_path).unwrap_or_default();
                let mut parser = tree_sitter::Parser::new();
                if parser.set_language(&lang.ts_language()).is_ok() {
                    if let Some(tree) = parser.parse(&source, None) {
                        let result = lang.extract(&source, &tree);
                        parse_results.insert(file.relative_path.clone(), result);
                    }
                }
            }
        }
    }

    if verbose {
        eprintln!("cxpak: parsed {} files", parse_results.len());
    }

    // 3. Index
    let index = CodebaseIndex::build(files, parse_results, &counter);

    if verbose {
        eprintln!(
            "cxpak: indexed {} files, ~{} tokens total",
            index.total_files, index.total_tokens
        );
    }

    // Warn if budget is very small relative to repo
    if token_budget < index.total_tokens / 10 {
        eprintln!(
            "cxpak: warning: repo estimated at ~{}k tokens, budget is {}k. Output will be heavily truncated.",
            index.total_tokens / 1000,
            token_budget / 1000
        );
    }

    // 4. Budget + render sections
    let alloc = BudgetAllocation::allocate(token_budget);

    let metadata = render_metadata(&index);
    let directory_tree = render_directory_tree(&index, alloc.directory_tree, &counter);
    let module_map = render_module_map(&index, alloc.module_map, &counter);
    let dependency_graph = render_dependency_graph(&index, alloc.dependency_graph, &counter);
    let key_files = render_key_files(&index, alloc.key_files, &counter);
    let signatures = render_signatures(&index, alloc.signatures, &counter);
    let git_context = render_git_context(path, alloc.git_context, &counter);

    let sections = OutputSections {
        metadata,
        directory_tree,
        module_map,
        dependency_graph,
        key_files,
        signatures,
        git_context,
    };

    // 5. Render to format
    let rendered = output::render(&sections, format);

    // 6. Output
    match out {
        Some(path) => {
            std::fs::write(path, &rendered)?;
            if verbose {
                eprintln!("cxpak: written to {}", path.display());
            }
        }
        None => {
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            handle.write_all(rendered.as_bytes())?;
        }
    }

    Ok(())
}

fn render_metadata(index: &CodebaseIndex) -> String {
    let mut out = String::new();

    out.push_str(&format!("- **Files:** {}\n", index.total_files));
    out.push_str(&format!(
        "- **Total size:** {:.1} KB\n",
        index.total_bytes as f64 / 1024.0
    ));
    out.push_str(&format!(
        "- **Estimated tokens:** ~{}k\n",
        index.total_tokens / 1000
    ));

    if !index.language_stats.is_empty() {
        out.push_str("- **Languages:**\n");
        let mut langs: Vec<_> = index.language_stats.iter().collect();
        langs.sort_by(|a, b| b.1.file_count.cmp(&a.1.file_count));
        for (lang, stats) in &langs {
            let pct = (stats.file_count as f64 / index.total_files as f64 * 100.0) as usize;
            out.push_str(&format!(
                "  - {} — {} files ({}%)\n",
                lang, stats.file_count, pct
            ));
        }
    }

    out
}

fn render_directory_tree(
    index: &CodebaseIndex,
    budget: usize,
    counter: &TokenCounter,
) -> String {
    let mut tree = String::new();
    for file in &index.files {
        tree.push_str(&file.relative_path);
        tree.push('\n');
    }

    let (result, _, _) = degrader::truncate_to_budget(&tree, budget, counter, "directory tree");
    result
}

fn render_module_map(
    index: &CodebaseIndex,
    budget: usize,
    counter: &TokenCounter,
) -> String {
    let mut out = String::new();

    for file in &index.files {
        if let Some(pr) = &file.parse_result {
            if pr.symbols.is_empty() {
                continue;
            }
            out.push_str(&format!("### {}\n", file.relative_path));
            for sym in &pr.symbols {
                let vis = match sym.visibility {
                    crate::parser::language::Visibility::Public => "pub ",
                    crate::parser::language::Visibility::Private => "",
                };
                out.push_str(&format!(
                    "- {}{:?}: `{}`\n",
                    vis, sym.kind, sym.name
                ));
            }
            out.push('\n');
        }
    }

    let (result, _, _) = degrader::truncate_to_budget(&out, budget, counter, "module map");
    result
}

fn render_dependency_graph(
    index: &CodebaseIndex,
    budget: usize,
    counter: &TokenCounter,
) -> String {
    let mut out = String::new();

    for file in &index.files {
        if let Some(pr) = &file.parse_result {
            if pr.imports.is_empty() {
                continue;
            }
            out.push_str(&format!("**{}** imports:\n", file.relative_path));
            for imp in &pr.imports {
                if imp.names.is_empty() {
                    out.push_str(&format!("- `{}`\n", imp.source));
                } else {
                    out.push_str(&format!(
                        "- `{}` — {}\n",
                        imp.source,
                        imp.names.join(", ")
                    ));
                }
            }
            out.push('\n');
        }
    }

    let (result, _, _) =
        degrader::truncate_to_budget(&out, budget, counter, "dependency graph");
    result
}

fn render_key_files(
    index: &CodebaseIndex,
    budget: usize,
    counter: &TokenCounter,
) -> String {
    let mut out = String::new();
    let mut remaining = budget;

    let key_files: Vec<_> = index
        .files
        .iter()
        .filter(|f| CodebaseIndex::is_key_file(&f.relative_path))
        .collect();

    for file in key_files {
        let header = format!("### {}\n\n```\n", file.relative_path);
        let footer = "\n```\n\n";
        let header_tokens = counter.count(&header) + counter.count(footer);

        if remaining <= header_tokens {
            out.push_str(&degrader::omission_marker(
                &format!("key file: {}", file.relative_path),
                file.token_count,
                budget + file.token_count,
            ));
            out.push('\n');
            continue;
        }

        let content_budget = remaining - header_tokens;
        let (content, used, omitted) = degrader::truncate_to_budget(
            &file.content,
            content_budget,
            counter,
            &format!("key file: {}", file.relative_path),
        );

        out.push_str(&header);
        out.push_str(&content);
        out.push_str(footer);

        remaining = remaining.saturating_sub(used + header_tokens);
    }

    out
}

fn render_signatures(
    index: &CodebaseIndex,
    budget: usize,
    counter: &TokenCounter,
) -> String {
    let mut out = String::new();

    for file in &index.files {
        if let Some(pr) = &file.parse_result {
            let public_syms: Vec<_> = pr
                .symbols
                .iter()
                .filter(|s| s.visibility == crate::parser::language::Visibility::Public)
                .collect();

            if public_syms.is_empty() {
                continue;
            }

            out.push_str(&format!("### {}\n\n", file.relative_path));
            for sym in public_syms {
                out.push_str(&format!("```\n{}\n```\n\n", sym.signature));
            }
        }
    }

    let (result, _, _) = degrader::truncate_to_budget(&out, budget, counter, "signatures");
    result
}

fn render_git_context(
    path: &Path,
    budget: usize,
    counter: &TokenCounter,
) -> String {
    let ctx = match git::extract_git_context(path, 20) {
        Ok(ctx) => ctx,
        Err(_) => return String::new(),
    };

    let mut out = String::new();

    out.push_str("### Recent Commits\n\n");
    for commit in &ctx.recent_commits {
        out.push_str(&format!(
            "- `{}` {} — {} ({})\n",
            commit.hash, commit.message, commit.author, commit.date
        ));
    }

    out.push_str("\n### Most Changed Files\n\n");
    for file in &ctx.most_changed_files {
        out.push_str(&format!(
            "- `{}` — {} commits\n",
            file.path, file.commit_count
        ));
    }

    out.push_str("\n### Contributors\n\n");
    for contrib in &ctx.contributors {
        out.push_str(&format!(
            "- {} — {} commits\n",
            contrib.name, contrib.commit_count
        ));
    }

    let (result, _, _) = degrader::truncate_to_budget(&out, budget, counter, "git context");
    result
}
```

**Step 2: Create src/commands/trace.rs stub**

```rust
use crate::cli::OutputFormat;
use std::path::Path;

pub fn run(
    _target: &str,
    _token_budget: usize,
    _format: &OutputFormat,
    _out: Option<&Path>,
    _verbose: bool,
    _all: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("cxpak: trace command is not yet implemented (coming in v2)");
    std::process::exit(1);
}
```

**Step 3: Create src/commands/mod.rs**

```rust
pub mod overview;
pub mod trace;
```

**Step 4: Wire commands into src/main.rs**

Replace the `main()` match arms to call the command modules:

```rust
mod cli;
mod commands;

pub mod budget;
pub mod git;
pub mod index;
pub mod output;
pub mod parser;
pub mod scanner;

use clap::Parser;
use cli::{Cli, Commands, parse_token_count};

fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Overview {
            tokens,
            out,
            format,
            verbose,
            path,
        } => {
            let token_budget = match parse_token_count(tokens) {
                Ok(n) if n == 0 => {
                    eprintln!("Error: --tokens must be greater than 0");
                    std::process::exit(1);
                }
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            commands::overview::run(
                path,
                token_budget,
                format,
                out.as_deref(),
                *verbose,
            )
        }
        Commands::Trace {
            tokens,
            out,
            format,
            verbose,
            all,
            target,
        } => {
            let token_budget = match parse_token_count(tokens) {
                Ok(n) if n == 0 => {
                    eprintln!("Error: --tokens must be greater than 0");
                    std::process::exit(1);
                }
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            commands::trace::run(
                target,
                token_budget,
                format,
                out.as_deref(),
                *verbose,
                *all,
            )
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
```

**Step 5: Run `cargo build` to verify compilation**

Run: `cargo build`
Expected: Compiles successfully

**Step 6: Run all tests**

Run: `cargo test`
Expected: All tests PASS

**Step 7: Commit**

```bash
git add src/commands/ src/main.rs
git commit -m "feat: overview command — full pipeline wiring"
```

---

### Task 12: Integration Test — End to End

**Files:**
- Create: `tests/integration/overview_test.rs`
- Enhance: `tests/fixtures/simple_repo/` (add more files)

**Step 1: Write end-to-end test**

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/simple_repo")
}

#[test]
fn test_overview_markdown_output() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "50k"])
        .arg(fixture_path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## Project Metadata"))
        .stdout(predicate::str::contains("## Directory Tree"));
}

#[test]
fn test_overview_json_output() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "50k", "--format", "json"])
        .arg(fixture_path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"metadata\""));
}

#[test]
fn test_overview_xml_output() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "50k", "--format", "xml"])
        .arg(fixture_path())
        .assert()
        .success()
        .stdout(predicate::str::contains("<cxpak>"));
}

#[test]
fn test_overview_out_flag() {
    let dir = tempfile::TempDir::new().unwrap();
    let out_file = dir.path().join("output.md");

    Command::cargo_bin("cxpak")
        .unwrap()
        .args([
            "overview",
            "--tokens", "50k",
            "--out", out_file.to_str().unwrap(),
        ])
        .arg(fixture_path())
        .assert()
        .success();

    let content = std::fs::read_to_string(&out_file).unwrap();
    assert!(content.contains("## Project Metadata"));
}

#[test]
fn test_overview_small_budget_shows_omission_markers() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "500"])
        .arg(fixture_path())
        .assert()
        .success()
        .stdout(predicate::str::contains("omitted"));
}

#[test]
fn test_overview_not_git_repo() {
    let dir = tempfile::TempDir::new().unwrap();
    // No .git directory

    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "50k"])
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a git repository"));
}

#[test]
fn test_trace_not_yet_implemented() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["trace", "--tokens", "50k", "main"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not yet implemented"));
}
```

**Step 2: Ensure fixtures have a real .git directory**

The test fixture needs a proper git repo. Update the fixture setup — either init a real git repo in the fixture directory (in a build script or test setup), or create the repo in a tempdir during the test. The tempdir approach is more reliable:

Update tests to create a temp repo with known files rather than relying on a static fixture for tests that need git.

**Step 3: Run integration tests**

Run: `cargo test --test overview_test`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add tests/
git commit -m "test: end-to-end integration tests for overview command"
```

---

### Task 13: Self-Test — Run cxpak on Itself

**Files:** None — this is a manual validation step.

**Step 1: Build release binary**

Run: `cargo build --release`

**Step 2: Run cxpak on its own repo**

```bash
./target/release/cxpak overview --tokens 50k .
```

Expected: Structured markdown output with all 7 sections populated. Verify:
- Metadata shows Rust as primary language
- Directory tree shows src/ structure
- Module map lists all public types and functions
- Signatures section has the public API
- Git context shows recent commits

**Step 3: Test with small budget**

```bash
./target/release/cxpak overview --tokens 5k .
```

Expected: Heavily truncated output with omission markers.

**Step 4: Test all formats**

```bash
./target/release/cxpak overview --tokens 20k --format json .
./target/release/cxpak overview --tokens 20k --format xml .
```

**Step 5: Test --out flag**

```bash
./target/release/cxpak overview --tokens 50k --out /tmp/cxpak-self.md .
cat /tmp/cxpak-self.md
```

**Step 6: Fix any issues discovered during self-test**

**Step 7: Commit any fixes**

```bash
git add -A
git commit -m "fix: issues discovered during self-test"
```

---

### Task 14: Polish + Push

**Files:**
- Modify: `README.md` (update with real usage examples from self-test)
- Create: `.github/workflows/ci.yml`

**Step 1: Update README with real output examples**

Add a "Quick Example" section showing actual cxpak output from the self-test.

**Step 2: Create CI workflow**

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --verbose
      - run: cargo test --verbose

  clippy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - run: cargo clippy -- -D warnings

  fmt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt -- --check
```

**Step 3: Run CI checks locally**

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

**Step 4: Commit and push**

```bash
git add README.md .github/
git commit -m "chore: CI workflow and README polish"
git push origin main
```

---

---

## TDD Enforcement Patch

**MANDATORY**: Every task must follow strict Red-Green-Refactor. No implementation code is written before a failing test exists for it. Coverage is measured and gated at every commit.

### Global: Coverage Tooling (apply during Task 1)

Add `cargo-tarpaulin` to the dev workflow. Every `cargo test` step in every task below must be followed by a coverage check:

```bash
cargo tarpaulin --out Html --output-dir coverage/ --skip-clean -- --test-threads=1
# Open coverage/tarpaulin-report.html to verify 100% on changed code
```

Add to `Cargo.toml` dev-dependencies:

```toml
[dev-dependencies]
# ... existing ...
tempfile = "3"
```

Add to `.gitignore`:

```
coverage/
```

The CI workflow (Task 14) must gate on coverage:

```yaml
- name: Coverage
  run: |
    cargo install cargo-tarpaulin
    cargo tarpaulin --fail-under 95 --skip-clean -- --test-threads=1
```

---

### Task 1 Patch: Add coverage tooling setup

After Step 7 ("Verify it compiles"), add:

**Step 7b: Install and verify coverage tooling**

Run: `cargo install cargo-tarpaulin`
Run: `cargo tarpaulin --skip-clean`
Expected: 0 tests, 100% trivially (no code to cover yet)

---

### Task 2 Patch: Add missing error-path CLI tests

After the existing tests in Step 1, add these failing tests:

```rust
#[test]
fn test_overview_zero_tokens_rejected() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("must be greater than 0"));
}

#[test]
fn test_overview_invalid_tokens_rejected() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "abc"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid token count"));
}

#[test]
fn test_overview_negative_tokens_rejected() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "-5"])
        .assert()
        .failure();
}

#[test]
fn test_no_subcommand_shows_help() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn test_overview_verbose_flag() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "50k", "--verbose"])
        .assert(); // just verify it doesn't panic
}

#[test]
fn test_overview_all_formats_accepted() {
    for format in &["markdown", "xml", "json"] {
        Command::cargo_bin("cxpak")
            .unwrap()
            .args(["overview", "--tokens", "50k", "--format", format])
            .assert(); // verify format flag is accepted
    }
}

#[test]
fn test_overview_invalid_format_rejected() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "50k", "--format", "yaml"])
        .assert()
        .failure();
}
```

Add to `parse_token_count` unit tests:

```rust
#[test]
fn test_parse_token_count_whitespace() {
    assert_eq!(parse_token_count("  50k  ").unwrap(), 50000);
}

#[test]
fn test_parse_token_count_zero() {
    assert_eq!(parse_token_count("0").unwrap(), 0);
}

#[test]
fn test_parse_token_count_very_large() {
    assert_eq!(parse_token_count("1000k").unwrap(), 1_000_000);
    assert_eq!(parse_token_count("2m").unwrap(), 2_000_000);
}
```

After Step 5 add:

**Step 5b: Run coverage and verify 100% on cli module**

Run: `cargo tarpaulin --skip-clean -- --test-threads=1 2>&1 | grep "src/cli"`
Expected: 100% coverage on `src/cli/mod.rs`

---

### Task 3 Patch: Add .cxpakignore, error path, and edge case tests

Add to the test fixture `tests/fixtures/simple_repo/`:

```
├── .cxpakignore     (contains: "tests/\n*.toml")
├── node_modules/
│   └── dep/
│       └── index.js (contains: "module.exports = {};")
├── empty_dir/       (empty directory)
```

Add these tests to `scanner_test.rs`:

```rust
#[test]
fn test_scanner_respects_cxpakignore() {
    // Create a temp repo with a .cxpakignore that excludes tests/
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path();

    // Init git repo
    std::process::Command::new("git").args(["init"]).current_dir(path).output().unwrap();

    // Create files
    std::fs::create_dir_all(path.join("src")).unwrap();
    std::fs::create_dir_all(path.join("tests")).unwrap();
    std::fs::write(path.join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(path.join("tests/test.rs"), "#[test] fn t() {}").unwrap();
    std::fs::write(path.join(".cxpakignore"), "tests/").unwrap();

    let scanner = cxpak::scanner::Scanner::new(path).unwrap();
    let files = scanner.scan().unwrap();
    let paths: Vec<String> = files.iter().map(|f| f.relative_path.clone()).collect();

    assert!(paths.iter().any(|p| p.contains("src/main.rs")));
    assert!(!paths.iter().any(|p| p.contains("tests/")));
}

#[test]
fn test_scanner_builtin_ignores_node_modules() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path();

    std::process::Command::new("git").args(["init"]).current_dir(path).output().unwrap();

    std::fs::create_dir_all(path.join("node_modules/dep")).unwrap();
    std::fs::write(path.join("node_modules/dep/index.js"), "module.exports = {};").unwrap();
    std::fs::create_dir_all(path.join("src")).unwrap();
    std::fs::write(path.join("src/app.js"), "console.log('hi');").unwrap();

    let scanner = cxpak::scanner::Scanner::new(path).unwrap();
    let files = scanner.scan().unwrap();
    let paths: Vec<String> = files.iter().map(|f| f.relative_path.clone()).collect();

    assert!(!paths.iter().any(|p| p.contains("node_modules")));
    assert!(paths.iter().any(|p| p.contains("src/app.js")));
}

#[test]
fn test_scanner_not_a_git_repo() {
    let dir = tempfile::TempDir::new().unwrap();
    // No .git directory
    let result = cxpak::scanner::Scanner::new(dir.path());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not a git repository"));
}

#[test]
fn test_scanner_invalid_path() {
    let result = cxpak::scanner::Scanner::new(std::path::Path::new("/nonexistent/path/xyz"));
    assert!(result.is_err());
}

#[test]
fn test_scanner_empty_repo() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path();
    std::process::Command::new("git").args(["init"]).current_dir(path).output().unwrap();

    let scanner = cxpak::scanner::Scanner::new(path).unwrap();
    let files = scanner.scan().unwrap();
    assert!(files.is_empty());
}

#[test]
fn test_scanner_file_size_populated() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path();
    std::process::Command::new("git").args(["init"]).current_dir(path).output().unwrap();
    std::fs::write(path.join("hello.rs"), "fn main() {}").unwrap();

    let scanner = cxpak::scanner::Scanner::new(path).unwrap();
    let files = scanner.scan().unwrap();
    assert!(!files.is_empty());
    assert!(files[0].size_bytes > 0);
}
```

Add unit tests for `detect_language` covering every supported extension:

```rust
#[cfg(test)]
mod detect_tests {
    use super::detect_language;

    #[test]
    fn test_all_supported_extensions() {
        assert_eq!(detect_language("foo.rs"), Some("rust".into()));
        assert_eq!(detect_language("foo.ts"), Some("typescript".into()));
        assert_eq!(detect_language("foo.tsx"), Some("typescript".into()));
        assert_eq!(detect_language("foo.js"), Some("javascript".into()));
        assert_eq!(detect_language("foo.jsx"), Some("javascript".into()));
        assert_eq!(detect_language("foo.mjs"), Some("javascript".into()));
        assert_eq!(detect_language("foo.cjs"), Some("javascript".into()));
        assert_eq!(detect_language("foo.java"), Some("java".into()));
        assert_eq!(detect_language("foo.py"), Some("python".into()));
        assert_eq!(detect_language("foo.go"), Some("go".into()));
        assert_eq!(detect_language("foo.c"), Some("c".into()));
        assert_eq!(detect_language("foo.h"), Some("c".into()));
        assert_eq!(detect_language("foo.cpp"), Some("cpp".into()));
        assert_eq!(detect_language("foo.hpp"), Some("cpp".into()));
        assert_eq!(detect_language("foo.cc"), Some("cpp".into()));
        assert_eq!(detect_language("foo.hh"), Some("cpp".into()));
        assert_eq!(detect_language("foo.cxx"), Some("cpp".into()));
    }

    #[test]
    fn test_unsupported_extensions() {
        assert_eq!(detect_language("foo.md"), None);
        assert_eq!(detect_language("foo.txt"), None);
        assert_eq!(detect_language("foo.yaml"), None);
        assert_eq!(detect_language("foo.json"), None);
        assert_eq!(detect_language("Makefile"), None);
    }

    #[test]
    fn test_no_extension() {
        assert_eq!(detect_language("Dockerfile"), None);
    }

    #[test]
    fn test_nested_path() {
        assert_eq!(detect_language("src/deep/nested/file.rs"), Some("rust".into()));
    }
}
```

Add `ScanError` Display coverage tests:

```rust
#[test]
fn test_scan_error_display() {
    let err = ScanError::NotAGitRepo(PathBuf::from("/tmp/foo"));
    assert!(err.to_string().contains("not a git repository"));

    let err = ScanError::InvalidPath {
        path: PathBuf::from("/bad"),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "nope"),
    };
    assert!(err.to_string().contains("invalid path"));
}
```

After Step 7 add:

**Step 7b: Run coverage and verify 100% on scanner module**

Run: `cargo tarpaulin --skip-clean -- --test-threads=1 2>&1 | grep "src/scanner"`
Expected: 100% coverage on `src/scanner/mod.rs` and `src/scanner/defaults.rs`

---

### Task 4 Patch: Add edge case tests for token counter

Add these tests:

```rust
#[test]
fn test_count_unicode() {
    let counter = TokenCounter::new();
    let count = counter.count("こんにちは世界");
    assert!(count > 0);
}

#[test]
fn test_count_very_long_input() {
    let counter = TokenCounter::new();
    let long_text = "word ".repeat(10000);
    let count = counter.count(&long_text);
    assert!(count > 1000);
}

#[test]
fn test_count_special_characters() {
    let counter = TokenCounter::new();
    let count = counter.count("!@#$%^&*()_+-=[]{}|;':\",./<>?");
    assert!(count > 0);
}

#[test]
fn test_count_or_zero_nonempty() {
    let counter = TokenCounter::new();
    let count = counter.count_or_zero("hello");
    assert!(count > 0);
    assert_eq!(count, counter.count("hello"));
}

#[test]
fn test_count_newlines_only() {
    let counter = TokenCounter::new();
    let count = counter.count("\n\n\n");
    assert!(count > 0);
}
```

**Enforce TDD order**: Write tests first in a separate file, run to see them fail, then add the implementation.

After Step 4 add:

**Step 4b: Run coverage and verify 100% on budget/counter module**

Run: `cargo tarpaulin --skip-clean -- --test-threads=1 2>&1 | grep "src/budget"`
Expected: 100% coverage on `src/budget/counter.rs`

---

### Task 5 Patch: Add edge case and error path tests for parser

Add to the Rust language tests:

```rust
#[test]
fn test_extract_empty_source() {
    let result = parse_rust("");
    assert!(result.symbols.is_empty());
    assert!(result.imports.is_empty());
    assert!(result.exports.is_empty());
}

#[test]
fn test_extract_comment_only() {
    let result = parse_rust("// just a comment\n/* block comment */");
    assert!(result.symbols.is_empty());
}

#[test]
fn test_extract_multiple_functions() {
    let source = "pub fn a() {}\npub fn b() {}\nfn c() {}";
    let result = parse_rust(source);
    assert_eq!(result.symbols.len(), 3);
    assert_eq!(result.exports.len(), 2); // only pub ones
}

#[test]
fn test_extract_const() {
    let source = "pub const MAX: usize = 100;";
    let result = parse_rust(source);
    // Constants may or may not be extracted depending on implementation.
    // This test documents the behavior.
}

#[test]
fn test_extract_type_alias() {
    let source = "pub type Result<T> = std::result::Result<T, Error>;";
    let result = parse_rust(source);
    // Documents behavior for type aliases
}

#[test]
fn test_extract_nested_impl_in_mod() {
    let source = r#"
mod inner {
    pub struct Bar;
    impl Bar {
        pub fn method(&self) {}
    }
}
"#;
    let result = parse_rust(source);
    // Documents behavior for nested modules
}

#[test]
fn test_extract_multiple_use_statements() {
    let source = "use std::io;\nuse std::collections::HashMap;\nuse std::fmt::Display;";
    let result = parse_rust(source);
    assert_eq!(result.imports.len(), 3);
}

#[test]
fn test_extract_grouped_use() {
    let source = "use std::collections::{HashMap, BTreeMap};";
    let result = parse_rust(source);
    assert!(!result.imports.is_empty());
}

#[test]
fn test_function_line_numbers() {
    let source = "\n\npub fn hello() {\n}\n";
    let result = parse_rust(source);
    assert_eq!(result.symbols[0].start_line, 3);
    assert_eq!(result.symbols[0].end_line, 4);
}
```

Add registry tests:

```rust
#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn test_registry_has_rust() {
        let registry = LanguageRegistry::new();
        assert!(registry.get("rust").is_some());
    }

    #[test]
    fn test_registry_unknown_language() {
        let registry = LanguageRegistry::new();
        assert!(registry.get("brainfuck").is_none());
    }

    #[test]
    fn test_registry_supported_languages_nonempty() {
        let registry = LanguageRegistry::new();
        assert!(!registry.supported_languages().is_empty());
    }
}
```

After Step 5 add:

**Step 5b: Run coverage and verify 100% on parser module**

Run: `cargo tarpaulin --skip-clean -- --test-threads=1 2>&1 | grep "src/parser"`
Expected: 100% coverage on all parser files

---

### Task 6 Patch: Mandatory test matrix per language

**CRITICAL**: Task 6 is the biggest gap. Each language implementation MUST have these tests before the implementation is written:

For **each** of the 7 languages (TypeScript, JavaScript, Java, Python, Go, C, C++), write the following test suite FIRST, run it to see it FAIL, then implement:

```rust
// Template — replace Lang with actual language struct, adjust source snippets

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_LANG(source: &str) -> ParseResult {
        let lang = LANGLanguage;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang.ts_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        lang.extract(source, &tree)
    }

    #[test]
    fn test_extract_empty_source() { /* ... */ }

    #[test]
    fn test_extract_public_function() { /* ... */ }

    #[test]
    fn test_extract_private_function() { /* ... */ }

    #[test]
    fn test_extract_class_or_struct() { /* ... */ }

    #[test]
    fn test_extract_interface_or_trait() { /* ... */ }

    #[test]
    fn test_extract_enum() { /* ... */ }

    #[test]
    fn test_extract_imports() { /* ... */ }

    #[test]
    fn test_extract_multiple_imports() { /* ... */ }

    #[test]
    fn test_extract_exports() { /* ... */ }

    #[test]
    fn test_extract_methods_in_class() { /* ... */ }

    #[test]
    fn test_visibility_detection() { /* ... */ }

    #[test]
    fn test_line_numbers_correct() { /* ... */ }

    #[test]
    fn test_signature_extraction() { /* ... */ }

    #[test]
    fn test_comment_only_file() { /* ... */ }
}
```

**Per-language source snippets** (use idiomatic code for each):

**TypeScript:**
```typescript
// Public function
export function greet(name: string): string { return `hello ${name}`; }
// Interface
export interface User { name: string; age: number; }
// Class
export class UserService { constructor() {} getUser(): User { return { name: "", age: 0 }; } }
// Enum
export enum Status { Active, Inactive }
// Import
import { Something } from './module';
```

**JavaScript:**
```javascript
// Function
export function add(a, b) { return a + b; }
// Class
export class Calculator { add(a, b) { return a + b; } }
// Import
import { foo } from './bar';
const baz = require('./qux');
```

**Java:**
```java
// Class
public class UserService {
    private String name;
    public UserService(String name) { this.name = name; }
    public String getName() { return name; }
    private void internal() {}
}
// Interface
public interface Repository { void save(Object entity); }
// Enum
public enum Status { ACTIVE, INACTIVE }
// Import
import java.util.List;
```

**Python:**
```python
# Function
def public_func(x: int) -> int:
    return x + 1

def _private_func():
    pass

# Class
class MyClass:
    def method(self):
        pass

    def _private_method(self):
        pass

# Imports
import os
from collections import defaultdict
```

**Go:**
```go
// Public function (capitalized)
func PublicFunc() string { return "hello" }
// Private function
func privateFunc() {}
// Struct
type Config struct { Name string; port int }
// Interface
type Reader interface { Read(p []byte) (n int, err error) }
// Import
import "fmt"
import ( "os"; "io" )
```

**C:**
```c
#include <stdio.h>
#include "myheader.h"
void public_func(int x) { printf("%d", x); }
static void private_func() {}
struct Config { int port; char* name; };
enum Status { ACTIVE, INACTIVE };
typedef struct { int x; int y; } Point;
```

**C++:**
```cpp
#include <string>
#include "myheader.h"
class MyClass {
public:
    void publicMethod() {}
    int getValue() const { return value; }
private:
    int value;
    void privateMethod() {}
};
namespace utils { void helper() {} }
```

**Step order for each language:**
1. Write the full test suite with language-appropriate snippets
2. Run: `cargo test parser::languages::LANG` — verify FAIL
3. Implement the language support
4. Run: `cargo test parser::languages::LANG` — verify PASS
5. Run: `cargo tarpaulin --skip-clean -- --test-threads=1 2>&1 | grep "src/parser/languages/LANG"`
6. Verify 100% coverage
7. Commit: `git commit -m "feat: LANG language support with tests"`

---

### Task 7 Patch: Add index building and query tests

Add these tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::language::*;

    fn make_scanned_file(path: &str, lang: Option<&str>, content: &str) -> (ScannedFile, String) {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join(path);
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, content).unwrap();

        (ScannedFile {
            relative_path: path.to_string(),
            absolute_path: file_path,
            language: lang.map(|l| l.to_string()),
            size_bytes: content.len() as u64,
        }, content.to_string())
    }

    #[test]
    fn test_build_index_empty() {
        let counter = TokenCounter::new();
        let index = CodebaseIndex::build(vec![], HashMap::new(), &counter);
        assert_eq!(index.total_files, 0);
        assert_eq!(index.total_bytes, 0);
        assert_eq!(index.total_tokens, 0);
    }

    #[test]
    fn test_build_index_language_stats() {
        // Create temp files and build index, verify language_stats counts
    }

    #[test]
    fn test_all_public_symbols_filters_private() {
        // Build index with mixed visibility symbols, verify only public returned
    }

    #[test]
    fn test_all_public_symbols_empty_when_no_parse_results() {
        // Build index with no parse results, verify empty
    }

    #[test]
    fn test_all_imports() {
        // Build index with imports, verify they are collected
    }

    #[test]
    fn test_all_imports_empty() {
        // Build index with no imports, verify empty
    }

    #[test]
    fn test_is_key_file_all_variants() {
        // Test every key file pattern
        assert!(CodebaseIndex::is_key_file("README.md"));
        assert!(CodebaseIndex::is_key_file("readme.md"));
        assert!(CodebaseIndex::is_key_file("README"));
        assert!(CodebaseIndex::is_key_file("Cargo.toml"));
        assert!(CodebaseIndex::is_key_file("package.json"));
        assert!(CodebaseIndex::is_key_file("pom.xml"));
        assert!(CodebaseIndex::is_key_file("build.gradle"));
        assert!(CodebaseIndex::is_key_file("build.gradle.kts"));
        assert!(CodebaseIndex::is_key_file("go.mod"));
        assert!(CodebaseIndex::is_key_file("pyproject.toml"));
        assert!(CodebaseIndex::is_key_file("setup.py"));
        assert!(CodebaseIndex::is_key_file("setup.cfg"));
        assert!(CodebaseIndex::is_key_file("makefile"));
        assert!(CodebaseIndex::is_key_file("Makefile"));
        assert!(CodebaseIndex::is_key_file("dockerfile"));
        assert!(CodebaseIndex::is_key_file("Dockerfile"));
        assert!(CodebaseIndex::is_key_file("docker-compose.yml"));
        assert!(CodebaseIndex::is_key_file("docker-compose.yaml"));
        assert!(CodebaseIndex::is_key_file(".env.example"));
        assert!(CodebaseIndex::is_key_file("src/main.rs"));
        assert!(CodebaseIndex::is_key_file("src/main.go"));
        assert!(CodebaseIndex::is_key_file("src/main.py"));
        assert!(CodebaseIndex::is_key_file("src/main.java"));
        assert!(CodebaseIndex::is_key_file("app.py"));
        assert!(CodebaseIndex::is_key_file("src/index.ts"));
        assert!(CodebaseIndex::is_key_file("src/index.js"));
    }

    #[test]
    fn test_is_key_file_negative() {
        assert!(!CodebaseIndex::is_key_file("src/utils.rs"));
        assert!(!CodebaseIndex::is_key_file("tests/test_foo.py"));
        assert!(!CodebaseIndex::is_key_file("lib/helper.js"));
        assert!(!CodebaseIndex::is_key_file("src/service.java"));
    }
}
```

Add `DependencyGraph` tests:

```rust
#[cfg(test)]
mod graph_tests {
    use super::*;

    #[test]
    fn test_graph_add_edge() {
        let mut graph = DependencyGraph::new();
        graph.add_edge("a.rs", "b.rs");
        assert!(graph.dependencies("a.rs").unwrap().contains("b.rs"));
    }

    #[test]
    fn test_graph_dependents() {
        let mut graph = DependencyGraph::new();
        graph.add_edge("a.rs", "b.rs");
        graph.add_edge("c.rs", "b.rs");
        let deps = graph.dependents("b.rs");
        assert!(deps.contains(&"a.rs"));
        assert!(deps.contains(&"c.rs"));
    }

    #[test]
    fn test_graph_no_dependencies() {
        let graph = DependencyGraph::new();
        assert!(graph.dependencies("nonexistent.rs").is_none());
    }

    #[test]
    fn test_graph_no_dependents() {
        let graph = DependencyGraph::new();
        assert!(graph.dependents("nonexistent.rs").is_empty());
    }

    #[test]
    fn test_graph_multiple_deps() {
        let mut graph = DependencyGraph::new();
        graph.add_edge("a.rs", "b.rs");
        graph.add_edge("a.rs", "c.rs");
        let deps = graph.dependencies("a.rs").unwrap();
        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn test_graph_duplicate_edge() {
        let mut graph = DependencyGraph::new();
        graph.add_edge("a.rs", "b.rs");
        graph.add_edge("a.rs", "b.rs"); // duplicate
        assert_eq!(graph.dependencies("a.rs").unwrap().len(), 1);
    }
}
```

After Step 4 add:

**Step 4b: Run coverage and verify 100% on index module**

Run: `cargo tarpaulin --skip-clean -- --test-threads=1 2>&1 | grep "src/index"`
Expected: 100% on all index files

---

### Task 8 Patch: Add budget edge case and surplus redistribution tests

Add these tests to `src/budget/mod.rs`:

```rust
#[test]
fn test_allocate_metadata_larger_than_budget() {
    // Budget smaller than the fixed metadata allocation
    let alloc = BudgetAllocation::allocate(200);
    // metadata should be capped, other sections should be 0
    assert!(alloc.total() <= 200);
}

#[test]
fn test_allocate_exact_500() {
    let alloc = BudgetAllocation::allocate(500);
    assert_eq!(alloc.metadata, 500);
    assert_eq!(alloc.directory_tree, 0);
    assert_eq!(alloc.git_context, 0);
}

#[test]
fn test_allocate_proportions_correct() {
    let alloc = BudgetAllocation::allocate(100_500); // 500 metadata + 100k distributable
    assert_eq!(alloc.directory_tree, 5000);   // 5%
    assert_eq!(alloc.module_map, 20000);      // 20%
    assert_eq!(alloc.dependency_graph, 15000); // 15%
    assert_eq!(alloc.key_files, 20000);        // 20%
    assert_eq!(alloc.signatures, 30000);       // 30%
    assert_eq!(alloc.git_context, 10000);      // 10%
}

#[test]
fn test_allocate_1m_budget() {
    let alloc = BudgetAllocation::allocate(1_000_000);
    assert!(alloc.total() <= 1_000_000);
    assert!(alloc.signatures > 200_000);
}
```

Add these tests to `src/budget/degrader.rs`:

```rust
#[test]
fn test_omission_marker_exact_1k_boundary() {
    let marker = omission_marker("test", 1000, 2000);
    assert!(marker.contains("~1.0k"));
    assert!(marker.contains("2k+"));
}

#[test]
fn test_omission_marker_sub_1k_boundary() {
    let marker = omission_marker("test", 999, 999);
    assert!(marker.contains("~999"));
    assert!(marker.contains("999+"));
}

#[test]
fn test_truncate_zero_budget() {
    let counter = crate::budget::counter::TokenCounter::new();
    let content = "hello world";
    let (result, _used, omitted) = truncate_to_budget(content, 0, &counter, "test");
    assert!(omitted > 0);
    assert!(result.contains("<!-- test omitted"));
}

#[test]
fn test_truncate_budget_exactly_fits() {
    let counter = crate::budget::counter::TokenCounter::new();
    let content = "hi";
    let total = counter.count(content);
    let (result, used, omitted) = truncate_to_budget(content, total, &counter, "test");
    assert_eq!(omitted, 0);
    assert_eq!(used, total);
    assert_eq!(result, content);
}

#[test]
fn test_truncate_multiline_partial() {
    let counter = crate::budget::counter::TokenCounter::new();
    let content = (0..50)
        .map(|i| format!("line {} with some text", i))
        .collect::<Vec<_>>()
        .join("\n");

    let total_tokens = counter.count(&content);
    let budget = total_tokens / 2;
    let (result, _used, omitted) = truncate_to_budget(&content, budget, &counter, "partial");

    assert!(omitted > 0);
    assert!(result.contains("<!-- partial omitted"));
    assert!(result.lines().count() < content.lines().count());
}

#[test]
fn test_truncate_empty_content() {
    let counter = crate::budget::counter::TokenCounter::new();
    let (result, used, omitted) = truncate_to_budget("", 100, &counter, "empty");
    assert_eq!(result, "");
    assert_eq!(used, 0);
    assert_eq!(omitted, 0);
}

#[test]
fn test_truncate_single_line_exceeds() {
    let counter = crate::budget::counter::TokenCounter::new();
    // One very long line that exceeds budget
    let content = "word ".repeat(1000);
    let (result, _used, omitted) = truncate_to_budget(&content, 10, &counter, "long line");
    assert!(omitted > 0);
}
```

After Step 3 add:

**Step 3b: Run coverage and verify 100% on budget module**

Run: `cargo tarpaulin --skip-clean -- --test-threads=1 2>&1 | grep "src/budget"`
Expected: 100% on `src/budget/mod.rs`, `src/budget/counter.rs`, `src/budget/degrader.rs`

---

### Task 9 Patch: Add git edge case tests

Add these tests:

```rust
#[test]
fn test_extract_git_context_empty_repo() {
    let dir = TempDir::new().unwrap();
    let path = dir.path();
    Command::new("git").args(["init"]).current_dir(path).output().unwrap();
    Command::new("git").args(["config", "user.email", "t@t.com"]).current_dir(path).output().unwrap();
    Command::new("git").args(["config", "user.name", "T"]).current_dir(path).output().unwrap();

    // Repo with no commits — revwalk should handle gracefully
    let result = extract_git_context(path, 20);
    // May error (no HEAD) or return empty — either is acceptable, but must not panic
    match result {
        Ok(ctx) => assert!(ctx.recent_commits.is_empty()),
        Err(_) => {} // acceptable
    }
}

#[test]
fn test_extract_git_context_single_commit() {
    let dir = TempDir::new().unwrap();
    let path = dir.path();
    Command::new("git").args(["init"]).current_dir(path).output().unwrap();
    Command::new("git").args(["config", "user.email", "t@t.com"]).current_dir(path).output().unwrap();
    Command::new("git").args(["config", "user.name", "T"]).current_dir(path).output().unwrap();
    std::fs::write(path.join("f.txt"), "x").unwrap();
    Command::new("git").args(["add", "."]).current_dir(path).output().unwrap();
    Command::new("git").args(["commit", "-m", "first"]).current_dir(path).output().unwrap();

    let ctx = extract_git_context(path, 20).unwrap();
    assert_eq!(ctx.recent_commits.len(), 1);
    assert_eq!(ctx.recent_commits[0].message, "first");
    assert_eq!(ctx.contributors.len(), 1);
}

#[test]
fn test_extract_git_context_max_commits_limit() {
    let dir = TempDir::new().unwrap();
    let path = dir.path();
    Command::new("git").args(["init"]).current_dir(path).output().unwrap();
    Command::new("git").args(["config", "user.email", "t@t.com"]).current_dir(path).output().unwrap();
    Command::new("git").args(["config", "user.name", "T"]).current_dir(path).output().unwrap();

    // Create 10 commits
    for i in 0..10 {
        std::fs::write(path.join("f.txt"), format!("v{i}")).unwrap();
        Command::new("git").args(["add", "."]).current_dir(path).output().unwrap();
        Command::new("git").args(["commit", "-m", &format!("commit {i}")]).current_dir(path).output().unwrap();
    }

    // Request only 3
    let ctx = extract_git_context(path, 3).unwrap();
    assert_eq!(ctx.recent_commits.len(), 3);
}

#[test]
fn test_extract_git_context_multiple_contributors() {
    let dir = TempDir::new().unwrap();
    let path = dir.path();
    Command::new("git").args(["init"]).current_dir(path).output().unwrap();

    // Commit as user A
    Command::new("git").args(["config", "user.email", "a@a.com"]).current_dir(path).output().unwrap();
    Command::new("git").args(["config", "user.name", "Alice"]).current_dir(path).output().unwrap();
    std::fs::write(path.join("a.txt"), "a").unwrap();
    Command::new("git").args(["add", "."]).current_dir(path).output().unwrap();
    Command::new("git").args(["commit", "-m", "alice commit"]).current_dir(path).output().unwrap();

    // Commit as user B
    Command::new("git").args(["config", "user.name", "Bob"]).current_dir(path).output().unwrap();
    Command::new("git").args(["config", "user.email", "b@b.com"]).current_dir(path).output().unwrap();
    std::fs::write(path.join("b.txt"), "b").unwrap();
    Command::new("git").args(["add", "."]).current_dir(path).output().unwrap();
    Command::new("git").args(["commit", "-m", "bob commit"]).current_dir(path).output().unwrap();

    let ctx = extract_git_context(path, 20).unwrap();
    assert_eq!(ctx.contributors.len(), 2);
}

#[test]
fn test_extract_git_context_most_changed_files_sorted() {
    let dir = TempDir::new().unwrap();
    let path = dir.path();
    Command::new("git").args(["init"]).current_dir(path).output().unwrap();
    Command::new("git").args(["config", "user.email", "t@t.com"]).current_dir(path).output().unwrap();
    Command::new("git").args(["config", "user.name", "T"]).current_dir(path).output().unwrap();

    // Change hot.txt 5 times, cold.txt once
    std::fs::write(path.join("cold.txt"), "x").unwrap();
    std::fs::write(path.join("hot.txt"), "x").unwrap();
    Command::new("git").args(["add", "."]).current_dir(path).output().unwrap();
    Command::new("git").args(["commit", "-m", "init"]).current_dir(path).output().unwrap();

    for i in 0..4 {
        std::fs::write(path.join("hot.txt"), format!("v{i}")).unwrap();
        Command::new("git").args(["add", "."]).current_dir(path).output().unwrap();
        Command::new("git").args(["commit", "-m", &format!("update hot {i}")]).current_dir(path).output().unwrap();
    }

    let ctx = extract_git_context(path, 20).unwrap();
    assert_eq!(ctx.most_changed_files[0].path, "hot.txt");
    assert!(ctx.most_changed_files[0].commit_count > ctx.most_changed_files.last().unwrap().commit_count);
}

#[test]
fn test_extract_git_context_not_a_repo() {
    let dir = TempDir::new().unwrap();
    let result = extract_git_context(dir.path(), 20);
    assert!(result.is_err());
}
```

After Step 3 add:

**Step 3b: Run coverage and verify 100% on git module**

Run: `cargo tarpaulin --skip-clean -- --test-threads=1 2>&1 | grep "src/git"`
Expected: 100% on `src/git/mod.rs`

---

### Task 10 Patch: Add renderer tests for all formats

Add XML renderer tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::OutputSections;

    #[test]
    fn test_xml_render_includes_sections() {
        let sections = OutputSections {
            metadata: "Rust project".into(),
            directory_tree: "src/\n  main.rs".into(),
            module_map: String::new(),
            dependency_graph: String::new(),
            key_files: String::new(),
            signatures: String::new(),
            git_context: String::new(),
        };
        let output = render(&sections);
        assert!(output.starts_with("<cxpak>"));
        assert!(output.ends_with("</cxpak>\n"));
        assert!(output.contains("<metadata>"));
        assert!(output.contains("</metadata>"));
        assert!(output.contains("<directory-tree>"));
        assert!(!output.contains("<module-map>")); // empty section omitted
    }

    #[test]
    fn test_xml_escapes_special_chars() {
        let sections = OutputSections {
            metadata: "a < b && c > d \"quoted\"".into(),
            directory_tree: String::new(),
            module_map: String::new(),
            dependency_graph: String::new(),
            key_files: String::new(),
            signatures: String::new(),
            git_context: String::new(),
        };
        let output = render(&sections);
        assert!(output.contains("&lt;"));
        assert!(output.contains("&amp;"));
        assert!(output.contains("&gt;"));
        assert!(output.contains("&quot;"));
    }

    #[test]
    fn test_xml_all_empty() {
        let sections = OutputSections {
            metadata: String::new(),
            directory_tree: String::new(),
            module_map: String::new(),
            dependency_graph: String::new(),
            key_files: String::new(),
            signatures: String::new(),
            git_context: String::new(),
        };
        let output = render(&sections);
        assert_eq!(output, "<cxpak>\n</cxpak>\n");
    }
}
```

Add JSON renderer tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::OutputSections;

    #[test]
    fn test_json_render_valid_json() {
        let sections = OutputSections {
            metadata: "test".into(),
            directory_tree: String::new(),
            module_map: String::new(),
            dependency_graph: String::new(),
            key_files: String::new(),
            signatures: String::new(),
            git_context: String::new(),
        };
        let output = render(&sections);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["metadata"], "test");
    }

    #[test]
    fn test_json_skips_empty_sections() {
        let sections = OutputSections {
            metadata: "test".into(),
            directory_tree: String::new(),
            module_map: String::new(),
            dependency_graph: String::new(),
            key_files: String::new(),
            signatures: String::new(),
            git_context: String::new(),
        };
        let output = render(&sections);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("directory_tree").is_none());
    }

    #[test]
    fn test_json_all_sections_present() {
        let sections = OutputSections {
            metadata: "a".into(),
            directory_tree: "b".into(),
            module_map: "c".into(),
            dependency_graph: "d".into(),
            key_files: "e".into(),
            signatures: "f".into(),
            git_context: "g".into(),
        };
        let output = render(&sections);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["metadata"], "a");
        assert_eq!(parsed["directory_tree"], "b");
        assert_eq!(parsed["git_context"], "g");
    }
}
```

Add `output::render` dispatch test:

```rust
#[cfg(test)]
mod dispatch_tests {
    use super::*;
    use crate::cli::OutputFormat;

    fn test_sections() -> OutputSections {
        OutputSections {
            metadata: "test".into(),
            directory_tree: String::new(),
            module_map: String::new(),
            dependency_graph: String::new(),
            key_files: String::new(),
            signatures: String::new(),
            git_context: String::new(),
        }
    }

    #[test]
    fn test_render_markdown() {
        let out = render(&test_sections(), &OutputFormat::Markdown);
        assert!(out.contains("## Project Metadata"));
    }

    #[test]
    fn test_render_xml() {
        let out = render(&test_sections(), &OutputFormat::Xml);
        assert!(out.contains("<cxpak>"));
    }

    #[test]
    fn test_render_json() {
        let out = render(&test_sections(), &OutputFormat::Json);
        assert!(out.contains("\"metadata\""));
    }
}
```

After Step 6 add:

**Step 6b: Run coverage and verify 100% on output module**

Run: `cargo tarpaulin --skip-clean -- --test-threads=1 2>&1 | grep "src/output"`
Expected: 100% on all output files

---

### Task 11 Patch: Add unit tests for overview orchestration

The overview command has render helper functions (`render_metadata`, `render_directory_tree`, etc.) that are currently only tested through integration tests. Add unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::counter::TokenCounter;

    // Test render_metadata
    #[test]
    fn test_render_metadata_empty_index() {
        let index = CodebaseIndex {
            files: vec![],
            language_stats: HashMap::new(),
            total_files: 0,
            total_bytes: 0,
            total_tokens: 0,
        };
        let result = render_metadata(&index);
        assert!(result.contains("Files:** 0"));
    }

    #[test]
    fn test_render_metadata_with_languages() {
        let mut stats = HashMap::new();
        stats.insert("rust".into(), crate::index::LanguageStats {
            file_count: 5,
            total_bytes: 1000,
            total_tokens: 500,
        });
        let index = CodebaseIndex {
            files: vec![],
            language_stats: stats,
            total_files: 5,
            total_bytes: 1000,
            total_tokens: 500,
        };
        let result = render_metadata(&index);
        assert!(result.contains("rust"));
        assert!(result.contains("5 files"));
    }

    // Test render_directory_tree
    #[test]
    fn test_render_directory_tree_within_budget() {
        let counter = TokenCounter::new();
        // Build a small index with a few files
        let index = CodebaseIndex {
            files: vec![
                IndexedFile {
                    relative_path: "src/main.rs".into(),
                    language: Some("rust".into()),
                    size_bytes: 100,
                    token_count: 50,
                    parse_result: None,
                    content: String::new(),
                },
            ],
            language_stats: HashMap::new(),
            total_files: 1,
            total_bytes: 100,
            total_tokens: 50,
        };
        let result = render_directory_tree(&index, 1000, &counter);
        assert!(result.contains("src/main.rs"));
    }

    // Test render_git_context with non-repo
    #[test]
    fn test_render_git_context_not_a_repo() {
        let counter = TokenCounter::new();
        let dir = tempfile::TempDir::new().unwrap();
        let result = render_git_context(dir.path(), 1000, &counter);
        assert!(result.is_empty()); // graceful fallback
    }
}
```

After Step 6 add:

**Step 6b: Run coverage and verify 100% on commands module**

Run: `cargo tarpaulin --skip-clean -- --test-threads=1 2>&1 | grep "src/commands"`
Expected: 100% on `src/commands/overview.rs`, 100% on `src/commands/trace.rs`

---

### Task 12 Patch: Expand integration tests

Add these integration tests:

```rust
#[test]
fn test_overview_verbose_shows_progress() {
    let dir = create_test_repo(); // helper that creates a temp git repo
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "50k", "--verbose"])
        .arg(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("cxpak: scanning"));
}

#[test]
fn test_overview_tiny_budget_warns() {
    let dir = create_large_test_repo(); // helper with many files
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "100"])
        .arg(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("heavily truncated"));
}

#[test]
fn test_overview_output_is_valid_json() {
    let dir = create_test_repo();
    let output = Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "50k", "--format", "json"])
        .arg(dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let _: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON output");
}

#[test]
fn test_overview_output_is_valid_xml() {
    let dir = create_test_repo();
    let output = Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "50k", "--format", "xml"])
        .arg(dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("<cxpak>"));
    assert!(stdout.trim().ends_with("</cxpak>"));
}

#[test]
fn test_overview_out_flag_creates_file() {
    let dir = create_test_repo();
    let out_dir = tempfile::TempDir::new().unwrap();
    let out_file = out_dir.path().join("result.md");

    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "50k", "--out", out_file.to_str().unwrap()])
        .arg(dir.path())
        .assert()
        .success();

    assert!(out_file.exists());
    let content = std::fs::read_to_string(&out_file).unwrap();
    assert!(!content.is_empty());
    assert!(content.contains("## Project Metadata"));
}

#[test]
fn test_overview_out_flag_stdout_is_empty() {
    let dir = create_test_repo();
    let out_dir = tempfile::TempDir::new().unwrap();
    let out_file = out_dir.path().join("result.md");

    let output = Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "50k", "--out", out_file.to_str().unwrap()])
        .arg(dir.path())
        .output()
        .unwrap();

    // stdout should be empty when --out is used
    assert!(output.stdout.is_empty());
}
```

---

### Task 14 Patch: Coverage gate in CI

Update the CI workflow to include a hard coverage gate:

```yaml
  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin
      - name: Run coverage
        run: cargo tarpaulin --fail-under 95 --skip-clean -- --test-threads=1
```

**Why 95 and not 100**: Some lines are genuinely unreachable in tests (e.g., `process::exit`, `unwrap` on infallible paths). Target 100% on business logic, accept 95% as the CI gate to avoid flaky failures on platform-specific code paths.

---

## TDD Workflow Enforcement — Mandatory Per-Task Sequence

Every task MUST follow this exact sequence. No exceptions.

```
1. Write failing test(s) for the feature/function
2. Run: cargo test — verify FAIL with expected error
3. Write minimal implementation to make test pass
4. Run: cargo test — verify PASS
5. Run: cargo tarpaulin --skip-clean -- --test-threads=1
6. Verify 100% coverage on changed files
7. Refactor if needed (tests must still pass)
8. Commit ONLY when tests pass AND coverage is 100%
```

If at step 6 any line is uncovered:
- Write a test that hits that line
- Go back to step 4

---

## Task Summary

| Task | Description | Depends On |
|------|-------------|------------|
| 1 | Project scaffold + Cargo.toml | — |
| 2 | CLI argument parsing | 1 |
| 3 | Scanner — file discovery + ignore rules | 1 |
| 4 | Token counter | 1 |
| 5 | Language trait + Rust support | 1 |
| 6 | Remaining language implementations | 5 |
| 7 | Index — central data structure | 3, 4, 5 |
| 8 | Budget allocation + degradation | 4 |
| 9 | Git context | 1 |
| 10 | Output renderers | — |
| 11 | Overview command — orchestration | 2, 3, 7, 8, 9, 10 |
| 12 | Integration tests | 11 |
| 13 | Self-test on own repo | 12 |
| 14 | Polish + CI + push | 13 |

### Parallelizable groups:
- **Tasks 2, 3, 4, 5, 9, 10** can all be done in parallel after Task 1
- **Task 6** depends only on Task 5
- **Tasks 7, 8** can be done in parallel after their deps
- **Tasks 11–14** are sequential
