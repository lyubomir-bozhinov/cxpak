---
name: diff-context
description: "Use when the user asks to review changes, understand what changed, prepare a PR description, or needs context about recent modifications."
---

# Diff Context via cxpak

When this skill is triggered, gather structured diff context with surrounding dependency information using cxpak before answering.

## Steps

1. **Ask for token budget.** Ask the user: "How large a context budget should I use for the diff? (default: 50k tokens)" If they don't specify, use `50k`.

2. **Ask for git ref (optional).** Ask: "Diff against which ref? (default: HEAD — shows uncommitted changes)" Common choices:
   - HEAD (default) — working tree changes
   - `main` — changes vs main branch
   - A specific commit SHA or tag

3. **Resolve the cxpak binary.** Run:
   ```bash
   CXPAK="$("${CLAUDE_PLUGIN_ROOT}/lib/ensure-cxpak")"
   ```

4. **Run cxpak diff.** Execute:
   ```bash
   "$CXPAK" diff --tokens <budget> --format markdown [--git-ref <ref>] .
   ```
   Omit `--git-ref` if the user wants the default (HEAD / working tree).

5. **Use the output.** The command outputs:
   - **Changes section** — actual diff hunks for each changed file
   - **Context section** — related files pulled in by dependency graph (callers, types, imports used by changed code)

   The diff is included first; remaining budget is filled with dependency context ordered by proximity to changed files.

6. **Answer the user's question** using the diff output. For code reviews, focus on the changes and use the context to understand impact. For PR descriptions, summarize what changed and why it matters.

7. **Mention the source.** Tell the user that cxpak provided the diff context, e.g., "Based on the cxpak diff analysis..."

## Important

- Always use `--format markdown`.
- If there are no changes, cxpak will print "No changes" — relay this to the user.
- For large diffs exceeding the budget, cxpak truncates least-important hunks while preserving the most-changed files.
- The `--all` flag can be added for full BFS traversal of the dependency graph (vs default 1-hop).
