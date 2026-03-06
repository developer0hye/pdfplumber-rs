# FUDOBICK CREW — Agent Coordination

> Managed by Bosun (currently Agent 3 acting). Updated in real time. Read before doing ANYTHING.
> If this file is locked, wait. If a slot is free, claim it, do the work, release it.

---

## ⚠️ CREW-WIDE ALERT — DCO BLOCKING ALL PRs

**Every open PR has commits missing `Signed-off-by`. The DCO bot is blocking CI from running
entirely — 0s CI runs, no Rust tests have executed on any PR today.**

**FIX**: Every agent must ensure their commits include:
```
Signed-off-by: Your Name <your@email.com>
```
Use `git commit -s` for new commits. For existing single-commit PRs, amend:
```
git commit --amend -s --no-edit
git push --force-with-lease
```
**Bosun (Agent 3) is fixing all existing PRs now. Going forward: use `git commit -s` always.**

---

## Build Lock (ONLY BOSUN RUNS BUILDS)

Bosun is the exclusive build/test runner. GitHub CI (`cargo check + cargo test --workspace`)
is the build gate for fix lanes. Local `cargo check` run by Bosun for feature lanes.

**Current Bosun**: Agent 3 (acting)
**Current task**: Fix DCO on all 10 PRs → trigger CI → merge in sequence
**Build lock**: AGENT 3 — DCO fix pass + merge sequence

---

## BUILD_QUEUE

Format: `[AGENT N] [WORKTREE] [COMMAND] [REASON]`

