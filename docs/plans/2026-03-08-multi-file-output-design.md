# Multi-File Output (Pack Mode) Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** When a repo exceeds the token budget, preserve the full CPU analysis in detail files alongside the budgeted overview.

**Motto:** "Spends CPU cycles so you don't spend tokens." The briefing packet is always one file within budget. The detail files preserve everything cxpak computed, so the LLM can pull them on demand instead of groping in the dark.

---

## Decision Logic

```
total_repo_tokens = index.total_tokens
if total_repo_tokens <= token_budget:
    → Single-file mode (current behavior, unchanged)
else:
    → Pack mode: overview file + .cxpak/ directory
```

## Single-File Mode (unchanged)

- Triggers when repo fits within token budget
- Output goes to stdout or `--out path.md`
- No `.cxpak/` directory created
- Identical to current v1 behavior

## Pack Mode

When repo exceeds token budget:

### Overview File

Goes to stdout or `--out` — budgeted, same structure as today, with two changes:

1. **Metadata gains a detail-files line:**

```markdown
## Project Metadata

- **Files:** 498
- **Total size:** 1258648.7 KB
- **Estimated tokens:** ~646k
- **Token budget:** 50k
- **Detail files:** `.cxpak/` (full untruncated analysis)
- **Languages:**
  - python — 307 files (61%)
```

2. **Omission markers become pointers:**

Instead of:
```
<!-- signatures omitted: ~39.4k tokens. Use --tokens 54k+ to include -->
```

Pack mode emits:
```
<!-- full content: .cxpak/signatures.md (~39.4k tokens) -->
```

Sections that fit in their budget allocation render normally in the overview — no pointer. Only truncated sections get pointers.

### Detail Files

Written to `.cxpak/` in the target repo's root:

```
<repo>/
  .cxpak/
    tree.md
    modules.md
    dependencies.md
    signatures.md
    key-files.md
    git.md
  .gitignore          ← ".cxpak/" appended if not already present
```

Each detail file is:
- **Standalone and self-contained** — no budget, no truncation
- **Full output** of the CPU analysis for that section
- **Same format** as the overview (`--format` flag applies)
- Metadata is the only section that stays overview-only (~500 tokens, always fits)

Detail file contents:
- `tree.md` — Complete directory tree, every file
- `modules.md` — Full module map, every file with symbols, every symbol listed
- `dependencies.md` — Full dependency graph, every file's imports
- `signatures.md` — Every public symbol's signature (biggest win for large repos)
- `key-files.md` — Full content of key files (README, Cargo.toml, etc.)
- `git.md` — Full git context (all commits, churn, contributors)

### .gitignore Handling

On pack mode, cxpak reads the repo's `.gitignore` and appends `.cxpak/` if not already present. Idempotent — won't duplicate.

## What Changes

1. **`src/commands/overview.rs`** — After rendering, check if any section was truncated. If yes, write `.cxpak/` with unbudgeted detail files. Swap omission markers for pointer markers.
2. **`src/budget/degrader.rs`** — New `omission_pointer` function for pack-mode markers. `truncate_to_budget` returns truncation info.
3. **`src/output/mod.rs`** — Render individual sections to standalone files (reuse existing per-section rendering).
4. **Gitignore utility** — Small function to append `.cxpak/` to `.gitignore`.
5. **Tests** — Integration tests for both modes.

## What Doesn't Change

- CLI args (no new flags)
- Scanner, parser, indexer
- Budget allocation percentages
- Single-file mode behavior
- Output format selection (md/xml/json)

## Estimated Scope

~200 lines of new/changed Rust code, ~100 lines of new tests.
