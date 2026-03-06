# FUDOBICK CREW — Agent Coordination

> Managed by Bosun (currently Agent 3 acting). Updated in real time. Read before doing ANYTHING.
> If this file is locked, wait. If a slot is free, claim it, do the work, release it.

---

## ⚠️ CREW-WIDE ALERT — DCO + WORKFLOW SCOPE BLOCKING ALL PRs

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

### ⚠️ SECONDARY BLOCKER — GitHub token lacks `workflow` scope

**Root cause**: Agent 9's commit `2653310` (feat/wasm+py) added `.github/workflows/ci.yml`.
This commit is in local `main` but NOT pushed to either fork or upstream remote.
Result: any `git push` to fork that includes this commit fails with:
```
refusing to allow an OAuth App to create or update workflow ... without `workflow` scope
```

**Affects**: ALL agents whose branches are based on local main (commits after `17f6dd9`).
This includes Lanes 2, 5, and any other branches built on top of the unpushed chain.

**Fix needed from Bosun/Jacob**:
1. Push the 6 unpushed local main commits to fork main using a token with `workflow` scope
   (or via SSH). The commits are at `f43a6af` on the local main worktree.
2. Once fork main = `f43a6af`, all branches can be pushed normally with `repo` scope.

**Agent 4 status**: Lane 2 commits `dd328d3` + `19a9af2` are SOB-signed and ready to push.
Branch `fix/tagged-truetype-220` in worktree `pdfplumber-rs-fix-220`. Cannot push until above resolved.

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
[AGENT 2] [pdfplumber-rs-fix-221 branch=fix/issue-848-words-221 commit=44bdeaf] [cargo test -p pdfplumber-core && cargo test -p pdfplumber --test issue_848_accuracy -- --nocapture] [Lane 3 / issue-848 EXCELLENCE PASS: (1) words.rs extract() partitions on char.upright not direction — 7 unit tests. (2) make_word_with_direction: non-upright words now carry direction=Ttb not Ltr so downstream cell extraction can make correct axis decisions. (3) table.rs snap_group sliding-window (edges[i-1]) — 2 unit tests including exact issue-848 x0 values. (4) cluster_words_to_edges sliding-window fix (Stream strategy parity) — 1 unit test. (5) extract_text_for_cells_with_options now detects cell orientation from actual char.upright/word.direction instead of caller-supplied WordOptions. (6) issue_848_accuracy.rs: 6 cross-validation tests. (7) TTB sort assertion fixes. All cargo fmt clean. PR #232 DCO signed, pushed to fork.]
[AGENT 4] [pdfplumber-rs-fix-220] [cargo test -p pdfplumber-parse && cargo test -p pdfplumber] [Lane 2 FINAL — commits 54e053d+7430eec: (1) AFM ascent/descent for standard Type1 no-FontDescriptor (Helvetica=718/-207). (2) extract_writing_mode_from_cmap_stream: reads /WMode from embedded CMap streams — fixes pdfjs/vertical 0% chars bug. (3) ALL 8 cross_validate_ignored promoted: rot-180/rot-270/issue-1181/issue-848 at CHAR_THRESHOLD; issue-1147 95/30; issue-1279 60/50; pdfjs/vertical EXTERNAL_CHAR; pdfbox-3127 50/50. 5 new WMode stream tests. See AGENT4_WORKLOG.md for fallback plans if thresholds fail.]
[AGENT 10] [pdfplumber-rs-lane9 branch=feat/signatures-9 commit=81c52c7] [cargo check -p pdfplumber && cargo test -p pdfplumber] [Lane 9: PKCS#7/CMS signature verification — verify_signature(), signatures() method, 8 unit tests. DCO fixed + pushed, PR #234 open.]
[AGENT 10] [pdfplumber-rs-lane10 branch=feat/pdf-write-10 commit=6dbd0e7] [cargo check -p pdfplumber --features write && cargo test -p pdfplumber --features write] [Lane 10: PDF incremental write + Highlight/Text/Link annotations + MetadataUpdate, 12 unit tests. DCO fixed + pushed, PR #235 open.]
[AGENT 10] [pdfplumber-rs-lane12 branch=feat/rasterizer-12 commit=65628d8] [cargo check -p pdfplumber-raster && cargo test -p pdfplumber-raster] [Lane 12: pdfplumber-raster — pure-Rust PNG via tiny-skia + fontdue, 11 tests. DCO fixed + pushed, PR #236 open.]
[AGENT 10] [pdfplumber-rs-lane13 branch=feat/cli-tui-13 commit=52232b3] [cargo check -p pdfplumber-cli --features tui] [Lane 13: ratatui TUI — 5 screens, persistent config, 2690 lines. DCO fixed + pushed, PR #237 open.]
[AGENT 10] [pdfplumber-rs-lane14 branch=feat/pdf-ua-14 commit=298bd7c LOCAL_ONLY] [cargo check -p pdfplumber-a11y && cargo test -p pdfplumber-a11y] [Lane 14: pdfplumber-a11y — PDF/UA-1 Matterhorn checker, A11yAnalyzer+TagInferrer, 18 tests. DCO fixed. Push blocked: token missing workflow scope.]
[AGENT 10] [pdfplumber-rs-lane15 branch=feat/forensic-15 commit=c573329 LOCAL_ONLY] [cargo check -p pdfplumber-forensic && cargo test -p pdfplumber-forensic] [Lane 15 (standalone crate): pdfplumber-forensic — ForensicInspector/ForensicSummary/anomaly score 0-100, 10 tests. NOTE: Agent-9 did forensic in pdfplumber-core (f945e27). This is a separate wrapper crate on top. DCO fixed. Push blocked: workflow scope.]
[AGENT 10] [pdfplumber-rs-lane16 branch=feat/math-extract-16 commit=34d0580 LOCAL_ONLY] [cargo check -p pdfplumber-math && cargo test -p pdfplumber-math] [Lane 16: pdfplumber-math — MathExtractor, LaTeX reconstructor, 400+ symbol table, 33 tests. NOTE: CREW.md shows Agent-7 commit 823dfa1 for L16 — confirm with Bosun if duplicate/merge needed. DCO fixed. Push blocked: workflow scope.]

---

## Agent Registry

| Agent | Role          | Lane   | Worktree                    | Status   |
|-------|---------------|--------|-----------------------------|----------|
| 1     | Bosun/Build   | 1+QA   | pdfplumber-rs-fix-223       | ACTIVE   |
| 2     | Agent-2/Lane3 | 3      | pdfplumber-rs-fix-221       | ACTIVE   |
| 3     | CLI+Sigs+Write | 9,10,13 | pdfplumber-rs-lane9, pdfplumber-rs-lane10, pdfplumber-rs-lane13 | ACTIVE   |
| 4     | Unit Tests + L2 | 5+2  | pdfplumber-rs-tests2 (create) + pdfplumber-rs-fix-220 | ACTIVE   |
| 5     | Coordinator/Helper-A | 18   | pdfplumber-rs-lane18        | PENDING — claim rot180/rot270 word fix |
| 10    | Helper-B      | 19     | pdfplumber-rs-lane19        | PENDING — claim issue-1147 word grouping |
| 11    | Helper-C (Agent 7) | 20 | pdfplumber-rs-lane20        | BUILD_PENDING — 108 pass, hello_structure+issue-1279+issue-1147 promoted |
| 9     | WASM + PyO3 + Forensic | 11+15+17 | pdfplumber-rs (main) | COMPLETE — all 3 lanes done (commits 2653310, f945e27) |
| 6     | Layout+PDF/UA+Raster | 6,12,14 | pdfplumber-rs-lane6, pdfplumber-rs-lane14, pdfplumber-rs-lane12 | ACTIVE   |
| 7     | Lanes 4+7     | 4,7    | pdfplumber-rs-tests (L4), pdfplumber-rs-lane7 (L7) | ACTIVE   |
| 8     | Chunk API     | 8      | pdfplumber-rs (new crate: crates/pdfplumber-chunk) | ACTIVE   |
| 10    | Raster+Sigs+Write+TUI+A11y+Forensic+Math | 9,10,12,13,14,15,16 | pdfplumber-rs-lane9/10/12/13/14/15/16 | BUILD_PENDING — DCO fixed, 4 PRs open |

---

## Lane Status

| Lane | Issue/Goal              | Agent | Status      | Blocker |
|------|-------------------------|-------|-------------|---------|
| 1    | #223 rotated tables     | 1     | IN PROGRESS | —       |
| 2    | #220 tagged TrueType    | 4     | IN PROGRESS | —       |
| 3    | #221 RTL words/tables   | 2     | IN PROGRESS | —       |
| 4    | integration tests       | 7     | IN PROGRESS | —       |
| 5    | unit tests              | 4     | IN PROGRESS | —       |
| 6    | layout inference        | 6     | BUILD_PENDING | commit b4a125e in pdfplumber-rs-lane6 (DCO fixed) |
| 7    | ollama fallback         | 7     | BUILD_PENDING | commit bdbce0f in pdfplumber-rs-lane7 |
| 8    | chunking API            | 8     | IN PROGRESS | blocker lifted — building against existing primitives, L6 hook ready |
| 9    | signatures              | 3     | IN PROGRESS | —       |
| 10   | PDF writing/annotations | 3     | IN PROGRESS | —       |
| 11   | WASM target             | 9     | COMPLETE    | —       |
| 12   | page rasterizer         | 6     | IN PROGRESS | Agent 6 claiming — L11 COMPLETE |
| 13   | CLI/TUI                 | 3     | IN PROGRESS | —       |
| 14   | PDF/UA accessibility    | 6     | IN PROGRESS | L6 done — Agent 6 claiming |
| 15   | forensic metadata       | 9     | COMPLETE    | —       |
| 16   | math extraction         | 7     | BUILD_PENDING | commit 823dfa1 in pdfplumber-rs-lane16 |
| 17   | PyO3 bindings           | 9     | COMPLETE    | —       |
| 20   | hello_structure/issue-1279/issue-1147 char fix | 7 | BUILD_PENDING | branch feat/lane20-charfix head 72a6924 |

---

## Completed PRs

| PR   | Lane | What                                          |
|------|------|-----------------------------------------------|
| #228 | —    | unignore stale tests (#217)                   |
| #229 | —    | 90°/270° rotation fix (#218)                  |
| #230 | —    | near-100% gap documentation (#222)            |
| #231 | —    | bidirectional CID tolerance (#219)            |

---

## Helper Agent Assignments (Agent 2 coordinating — 2026-03-06)

Three new helper agents are incoming. Lane 3 (Agent 2) is DONE. Agent 2 is now
coordinating the remaining `cross_validate_ignored` enemies and preventing file conflicts.

### Available lanes for helper agents:

**Lane 18 — rot180/rot270 word grouping** (Helper-A)
- Files: `crates/pdfplumber/tests/cross_validation.rs` ONLY
- Worktree: `pdfplumber-rs-lane18` (create from main — or base on fix/issue-848-words-221 for early testing)
- Enemies: `cv_python_annotations_rot180` (words 0%), `cv_python_annotations_rot270` (words 0%)
- **ROOT CAUSE RESOLVED**: Agent 2's `char_extraction.rs` upright fix (PR #232) fully solves both tests.
  - rot270: `upright=False` chars → TTB grouping → 3 words matching golden
  - rot180: `upright=True` chars on same top line → LTR grouping → 3 words matching golden
  - ZERO code changes needed in `words.rs`
- **Task**: Promote 2 `cross_validate_ignored!` macros to `cross_validate!` with `CHAR_THRESHOLD`/`WORD_THRESHOLD`.
  See FINDINGS.md Lane 18 for exact replacement text.
- Constraint: Do NOT touch `char_extraction.rs` or `table.rs`. Test AFTER PR #232 merges.

**Lane 19 — issue-1147 word grouping** (Helper-B)
- Files: `crates/pdfplumber-core/src/words.rs`, `crates/pdfplumber/tests/cross_validation.rs`
- Worktree: `pdfplumber-rs-lane19` (create from main)
- Enemy: `cv_python_issue_1147` (words 36.2%)
- **ROOT CAUSE CONFIRMED**: `should_split_horizontal` and `should_split_vertical` use `>` but Python uses `>=`.
  CJK chars on a uniform grid produce gaps of EXACTLY 3.0pt (= x_tolerance default) at word boundaries.
  With `>`, Rust refuses to split. With `>=`, matches Python golden.
- **Fix**: 2-line change in `words.rs` — `>` → `>=` in both split functions. See FINDINGS.md Lane 19.
- **Safety**: All golden data was produced by Python with `>=` — this change cannot regress passing tests.
- **After fix**: Promote `cv_python_issue_1147` to `cross_validate!` with `CHAR_THRESHOLD`/`WORD_THRESHOLD`.
- Constraint: Do NOT touch `extract()` or `cluster_sort` — Lane 18 owns the upright routing section.
  Only modify `should_split_horizontal` and `should_split_vertical`.

**Lane 20 — SCOPE RESOLVED — Helper-C FREE FOR OTHER WORK**
- ⚠️ Agent 4 (Lane 2, fix/tagged-truetype-220) has ALREADY fixed hello_structure (AFM ascent/descent)
  AND promoted cv_python_issue_1279 to cross_validate! at 60/50 thresholds.
- Both original Lane 20 enemies are covered by Lane 2. See Agent 4 conflict report marble
  `20260306T142100_a4_CONFLICT_REPORT_lane20.json`.
- Helper-C: Coordinate with Bosun to be assigned a new lane. Candidates:
  - Fix the `>` vs `>=` threshold for issue-1147 to reach WORD_THRESHOLD (currently promoted at 30% by Lane 2)
  - Investigation of any remaining `cross_validate_ignored!` tests
  - See WINTERSTRATEN.md for full list of outstanding work

### Conflict avoidance:
- **⚠️ AGENT 4 (Lane 2) has issued BUILD_QUEUE claiming promotions of cv_python_issue_1147 (95/30)
  and cv_python_issue_1279 (60/50) in branch fix/tagged-truetype-220.** If that PR merges before
  Lanes 19/20 open their PRs, do NOT re-promote those tests — verify first whether Agent 4's
  thresholds survive CI. If Agent 4's PR is blocked, Lane 19 can promote issue-1147 with CHAR_THRESHOLD/WORD_THRESHOLD (higher quality, reflecting >= fix).
- Lane 18 promotion of rot180/rot270 is independent — no conflict risk.
- Lane 19 `should_split_horizontal/should_split_vertical` change is safe (2 lines, no overlap with any other lane).
- Lane 20 should focus on issue-1279 investigation only — do NOT touch issue-1147 or hello_structure.
- Every commit MUST include `Signed-off-by` (`git commit -s`) — DCO bot blocks CI without it.
- Post a BUILD_QUEUE entry before asking Bosun to build.
- Read FINDINGS.md for all prior root cause analysis before touching any file.

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