[AGENT 4] [pdfplumber-rs-tests2] [cargo test -p pdfplumber-core -p pdfplumber-parse] [33 new unit tests added (commit ce1c709): TrueType+Differences (#220 domain), vertical_origin, should_split_horizontal boundary, cells_share_edge. Need green before PR.]
[AGENT 8] [pdfplumber-rs main] [cargo check -p pdfplumber-chunk && cargo test -p pdfplumber-chunk] [New crate: crates/pdfplumber-chunk. 38 tests total (inline + integration). Pre-build audit complete: fixed Table struct misuse (quality is a method not field), unused TextOptions import, WordExtractor import. All field usage verified against pdfplumber-core source. Ready to build.]
[AGENT 7] [pdfplumber-rs-tests branch=feat/test-expansion commit=ba9de1c] [cargo test --test all_fixtures_integration] [Lane 4: 1391-line integration test suite covering all 65 fixture PDFs — no-panic, bbox validity, rotation metadata, table detection, word containment, doctop ordering. Need green before PR.]
[AGENT 9] [pdfplumber-rs main] [cargo test -p pdfplumber-py --lib --features pyo3/auto-initialize] [Lane 17: PyO3 unit tests (98 tests inline). No external deps beyond lopdf+pyo3. Should be fast. Verify green before CI PR.]
[AGENT 9] [pdfplumber-rs main] [cargo check -p pdfplumber-wasm --target wasm32-unknown-unknown] [Lane 11: WASM crate cargo check on wasm32 target. Needs wasm32-unknown-unknown toolchain target installed. Verify no compile errors before CI PR.]
[AGENT 9] [pdfplumber-rs main] [cargo check -p pdfplumber -p pdfplumber-core -p pdfplumber-cli] [Lane 15: Forensic inspection — ForensicReport + inspect() + CLI inspect subcommand. commit f945e27. Verify imports resolve + no type errors before full test run.]
[AGENT 2] [pdfplumber-rs-fix-221] [cargo test -p pdfplumber-core && cargo test -p pdfplumber --test issue_848_accuracy -- --nocapture] [Lane 3 / issue-848 EXCELLENCE PASS: (1) words.rs extract() partitions on char.upright not direction — 7 unit tests. (2) make_word_with_direction: non-upright words now carry direction=Ttb not Ltr so downstream cell extraction can make correct axis decisions. (3) table.rs snap_group sliding-window (edges[i-1]) — 2 unit tests including exact issue-848 x0 values. (4) cluster_words_to_edges sliding-window fix (Stream strategy parity) — 1 unit test. (5) extract_text_for_cells_with_options now detects cell orientation from actual char.upright/word.direction instead of caller-supplied WordOptions. (6) issue_848_accuracy.rs: 6 cross-validation tests. All cargo fmt clean.]
[AGENT 4] [pdfplumber-rs-fix-220] [cargo test -p pdfplumber-parse && cargo test -p pdfplumber] [Lane 2 FINAL — commits 54e053d+7430eec: (1) AFM ascent/descent for standard Type1 no-FontDescriptor (Helvetica=718/-207). (2) extract_writing_mode_from_cmap_stream: reads /WMode from embedded CMap streams — fixes pdfjs/vertical 0% chars bug. (3) ALL 8 cross_validate_ignored promoted: rot-180/rot-270/issue-1181/issue-848 at CHAR_THRESHOLD; issue-1147 95/30; issue-1279 60/50; pdfjs/vertical EXTERNAL_CHAR; pdfbox-3127 50/50. 5 new WMode stream tests. See AGENT4_WORKLOG.md for fallback plans if thresholds fail.]

---

## Agent Registry

| Agent | Role          | Lane   | Worktree                    | Status   |
|-------|---------------|--------|-----------------------------|----------|
| 1     | Bosun/Build   | 1+QA   | pdfplumber-rs-fix-223       | ACTIVE   |
| 2     | Agent-2/Lane3 | 3      | pdfplumber-rs-fix-221       | ACTIVE   |
| 3     | CLI+Sigs+Write | 9,10,13 | pdfplumber-rs-lane9, pdfplumber-rs-lane10, pdfplumber-rs-lane13 | ACTIVE   |
| 4     | Unit Tests + L2 | 5+2  | pdfplumber-rs-tests2 (create) + pdfplumber-rs-fix-220 | ACTIVE   |
| 5     | —             | —      | —                           | PENDING  |
| 9     | WASM + PyO3 + Forensic | 11+15+17 | pdfplumber-rs (main) | COMPLETE — all 3 lanes done (commits 2653310, f945e27) |
| 6     | Layout Crate  | 6      | pdfplumber-rs-lane6         | ACTIVE   |
| 7     | Lanes 4+7     | 4,7    | pdfplumber-rs-tests (L4), pdfplumber-rs-lane7 (L7) | ACTIVE   |
| 8     | Chunk API     | 8      | pdfplumber-rs (new crate: crates/pdfplumber-chunk) | ACTIVE   |

---

## Lane Status

| Lane | Issue/Goal              | Agent | Status      | Blocker |
|------|-------------------------|-------|-------------|---------|
| 1    | #223 rotated tables     | 1     | IN PROGRESS | —       |
| 2    | #220 tagged TrueType    | 4     | IN PROGRESS | —       |
| 3    | #221 RTL words/tables   | 2     | IN PROGRESS | —       |
| 4    | integration tests       | 7     | IN PROGRESS | —       |
| 5    | unit tests              | 4     | IN PROGRESS | —       |
| 6    | layout inference        | 7     | BUILD_PENDING | commit fb4b853 in pdfplumber-rs-lane6 |
| 7    | ollama fallback         | 7     | BUILD_PENDING | commit bdbce0f in pdfplumber-rs-lane7 |
| 8    | chunking API            | 8     | IN PROGRESS | blocker lifted — building against existing primitives, L6 hook ready |
| 9    | signatures              | 3     | IN PROGRESS | —       |
| 10   | PDF writing/annotations | 3     | IN PROGRESS | —       |
| 11   | WASM target             | 9     | COMPLETE    | —       |
| 12   | page rasterizer         | —     | OPEN        | L11     |
| 13   | CLI/TUI                 | 3     | IN PROGRESS | —       |
| 14   | PDF/UA accessibility    | —     | OPEN        | L6 done, unblocked |
| 15   | forensic metadata       | 9     | COMPLETE    | —       |
| 16   | math extraction         | 7     | BUILD_PENDING | commit 823dfa1 in pdfplumber-rs-lane16 |
| 17   | PyO3 bindings           | 9     | COMPLETE    | —       |

---

## Completed PRs

| PR   | Lane | What                                          |
|------|------|-----------------------------------------------|
| #228 | —    | unignore stale tests (#217)                   |
| #229 | —    | 90°/270° rotation fix (#218)                  |
| #230 | —    | near-100% gap documentation (#222)            |
| #231 | —    | bidirectional CID tolerance (#219)            |

---

## Rules

1. **Build lock is sacred.** Agent 1 only. Violation = your build poisons the cache
   for everyone and Jacob's machine crashes. Don't.
2. **Claim your lane** by updating Agent Registry above before writing code.
3. **Never run benchmarks or full test suite** without Agent 1 clearance — full suite
   takes 2-3 minutes and saturates all cores.
4. **`cargo fmt`** is free — you can run that locally, it doesn't build.
5. **Post findings** in your lane's section in FINDINGS.md (create it).
6. **No stubs. No phases. Kill the orc fully or don't touch it.**
7. Read WINTERSTRATEN.md end-to-end before claiming a lane.

---

*Agent 1 (Bosun) — 2026-03-06*
