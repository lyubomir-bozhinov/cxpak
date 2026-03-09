pub mod defaults;

use std::fmt;
use std::path::{Path, PathBuf};

use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;

use defaults::BUILTIN_IGNORES;

/// A single file discovered by the scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedFile {
    /// Path relative to the scanned root, using forward slashes.
    pub relative_path: String,
    /// Absolute path on disk.
    pub absolute_path: PathBuf,
    /// Detected programming language, if any.
    pub language: Option<String>,
    /// File size in bytes.
    pub size_bytes: u64,
}

/// Errors that can occur during scanning.
#[derive(Debug)]
pub enum ScanError {
    /// The given root path does not contain a `.git` directory, so it is not
    /// recognised as a repository root.
    NotARepository(PathBuf),
    /// An I/O or `ignore`-crate error occurred during directory walking.
    Walk(String),
    /// Failed to build the override rule set.
    Override(String),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::NotARepository(p) => {
                write!(
                    f,
                    "not a git repository (no .git directory found in {})",
                    p.display()
                )
            }
            ScanError::Walk(msg) => write!(f, "directory walk error: {msg}"),
            ScanError::Override(msg) => write!(f, "override builder error: {msg}"),
        }
    }
}

impl std::error::Error for ScanError {}

/// Scans a repository root, honouring `.gitignore`, an optional `.cxpakignore`,
/// and a set of built-in default patterns.
pub struct Scanner {
    root: PathBuf,
}

impl Scanner {
    /// Create a new `Scanner` rooted at `root`.
    ///
    /// Returns `ScanError::NotARepository` when `root/.git` does not exist.
    pub fn new(root: &Path) -> Result<Self, ScanError> {
        let git_dir = root.join(".git");
        if !git_dir.exists() {
            return Err(ScanError::NotARepository(root.to_path_buf()));
        }
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    /// Walk the repository and return all matching files, sorted by relative path.
    pub fn scan(&self) -> Result<Vec<ScannedFile>, ScanError> {
        // Build override rules that exclude built-in patterns.
        // The `ignore` crate treats overrides as *include* rules; prefixing with `!`
        // makes them negative (i.e. "exclude these").
        let mut override_builder = OverrideBuilder::new(&self.root);
        for pattern in BUILTIN_IGNORES {
            // A `!` prefix means "this path should NOT be included".
            let negated = format!("!{pattern}");
            override_builder
                .add(&negated)
                .map_err(|e| ScanError::Override(e.to_string()))?;
        }
        let overrides = override_builder
            .build()
            .map_err(|e| ScanError::Override(e.to_string()))?;

        // Build the walker.
        let mut builder = WalkBuilder::new(&self.root);
        builder
            .git_ignore(true) // respect .gitignore
            .git_global(false) // skip global gitignore for determinism
            .git_exclude(false) // skip .git/info/exclude for determinism
            .hidden(true) // visit hidden files (we handle .git via overrides)
            .overrides(overrides);

        // Load .cxpakignore if present.
        let cxpak_ignore = self.root.join(".cxpakignore");
        if cxpak_ignore.is_file() {
            builder.add_ignore(&cxpak_ignore);
        }

        let mut files: Vec<ScannedFile> = Vec::new();

        for result in builder.build() {
            let entry = result.map_err(|e| ScanError::Walk(e.to_string()))?;

            // Skip directories themselves; we only want files.
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                continue;
            }

            let absolute_path = entry.path().to_path_buf();

            // Compute relative path from root, normalised to forward slashes.
            let relative_path = absolute_path
                .strip_prefix(&self.root)
                .unwrap_or(&absolute_path)
                .to_string_lossy()
                .replace('\\', "/");

            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);

            let language = detect_language(&absolute_path);

            files.push(ScannedFile {
                relative_path,
                absolute_path,
                language,
                size_bytes,
            });
        }

        files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        Ok(files)
    }
}

/// Detect a programming language from a file's extension.
fn detect_language(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    let lang = match ext.as_str() {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "java" => "java",
        "py" => "python",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "hh" | "cxx" => "cpp",
        "rb" => "ruby",
        "cs" => "csharp",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        _ => return None,
    };
    Some(lang.to_string())
}
