# cxpak Design Document

> cxpak spends CPU cycles so you don't spend tokens. The LLM gets a briefing packet instead of a flashlight in a dark room.

## Overview

cxpak is a Rust CLI that indexes codebases using tree-sitter and produces token-budgeted context bundles for LLMs. It ships as a single binary with all supported language grammars compiled in (optionally slimmable via Cargo feature flags).

### The Problem

LLM exploration of a codebase is fundamentally wasteful:

- Each file read and navigation decision burns tokens on *orientation*, not *problem-solving*
- The LLM has no global view — it makes greedy local decisions about what to read
- By the time it's oriented, 50-100k tokens are spent on exploration alone
- The process is nondeterministic — different runs explore differently

### The Solution

cxpak inverts this by performing indexing *outside* the token economy:

| | LLM Explores | cxpak |
|---|---|---|
| Tokens spent navigating | 50-100k | 0 |
| Tokens spent on actual task | whatever's left | full budget |
| Has global codebase view | never | always |
| Deterministic | no | yes |
| Works offline | no | yes |
| Reusable across models | no | yes |
| Time to first useful response | minutes | seconds |

## Commands

| Command | Purpose | Phase |
|---------|---------|-------|
| `cxpak overview` | Structured repo summary within a token budget | MVP |
| `cxpak trace` | Trace from error/function/line, pack relevant code paths | v2 |

## CLI Interface

```
cxpak - spends CPU cycles so you don't spend tokens

USAGE:
    cxpak <COMMAND> [OPTIONS]

COMMANDS:
    overview    Structured repo summary within a token budget
    trace       Trace from error/function, pack relevant code paths

COMMON OPTIONS:
    --tokens <COUNT>    Token budget (required)
    --out <FILE>        Write output to file instead of stdout
    --format <FORMAT>   Output format: markdown (default), xml, json
    --verbose           Show indexing progress on stderr
    --help              Print help
    --version           Print version
```

### overview

```
cxpak overview --tokens <COUNT> [OPTIONS] [PATH]

PATH    Repo path (defaults to current directory)
```

### trace

```
cxpak trace --tokens <COUNT> [OPTIONS] <TARGET>

TARGET:
    Function/method name:     cxpak trace --tokens 50k "parse_module"
    File + line:              cxpak trace --tokens 50k "src/parser/mod.rs:42"
    Error message:            cxpak trace --tokens 50k "index out of bounds"

OPTIONS:
    --all    Pack all matches when target is ambiguous (budget split across them)
```

## Supported Languages

Tree-sitter grammars compiled in, each behind a Cargo feature flag. All enabled by default.

| Language | Feature Flag | File Extensions |
|----------|-------------|-----------------|
| Rust | `lang-rust` | `.rs` |
| TypeScript | `lang-typescript` | `.ts`, `.tsx` |
| JavaScript | `lang-javascript` | `.js`, `.jsx`, `.mjs`, `.cjs` |
| Java | `lang-java` | `.java` |
| Python | `lang-python` | `.py` |
| Go | `lang-go` | `.go` |
| C | `lang-c` | `.c`, `.h` |
| C++ | `lang-cpp` | `.cpp`, `.hpp`, `.cc`, `.hh`, `.cxx` |

### Unsupported files

- Counted in metadata (file count, size)
- Included verbatim in "Key Files" if they match (README, configs, Dockerfiles, etc.)
- No AST analysis — treated as opaque text

## overview Command

### Pre-flight

Progress and warnings go to stderr. Scan metadata is folded into the Project Metadata section in the output.

1. Detect project root — walk up from CWD looking for `.git/`. Error if not in a git repo.
2. Language detection — scan file extensions, map to tree-sitter grammars. Report unsupported files but don't fail.
3. Token budget validation — `--tokens` is required. Reject nonsense values. Warn if budget is very small for repo size.
4. Ignore rules — load `.gitignore`, apply built-in defaults, load `.cxpakignore` if present.
5. Indexing — parse all non-ignored files through tree-sitter.

### Output Sections (priority order)

| Priority | Section | Contents |
|----------|---------|----------|
| 1 | Project Metadata | Languages detected (% breakdown), build system, entry points, key dependencies, scan metadata |
| 2 | Directory Tree | Filtered structure, no ignored dirs, depth-limited based on budget |
| 3 | Module/Component Map | Key types, traits, interfaces, structs, exports. How they relate. |
| 4 | Dependency Graph | Internal: module-to-module relationships. External: third-party deps and where used. |
| 5 | Key Files | README, config files, main entry points — verbatim |
| 6 | Function/Type Signatures | Public API surface without implementations |
| 7 | Git Context | Recent commits (last 20), most-changed files, active contributors |

### Degradation

Top-down degradation with omission markers. Higher-priority sections get space first. When content exceeds budget:

- Start with full detail (signatures + bodies), progressively strip to signatures-only, names-only, then omit entirely
- Prioritize by: entry points first, then most-connected modules, then leaf files
- Wherever content is cut, insert a marker:

```
<!-- [section/module] omitted: ~Nk tokens. Use --tokens Xk+ to include -->
```

### Token Budget Allocation

