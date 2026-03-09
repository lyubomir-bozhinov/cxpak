# cxpak v0.3.0 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement the trace command, polish pack mode, add 4 new languages, and create project CLAUDE.md.

**Architecture:** Seven independent features in priority order. Each is self-contained with its own tests. The trace command is the biggest piece — it reuses the existing scanner/parser/index pipeline but walks the dependency graph from a target symbol.

**Tech Stack:** Rust, tree-sitter, clap, tiktoken-rs. New crates: tree-sitter-ruby, tree-sitter-c-sharp, tree-sitter-swift, tree-sitter-kotlin.

---

### Task 1: Trace Command — Core Implementation

The trace command finds a target symbol (function/type/error string), then walks the dependency graph to collect all relevant code paths. Output is a token-budgeted bundle of the trace results.

**Files:**
- Modify: `src/commands/trace.rs` (replace stub)
- Modify: `src/index/mod.rs` (add `find_symbol` method)
- Modify: `src/index/graph.rs` (add `reachable_from` method)
- Create: `tests/trace_test.rs`

**Step 1: Add `find_symbol` to CodebaseIndex**

In `src/index/mod.rs`, add a method to search for a symbol by name:

```rust
/// Find files containing a symbol matching the target name.
/// Returns (file_path, symbol) pairs.
pub fn find_symbol(&self, target: &str) -> Vec<(&str, &Symbol)> {
    let target_lower = target.to_lowercase();
    self.files
        .iter()
        .filter_map(|f| {
            f.parse_result.as_ref().map(|pr| {
                pr.symbols
                    .iter()
                    .filter(|s| s.name.to_lowercase() == target_lower)
                    .map(|s| (f.relative_path.as_str(), s))
                    .collect::<Vec<_>>()
            })
        })
        .flatten()
        .collect()
}

/// Find files that contain the target string in their content (for error messages).
pub fn find_content_matches(&self, target: &str) -> Vec<&str> {
    self.files
        .iter()
        .filter(|f| f.content.contains(target))
        .map(|f| f.relative_path.as_str())
        .collect()
}
```

**Step 2: Add `reachable_from` to DependencyGraph**

In `src/index/graph.rs`, add BFS traversal:

```rust
/// BFS from a set of starting files, returning all reachable files.
pub fn reachable_from(&self, start_files: &[&str]) -> HashSet<String> {
    let mut visited = HashSet::new();
    let mut queue: Vec<String> = start_files.iter().map(|s| s.to_string()).collect();

    while let Some(file) = queue.pop() {
        if !visited.insert(file.clone()) {
            continue;
        }
        // Follow outgoing dependencies
        if let Some(deps) = self.dependencies(&file) {
            for dep in deps {
                if !visited.contains(dep) {
                    queue.push(dep.clone());
                }
            }
        }
        // Follow incoming dependents
        for dep in self.dependents(&file) {
            if !visited.contains(dep) {
                queue.push(dep.to_string());
            }
        }
    }
    visited
}
```

**Step 3: Implement the trace command**

Replace `src/commands/trace.rs` with:

```rust
use crate::budget::{counter::TokenCounter, degrader, BudgetAllocation};
use crate::cli::OutputFormat;
use crate::index::CodebaseIndex;
use crate::output::{self, OutputSections};
use crate::parser::LanguageRegistry;
use crate::scanner::Scanner;
use std::path::Path;

pub fn run(
    target: &str,
    token_budget: usize,
    format: &OutputFormat,
    out: Option<&Path>,
    verbose: bool,
    all: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(".");
    // Reuse the same scan/parse/index pipeline as overview
    let scanner = Scanner::new(path)?;
    let files = scanner.scan()?;
    if verbose {
        eprintln!("cxpak: scanned {} files", files.len());
    }

    let registry = LanguageRegistry::new();
    let mut parse_results = std::collections::HashMap::new();
    for file in &files {
        if let Some(ref lang) = file.language {
            if let Some(support) = registry.get(lang) {
                let content = std::fs::read_to_string(&file.absolute_path)?;
                let mut parser = tree_sitter::Parser::new();
                parser.set_language(&support.ts_language())?;
                if let Some(tree) = parser.parse(&content, None) {
                    let result = support.extract(&content, &tree);
                    parse_results.insert(file.relative_path.clone(), result);
                }
            }
        }
    }

    let counter = TokenCounter::new();
    let index = CodebaseIndex::build(files, parse_results, &counter);

    // Build dependency graph
    let mut graph = crate::index::graph::DependencyGraph::new();
    for file in &index.files {
        if let Some(ref pr) = file.parse_result {
            for import in &pr.imports {
                graph.add_edge(&file.relative_path, &import.source);
            }
        }
    }

    // Find the target: first try symbol name, then content match
    let symbol_matches = index.find_symbol(target);
    let mut start_files: Vec<&str> = symbol_matches.iter().map(|(f, _)| *f).collect();

    if start_files.is_empty() {
        // Fall back to content search
        start_files = index.find_content_matches(target);
    }

    if start_files.is_empty() {
        eprintln!("cxpak: target '{}' not found in codebase", target);
        std::process::exit(1);
    }

    if verbose {
        eprintln!("cxpak: found '{}' in {} files", target, start_files.len());
    }

    // Walk the graph to find all relevant files
    let relevant_files: std::collections::HashSet<String> = if all {
        graph.reachable_from(&start_files)
    } else {
        // Default: just direct dependencies + dependents (1 hop)
        let mut relevant = std::collections::HashSet::new();
        for f in &start_files {
            relevant.insert(f.to_string());
            if let Some(deps) = graph.dependencies(f) {
                for d in deps {
                    relevant.insert(d.clone());
                }
            }
            for d in graph.dependents(f) {
                relevant.insert(d.to_string());
            }
        }
        relevant
    };

    if verbose {
        eprintln!("cxpak: tracing {} relevant files", relevant_files.len());
    }

    // Build trace output sections
    let alloc = BudgetAllocation::allocate(token_budget);

    // Target info section
    let mut target_info = format!("**Target:** `{}`\n", target);
    target_info.push_str(&format!("**Found in:** {} files\n", start_files.len()));
    for (file, sym) in &symbol_matches {
        target_info.push_str(&format!(
            "- `{}` — {} `{}` (lines {}-{})\n",
            file,
            format!("{:?}", sym.kind).to_lowercase(),
            sym.name,
            sym.start_line,
            sym.end_line,
        ));
    }
    target_info.push_str(&format!(
        "**Relevant files:** {} (via dependency graph)\n",
        relevant_files.len()
    ));

    // Signatures of relevant symbols
    let mut sigs = String::new();
    for file in &index.files {
        if !relevant_files.contains(&file.relative_path) {
            continue;
        }
        if let Some(ref pr) = file.parse_result {
            let public_syms: Vec<_> = pr
                .symbols
                .iter()
                .filter(|s| s.visibility == crate::parser::language::Visibility::Public)
                .collect();
            if !public_syms.is_empty() {
                sigs.push_str(&format!("### {}\n", file.relative_path));
                for sym in public_syms {
                    sigs.push_str(&format!(
                        "- {} `{}`\n",
                        format!("{:?}", sym.kind),
                        sym.signature
                    ));
                }
                sigs.push('\n');
            }
        }
    }
    let (sigs_budgeted, _, _) =
        degrader::truncate_to_budget(&sigs, alloc.signatures, &counter, "signatures");

    // Source code of matched symbols (the key value: actual code paths)
    let mut source = String::new();
    for (file, sym) in &symbol_matches {
        source.push_str(&format!("### {} — `{}`\n\n```\n", file, sym.name));
        source.push_str(&sym.body);
        source.push_str("\n```\n\n");
    }
    let (source_budgeted, _, _) =
        degrader::truncate_to_budget(&source, alloc.key_files, &counter, "source code");

    // Dependency subgraph
    let mut deps = String::new();
    for file in &index.files {
        if !relevant_files.contains(&file.relative_path) {
            continue;
        }
        if let Some(ref pr) = file.parse_result {
            if !pr.imports.is_empty() {
                deps.push_str(&format!("**{}** imports:\n", file.relative_path));
                for import in &pr.imports {
                    let names = if import.names.is_empty() {
                        String::new()
                    } else {
                        format!(" — {}", import.names.join(", "))
                    };
                    deps.push_str(&format!("- `{}`{}\n", import.source, names));
                }
                deps.push('\n');
            }
        }
    }
    let (deps_budgeted, _, _) =
        degrader::truncate_to_budget(&deps, alloc.dependency_graph, &counter, "dependencies");

    let sections = OutputSections {
        metadata: target_info,
        directory_tree: String::new(),
        module_map: source_budgeted,
        dependency_graph: deps_budgeted,
        key_files: String::new(),
        signatures: sigs_budgeted,
        git_context: String::new(),
    };

    let rendered = output::render(&sections, format);

    if let Some(out_path) = out {
        std::fs::write(out_path, &rendered)?;
    } else {
        print!("{}", rendered);
    }

    Ok(())
}
```

**Step 4: Write integration tests**

Create `tests/trace_test.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn make_trace_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();

    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();

    // main.rs calls helper
    std::fs::write(
        src_dir.join("main.rs"),
        "use crate::helper::greet;\n\nfn main() {\n    greet(\"world\");\n}\n",
    )
    .unwrap();

    // helper.rs defines greet
    std::fs::write(
        src_dir.join("helper.rs"),
        "pub fn greet(name: &str) {\n    println!(\"Hello, {}!\", name);\n}\n",
    )
    .unwrap();

    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"trace-test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    let mut index = repo.index().unwrap();
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("Test", "test@test.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
        .unwrap();

    dir
}

