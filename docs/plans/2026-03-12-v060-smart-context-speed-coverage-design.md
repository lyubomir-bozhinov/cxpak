# cxpak v0.6.0 — Smart Context, Speed, Coverage

**Goal:** Make cxpak's output smarter (importance-weighted budget allocation), faster (parallel parsing), and bulletproof (95%+ test coverage targeting 100%).

---

## Workstream 1: Smart Context

### Problem

Budget allocation treats all files equally. A utility helper gets the same token budget as the central orchestrator that everything imports. This wastes tokens on low-value content and truncates high-value content.

### Solution

Graph-based importance scoring using data already available in the `DependencyGraph` and git history.

### New Module: `src/index/ranking.rs`

```rust
pub struct FileScore {
    pub path: String,
    pub in_degree: usize,      // files that import this
    pub out_degree: usize,     // files this imports
    pub git_recency: f64,      // 0.0-1.0, based on last commit age
    pub git_churn: f64,        // 0.0-1.0, based on commit frequency
    pub composite: f64,        // weighted combination
}
```

**Weights:** `in_degree * 0.4 + out_degree * 0.1 + git_recency * 0.3 + git_churn * 0.2`

Rationale:
- `in_degree` (0.4): Files that many others depend on are foundational — most important signal
- `git_recency` (0.3): Recently changed files are actively relevant
- `git_churn` (0.2): Frequently changed files are hotspots worth understanding
- `out_degree` (0.1): Orchestrators that import many modules are useful but less critical than foundations

### Budget Impact (`src/budget/mod.rs`)

- Files above median score: full budget (signatures + content in pack mode)
- Files below median: signatures only
- Bottom 10%: name only (listed but no content)

The ranking feeds into `allocate()` as a weight map. No new budget algorithm — just weighted input.

### New CLI Flag: `--focus <path>`

Works with `overview`, `trace`, and `diff`.

- Boosts scores for files under the given path by 2x
- Boosts scores for their direct dependencies by 1.5x
- Without `--focus`, behavior improves silently via ranking

### No Breaking Changes

Existing output format is unchanged. Content is reordered and weighted, not restructured. Users see better context without changing their workflow.

---

## Workstream 2: Speed

### Step 1: `--timing` Flag

Add to all commands (`overview`, `trace`, `diff`). Output to stderr:

```
[timing] scan:    12ms (143 files)
[timing] parse:  847ms (143 files, 89 cached)
[timing] index:   23ms
[timing] budget:   8ms
[timing] output:  15ms
[timing] total:  905ms
```

Implementation: `std::time::Instant` around each pipeline stage. Available on all subcommands.

### Step 2: Parallel Parsing with Rayon

Add `rayon` dependency. Change file parsing loop to `par_iter()`.

- Tree-sitter parsers are thread-safe (each thread creates its own `Parser` instance)
- Cache lookups happen before parsing — cached files skip work entirely
- Expected ~2-4x speedup on large repos with cold cache
- Minimal impact on small repos or warm cache

### Step 3: Measure and Decide

After rayon lands, run `--timing` on real repos of varying sizes. If another stage dominates, address it then. No premature optimization beyond parsing.

---

## Workstream 3: Test Coverage (76% → 95%+, targeting 100%)

### Current State

Overall: 76.04% (2148/2825 lines)

Well-covered (90%+): budget/mod, commands/clean, output/*, util, python, rust, scanner
Moderate (70-90%): javascript, commands/overview, commands/trace, commands/diff, go, ruby
Weak (<70%): c (57%), cpp (57%), csharp (56%), java (58%), typescript (65%), kotlin (68%), swift (68%), index/graph (63%)

### Phase 1: Language Parsers

Each language gets tests for untested `walk_tree` branches:

- **c.rs** (57% → 95%+): structs, enums, multi-include, function pointers, static functions
- **cpp.rs** (57% → 95%+): namespaces, templates, nested classes, operator overloads, virtual methods
- **csharp.rs** (56% → 95%+): properties, generics, nested types, enum members, interfaces
- **java.rs** (58% → 95%+): generics, inner classes, annotations, enum constants, static methods
- **typescript.rs** (65% → 95%+): generics, decorators, mapped types, conditional types, re-exports
- **kotlin.rs** (68% → 95%+): data classes, companion objects, extension functions, sealed classes
- **swift.rs** (68% → 95%+): protocols, extensions, computed properties, enum cases, access levels

### Phase 2: Index Module

- **graph.rs**: graph construction, edge creation, traversal, empty graph, cycle handling
- **mod.rs**: `find_symbol`, `find_content_matches`, `CodebaseIndex` building, edge cases

### Phase 3: Commands and CLI

- **main.rs**: every error path via integration tests (bad subcommand, missing args, invalid tokens)
- **commands/overview.rs**: remaining pack mode branches, format-specific paths
- **commands/trace.rs**: no matches, multiple matches, BFS depth edge cases
- **commands/diff.rs**: binary files, permission errors, edge cases

### Phase 4: Mop-up

- **git/mod.rs**: no-commit repo, no-remote repo, empty log
- **cache/mod.rs**: 2 remaining lines
- **cache/parse.rs**: 2 remaining lines
- **budget/counter.rs**: 2 remaining lines
- **cli/mod.rs**: 1 remaining line

Anything genuinely unreachable (OS-level failure arms) gets a comment explaining why.

---

## Sequencing

1. **Test coverage first** — safety net before changing behavior
2. **Smart context second** — new feature, validated by the test suite
3. **Speed third** — rayon changes parsing internals, tests catch regressions

## Not In Scope

- `cxpak watch` (future)
- New languages
- Plugin changes (version bump in plugin.json at release)

## Release

Tag as **v0.6.0** when all three workstreams land.