| Section | Weight | Notes |
|---------|--------|-------|
| Project Metadata | fixed ~500 | Always small, always included |
| Directory Tree | 5% | Structural, compresses well |
| Module/Component Map | 20% | Core value of the tool |
| Dependency Graph | 15% | Relationships matter |
| Key Files | 20% | README, configs verbatim |
| Function/Type Signatures | 30% | Biggest section, most degradable |
| Git Context | 10% | Nice-to-have, first to cut |

If a section comes in under budget, its remainder flows to the next section down. If a section can't fill its budget, the surplus distributes proportionally.

## trace Command

### What it does

1. Resolve target — find the function, line, or grep for the error string across the codebase.
2. Trace dependencies — walk the call graph outward: what does this function call? What calls it? What types does it use?
3. Determine depth — budget controls how far out the trace goes. More tokens = more hops.
4. Pack context — target in full, direct deps with full bodies, transitive deps with signatures only, then omission markers.

### Output Sections (priority order)

| Priority | Section | Contents |
|----------|---------|----------|
| 1 | Target | Full source of the matched function/file region |
| 2 | Direct dependencies | Functions/types called by or calling the target — full bodies |
| 3 | Type context | Structs, enums, traits, interfaces used in the chain |
| 4 | Transitive dependencies | One more hop out — signatures only |
| 5 | Module context | Where these files sit in the project structure |
| 6 | Test context | Existing tests for the target and its direct deps |
| 7 | Git blame | Recent changes to the target region — who changed what, when |

### Ambiguity Handling

If the target matches multiple locations:
- `--all` flag packs all matches (budget split across them)
- Default: list matches on stderr, pack the first match

## Ignore Rules

Three layers, applied in order:

1. `.gitignore` — standard git ignore semantics
2. Built-in smart defaults — `node_modules`, `target/`, `dist/`, `__pycache__`, lock files, binary files, etc.
3. `.cxpakignore` (optional) — same format as `.gitignore`, for project-specific exclusions

## Token Counting

- Tokenizer: `tiktoken-rs` (cl100k_base encoding). Widely used baseline, close enough across models.
- Count once during index pass, budget from cached counts. No re-tokenization.

## Architecture

### Project Structure

```
cxpak/
├── src/
│   ├── main.rs                 # CLI entry point (clap)
│   ├── cli/
│   │   └── mod.rs              # Argument parsing, command dispatch
│   ├── scanner/
│   │   ├── mod.rs              # File discovery, ignore rules
│   │   ├── gitignore.rs        # .gitignore + .cxpakignore loading
│   │   └── defaults.rs         # Built-in ignore patterns
│   ├── parser/
│   │   ├── mod.rs              # Tree-sitter orchestration
│   │   ├── language.rs         # Language trait + registry
│   │   └── languages/          # Per-language implementations
│   │       ├── rust.rs
│   │       ├── typescript.rs
│   │       ├── javascript.rs
│   │       ├── java.rs
│   │       ├── python.rs
│   │       ├── go.rs
│   │       ├── c.rs
│   │       └── cpp.rs
│   ├── index/
│   │   ├── mod.rs              # Codebase index: files, symbols, deps
│   │   ├── symbols.rs          # Functions, types, traits, interfaces
│   │   └── graph.rs            # Dependency graph (internal + external)
│   ├── budget/
│   │   ├── mod.rs              # Token budget allocation
│   │   ├── counter.rs          # tiktoken-rs wrapper
│   │   └── degrader.rs         # Top-down degradation + omission markers
│   ├── output/
│   │   ├── mod.rs              # Format dispatch
│   │   ├── markdown.rs         # Markdown renderer
│   │   ├── xml.rs              # XML renderer
│   │   └── json.rs             # JSON renderer
│   ├── git/
│   │   └── mod.rs              # Recent commits, churn analysis, contributors
│   └── commands/
│       ├── mod.rs
│       ├── overview.rs         # overview command orchestration
│       └── trace.rs            # trace command orchestration
├── Cargo.toml
├── .cxpakignore.example
├── README.md
├── LICENSE
└── tests/
    ├── fixtures/               # Small test repos per language
    └── integration/            # End-to-end tests
```

### Data Flow

```
CLI args
  -> Scanner (discover files, apply ignore rules)
  -> Parser (tree-sitter AST per file)
  -> Index (symbols, deps, graph)
  -> Budget (allocate tokens, degrade, mark omissions)
  -> Output (render to chosen format)
  -> stdout or --out file
```

### Key Design Decisions

- **Language trait** — each language implements extraction of symbols, imports, exports. Clean boundary if plugin architecture is ever needed.
- **Index is the central data structure** — Scanner and Parser populate it, Budget reads from it, Output renders from it. Single source of truth.
- **Git ops via git2-rs** — no shelling out to git. Library-level access.
- **No async** — everything is filesystem I/O and CPU-bound parsing. Synchronous is simpler and sufficient.
- **No logging framework** — `--verbose` writes to stderr via eprintln.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing with derive macros |
| `tree-sitter` + per-language grammar crates | AST parsing |
| `tiktoken-rs` | Token counting (cl100k_base) |
| `git2` | Git history, blame, contributors |
| `ignore` | .gitignore + custom ignore pattern handling (ripgrep ecosystem) |
| `serde` + `serde_json` | JSON output format |
| `quick-xml` | XML output format |

## Distribution

- `cargo install cxpak` (crates.io)
- Pre-built binaries via GitHub Releases

## Not In Scope

- Plugin/dynamic grammar loading
- Homebrew formula
- Async runtime
- Config file (flags only)
