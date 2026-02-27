# Ralph Agent Instructions for pdfplumber-rs

You are an autonomous coding agent working on the **pdfplumber-rs** project — a pure Rust library that extracts chars/words/lines/rects/curves/tables from PDF 1.7 documents with precise coordinates. It is a Rust implementation of Python's pdfplumber.

## Project Context

- **Project root**: The current working directory
- **Language**: Rust (edition 2024, MSRV 1.85)
- **Architecture**: 5-layer stack — PDF Parsing → Content Stream Interpreter → Object Extraction → Text Grouping → Table Detection
- **Workspace**: Three crates — `crates/pdfplumber-parse` (Layer 1-2: parsing + interpreter), `crates/pdfplumber-core` (Layer 3-5: algorithms), `crates/pdfplumber` (public API facade)
- **Coordinate system**: Top-left origin (x0, top, x1, bottom) matching Python pdfplumber

Read these project files for full context:
- `pdfplumber-rs-PRD.revised.v0.2.md` — Full product requirements
- `CLAUDE.md` — Development guidelines and conventions
- `METHODOLOGY.md` — Software methodology principles
- `CONTEXT-MANAGEMENT.md` — Project memory management

## Your Task

1. Read the PRD at `scripts/ralph/prd.json`
2. Read the progress log at `scripts/ralph/progress.txt` (check Codebase Patterns section first)
3. Check you're on the correct branch from PRD `branchName`. If not, create it from `main`.
4. Pick the **highest priority** user story where `passes: false`
5. **Write tests first** (TDD: Red-Green-Refactor cycle)
6. Implement that single user story
7. Run quality checks (see below)
8. If checks pass, commit ALL changes with message: `feat: [Story ID] - [Story Title]`
9. Update the PRD to set `passes: true` for the completed story
10. Append your progress to `scripts/ralph/progress.txt`
11. **Push, create PR, verify CI, and merge** (see Per-Story PR Flow below)

## Quality Check Commands

Run ALL of these before committing. ALL must pass:

```bash
cargo fmt --all -- --check        # Format check
cargo clippy --workspace -- -D warnings  # Lint check
cargo test --workspace            # All tests
cargo check --workspace           # Compile check
```

If any check fails, fix the issues before committing.

## Rust Development Rules

- **Prefer Rust native implementations.** Avoid unnecessary external dependencies. Use the standard library as much as possible.
- **Follow TDD.** Write failing tests first, then minimal implementation, then refactor.
- **Only add third-party crates when clearly justified** (e.g., `lopdf` for PDF parsing, `thiserror` for errors).
- When adding a new dependency, add it to the appropriate `Cargo.toml` (workspace root or crate-level).
- Follow existing code patterns — check how existing modules are structured before adding new ones.
- Keep all public APIs documented with doc comments.

## Git Rules

- Verify git config before first commit: `git config user.name` and `git config user.email`
- All commits must include `Signed-off-by` line: use `git commit -s`
- Commit message format: `feat: [Story ID] - [Story Title]`
- Commit only code that passes ALL quality checks

## Key Dependencies (approved for use)

| Crate | Purpose |
|---|---|
| `lopdf` | Default PDF parsing backend |
| `thiserror` | Library error types |
| `serde`, `serde_json` | Optional serialization (behind `serde` feature) |
| `rayon` | Optional parallel processing (behind `parallel` feature) |
| `tracing` | Optional debug/observability (behind `tracing` feature) |

Do NOT add dependencies beyond these unless absolutely necessary. If you must add one, document why in the progress log.

## Architecture Overview

```
┌──────────────────────────────────────────────────────────────┐
│  Layer 5: Table Detection (Lattice / Stream / Explicit)      │
├──────────────────────────────────────────────────────────────┤
│  Layer 4: Text Grouping & Reading Order                      │
│  Characters → Words → Lines → TextBlocks                     │
├──────────────────────────────────────────────────────────────┤
│  Layer 3: Object Extraction                                  │
│  Chars (bbox/font/size/color), Paths (lines/rects/curves)    │
├──────────────────────────────────────────────────────────────┤
│  Layer 2: Content Stream Interpreter                         │
│  Text state, Graphics state, CTM, XObject Do                 │
├──────────────────────────────────────────────────────────────┤
│  Layer 1: PDF Parsing (pluggable backend via PdfBackend)     │
│  lopdf (default) / pdf-rs (optional)                         │
└──────────────────────────────────────────────────────────────┘
```

## Test Fixtures

For integration tests requiring actual PDF files, place small test fixture files in `tests/fixtures/`. Create minimal PDF files programmatically where possible using lopdf's PDF writing capabilities for controlled test cases.

## Progress Report Format

APPEND to `scripts/ralph/progress.txt` (never replace, always append):
```
## [Date/Time] - [Story ID]
- What was implemented
- Files changed
- Dependencies added (if any)
- **Learnings for future iterations:**
  - Patterns discovered
  - Gotchas encountered
  - Useful context
---
```

## Consolidate Patterns

If you discover a **reusable pattern**, add it to the `## Codebase Patterns` section at the TOP of `scripts/ralph/progress.txt` (create it if it doesn't exist). Only add patterns that are general and reusable.

## Per-Story PR Flow

After EACH completed user story (step 11), you MUST push, create a PR, verify CI, and merge:

1. **Push**: `git push -u origin <branchName>` (branchName from PRD)
2. **Create PR**:
   - `gh pr create --title "feat: [Story ID] - [Story Title]" --body "<summary of changes + test plan>" --base main`
   - If a PR already exists for this branch, skip creation
3. **Wait for CI** (30s then watch): `sleep 30 && gh pr checks <number> --watch`
4. **If CI fails** (retry up to 3 times):
   a. `gh run list --branch <branchName> --status failure --json databaseId --jq '.[0].databaseId'`
   b. `gh run view <run-id> --log-failed 2>&1 | head -200`
   c. Fix errors, run local quality checks, commit: `git commit -s -m "fix: resolve CI failures for [Story ID]"`
   d. Push and go back to step 3
5. **Merge PR**: `gh pr merge <number> --merge`
6. **Sync branch with main**:
   ```bash
   git fetch origin main
   git reset --hard origin/main
   git push --force-with-lease origin <branchName>
   ```

If merge or CI fails after retries, leave the PR open and continue to the next story (the orchestrator or a human will handle it).

## Stop Condition

After completing a user story and merging its PR, check if ALL stories have `passes: true`.

If ALL stories are complete, respond with `<promise>COMPLETE</promise>`.

If there are still stories with `passes: false`, end your response normally (another iteration will pick up the next story).

## Important

- Work on ONE story per iteration
- Write tests FIRST (TDD)
- Commit frequently
- Keep CI green (all quality checks must pass)
- Read the Codebase Patterns section in progress.txt before starting
- Do NOT modify this file (CLAUDE.md) during execution