#[test]
fn test_trace_finds_function() {
    let repo = make_trace_repo();
    Command::new(assert_cmd::cargo_bin!("cxpak"))
        .args(["trace", "--tokens", "50k", "greet"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("greet"));
}

#[test]
fn test_trace_not_found() {
    let repo = make_trace_repo();
    Command::new(assert_cmd::cargo_bin!("cxpak"))
        .args(["trace", "--tokens", "50k", "nonexistent_symbol"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_trace_json_output() {
    let repo = make_trace_repo();
    Command::new(assert_cmd::cargo_bin!("cxpak"))
        .args(["trace", "--tokens", "50k", "--format", "json", "greet"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\""));
}

#[test]
fn test_trace_all_flag() {
    let repo = make_trace_repo();
    Command::new(assert_cmd::cargo_bin!("cxpak"))
        .args(["trace", "--tokens", "50k", "--all", "greet"])
        .current_dir(repo.path())
        .assert()
        .success();
}
```

**Step 5: Run tests**

```bash
cargo test --test trace_test -v
```
Expected: All 4 tests pass.

**Step 6: Commit**

```bash
git add src/commands/trace.rs src/index/mod.rs src/index/graph.rs tests/trace_test.rs
git commit -m "feat: implement trace command with dependency graph walking"
```

---

### Task 2: Trace Command — Wire Up Path Argument

The trace command currently hardcodes `Path::new(".")` but should use the target repo path from CLI args.

**Files:**
- Modify: `src/commands/trace.rs` (use path from CLI)
- Modify: `src/main.rs` (pass path to trace::run)
- Modify: `src/cli/mod.rs` (add path arg to Trace if missing)

**Step 1: Check CLI and main.rs for trace path handling**

Read `src/main.rs` to see how trace is dispatched. The Trace variant in `cli/mod.rs` doesn't have a `path` argument — add one with `default_value = "."`, same as Overview. Update `main.rs` to pass it. Update `trace.rs` to accept and use it.

**Step 2: Run tests**

```bash
cargo test -v
```
Expected: All tests pass (trace tests already use `current_dir`).

**Step 3: Commit**

```bash
git commit -am "feat: add path argument to trace command"
```

---

### Task 3: Stale .cxpak/ Cleanup

When pack mode runs, clean `.cxpak/` directory before writing new files. When single-file mode runs, remove `.cxpak/` if it exists from a previous run.

**Files:**
- Modify: `src/commands/overview.rs` (add cleanup logic)
- Modify: `tests/overview_test.rs` (add cleanup test)

**Step 1: Write failing test**

Add to `tests/overview_test.rs`:

```rust
#[test]
fn test_stale_cxpak_cleaned_on_single_file_mode() {
    let repo = make_temp_repo();

    // Create stale .cxpak/ directory with a leftover file
    let cxpak_dir = repo.path().join(".cxpak");
    std::fs::create_dir_all(&cxpak_dir).unwrap();
    std::fs::write(cxpak_dir.join("stale.md"), "stale content").unwrap();

    // Run in single-file mode (repo fits in budget)
    Command::new(assert_cmd::cargo_bin!("cxpak"))
        .args(["overview", "--tokens", "50k"])
        .arg(repo.path())
        .assert()
        .success();

    // .cxpak/ should be cleaned up
    assert!(!cxpak_dir.exists(), "stale .cxpak/ should be removed in single-file mode");
}

#[test]
fn test_stale_cxpak_cleaned_on_pack_mode() {
    let repo = make_large_temp_repo();

    // Create stale file in .cxpak/
    let cxpak_dir = repo.path().join(".cxpak");
    std::fs::create_dir_all(&cxpak_dir).unwrap();
    std::fs::write(cxpak_dir.join("stale.md"), "stale content").unwrap();

    Command::new(assert_cmd::cargo_bin!("cxpak"))
        .args(["overview", "--tokens", "500"])
        .arg(repo.path())
        .assert()
        .success();

    // stale.md should not exist (dir was cleaned before new files written)
    assert!(!cxpak_dir.join("stale.md").exists(), "stale files should be cleaned");
    // But new detail files should exist
    assert!(cxpak_dir.exists(), ".cxpak/ should be recreated with fresh files");
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test test_stale_cxpak -v
```

**Step 3: Implement cleanup**

In `src/commands/overview.rs`, in the `run()` function, add cleanup before the pack mode write block:

```rust
// Clean up stale .cxpak/ from previous runs
let cxpak_dir = path.join(".cxpak");
if cxpak_dir.exists() {
    std::fs::remove_dir_all(&cxpak_dir)?;
}

if pack_mode {
    std::fs::create_dir_all(&cxpak_dir)?;
    // ... existing detail file writing ...
}
```

**Step 4: Run tests**

```bash
cargo test -v
```

**Step 5: Commit**

```bash
git commit -am "fix: clean stale .cxpak/ directory between runs"
```

---

### Task 4: Detail File Extensions Match --format

Detail files in `.cxpak/` should use the correct extension: `.md` for markdown, `.json` for json, `.xml` for xml.

**Files:**
- Modify: `src/commands/overview.rs` (dynamic file extension)
- Modify: `src/budget/degrader.rs` (pointer uses correct extension)
- Modify: `tests/overview_test.rs` (add extension test)

**Step 1: Write failing test**

```rust
#[test]
fn test_pack_mode_json_detail_file_extension() {
    let repo = make_large_temp_repo();
    Command::new(assert_cmd::cargo_bin!("cxpak"))
        .args(["overview", "--tokens", "500", "--format", "json"])
        .arg(repo.path())
        .assert()
        .success();

    let cxpak_dir = repo.path().join(".cxpak");
    // Should have .json files, not .md
    let has_json = std::fs::read_dir(&cxpak_dir)
        .unwrap()
        .any(|e| e.unwrap().path().extension().map_or(false, |ext| ext == "json"));
    assert!(has_json, "detail files should have .json extension when --format json");
}
```

**Step 2: Run test to verify it fails**

**Step 3: Implement**

Add a helper function to get the file extension from format:

```rust
fn detail_file_ext(format: &OutputFormat) -> &'static str {
    match format {
        OutputFormat::Markdown => "md",
        OutputFormat::Xml => "xml",
        OutputFormat::Json => "json",
    }
}
```

Update the detail file names in the `detail_sections` array and in the omission pointer calls to use `format!("tree.{}", detail_file_ext(format))` instead of hardcoded `"tree.md"`.

Also update the render functions that pass detail filenames (e.g., `"modules.md"`) to use the dynamic extension.

**Step 4: Run tests**

```bash
cargo test -v
```

**Step 5: Commit**

```bash
git commit -am "fix: detail file extensions match --format flag"
```

---

### Task 5: XML Pointer Escaping Fix

HTML comment pointers (`<!-- ... -->`) get escaped by `escape_xml()` in the XML renderer, making them unreadable. Fix: use an XML element for pointers instead.

**Files:**
- Modify: `src/output/xml.rs` (preserve HTML comments or use XML element)
- Modify: `tests/overview_test.rs` (add XML pointer test)

**Step 1: Write failing test**

```rust
#[test]
fn test_pack_mode_xml_pointers_not_escaped() {
    let repo = make_large_temp_repo();
    let output = Command::new(assert_cmd::cargo_bin!("cxpak"))
        .args(["overview", "--tokens", "500", "--format", "xml"])
        .arg(repo.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Pointers should be readable, not escaped as &lt;!-- ... --&gt;
    assert!(
        !stdout.contains("&lt;!--"),
        "XML output should not escape pointer comments"
    );
    // Should contain actual pointer reference
    assert!(
        stdout.contains(".cxpak/"),
        "XML output should contain .cxpak/ pointer reference"
    );
}
```

**Step 2: Run test to verify it fails**

**Step 3: Implement**

In `src/output/xml.rs`, modify `emit_section` to detect lines that are omission pointers and emit them as XML elements instead of escaped text:

```rust
fn emit_section(out: &mut String, tag: &str, content: &str) {
    if !content.is_empty() {
        out.push_str(&format!("  <{tag}>\n"));
        for line in content.lines() {
            if line.trim_start().starts_with("<!-- ") && line.trim_end().ends_with(" -->") {
                // Omission pointer — emit as XML element instead of escaped comment
                let inner = line.trim_start().strip_prefix("<!-- ").unwrap()
                    .strip_suffix(" -->").unwrap();
                out.push_str(&format!("    <detail-ref>{}</detail-ref>\n", escape_xml(inner)));
            } else {
                out.push_str(&format!("    {}\n", escape_xml(line)));
            }
        }
        out.push_str(&format!("  </{tag}>\n"));
    }
}
```

**Step 4: Run tests**

```bash
cargo test -v
```

**Step 5: Commit**

```bash
git commit -am "fix: emit XML pointer references as elements instead of escaped comments"
```

---

### Task 6: Tests for XML/JSON Format Detail Files

Add integration tests verifying detail file content is correct for non-markdown formats.

**Files:**
- Modify: `tests/overview_test.rs`

**Step 1: Add tests**

```rust
#[test]
fn test_pack_mode_json_detail_file_content() {
    let repo = make_large_temp_repo();
    Command::new(assert_cmd::cargo_bin!("cxpak"))
        .args(["overview", "--tokens", "500", "--format", "json"])
        .arg(repo.path())
        .assert()
        .success();

    let cxpak_dir = repo.path().join(".cxpak");
    // Find a .json detail file and verify it's valid JSON
    for entry in std::fs::read_dir(&cxpak_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().map_or(false, |e| e == "json") {
            let content = std::fs::read_to_string(&path).unwrap();
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&content);
            assert!(parsed.is_ok(), "detail file {} should be valid JSON", path.display());
            return;
        }
    }
    panic!("no JSON detail files found in .cxpak/");
}

#[test]
fn test_pack_mode_xml_detail_file_content() {
    let repo = make_large_temp_repo();
    Command::new(assert_cmd::cargo_bin!("cxpak"))
        .args(["overview", "--tokens", "500", "--format", "xml"])
        .arg(repo.path())
        .assert()
        .success();

    let cxpak_dir = repo.path().join(".cxpak");
    // Find an .xml detail file and verify it contains <cxpak> root
    for entry in std::fs::read_dir(&cxpak_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().map_or(false, |e| e == "xml") {
            let content = std::fs::read_to_string(&path).unwrap();
            assert!(content.contains("<cxpak>"), "detail file {} should have <cxpak> root", path.display());
            return;
        }
    }
    panic!("no XML detail files found in .cxpak/");
}
```

**Step 2: Run tests**

```bash
cargo test test_pack_mode_json_detail test_pack_mode_xml_detail -v
```

These should pass if Tasks 4 and 5 are already done.

**Step 3: Commit**

```bash
git commit -am "test: add XML and JSON format detail file integration tests"
```

---

### Task 7: Add Ruby Language Support

**Files:**
- Modify: `Cargo.toml` (add tree-sitter-ruby dependency)
- Create: `src/parser/languages/ruby.rs`
- Modify: `src/parser/languages/mod.rs` (add ruby module)
- Modify: `src/parser/mod.rs` (register ruby)
- Modify: `src/scanner/mod.rs` (detect .rb files)

**Step 1: Add dependency to Cargo.toml**

```toml
tree-sitter-ruby = { version = "0.23", optional = true }
```

Add to `[features]`:
```toml
lang-ruby = ["dep:tree-sitter-ruby"]
```

Add `"lang-ruby"` to the `default` feature list.

**Step 2: Add language detection in scanner**

In `src/scanner/mod.rs`, in the `detect_language` function, add:
```rust
"rb" => Some("ruby".to_string()),
```

**Step 3: Create ruby.rs language implementation**

Follow the same pattern as `rust.rs`. Ruby tree-sitter node kinds:
- `method` — method definitions (with `def`/`end`)
- `singleton_method` — class methods (`def self.foo`)
- `class` — class definitions
- `module` — module definitions
- `call` — require/require_relative for imports
- `assignment` — constant assignments (UPPER_CASE)

The `extract` method should walk the AST looking for these node kinds, extract names, determine visibility (Ruby: methods are public by default unless after `private`/`protected` keywords), and build imports from `require` calls.

```rust
pub struct RubyLanguage;

impl LanguageSupport for RubyLanguage {
    fn ts_language(&self) -> TsLanguage {
        tree_sitter_ruby::LANGUAGE.into()
    }

    fn name(&self) -> &str {
        "ruby"
    }

    fn extract(&self, source: &str, tree: &tree_sitter::Tree) -> ParseResult {
        // Walk tree, extract methods, classes, modules, requires
    }
}
```

**Step 4: Register in parser**

In `src/parser/languages/mod.rs`:
```rust
#[cfg(feature = "lang-ruby")]
pub mod ruby;
```

In `src/parser/mod.rs` `register_defaults`:
```rust
#[cfg(feature = "lang-ruby")]
self.register(Box::new(languages::ruby::RubyLanguage));
```

**Step 5: Write unit tests**

Add tests at the bottom of `ruby.rs` testing extraction of methods, classes, modules, and require imports from sample Ruby code.

**Step 6: Run tests**

```bash
cargo test ruby -v
```

**Step 7: Commit**

```bash
git commit -am "feat: add Ruby language support"
```

---

### Task 8: Add C# Language Support

**Files:**
- Modify: `Cargo.toml` (add tree-sitter-c-sharp)
- Create: `src/parser/languages/csharp.rs`
- Modify: `src/parser/languages/mod.rs`
- Modify: `src/parser/mod.rs`
- Modify: `src/scanner/mod.rs`

Same pattern as Task 7. C# node kinds:
- `method_declaration` — methods
- `class_declaration` — classes
- `interface_declaration` — interfaces
- `struct_declaration` — structs
- `enum_declaration` — enums
- `using_directive` — imports
- `namespace_declaration` — namespace context

Visibility: check for `public`/`private`/`protected`/`internal` modifier nodes.

Scanner: detect `.cs` extension → `"csharp"`.

Cargo.toml:
```toml
tree-sitter-c-sharp = { version = "0.23", optional = true }
```
Feature: `lang-csharp = ["dep:tree-sitter-c-sharp"]`

**Commit:** `feat: add C# language support`

---

### Task 9: Add Swift Language Support

**Files:**
- Modify: `Cargo.toml` (add tree-sitter-swift)
- Create: `src/parser/languages/swift.rs`
- Modify: `src/parser/languages/mod.rs`
- Modify: `src/parser/mod.rs`
- Modify: `src/scanner/mod.rs`

Swift node kinds:
- `function_declaration` — functions
- `class_declaration` — classes
- `struct_declaration` — structs
- `protocol_declaration` — protocols (like traits/interfaces)
- `enum_declaration` — enums
- `import_declaration` — imports

Visibility: check for `public`/`private`/`internal`/`open` modifiers.

Scanner: detect `.swift` extension → `"swift"`.

Cargo.toml:
```toml
tree-sitter-swift = { version = "0.7", optional = true }
```
Feature: `lang-swift = ["dep:tree-sitter-swift"]`

**Commit:** `feat: add Swift language support`

---

### Task 10: Add Kotlin Language Support

**Files:**
- Modify: `Cargo.toml` (add tree-sitter-kotlin)
- Create: `src/parser/languages/kotlin.rs`
- Modify: `src/parser/languages/mod.rs`
- Modify: `src/parser/mod.rs`
- Modify: `src/scanner/mod.rs`

Kotlin node kinds:
- `function_declaration` — functions
- `class_declaration` — classes
- `object_declaration` — objects/companions
- `interface_declaration` — interfaces
- `import_header` — imports

Visibility: check for `public`/`private`/`protected`/`internal` modifiers.

Scanner: detect `.kt` and `.kts` extension → `"kotlin"`.

Cargo.toml:
```toml
tree-sitter-kotlin = { version = "0.3", optional = true }
```
Feature: `lang-kotlin = ["dep:tree-sitter-kotlin"]`

**Commit:** `feat: add Kotlin language support`

---

### Task 11: Update README for New Languages

**Files:**
- Modify: `README.md`

Update the "Supported Languages" section to include Ruby, C#, Swift, Kotlin (total: 12 languages). Update the feature flags example.

**Commit:** `docs: update README with new language support`

---

### Task 12: Create CLAUDE.md for cxpak

**Files:**
- Create: `CLAUDE.md`

The CLAUDE.md should contain:
- Build system: `cargo build`, `cargo test`
- Lint: `cargo fmt -- --check && cargo clippy --all-targets -- -D warnings`
- Test: `cargo test --verbose`
- Architecture overview: scanner → parser → index → budget → output pipeline
- Key patterns: LanguageSupport trait for adding languages, SectionContent for pack mode
- Pre-commit hooks: `bash scripts/install-hooks.sh`
- Release process: tag with `v*` triggers CI build + crates.io publish

**Commit:** `docs: add CLAUDE.md with project conventions`

---

### Task 13: Version Bump and Final Validation

**Files:**
- Modify: `Cargo.toml` (bump to 0.3.0)

**Steps:**
1. `cargo fmt`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --verbose` — all tests pass
4. Test trace command on cxpak itself: `cargo run -- trace --tokens 50k "render_metadata"`
5. Test trace on a larger repo if available
6. Bump version to 0.3.0
7. Commit and push

**Commit:** `chore: bump version to 0.3.0`
