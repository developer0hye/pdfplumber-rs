---
document: "Python pdfplumber Compatibility and Replacement Roadmap"
repository: "developer0hye/pdfplumber-rs"
status: "ACTIVE"
last_full_audit: "2026-08-16"
rust_audit_baseline: "da0663ce27f35bfc641055c0cebf8fae97932ac4"
python_compatibility_target: "jsvine/pdfplumber v0.11.10"
python_target_commit: "7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62"
---

# Product Requirements Document (PRD) — Make `pdfplumber-rs` a Drop-in Replacement for Python `pdfplumber`

> **Living document. Keep this file at the repository root as `PRD.md`.**
>
> This is the source of truth for Claude, Codex, and human contributors working on Python `pdfplumber` compatibility. Update this file in the same pull request as the code and tests that change a task's status.

## Terminology

| Full term | Abbreviation used below |
|---|---|
| Portable Document Format | PDF |
| Application Programming Interface | API |
| Command-Line Interface | CLI |
| Continuous Integration | CI |
| Product Requirements Document | PRD |
| Test-Driven Development | TDD |
| Optical Character Recognition | OCR |
| Chinese, Japanese, and Korean | CJK |
| WebAssembly | WASM |
| Hypertext Markup Language | HTML |
| Scalable Vector Graphics | SVG |
| Global Interpreter Lock | GIL |
| JavaScript Object Notation | JSON |
| Comma-Separated Values | CSV |
| Input/Output | I/O |
| Semantic Versioning | SemVer |
| Uniform Resource Identifier | URI |
| Red, Green, Blue | RGB |
| Cyan, Magenta, Yellow, and Key (black) | CMYK |
| Multipurpose Internet Mail Extensions | MIME |
| International Color Consortium | ICC |
| Current Transformation Matrix | CTM |
| Marked-Content Identifier | MCID |
| Dots per inch | DPI |
| Character Map | CMap |
| Compact Font Format | CFF |
| Rivest Cipher 4 | RC4 |
| Software Bill of Materials | SBOM |
| Right-to-left | RTL |
| Central Processing Unit | CPU |
| Normalization Form Canonical Composition | NFC |
| Normalization Form Canonical Decomposition | NFD |
| Normalization Form Compatibility Composition | NFKC |
| Normalization Form Compatibility Decomposition | NFKD |
| Discrete Cosine Transform | DCT |
| Joint Photographic Experts Group | JPEG |
| JPEG 2000 codestream | JPX |
| Consultative Committee for International Telegraphy and Telephony | CCITT |
| Joint Bi-level Image Experts Group 2 | JBIG2 |
| Lempel-Ziv-Welch | LZW |
| American Standard Code for Information Interchange | ASCII |
| National Instant Criminal Background Check System | NICS |
| Open Source Software | OSS |
| Identifier | ID |

## 1. Goal

Make `pdfplumber-rs` a production-ready Rust implementation that can replace Python `pdfplumber` for its supported use cases, with two compatible surfaces:

1. A strong native Rust API.
2. A Python package imported as `pdfplumber` that preserves Python `pdfplumber`'s documented API, defaults, return schemas, ordering, exceptions, and observable behavior.

The target for this document is Python `pdfplumber` **v0.11.10**. A faster implementation is valuable, but speed does not compensate for incompatible behavior.

### 1.1 What "drop-in replacement" means

A user should be able to replace the installed distribution and keep ordinary application code unchanged:

```python
import pdfplumber

with pdfplumber.open("document.pdf") as pdf:
    page = pdf.pages[0]
    text = page.extract_text()
    words = page.extract_words()
    tables = page.extract_tables()
```

The replacement is complete only when:

- Imports, module paths, classes, methods, properties, signatures, defaults, and exceptions are compatible.
- Page numbering and coordinate conventions are compatible.
- Object dictionaries contain compatible keys, value types, and ordering.
- Text, search, crop, table, serialization, visual-debugging, and Command-Line Interface behavior pass differential tests.
- Any deliberate deviation is recorded, justified, tested, and approved in the deviation registry.
- No compatibility test is ignored merely because it fails.
- Supported wheels install and run on every declared platform and Python version.
- Malformed or hostile input cannot cause an uncontrolled panic, unbounded resource use, or undefined behavior.

### 1.2 Upstream-aligned non-goals

The following are not required for parity because Python `pdfplumber` does not provide them as core features:

- Optical Character Recognition (OCR).
- PDF generation or modification.
- Strong table extraction from scanned-image-only PDFs.
- Semantic document understanding beyond the PDF structure tree.

Rust-only features such as image export, signatures, bookmarks, Hypertext Markup Language (HTML) export, Scalable Vector Graphics (SVG) export, WebAssembly (WASM), parallel page processing, and table quality scores are welcome, but they must not alter compatibility-mode behavior.

---

## 2. Baselines and Audit Scope

### 2.1 Fixed compatibility target

- Python repository: `jsvine/pdfplumber`
- Target release: `v0.11.10`
- Target commit: `7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62`
- Target public modules reviewed:
  - `pdfplumber/__init__.py`
  - `pdfplumber/pdf.py`
  - `pdfplumber/page.py`
  - `pdfplumber/container.py`
  - `pdfplumber/table.py`
  - `pdfplumber/display.py`
  - `pdfplumber/structure.py`
  - `pdfplumber/repair.py`
  - `pdfplumber/convert.py`
  - `pdfplumber/ctm.py`
  - `pdfplumber/cli.py`
  - `pdfplumber/utils/*`
  - upstream tests and PDF fixtures

Do not silently move the compatibility target to a newer upstream version. Create a dedicated target-upgrade task, regenerate snapshots, and review the changelog first.

### 2.2 Rust source audit baseline

- Rust repository: `developer0hye/pdfplumber-rs`
- Audited commit: `da0663ce27f35bfc641055c0cebf8fae97932ac4`
- Main areas reviewed:
  - `crates/pdfplumber-core`
  - `crates/pdfplumber-parse`
  - `crates/pdfplumber`
  - `crates/pdfplumber-cli`
  - `crates/pdfplumber-py`
  - `crates/pdfplumber-wasm`
  - `scripts/parity_report.py`
  - `crates/pdfplumber/tests/cross_validation.rs`
  - `.github/workflows/ci.yml`
  - `.github/workflows/release.yml`
  - open compatibility issues as of 2026-08-16

The status labels in the audit sections below are based on source inspection. They are not substitutes for runtime acceptance tests.

---

## 3. Agent Operating Contract

Claude, Codex, and human contributors must follow this loop.

### 3.1 Before coding

1. Read `AGENTS.md`, `CLAUDE.md`, `METHODOLOGY.md`, and this file.
2. Fetch the latest `main` and compare it with the audit baseline above.
3. Use a dedicated branch and worktree as required by `CLAUDE.md`.
4. Claim exactly one task in the **Active Work** table.
5. Select the highest-priority unchecked task whose dependencies are complete.
6. Read the corresponding Python `pdfplumber` v0.11.10 implementation and tests.
7. Run the relevant existing tests to establish the pre-change baseline.

### 3.2 While coding

1. Follow Test-Driven Development (TDD): red, green, refactor.
2. Add a failing differential or contract test before changing implementation.
3. Use at least two materially different fixtures when one fixture could encourage overfitting.
4. Implement the general rule, never a filename-specific, page-specific, glyph-specific, or expected-output-specific workaround.
5. Keep compatibility behavior separate from Rust-only enhancements.
6. Prefer a thin Python compatibility shim over forcing Python semantics into every Rust public type.
7. Preserve deterministic output.
8. Do not weaken tolerances, thresholds, assertions, or corpus coverage to make a test pass.
9. Do not mark a task complete based only on the existence of a function or type.

### 3.3 Before checking a task

A task may change from `[ ]` to `[x]` only when all of the following are true:

- Its implementation is complete.
- Its documented acceptance criteria are met.
- Focused tests pass.
- Relevant upstream differential tests pass.
- The full required test tier passes.
- No new ignored test or approved deviation was introduced without review.
- Documentation and type stubs are updated when public behavior changed.
- An entry is added to the **Evidence Ledger** with the commit or pull request and exact test commands.

A partially implemented task stays unchecked. Add a note to the Active Work table or pull request instead.

### 3.4 After coding

1. Run `cargo fmt`.
2. Run the focused test suite.
3. Run the required workspace, Python, and compatibility suites.
4. Update this file in the same commit.
5. Add evidence to the ledger.
6. Commit with sign-off and open a focused pull request.
7. Follow the `CLAUDE.md` PR merge procedure. Autonomously merge a PR only after every check on its exact current head succeeds, then verify it on `main`; leave a non-green PR open with its blocker documented.
8. Do not start another task until the current task is committed and its branch state is clear.

### 3.5 Multi-agent coordination

- One task identifier per branch.
- One active owner per task.
- Agents may work in parallel only when task dependencies do not overlap.
- Do not edit or reorder another agent's active task.
- Resolve shared-file conflicts by preserving both evidence entries and all newly discovered tasks.
- Never delete completed task identifiers; stable identifiers are required for history.

---

## 4. Status and Priority Legend

### 4.1 Checklist meaning

- `[x]` — verified complete under the rules above.
- `[ ]` — missing, partial, incorrect, blocked, or not yet proven.

### 4.2 Audit labels

- `PRESENT` — a relevant code surface exists.
- `PARTIAL` — some behavior exists, but required options, schema, or semantics are missing.
- `INCOMPATIBLE` — behavior exists but conflicts with Python `pdfplumber`.
- `MISSING` — no relevant compatibility surface was found.
- `EXTENSION` — useful Rust-only behavior, not an upstream parity feature.
- `UNVERIFIED` — source suggests support, but differential proof is absent.

### 4.3 Priorities

- `P0` — blocks any drop-in-compatibility claim.
- `P1` — required for a production replacement.
- `P2` — hardening, ecosystem completeness, and performance.
- `P3` — optional Rust-native enhancement.

---

## 5. Release-Wide Acceptance Rules

These rules apply to every compatibility task unless a stricter task-specific rule is stated.

### 5.1 API contract

- Public module names, exports, class names, inheritance relationships, call signatures, keyword-only behavior, defaults, properties, return types, and exception categories must match the target snapshot.
- API contract match rate: **100%** for the selected compatibility tier.
- Existing common code examples from the upstream README must run unchanged.

### 5.2 Behavioral differential testing

- Text output must match exactly unless an approved delta exists.
- Object count and object order must match exactly on the compatibility corpus.
- String, boolean, integer, `None`, list, tuple, and dictionary value types must match.
- Coordinates must differ by no more than **0.05 point** before user-requested rounding.
- User-requested precision and serialized output must match exactly.
- Table grid dimensions, `None` placement, cell order, and cell text must match exactly.
- Zero-width and whitespace-only search matches must be discarded identically.
- Exception type and failure phase must match; message text should match where applications commonly rely on it.

The current broad `95%` match thresholds and `1.0 point` coordinate tolerance are diagnostic only. They are not sufficient for a drop-in replacement.

### 5.3 Deviations

Create `compat/approved_deltas.toml` with one entry per intentional difference:

- stable identifier
- affected fixture and API
- upstream result
- Rust result
- technical reason
- compatibility risk
- approving maintainer
- regression test
- expiration or review condition

An unregistered delta is a failure.

### 5.4 Reliability

- No panic may cross the Rust public API or Python boundary.
- No malformed input may cause infinite recursion, unbounded allocation, decompression blow-up, or an uncontrolled hang.
- Resource-limit errors must be deterministic and actionable.
- Repeated extraction from the same page must be deterministic.
- Closing a document or page must release owned resources without invalid memory access.

---

## 6. Current Implementation Inventory

> In this section only, `[x]` means that a source-level implementation surface was observed at the audit baseline. It does **not** mean Python parity is complete.

### 6.1 Architecture and distribution surfaces

- [x] `pdfplumber-core` contains backend-independent data types and algorithms.
- [x] `pdfplumber-parse` contains PDF parsing, font, CMap, and content-stream interpretation code.
- [x] `pdfplumber` exposes a public Rust facade.
- [x] `pdfplumber-cli` exposes a Rust Command-Line Interface with multiple subcommands.
- [x] `pdfplumber-py` exposes PyO3-based Python classes.
- [x] `pdfplumber-wasm` provides a WebAssembly surface.
- [x] Optional `serde` serialization exists for many public data types.
- [x] Optional Rayon-based parallel page processing exists.
- [x] Release automation builds crates, Python wheels/source distribution, and a Node Package Manager package.

### 6.2 PDF and page processing

- [x] Open from bytes and filesystem paths.
- [x] Core password-protected PDF APIs exist.
- [x] A best-effort repair path exists.
- [x] Metadata extraction exists.
- [x] Page boxes, dimensions, rotation, and page iteration exist.
- [x] Characters, words, text lines, text blocks, and page text extraction exist.
- [x] Lines, rectangles, curves, edges, and image metadata extraction exist.
- [x] Crop, within-bounding-box, outside-bounding-box, filter, and character deduplication surfaces exist.
- [x] Search exists.
- [x] Lattice, strict-lattice, stream, explicit, and per-axis table strategies exist.
- [x] Table cells, rows, columns, merged-cell handling, and quality metrics exist.
- [x] Annotation, hyperlink, form-field, structure-tree, bookmark, and signature code exists.
- [x] Image-content extraction/export code exists.
- [x] HTML and SVG export/debug surfaces exist.
- [x] Unicode normalization, bidirectional text handling, CJK CMaps, glyph lists, and multiple font parsers exist.
- [x] Validation warnings and resource-budget concepts exist.

### 6.3 Existing compatibility infrastructure

- [x] Golden-data generation scripts exist.
- [x] A Python-versus-Rust parity report exists.
- [x] Cross-validation tests compare characters, words, lines, rectangles, and tables.
- [x] Multiple real-world and upstream issue fixtures are present.
- [x] Open issues already track several extraction outliers.

### 6.4 Python binding surface currently observed

- [x] `PDF.open(path)` and `PDF.open_bytes(data)`.
- [x] `PDF.pages` and `PDF.metadata`.
- [x] Basic `Page` dimensions and page number.
- [x] Basic character, text, word, table, geometry, crop, and search methods.
- [x] Basic `Table` bounding box, rows, extraction, and accuracy.
- [x] Basic Python exception subclasses for Rust errors.
- [x] Type stub and Python package metadata exist.

---

## 7. Critical Audit Findings

These are confirmed compatibility blockers at the audit baseline.

| Area | Current behavior | Python `pdfplumber` target | Status |
|---|---|---|---|
| Top-level open | `PDF.open(...)` only | `pdfplumber.open(...)` alias plus `PDF.open(...)` | `P0 MISSING` |
| Python module layout | One native extension module | Package with public submodules such as `page`, `table`, `utils`, `display`, `structure`, `repair`, and `ctm` | `P0 MISSING` |
| Page number | Python binding returns 0-based index | Public `page_number` is 1-based | `P0 INCOMPATIBLE` |
| Object collections | `chars()`, `lines()`, `rects()`, and similar methods | Properties such as `.chars`, `.lines`, `.rects` | `P0 INCOMPATIBLE` |
| Open inputs | Filesystem string and raw bytes | String, `pathlib.Path`, binary file object, and `BytesIO` | `P0 PARTIAL` |
| Open options | Binding does not expose the full option set | `pages`, `laparams`, `password`, `strict_metadata`, `unicode_norm`, `repair`, `gs_path`, `repair_setting`, `raise_unicode_errors` | `P0 PARTIAL` |
| Lifecycle | No compatible context manager or close behavior | `with`, `close`, owned/external stream handling, page cache flushing | `P0 MISSING` |
| Character schema | Small subset of keys | Full object dictionary including coordinate variants, matrix, advance, object type, page number, marked-content fields, and colors | `P0 PARTIAL` |
| Line schema | Uses names such as `line_width` and `stroke_color` | Uses `linewidth`, `stroking_color`, `non_stroking_color`, `object_type`, and full coordinate fields | `P0 INCOMPATIBLE` |
| Image schema | Uses `src_width`, `src_height`, `bits_per_component`, `color_space` | Uses `srcsize`, `bits`, `colorspace`, `stream`, `imagemask`, and common object keys | `P0 INCOMPATIBLE` |
| Crop | Includes objects by center and rebases coordinates to crop origin | Includes intersecting objects, clips partial objects, and preserves root-page coordinates | `P0 INCOMPATIBLE` |
| Text options | Binding mostly exposes `layout` and x/y tolerance | Full word, layout, direction, tolerance-ratio, rendering, line, and search options | `P0 PARTIAL` |
| Layout algorithm | Rust block/column algorithm with different defaults | Python `TextMap` grid-style layout with `x_density=7.25` and `y_density=13` | `P0 INCOMPATIBLE` |
| Search | Pattern, regex flag, and case flag only | Compiled patterns, `main_group`, groups, chars, layout, and all text options | `P0 PARTIAL` |
| Table API | Default settings only; simplified row representation | Full `TableSettings`, `TableFinder`, `Table`, `Row`, `Column`, largest-table APIs, and extraction options | `P0 PARTIAL` |
| Visual debugging | SVG-oriented Rust extension | `Page.to_image()` and Pillow-compatible `PageImage` API | `P1 MISSING` |
| Serialization | Rust/CLI serialization exists | Python `to_dict`, `to_json`, and `to_csv` contracts | `P1 PARTIAL` |
| CLI | Rich subcommand CLI | Upstream-compatible positional CLI and flags | `P1 INCOMPATIBLE` |
| Python/WASM CI | Main CI excludes Python and WASM crates | Every published surface must be tested | `P0 PARTIAL` |
| Golden environment | Setup installs unpinned latest `pdfplumber` | Exact target version and dependency lock | `P0 INCOMPATIBLE` |
| Differential gate | Broad percentages, first-page/report limitations, and tolerated ignores | Exact ordered contract across all pages and option matrices | `P0 PARTIAL` |

### 7.1 Recommended compatibility architecture

Use a mixed Python package:

```text
crates/pdfplumber-py/
├── python/pdfplumber/
│   ├── __init__.py
│   ├── pdf.py
│   ├── page.py
│   ├── container.py
│   ├── table.py
│   ├── display.py
│   ├── structure.py
│   ├── repair.py
│   ├── convert.py
│   ├── ctm.py
│   ├── cli.py
│   ├── utils/
│   └── py.typed
└── src/lib.rs       # native module exposed as pdfplumber._native
```

Reasons:

- Python properties, callbacks, context managers, compiled regular expressions, file-like objects, class relationships, and submodules are easier to reproduce in a thin Python layer.
- The Rust core can retain idiomatic zero-based indexes and Rust-native extension types while the shim presents exact Python semantics.
- A native module named only `pdfplumber` cannot cleanly provide the full package/submodule surface.
- Compatibility adapters can preserve the existing Rust API rather than forcing a SemVer-breaking rewrite.

---

## 8. Master Implementation Checklist

### 8.1 P0 — Reproducible Compatibility Harness

- [x] **PARITY-001** Pin Python `pdfplumber==0.11.10` and its complete dependency set in a lock file used by golden generation and CI.
- [ ] **PARITY-002** Record the upstream tag, commit, dependency lock hash, fixture hashes, generation command, operating system, architecture, and Python version in every golden artifact.
  - All 98 committed artifacts now carry a provenance block generated by Python `pdfplumber` v0.11.10 on the pinned Python 3.13 interpreter. A corpus-wide contract test verifies every required field and binds each artifact to its fixture hash.
  - Unchecked because the full section 10 all-target/all-feature tier remains red on pre-existing `CI-001`/`CI-002`/`CI-003` gaps. The focused, package, compatibility, and current CI-equivalent workspace tiers pass; see Active Work and the Evidence Ledger.
- [x] **PARITY-003** Replace the unpinned `scripts/setup_golden_venv.sh` behavior with a deterministic environment.
- [ ] **PARITY-004** Add isolated reference and candidate environments so the test runner cannot accidentally import the wrong `pdfplumber`.
  - Reference environment is built, verified, and enforced in `scripts/generate_golden.py` and `scripts/parity_report.py`.
  - `scripts/setup_candidate_venv.py` now requires the pinned Python 3.13 series, builds with exactly Maturin 1.14.1 or installs one explicit prebuilt local wheel with `--no-deps`, replaces only the configured repository-local `.venv-candidate`, and verifies that both `pdfplumber/__init__.py` and the compiled `pdfplumber._native` resolve beneath that environment. CI repeats the installed-wheel path.
  - The first installed candidate run reached the real behavioral gates instead of a missing-environment error: call, error, and serialization contracts each exited 1 with exact diffs, while the 161-case option runner retained all 161 cases as explicit errors. Representative gaps are missing `pdfplumber.utils`/`table`/`ctm`, `pathlib.Path` and stream rejection, and the unsupported `pages=` keyword; none was reclassified or omitted.
  - Unchecked because those exposed candidate behaviors remain incompatible and the full section 10 tier remains red on `CI-001`/`CI-002`/`CI-003`. The focused guard/setup contracts, isolated wheel install, origin verification, and installed native-layout tests pass; no dependency, case, failure, threshold, tolerance, or assertion was skipped or weakened.
- [ ] **PARITY-005** Snapshot the complete public module/export tree, including `__all__`, public classes, constants, functions, properties, descriptors, and call signatures.
  - The committed v0.11.10 snapshot recursively records all 20 importable modules, 452 exports, 27 canonically defined classes, and 401 class members. CI rebuilds it with the pinned reference interpreter and rejects byte-level drift.
  - Unchecked because the full section 10 all-target/all-feature tier remains red on pre-existing `CI-001`/`CI-002`/`CI-003` gaps. The focused, compatibility-harness, cross-validation, and current CI-equivalent workspace tiers pass; see Active Work and the Evidence Ledger.
- [ ] **PARITY-006** Add API-contract tests for positional arguments, keyword arguments, keyword-only arguments, defaults, invalid arguments, and exception types.
  - A deterministic 12-case contract now exercises all six categories against pinned upstream v0.11.10, including processed `TableSettings` defaults, Python binding failures, `Page.extract_text(**kwargs)` keyword-only behavior, and runtime exception classes. CI rejects reference drift, and candidate mode compares only behavioral outcomes.
  - Unchecked because no isolated candidate package exists before `PYAPI-002`, and the full section 10 all-target/all-feature tier remains red on pre-existing `CI-001`/`CI-002`/`CI-003` gaps. The reference, focused, compatibility-harness, cross-validation, and current CI-equivalent workspace tiers pass.
- [ ] **PARITY-007** Compare every page of every fixture; remove first-page-only reporting.
  - The parity report now extracts all Python pages, consumes all-page Rust CLI output, groups objects by 1-indexed page, records corpus-relative fixture IDs, and fails on processing or page-count errors. A full run retained 80 distinct fixture entries and compared 288 pages across 79 documents with no page-count mismatch; duplicate `pdffill-demo.pdf` basenames remained separate.
  - Unchecked because `password-example.pdf` still cannot be compared without credential-aware fixture handling (`PARITY-012`), and the full section 10 all-target/all-feature tier remains red on pre-existing `CI-001`/`CI-002`/`CI-003` gaps. The focused, full-corpus accounting, compatibility-harness, cross-validation, and current CI-equivalent workspace tiers otherwise pass; no failure was skipped, tolerance widened, or threshold lowered.
- [ ] **PARITY-008** Compare ordered object sequences rather than only greedy match percentages.
  - The parity report now compares character text, character boxes, and words at matching indexes and reports matched counts, total sequence length, ratios, and exact-order flags. Rust cross-validation uses the same positional rule for characters, words, lines, and rectangles; additions on either side remain in the denominator while the existing identity predicates and coordinate tolerances are unchanged.
  - A full pinned-upstream run compared 288 pages across 79 of 80 fixture entries with no page-count mismatch. It exposed real differences instead of hiding them: character-box sequences were exactly equal on 222 pages and word sequences on 243 pages; `issue-1279-example.pdf`, `issue-848.pdf`, and `pr-136-example.pdf` contain word-order differences. The password fixture remains an explicit processing failure.
  - Unchecked because the newly visible text gaps remain for `TEXT-WORD-020`/`TEXT-BUG-221`, password handling remains for `PARITY-012`, exact semantic gates remain for `PARITY-026`, and the section 10 all-target/all-feature tier remains red on pre-existing `CI-001`/`CI-002`/`CI-003`. No tolerance or threshold changed, no test was skipped, and no fixture-specific branch was added.
- [ ] **PARITY-009** Compare exact dictionary key sets, key spelling, value types, nested structures, and `None` placement.
  - The parity report now retains complete pinned-upstream character and word dictionaries and compares recursive structural signatures at matching object indexes. Signatures preserve exact key spelling, runtime scalar types, dictionary/list/tuple nesting, and positional `None`; dictionary insertion order and scalar values are intentionally outside the structural signature because dedicated exact/tolerance-aware comparisons cover values.
  - A full run compared 288 pages across 79 of 80 fixture entries. Only four empty-object pages had equal character or word dictionary sequences: 0/442,036 non-empty character objects and 0/75,480 non-empty word objects matched structurally. Table nesting, value types, and `None` placement matched on 288/288 pages. `basic_text.pdf` showed the representative gap: upstream characters expose 22 keys and Rust JSON 11, while upstream words expose 10 and Rust JSON 8.
  - Unchecked because the report correctly exposes the open common/character/word schema tasks (`OBJ-001`–`OBJ-008`, `OBJ-CHAR-004`–`OBJ-CHAR-010`, and `OBJ-WORD-001`), password handling remains for `PARITY-012`, exact semantic gating remains for `PARITY-026`, and section 10 remains red on pre-existing `CI-001`/`CI-002`/`CI-003`. No key was normalized away, `None` was not treated as an empty string, no tolerance or threshold changed, and no fixture-specific branch was added.
- [ ] **PARITY-010** Compare page text, layout text, simple text, text lines, words, search results, tables, annotations, hyperlinks, and structure trees.
  - The pinned differential report now invokes all ten APIs on every page, retains full upstream values until comparison, and emits explicit per-page exact and structural results. A hidden Rust compatibility transport serializes native text-line and structure-tree values without changing the public CLI help or projecting them into upstream-shaped dictionaries. The fixed search workload is the general non-whitespace regular expression `\S+`.
  - A full run retained all 80 corpus entries and compared 271 pages across 76 documents. Exact page counts were: page text 265, layout text 4, text lines 4, words 4, search 4, tables 269, annotations 236, hyperlinks 246, and structure trees 188. Rust has no distinct simple-text API, so all 271 simple-text results are explicit `TEXT-EXTRA-001` unsupported failures and make the report exit nonzero.
  - Four reference entries remain explicit processing failures: both corpus copies of `annotations-unicode-issues.pdf` fail while decoding annotation text, `federal-register-2020-17221.pdf` exceeds Python's recursion depth while traversing its structure tree, and `password-example.pdf` requires credential-aware handling. The task stays unchecked for those failures, the exact API gaps tracked by the text/word/search/annotation/structure tasks, `PARITY-012`/`PARITY-026`, and the pre-existing section 10 failures. No API, fixture, page, dictionary field, or mismatch was skipped or normalized into success.
- [ ] **PARITY-011** Add an option-matrix runner that generates reference output for non-default values of every documented text and table option.
  - A declarative pinned-upstream catalog now runs 161 non-default cases: 111 API-scoped text contracts and 50 table contracts. Shared keywords are exercised independently through `extract_words`, `extract_text`, `extract_text_simple`, `extract_text_lines`, `search`, and utility text extraction; the table side covers every non-default strategy, every `TableSettings` tolerance/threshold/input keyword, and every compatible `text_*` forwarding key. Dynamic bounding-box and explicit-line inputs are derived from page geometry rather than filename or expected output.
  - `.venv-reference/bin/python scripts/generate_option_matrix.py` records complete normalized values, exact container types, warnings/logs, resolved arguments and options, fixture/page identity and SHA-256, the dependency-lock digest, CPython version, and the exact v0.11.10 target. `--check` is byte-deterministic, every case completed with `status=ok`, and compatibility CI rejects a missing or stale snapshot.
  - Unchecked because the full section 10 all-target/all-feature tier remains red on pre-existing `CI-001`/`CI-002`/`CI-003` gaps. The focused, structural compatibility, pinned-reference, cross-validation, and current CI-equivalent workspace tiers pass. No option, result, warning, fixture, page, threshold, or tolerance was omitted or weakened, and no fixture-specific implementation branch was added.
- [ ] **PARITY-012** Add differential tests for exceptions, warnings, malformed metadata, passwords, repair, closed resources, and invalid bounding boxes.
  - A deterministic 23-case contract now covers all seven requested behavior families. It records exact exception classes, messages, arguments, failure phases, warning categories/messages, logger records, return values, and owned/external stream state. Password values are redacted, fixture and inline-resource hashes are recorded, and repair-wrapper behavior is isolated with a content-addressed temporary subprocess rather than an ambient Ghostscript installation.
  - The pinned reference command rejects byte-level drift in CI. Candidate mode compares the complete ordered case outcomes and refuses to run if it imports the upstream reference package, but it cannot yet execute against this project because `PYAPI-002` has not shipped an isolated importable compatibility package.
  - Unchecked because candidate behavior has not been proved and the full section 10 all-target/all-feature tier remains red on pre-existing `CI-001`/`CI-002`/`CI-003` gaps. The focused, compatibility-harness, pinned-reference, cross-validation, and current CI-equivalent workspace tiers pass. No exception text, phase, warning, resource state, case, threshold, tolerance, or fixture was omitted or weakened.
- [ ] **PARITY-013** Add exact JSON and CSV differential tests, including precision and include/exclude attribute behavior.
  - A deterministic 28-case contract now retains complete raw v0.11.10 output bytes for 15 JSON and 13 CSV behaviors across both `PDF` and `Page`. It covers default output, `StringIO` targets and `None` returns, precision 0/3, JSON indentation, include/exclude filters, character-only and empty object selections, and exact validation failures.
  - Two materially different pinned fixtures exercise mixed character/rectangle/image serialization and text-only serialization. The 758,882-byte artifact records both fixture hashes, the target/lock/interpreter/generation provenance, every invocation, exact exception details, and SHA-256 for every returned or streamed string. CI rejects byte-level reference drift; candidate mode compares all ordered outcomes without reparsing JSON or CSV.
  - Unchecked because `PYAPI-002` has not supplied an isolated candidate package and the full section 10 all-target/all-feature tier remains red on pre-existing `CI-001`/`CI-002`/`CI-003` gaps. Focused, compatibility-harness, pinned-reference, inherited differential, and current CI-equivalent tiers pass. No output byte, column, row, key order, precision value, filter result, exception, case, threshold, tolerance, or fixture was omitted or weakened.
- [ ] **PARITY-014** Run the upstream Python test suite against the compatibility package; maintain a machine-readable list of temporarily unsupported tests.
  - A reproducible runner now verifies the exact v0.11.10 commit, `tests` Git tree, 101 test/fixture files plus the one PDF referenced from upstream `examples/`, and a hash-locked 16-package pytest/xdist/pandas environment. It preflights the installed candidate origin in an isolated environment, rechecks that origin inside pytest workers, requires manifest-declared external commands, records every collected and failed node ID, and keeps pytest's nonzero status even when a failure is listed as temporarily unsupported.
  - The pinned reference baseline collected 171 tests. After the external PDF dependency was added to the verified bundle, 165 passed and six repair tests failed solely because this machine has no `gs` executable; the machine result recorded all 171 node IDs and exactly those six failures. `upstream-unsupported.toml` remains empty because no candidate result has been observed, rather than inventing or copying reference-environment failures into the compatibility-gap list.
  - Unchecked because `PYAPI-002` has not supplied an isolated importable candidate package, Ghostscript is absent locally, and section 10 remains red on pre-existing `CI-001`/`CI-002`/`CI-003`. The runner explicitly rejects both a missing candidate and the pinned reference package. No test was skipped, deselected, marked xfail, converted to success, or hidden by a threshold or tolerance.
- [ ] **PARITY-015** Require every temporary unsupported upstream test to reference an unchecked task in this document.
  - Upstream-suite preflight now parses only the master checklist between PRD sections 8 and 9, builds exact checked/unchecked state for task identifiers, and rejects every unsupported-test entry whose `task_id` is missing or already checked.
  - A synthetic contract proves that an unchecked `PYAPI-002` link passes while checked `PARITY-001` and unknown `UNKNOWN-999` links fail with the offending upstream node ID and task ID. The current empty manifest passes the same preflight and then reaches the independently expected missing-candidate failure.
  - Unchecked because section 10 remains red on pre-existing `CI-001`/`CI-002`/`CI-003`. No unsupported entry was invented, no task state was inferred outside section 8, and no pytest outcome, test selection, threshold, or tolerance changed.
- [ ] **PARITY-016** Create `compat/approved_deltas.toml` and fail CI on unregistered output differences.
  - `compat/approved_deltas.toml` now binds to v0.11.10 and its exact commit and requires one non-wildcard fixture/page/API entry per approval, with stable ID, reviewable upstream/Rust result summaries, type-preserving result SHA-256 values, technical reason, compatibility risk, approving maintainer, regression test, and review/expiration condition. The committed registry is empty because no current difference has maintainer approval.
  - The parity report now identifies every difference it already observes by exact upstream/Rust digests. Only an exact registry match avoids a delta-gate failure; changed results are simultaneously unregistered and make the old entry stale, while resolved/out-of-date entries are stale. Processing failures, missing pages, and unsupported APIs remain independent failures and cannot be approved away. CI validates the registry target/schema and runs contracts proving unregistered/stale rejection.
  - The full 80-entry run retained 76 compared documents, 271 pages, four existing pinned-reference processing failures, and zero digest failures. It reported 1,486 unregistered differences, zero approved, zero stale, and exit 1. Unchecked because the all-page report is not yet a CI execution gate (`PARITY-023`/`PARITY-026`), machine/human report work remains in `PARITY-017`/`PARITY-018`, and section 10 is red on `CI-001`/`CI-002`/`CI-003`. No difference was registered without approval, normalized, thresholded, or omitted.
- [ ] **PARITY-017** Generate a machine-readable parity report with per-API, per-option, per-fixture, and per-page results.
  - `scripts/parity_report.py --json` now writes deterministic schema-v1 JSON with the exact pinned target and reference environment, explicit reference/candidate environment states, sorted fixture records, every page, all 11 API outcomes per compared page, exact approved/unregistered delta dispositions, and every option-matrix case. The JSON has no timestamp, preserves processing/unsupported/different outcomes, and resolves option outputs only after their content SHA-256 is verified.
  - An isolated candidate can generate option results with `.venv-candidate/bin/python scripts/generate_option_matrix.py --candidate-output ...`; the report requires exactly the same 161 option IDs and fixture/page/API/options/arguments identities and compares complete results, warnings, logs, and errors. Missing, extra, identity-mutated, or content-address-mismatched cases fail. The pinned reference interpreter is rejected as a candidate rather than producing false parity.
  - The full report is 10,897,773 bytes (SHA-256 `8b097e605a0e0d73865f761f224d6d3604d28bb8263171a04331be8f25387a86`) and contains 80 fixture records, 76 compared documents, four explicit processing failures, 271 pages, 2,981 API records (1,224 equal, 1,486 different, 271 unsupported), 1,486 attached unregistered dispositions, and 161 blocked option records. Unchecked because `PYAPI-002` has not supplied candidate option results, the observed compatibility failures remain real, and section 10 is red on `CI-001`/`CI-002`/`CI-003`; no record, result, failure, page, API, option, threshold, or tolerance was omitted.
- [ ] **PARITY-018** Generate a human-readable summary artifact that shows the first differing object and a compact coordinate/text diff.
  - `scripts/parity_report.py --summary <path>` now renders deterministic Markdown from the same schema-v1 model used by `--json`. It selects the first differing fixture/page/API in stable corpus and API order, retains the exact sequence index and JSON-safe upstream/Rust objects, bounds text context to the first character difference, and reports signed common-coordinate deltas or explicitly states that the common coordinates are unchanged.
  - The full pinned run retained 80 fixtures, 76 compared documents, four explicit reference failures, 271 pages, all 2,981 API outcomes, all 161 blocked option cases, and all 1,486 unregistered differences. Every different API has a first-difference record. The 57,877,531-byte machine artifact has SHA-256 `27c16cd3146aab98ad6740a863c4b0e84482c39130f19a3c91631eb135e66462`; its 1,323-byte summary has SHA-256 `0b35ae799b42d2314279ad7f5368d2850b4c5a9e227d191c75499913a53bea08` and identifies the first character object in `150109DSP-Milw-505-90D.pdf` without hiding the remaining results.
  - Unchecked because the generated artifacts are not yet a CI execution/upload gate (`PARITY-023`/`PARITY-026`), the real corpus remains red, and section 10 still fails on `CI-001`/`CI-002`/`CI-003`. No object, text, coordinate, failure, fixture, page, API, option, threshold, or tolerance was omitted or weakened.
- [ ] **PARITY-019** Add a PRD linter that rejects duplicate task identifiers and checked tasks without an Evidence Ledger row.
  - `scripts/check_prd.py` now parses canonical task definitions only between sections 8 and 9 and Evidence Ledger rows only between sections 13 and 14. It rejects missing, repeated, or out-of-order contract sections, duplicate task definitions with every source line, and checked tasks without section-13 evidence; fenced Markdown examples and references elsewhere do not count.
  - The committed document contains 721 unique tasks, two checked tasks, and 19 Evidence Ledger rows. Synthetic contracts prove both required rejection paths and the section/fence boundaries, while compatibility CI runs the linter as a standalone structural gate before building the pinned reference environment.
  - Unchecked because the full section 10 tier remains red on the existing `CI-001`/`CI-002`/`CI-003` failures. No task state, evidence row, test, threshold, tolerance, fixture, or ignore was weakened.
- [ ] **PARITY-020** Add fixture provenance and license metadata; do not commit private or redistribution-restricted PDFs.
- [ ] **PARITY-021** Import the upstream v0.11.10 PDF fixture corpus and preserve its directory names for traceability.
  - The committed import contains all 81 `.pdf` paths from the pinned `tests/pdfs` tree under `compat/fixtures/upstream/pdfplumber-v0.11.10/tests/pdfs`, including the zero-byte `empty.pdf` error fixture and the original `from-oss-fuzz/load` hierarchy. An offline CI gate binds path names and bytes to source commit `7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62` with corpus SHA-256 `74b8ab0bc2dc561010b48a5488bca3aeaf1ab5b495267f8b34795b3a41f5a175`.
  - Unchecked because the full section 10 tier remains red on the existing `CI-001`/`CI-002`/`CI-003` failures. Focused corpus/provenance gates, the pinned compatibility harness, cross-validation, and the current CI-equivalent excluded workspace tier pass; no path was flattened, fixture omitted, threshold lowered, tolerance widened, or test skipped.
- [ ] **PARITY-022** Add the Rust repository's issue fixtures and external parser fixtures to one indexed corpus manifest.
  - `compat/fixture-provenance.toml` schema 2 is the single index for all 223 committed PDFs. Each path has exactly one immutable source and one primary collection: 81 exact `upstream-v0.11.10` paths, 88 `rust-regression` issue/regression fixtures, 28 licensed `external-parser` fixtures from PDF.js/PDFBox/Poppler, and 26 `project-generated` fixtures.
  - `scripts/check_corpus_index.py` and its CI step reject duplicate or unsafe paths, duplicate/unknown collections, unknown sources, missing classifications, digest drift, and files missing from either the index or repository. Queries return collections and paths in deterministic order.
  - Unchecked because the full section 10 tier remains red on the existing `CI-001`/`CI-002`/`CI-003` failures. Focused index/license gates, ambient and pinned compatibility harnesses, cross-validation, and the current CI-equivalent excluded workspace tier pass; no fixture, classification, assertion, threshold, tolerance, or test was omitted or weakened.
- [ ] **PARITY-023** Separate compatibility gates from performance benchmarks so faster output cannot hide semantic differences.
- [ ] **PARITY-024** Convert stale passing ignored tests to normal tests and close or update issue `#217`.
- [ ] **PARITY-025** Prohibit percentage-threshold reductions without maintainer approval and an Evidence Ledger entry.
- [ ] **PARITY-026** Replace the current `95%` release interpretation with exact-result gates plus explicit approved deltas.
- [ ] **PARITY-027** Tighten coordinate comparison from `1.0` point to `0.05` point for compatibility gates.
- [ ] **PARITY-028** Add deterministic re-run checks: the same input and options must produce byte-for-byte identical normalized results.
- [ ] **PARITY-029** Add compatibility tests for examples copied from the upstream README and notebooks.
- [ ] **PARITY-030** Add a target-upgrade procedure for future Python `pdfplumber` releases.

### 8.2 P0 — Python Packaging and Import Architecture

- [ ] **PYAPI-001** Rename or wrap the native extension as `pdfplumber._native` instead of using the native module as the whole package.
- [ ] **PYAPI-002** Ship a pure-Python `pdfplumber` compatibility package in the wheel and source distribution.
- [ ] **PYAPI-003** Implement top-level `pdfplumber.open = PDF.open`.
- [ ] **PYAPI-004** Match top-level `__all__`, `__version__`, `utils`, `pdfminer` compatibility policy, `open`, `repair`, and `set_debug`.
- [ ] **PYAPI-005** Provide import-compatible modules: `pdf`, `page`, `container`, `table`, `display`, `structure`, `repair`, `convert`, `ctm`, `cli`, `_typing`, and `utils`.
- [ ] **PYAPI-006** Match public class names, module names, `repr`, inheritance, and `isinstance` behavior.
- [ ] **PYAPI-007** Add `py.typed` and complete type stubs generated or verified against the runtime API.
- [ ] **PYAPI-008** Ensure static methods, class methods, properties, cached properties, and ordinary methods have matching descriptor behavior.
- [ ] **PYAPI-009** Support Python callbacks for `Page.filter(...)` without holding the Global Interpreter Lock (GIL) during unrelated native work.
- [ ] **PYAPI-010** Support compiled Python regular-expression objects wherever upstream accepts them.
- [ ] **PYAPI-011** Define a stable boundary for converting native objects to Python dictionaries without leaking mutable native state.
- [ ] **PYAPI-012** Ensure mutable returned dictionaries/lists behave like Python-owned values and cannot corrupt native caches.
- [ ] **PYAPI-013** Test editable installs, wheel installs, source distribution builds, and isolated virtual-environment imports.
- [ ] **PYAPI-014** Test installation when Python `pdfplumber` is already installed and provide a clear conflict policy.
- [ ] **PYAPI-015** Test supported CPython versions 3.10, 3.11, 3.12, 3.13, and 3.14.
- [ ] **PYAPI-016** Either test and support PyPy or remove unsupported PyPy classifiers and claims.
- [ ] **PYAPI-017** Test Linux x86-64 and AArch64, macOS x86-64 and Apple Silicon, and Windows x86-64 wheels.
- [ ] **PYAPI-018** Ensure the source distribution can build with the declared Minimum Supported Rust Version.
- [ ] **PYAPI-019** Keep package version, crate versions, native `__version__`, type stubs, and release tag synchronized.
- [ ] **PYAPI-020** Do not depend on the Python `pdfplumber` distribution at runtime.
- [ ] **PYAPI-021** Make any optional `pdfminer.six`, Pillow, or `pypdfium2` compatibility dependency explicit and feature-scoped.
- [ ] **PYAPI-022** Release the GIL around parsing, decompression, text extraction, table detection, rendering, and serialization where Python objects are not accessed.
- [ ] **PYAPI-023** Add Python thread-safety tests for concurrent reads from separate documents and separate pages.
- [ ] **PYAPI-024** Add subprocess tests proving crashes and Rust panics cannot terminate the Python interpreter.

### 8.3 P0 — `pdfplumber.open` and `PDF` Lifecycle

- [ ] **PDF-001** Accept `str` filesystem paths.
- [ ] **PDF-002** Accept `pathlib.Path`.
  - `PDF.open` now extracts its filesystem input as `PathBuf`, using Python's filesystem-path protocol for both `str` and `pathlib.Path` while preserving the top-level `open = PDF.open` alias. An installed CPython 3.13 wheel opens the pinned fixture through a real `PosixPath`, returns `PDF`, and exposes its pages.
  - The candidate call contract no longer contains the prior `PosixPath`-to-`PyString` conversion error. This task stays unchecked because the complete section 10 tier remains red on `CI-001`/`CI-002`/`CI-003`; stream inputs, lifecycle, options, modules, thresholds, tolerances, and unrelated APIs were not changed.
- [ ] **PDF-003** Accept an already-open binary file object.
  - `PDF.open` now distinguishes filesystem paths from file-like inputs, seeks caller-owned binary files to byte zero, reads the complete payload, and leaves the file open at EOF just like pinned upstream v0.11.10. A fresh installed CPython 3.13 wheel opens the real fixture and returns populated pages.
  - The task stays unchecked because the complete section 10 tier remains red on `CI-001`/`CI-002`/`CI-003`; close behavior, retained stream attributes, lifecycle, options, thresholds, tolerances, and unrelated APIs were not changed.
- [ ] **PDF-004** Accept `io.BytesIO` and equivalent seekable binary streams.
  - The same installed-artifact contract starts `BytesIO` and `BufferedReader` inputs at a nonzero offset, verifies both are rewound for parsing, remain open, and finish at EOF. Candidate call/error/serialization contracts no longer contain the prior stream-as-path conversion errors and continue to fail on the remaining exact behavior differences.
  - The task stays unchecked for the strict section 10 blockers and the independently tracked ownership/lifecycle/API gaps; no stream failure, test, threshold, tolerance, or assertion was skipped or weakened.
- [ ] **PDF-005** Preserve external-stream ownership: closing `PDF` must not close a caller-owned stream.
  - `PDF` retains the exact caller object in `.stream`; `close()` and context exit leave external `BytesIO`/`BufferedReader` objects open and are idempotent. Installed-wheel identity and closed-state assertions match pinned upstream v0.11.10.
  - Unchecked because section 10 remains red on `CI-001`/`CI-002`/`CI-003`; no externally owned stream was copied into a fabricated public object or closed to simplify cleanup.
- [ ] **PDF-006** Close internally opened streams when `PDF.close()` is called.
  - Path input now opens and retains a real Python `BufferedReader`; `close()` closes it exactly once semantically and repeated calls return `None`. Artifact tests promote leaked-resource warnings to errors.
  - Unchecked for the strict section 10 blockers; stream cleanup, tests, thresholds, tolerances, and assertions were not skipped or weakened.
- [ ] **PDF-007** Implement `PDF.__enter__` and `PDF.__exit__`.
  - `__enter__` returns the identical `PDF` object and `__exit__` delegates to ownership-aware `close()`: owned path streams close, caller streams remain open, and exceptions are not suppressed.
  - Unchecked because the complete section 10 tier remains red; the call contract no longer reports a missing context-manager protocol and continues to expose unrelated API differences.
- [ ] **PDF-008** Match behavior when operations are attempted after close.
  - Cached document/page state remains readable after idempotent close, matching the pinned reference observation that page count and previously obtained page content survive resource cleanup.
  - Unchecked because exact behavior across every uncached downstream API and the strict section 10 tier are not yet green; no after-close error was invented to make the contract pass.
- [ ] **PDF-009** Expose compatible `.stream`, `.path`, `.password`, and ownership state where public behavior depends on them.
  - Path documents expose their `pathlib.Path`, owned `BufferedReader`, and `password=None`; external documents expose the identical caller stream with `path=None` and `password=None`. Ownership remains private state used only to implement compatible close behavior.
  - Unchecked for `CI-001`/`CI-002`/`CI-003` and future password support; absent passwords were not represented by an empty string or other sentinel.
- [ ] **PDF-010** Implement `pages=` selection using 1-based page numbers and preserve upstream ordering and validation.
  - `PDF.open(path_or_fp, pages=None)` now retains the supplied Python selection and applies Python membership while iterating the document. Input order does not reorder the document, duplicates collapse naturally, zero/negative/out-of-range and non-matching values select nothing, booleans retain Python integer equality, and non-iterable inputs fail lazily when `.pages` is read, matching pinned v0.11.10 observations.
  - An installed CPython 3.13 wheel selects pages 3 and 5 from `(5, 3, 5, 0, -1, 99)`, preserves their exact candidate text relative to the full document, returns `[]` for empty and string-only selections, selects page 1 for `[True]`, and raises `TypeError` for `pages=1`. The task stays unchecked for strict `CI-001`/`CI-002`/`CI-003` and remaining serialization APIs; no invalid entry was clamped, remapped, or rejected earlier than upstream.
- [ ] **PDF-011** Match page-number and `doctop` behavior when only selected pages are loaded.
  - Selected pages retain their original 1-based document numbers while the Rust `Page` keeps its idiomatic 0-based index. Character document-top coordinates are rebased to the selected view: the first selected page starts at zero and each later selected page accumulates only previously selected page heights; derived word coordinates use the same rebased characters.
  - Pinned v0.11.10 showed pages `(3, 5)` as numbers `[3, 5]` with first-character `doctop - top` offsets `[0, 841.89]`. Red: the preceding installed wheel returned `[2, 4]`; green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran eleven artifact tests, all OK, including exact page numbers and char/word offsets. The task stays unchecked for strict `CI-001`/`CI-002`/`CI-003`; `.initial_doctop` remains separately tracked by `PAGE-003`.
- [ ] **PDF-012** Implement `laparams=` acceptance and define the native-versus-fallback strategy.
  - `PDF.open(..., laparams=None)` now accepts and privately retains Python mappings for all seven pinned `LAParams` keys. It rejects non-mappings, unknown keys, and non-numeric/out-of-range `boxes_flow` values at open time with upstream-shaped `TypeError`/`ValueError`; immutable mapping proxies remain valid.
  - DEC-004 selects a native-only implementation so the candidate keeps `PYAPI-020`'s no-runtime-`pdfplumber`/`pdfminer.six` boundary. A fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran twelve artifact tests, all OK. Higher-level layout objects and public `LAParams` state remain explicit `PAGE-018` work, so this task stays unchecked for that gap and strict `CI-001`/`CI-002`/`CI-003`.
- [ ] **PDF-013** Implement `password=` for paths and streams.
  - `PDF.open(..., password=None)` now routes supplied Python strings through the core password-aware reader for both paths and caller-owned streams, retains the exact string in `.password`, rewinds inputs, and preserves external stream ownership on success and failure.
  - The real pinned `password-example.pdf` opens with `password="test"`, exposes four populated pages from both a path and `BytesIO`, and closes only its owned path stream. A fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran thirteen artifact tests, all OK. Missing/wrong candidate exception classes remain explicit `PDF-014` work, so this task stays unchecked for that gap and strict `CI-001`/`CI-002`/`CI-003`.
- [ ] **PDF-014** Distinguish password-required, invalid-password, malformed-PDF, I/O, and unsupported-encryption failures compatibly.
  - Path-opening I/O remains the corresponding built-in `OSError` subtype, while malformed data, closed caller streams, password authentication failures, and unsupported encryption use `pdfplumber.utils.exceptions.PdfminerException` as pinned upstream does. Missing, empty, and wrong passwords retain upstream's empty public message; empty and non-PDF streams retain its `No /Root object!` message.
  - The parser now matches lopdf's structured incorrect-password variants instead of searching rendered error strings, so unsupported encryption remains a parse failure with a non-empty diagnostic. A fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran fourteen artifact tests with `ResourceWarning` promoted to error, all OK. The task stays unchecked for the shared utility/module surface and strict `CI-001`/`CI-002`/`CI-003` gates.
- [ ] **PDF-015** Implement `strict_metadata=`.
  - `PDF.open(..., strict_metadata=False)` accepts valid and cyclic-information inputs without changing ownership; `True` validates the raw `/Info` object graph and raises the pinned `RecursionError("maximum recursion depth exceeded")` for an indirect-reference cycle while leaving caller-owned streams open.
  - The validator follows arbitrary nested dictionaries, arrays, streams, and indirect references using an active-reference set rather than recognizing the test fixture. Red: the preceding wheel rejected the keyword. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran fifteen artifact tests with `ResourceWarning` promoted to error, all OK. Exact permissive metadata keys/values and warning logs remain `PDF-021`, so this task stays unchecked for that work and strict `CI-001`/`CI-002`/`CI-003`.
- [ ] **PDF-016** Implement `unicode_norm=` with `NFC`, `NFD`, `NFKC`, and `NFKD`.
  - `PDF.open(..., unicode_norm=None)` preserves the original code points and exposes `None`; the four supported strings are retained on `.unicode_norm` and apply canonical or compatibility normalization to each extracted character without changing its geometry.
  - Pinned v0.11.10 and the exact candidate wheel agree on composed accents from `basic_text.pdf` and the `ﬁ` ligature from `line-char-render-example.pdf`. Invalid strings and non-string values remain accepted by `open` and page enumeration, then raise the exact upstream `ValueError`/`TypeError` messages when page content is first accessed. The task stays unchecked for strict `CI-001`/`CI-002`/`CI-003`, with no form, value, fixture, test, threshold, tolerance, or assertion omitted or weakened.
- [ ] **PDF-017** Implement `repair=`.
  - `PDF.open(..., repair=True)` now follows the pinned Ghostscript-backed path rather than silently substituting the Rust extension repairer: path and external-stream inputs become a new internally owned `BytesIO`, `.path` is `None`, the original external stream remains open after being consumed from its current position, and closing the PDF closes only the repaired stream.
  - A deterministic temporary Ghostscript double rebuilds actual normal, offset, and encrypted fixtures for pinned v0.11.10 and the exact candidate wheel. Both preserve real page/text access and password handling; `repair=False` does not require Ghostscript, while `repair=True` without an executable raises the exact installation message. The task stays unchecked for `PDF-018`, `PDF-019`, the wider `REPAIR-001`–`REPAIR-011` surface, and strict `CI-001`/`CI-002`/`CI-003`; no executable lookup, input offset, credential, resource state, failure, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-018** Implement `gs_path=`.
  - `PDF.open(..., repair=True, gs_path=...)` accepts both `str` and `pathlib.Path`, bypasses executable discovery even when `PATH` is empty, and forwards the selected executable unchanged to the pinned Ghostscript subprocess contract. When `repair=False`, `gs_path` remains unevaluated, including arbitrary objects.
  - Pinned v0.11.10 and the exact candidate wheel used the same deterministic explicit executable and real PDF output; a missing `Path` raised the same platform-native `FileNotFoundError` as direct process execution, including errno, `.filename` representation, and public message. Red: the exact PDF-017 wheel ran eighteen artifact tests with three errors because `gs_path` was unexpected. Green: after replacing the growing binding signature with a general positional/keyword parser that preserves the exact public text signature and call-shape errors without a lint suppression, a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all eighteen artifact tests with `ResourceWarning` promoted to error. The task stays unchecked for `PDF-019`, the wider `REPAIR-001`–`REPAIR-011` surface, and strict `CI-001`/`CI-002`/`CI-003`; no path type, lookup state, failure, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-019** Implement `repair_setting=` values `default`, `prepress`, `printer`, `ebook`, and `screen`.
  - `PDF.open(..., repair=True, repair_setting=...)` now forwards `default`, `prepress`, `printer`, `ebook`, and `screen` as the exact Ghostscript arguments `-dPDFSETTINGS=/default`, `/prepress`, `/printer`, `/ebook`, and `/screen`; omitting the option retains `default`. When `repair=False`, arbitrary values remain unevaluated and no repair subprocess is invoked.
  - A pinned v0.11.10 subprocess probe and the exact candidate wheel used the same explicit executable and real output fixture and observed one exact preset argument per invocation. Red: the exact PDF-018 wheel ran nineteen artifact tests with six errors because `repair_setting` was unexpected across the five presets and the disabled-repair case. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all nineteen artifact tests with `ResourceWarning` promoted to error, and the exact public signature plus ninth positional argument were separately verified. The task stays unchecked for the wider `REPAIR-001`–`REPAIR-011` surface and strict `CI-001`/`CI-002`/`CI-003`; no preset, subprocess argument, disabled path, output, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-020** Implement `raise_unicode_errors=`.
  - `PDF.open(..., raise_unicode_errors=...)` now defaults to the identical Python `True` object and otherwise retains the exact supplied object without bool coercion, matching pinned v0.11.10 for `True`, `False`, `None`, integers, strings, and an opaque identity sentinel. The complete ten-argument open signature also accepts the value positionally.
  - The real `annotations-unicode-issues.pdf` establishes the remaining behavioral boundary: pinned mode raises `UnicodeDecodeError` while decoding `.annots` when the retained value is truthy, whereas a false value emits `UserWarning("Could not decode contents of annotation. contents will be missing.")` and preserves the raw bytes. Red: the exact PDF-019 wheel ran twenty artifact tests with nine errors—eight unexpected-keyword failures plus the missing default property. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all twenty artifact tests with `ResourceWarning` promoted to error and separately passed exact signature and tenth-positional-argument probes. The task stays unchecked because `Page.annots` is not yet exposed, leaving the decoding branch under `PAGE-017`, `OBJ-ANNOT-004`, `SEM-ANNOT-001`, and `SEM-ANNOT-005`, plus strict `CI-001`/`CI-002`/`CI-003`; no input type, identity, default, malformed fixture, failure, warning, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-021** Match metadata dictionary keys, decoded values, list-valued metadata, warnings, and strict failures.
- [ ] **PDF-022** Make `.pages` lazy/cached in a behaviorally compatible way.
  - The first `.pages` access creates lightweight page handles without interpreting page content, caches one mutable Python list, and returns that identical list and identical page instances on every subsequent access. Page selection, original 1-based numbers, geometry, and selected-view document-top rebasing remain unchanged.
  - Pinned v0.11.10 allows page enumeration and geometry access for every retained invalid `unicode_norm`, then raises the exact normalization error when objects or text materialize. It also retains the partial cached list when selection membership fails after a page is appended. Red: the exact PDF-021 wheel produced five enumeration errors, returned fresh page lists, and retried a failed selection; green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all twenty-four artifact tests with `ResourceWarning` promoted to error. The task stays unchecked for close/flush cleanup, wider repeated-property identity, and strict `CI-001`/`CI-002`/`CI-003`; no cache mutation, identity, selection, page, geometry, error phase, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-023** Implement `PDF.close()` and page cache cleanup.
  - Each `close()` invalidates the cached mutable `.pages` list before visiting the current pages, so subsequent access returns a new list with new page handles and discards caller mutations. Previously returned pages remain usable when their content was already materialized, and caller-owned streams remain open.
  - Pinned v0.11.10 materializes pages before closing an owned stream: an invalid scalar `pages=1` therefore raises the exact `TypeError` and leaves the stream open. Red: the exact PDF-022 wheel retained the original page cache and closed the scalar-selection stream without raising. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all twenty-five artifact tests with `ResourceWarning` promoted to error; the binding's fifty unit tests, 103 compatibility-harness tests, and binding Clippy also passed. The task stays unchecked for container/page `flush_cache` and page-close behavior under `PDF-024`, `PAGE-019`, `PAGE-020`, plus strict `CI-001`/`CI-002`/`CI-003`; no identity, mutation, failure phase, stream state, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-024** Implement `PDF.flush_cache()` behavior inherited from the container surface.
  - `flush_cache()` and `flush_cache(None)` invalidate the mutable page-list cache without closing the stream or invalidating previously returned page content. Passing `["_pages"]` does the same, while an empty list, unknown property list, and the iterable string `"_pages"` preserve the current cache.
  - Pinned v0.11.10 applies property names in order: a non-iterable raises exact `TypeError("'int' object is not iterable")`, and a non-string item raises exact `TypeError("attribute name must be string, not 'int'")` after retaining any earlier deletion. Red: the exact PDF-023 wheel exposed no `flush_cache` method. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all twenty-six artifact tests with `ResourceWarning` promoted to error; the binding's fifty tests, 103 compatibility-harness tests, binding Clippy, and updated native type-stub contract passed. The task stays unchecked for unimplemented document object/edge caches, page caches, the general container surface, and strict `CI-001`/`CI-002`/`CI-003` under `PDF-025`, `PAGE-020`, `SER-001`, and `PDF-032`; no property form, ordering, partial effect, identity, stream state, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-025** Implement `.objects` aggregated by object type.
  - `.objects` groups only present `char`, `line`, `rect`, `curve`, and `image` lists from the selected pages in document order, caches one mutable dictionary and its mutable lists, and returns those identical containers on repeated access. Real fixtures establish exact key order and counts: `basic_text.pdf` has 258 chars; `table-curves-example.pdf` has 1,992 chars, 208 rects, and 33 curves; `inline-image.pdf` has 22 chars and one image; and `empty-page.pdf` has no object keys.
  - `_objects` and `_pages` invalidate independently: selected pages 3 and 5 of `long_document.pdf` aggregate exactly 1,386 chars and two lines; flushing only pages preserves the caller-mutated object cache, flushing only objects preserves page identity while rebuilding the aggregate, and the default clears both. Red: the exact PDF-024 wheel raised five `AttributeError`s across the two focused tests because `.objects` was absent. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all twenty-eight artifact tests with `ResourceWarning` promoted to error; the binding's fifty tests, 103 compatibility-harness tests, binding Clippy with warnings denied, and the packaged native type-stub contract passed. The task stays unchecked because page-object identity, broader page properties, complete object schemas, repeated-property identity, serialization, and strict section 10 remain under `PAGE-016`, `PAGE-018`, `OBJ-001`–`OBJ-008`, `PDF-032`, and `CI-001`–`CI-003`; no object type, order, count, mutation, cache interaction, fixture, failure, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-026** Implement `.annots` aggregated across pages.
  - `PDF.annots` now returns a fresh list on every access by flattening fresh `Page.annots` lists across the selected pages in document order. The real three-page `issue-463-example.pdf` matches pinned v0.11.10's exact page counts `[2, 4, 0]`, aggregate count six, page-number sequence `[1, 1, 2, 2, 2, 2]`, `object_type="annot"`, and ordered fifteen-key public dictionary shape; `issue-598-example.pdf` retains its exact `http://www.ck12.org` URI.
  - Selecting only original page 2 retains page number 2, exactly four annotations, and selected-view `doctop == top`. Red: the exact PDF-025 wheel raised two `AttributeError`s because `Page.annots` was absent. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran both focused tests and all thirty artifact tests with `ResourceWarning` promoted to error; the binding's fifty tests, 103 compatibility-harness tests, binding Clippy with warnings denied, and packaged native type-stub contract passed. The task stays unchecked because raw `data`, malformed text decoding/warnings, rotation, and complete annotation values remain incompatible under `PAGE-017`, `OBJ-ANNOT-001`–`OBJ-ANNOT-004`, `SEM-ANNOT-001`, `SEM-ANNOT-004`, `SEM-ANNOT-005`, and strict `CI-001`–`CI-003`; no page, annotation, order, key, identity, selection, coordinate, failure, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-027** Implement `.hyperlinks` aggregated across pages.
  - `Page.hyperlinks` returns fresh annotation dictionaries only for Link annotations whose action type is explicitly `/URI`; `PDF.hyperlinks` returns another fresh list and fresh dictionaries by flattening selected pages in document order. `issue-982-example.pdf` matches pinned v0.11.10's exact page counts `[0, 0, 0, 0, 34, 0, 0, 0]`, including 34 page-5 links after selecting pages 2 and 5. `pdffill-demo.pdf` matches `[1, 1, 1, 4, 2, 7, 1]` and the exact seventeen-link page-number sequence, while `issue-463-example.pdf` returns a fresh empty list.
  - The existing Rust `Page.hyperlinks()` keeps all resolved `/URI`, `/GoTo`, `/GoToR`, and `/Dest` targets, while an additive action-aware backend view prevents URI-shaped named destinations such as `glo:CNN` from leaking into the Python compatibility surface. Red: the exact PDF-026 wheel raised two `AttributeError`s because `Page.hyperlinks` was absent. The first candidate exposed internal destinations, and a scheme-shape filter still produced `[0, 5, 10, 6, 34, 0, 0, 0]`; the final action-type implementation passed all thirty-two artifact tests with `ResourceWarning` promoted to error, the binding's fifty tests, 103 compatibility-harness tests, focused core URI/GoTo regressions, Clippy with warnings denied, formatting, and the packaged type-stub contract. The task stays unchecked for complete annotation `data`, rotation, decoding/warnings, and strict `CI-001`–`CI-003` under `PAGE-017`, `OBJ-ANNOT-001`–`OBJ-ANNOT-004`, and `SEM-ANNOT-002`–`SEM-ANNOT-005`; no action type, page, link, order, key, identity, selection, coordinate, failure, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-028** Implement `.structure_tree`.
  - `PDF.structure_tree` and `Page.structure_tree` return fresh compact dictionaries from one cached native hierarchy. The real `image_structure.pdf` exactly matches pinned v0.11.10 at both scopes: a `Document` root contains two `P` elements with MCIDs 0 and 1 plus a `Figure` with the exact two-paragraph alt text and MCID 2; document elements include 1-based `page_number=1`, while page elements omit page numbers. Untagged `basic_text.pdf` returns a fresh empty list at both scopes.
  - Red: the exact PDF-027 wheel raised two `AttributeError`s because `PDF.structure_tree` was absent. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran the two focused tests and all thirty-four artifact tests with `ResourceWarning` promoted to error; the binding's fifty tests, 103 compatibility-harness tests, eight parser structure-tree tests, fourteen real-fixture structure tests, Clippy with warnings denied, formatting, diff checks, and packaged type-stub contract passed. The task stays unchecked for revision, ID, title, attributes, RoleMap/ClassMap, ParentTree, MCR/OBJR, selected/multipage edge behavior, find helpers, and strict `CI-001`–`CI-003` under `SEM-STRUCT-003`–`SEM-STRUCT-015`; no node, field, page number, MCID, order, hierarchy, empty result, fixture, failure, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-029** Implement `.to_dict(object_types=None)`.
  - `PDF.to_dict` and `Page.to_dict` now return fresh ordered dictionaries. The document contains `metadata` and selected `pages`; each page contains the pinned eight geometry fields, followed by present default object lists plus annotations or the caller's explicit object types. Tuple/list order, single-use generator consumption across pages, selected original page numbers and doctops, and four exact invalid-input failures match pinned v0.11.10.
  - Red: the exact PDF-028 wheel raised two `AttributeError`s because `PDF.to_dict` was absent. The first candidate exposed raw backend f32 box values and made selected doctop differ from public page height. Green: fresh exact-Maturin 1.14.1 CPython 3.13 wheel and sdist runs passed all thirty-six artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, all pinned contracts were current, and isolated workspace check, tests, doctests, formatting, and Clippy with warnings denied passed. Six-fixture projection was exact for basic, selected multipage, and curve-table documents; rotated/non-default/non-zero-origin boxes, `issue-1181.pdf` object-key order, complete object schemas, higher-level layout types, serialization, repeated identity, and strict section 10 remain under `PAGE-004`–`PAGE-018`, `OBJ-001`–`OBJ-010`, `SER-008`–`SER-023`, `PDF-030`–`PDF-032`, and `CI-001`–`CI-003`, so the task stays unchecked. No object type, key, order, number, page, filter, failure, fixture, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-030** Implement `.to_json(...)`.
  - `PDF.to_json` and `Page.to_json` now serialize their compatibility `to_dict` output with the pinned v0.11.10 parameter order, compact/indented return forms, text-stream writes returning `None`, recursive float/bool/list/tuple/dict/bytes conversion, PDF-stream base64 conversion, required `object_type` filtering, and exact filter and call-shape failures. The adapter adds required object discriminators without substituting the native Serde schema.
  - Red: the exact PDF-029 wheel raised `AttributeError` for both container methods and lacked `pdfplumber.convert`; the recursive conversion regression separately failed with `ModuleNotFoundError`. Green: fresh exact-Maturin 1.14.1 CPython 3.13 wheel and sdist runs each passed all thirty-nine artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, isolated binding and core Clippy denied warnings, and the complete core workspace test/doctest command passed. The exact serialization differential is deliberately still red: 2/15 JSON cases match byte-for-byte and 13 retain existing object-schema, geometry-value/type, font-name, and precision gaps. A 60-document/253-page pinned corpus audit rejected an axis-based integer heuristic after it produced 1,604 runtime type mismatches; raw PDF-number provenance remains explicit rather than guessed. The task stays unchecked for those residuals, derived-page containers, `PSLiteral`, complete image streams, golden output from every object type, and strict `CI-001`–`CI-003` under `PAGE-004`–`PAGE-020`, `OBJ-001`–`OBJ-010`, `SER-009`–`SER-023`, `PDF-031`, and `PDF-032`; no value, type, key, order, stream state, error, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-031** Implement `.to_csv(...)`.
  - `PDF.to_csv` and `Page.to_csv` now match the pinned v0.11.10 parameter order, return and `StringIO` forms, CRLF bytes, required and prepended column ordering, sorted remaining scalar fields, object selection, serialization-time precision, attribute filters, required `object_type` validation, page-number rows, and exact call-shape failures. The adapter serializes the compatibility object dictionaries without substituting the native Serde schema.
  - Red: the exact PDF-030 wheel raised eleven `AttributeError`s across the two focused tests because both container methods were absent. A strengthened differential red then exposed `char,,31.18,T` instead of the required `char,1,31.18,T`; the writer now adds the containing page number without mutating public object dictionaries. Green: fresh exact-Maturin 1.14.1 CPython 3.13 wheel and sdist runs each passed all forty-one artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, isolated binding and core Clippy denied warnings, and the complete core workspace test/doctest command passed. The exact CSV differential is deliberately still red: 4/13 CSV cases match byte-for-byte, while nine retain existing object-field naming/schema, geometry-value/type, font-name, and image-stream gaps. The task stays unchecked for those residuals, derived-page containers, repeated object-type selection, complete object schemas/streams, and strict `CI-001`–`CI-003` under `PAGE-004`–`PAGE-020`, `OBJ-001`–`OBJ-010`, `SER-011`–`SER-023`, and `PDF-032`; no byte, column, row, value, type, stream state, error, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-032** Match repeated-property access and cache identity where observable.
  - Pinned v0.11.10 retains one `pathlib.Path`, the exact supplied password object, and one mutable metadata dictionary. `to_dict()` reuses that metadata dictionary, while `flush_cache()` and `close()` preserve both its identity and caller mutations; existing document streams, page lists, and object dictionaries remain cached, while annotation, hyperlink, and structure-tree lists remain fresh.
  - Red: the exact PDF-031 wheel returned fresh path, password, and metadata objects and lost metadata mutations from `to_dict()`, after flush, and after close. Green: fresh exact-Maturin 1.14.1 CPython 3.13 wheel and sdist runs each passed all forty-two artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, binding and core Clippy denied warnings, and the complete core workspace test/doctest command passed. The task stays unchecked for page-property caches under `PAGE-016`/`PAGE-020`, the broader candidate gaps, and strict `CI-001`–`CI-003`; no identity, mutation, cache invalidation, fixture, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-033** Match `repr(PDF)` and useful diagnostic attributes.
  - Pinned v0.11.10 identifies instances as `pdfplumber.pdf.PDF`, so the default `repr()` and `str()` use that exact qualified name. Its shared five-item `cached_properties` list retains class/instance identity, `pages_to_parse` retains the exact supplied page collection, and `stream_is_external` distinguishes path-owned from caller-owned streams.
  - Red: the exact PDF-032 wheel reported `builtins.PDF` and omitted all three diagnostic attributes. Green: fresh exact-Maturin 1.14.1 CPython 3.13 wheel and sdist runs each passed all forty-three artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, binding and core Clippy denied warnings, and the complete core workspace tests/doctests passed. The task stays unchecked because import-compatible `pdfplumber.pdf` module files and pdfminer-backed `.doc`, `.rsrcmgr`, and `.laparams` remain open under `PYAPI-005`, `PYAPI-006`, and `PDF-036`, plus strict `CI-001`–`CI-003`; no identity, attribute, ordering, ownership state, fixture, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-034** Add empty-document, zero-page, truncated-document, and invalid-page-selection tests.
  - Pinned v0.11.10 and the candidate reject a zero-byte caller stream without closing it; the real zero-page `issue-297-example.pdf` retains exact empty page/object/annotation/link/structure surfaces and exact dictionary, JSON, and CSV serialization; out-of-range/non-integer page collections remain empty while scalar `pages=1` raises at page materialization.
  - Across every 1–100-byte suffix removal from `basic_text.pdf`, pinned v0.11.10 accepts only removals 1–11 and 20–21. Red: the exact PDF-033 wheel rejected recoverable removals 5, 10, and 20. A first candidate over-accepted the partial-`startxref` band 12–19; the strengthened differential rejected it. Green: fresh exact-Maturin 1.14.1 CPython 3.13 wheel and sdist runs each passed all forty-five artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, binding and core Clippy denied warnings, and the complete core workspace tests/doctests passed. The task stays unchecked because severe-truncation messages and broader malformed-object fixtures remain under `PDF-014`/`REPAIR-012`, plus strict `CI-001`–`CI-003`; no acceptance boundary, malformed case, failure type, stream state, serialization value, fixture, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PDF-035** Expose Rust-only bookmarks, forms, signatures, validation, and image extraction under clearly non-upstream names or namespaces.
  - Pinned v0.11.10 defines none of these methods on `PDF`. The candidate's previous direct `.bookmarks()` method therefore occupied an upstream-facing name while forms, signatures, validation, and image-byte extraction were unavailable from Python.
  - All five families now live under the explicit `document.rust` namespace. `RustPDF` retains native 0-based page indexes, returns isolated extension dictionaries, and keeps signature inspection distinct from verification; the compatibility `PDF` class no longer exposes `.bookmarks()` directly. The task stays unchecked for extension stabilization under `EXT-001`–`EXT-004`, `EXT-008`, `EXT-013`–`EXT-015`, and strict `CI-001`–`CI-003`.
- [ ] **PDF-036** Decide and document deep compatibility for `.doc` and other `pdfminer.six` internals; do not silently return unrelated native types.

### 8.4 P0 — Page Geometry, Identity, and Cache Semantics

- [ ] **PAGE-001** Present public `Page.page_number` as 1-based in Python.
  - The Python getter adds one at the binding boundary and installed-wheel coverage verifies original document numbers 3 and 5 after filtered loading. This stays unchecked for the strict section 10 gates and broader page API parity.
- [ ] **PAGE-002** Preserve the idiomatic zero-based Rust page index without leaking it into compatibility mode.
  - `pdfplumber::Page::page_number()` and PDF extraction remain 0-based; only the Python compatibility getter translates it. Focused core/binding tests and the installed artifact cover both sides. This stays unchecked for the strict section 10 gates.
- [ ] **PAGE-003** Implement `.initial_doctop`.
  - Pinned v0.11.10 exposes full-document values `[0, 841.89, 1683.78, 2525.67, 3367.56]` and selected-view values `[0, 841.89]` for original pages 3 and 5. The first value is an exact Python `int`; later values are `float` for the real-valued `long_document.pdf` page boxes.
  - Red: the exact PDF-035 wheel had no public property and serialized the first value as `0.0`. Green: fresh exact-Maturin 1.14.1 CPython 3.13 wheel and isolated-target sdist runs each passed all forty-eight artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50 and compatibility-harness tests were 103/103. The task stays unchecked because raw PDF numeric provenance for later integer-valued page heights and strict `CI-001`–`CI-003` remain open; no numeric type was guessed from an integer-looking `f64`.
- [ ] **PAGE-004** Match `.rotation`, including normalization and inherited rotation.
  - Pinned v0.11.10 normalizes an inherited `/Rotate -90` to `270`, an explicit `/Rotate 450` to `90`, and `/Rotate 360` to `0` in both the direct property and `to_dict([])`, with integer values and rotation-aware dimensions. Red: the exact PAGE-003 wheel exposed no direct property and serialized the three raw values as `-90`, `450`, and `360`. Green: fresh exact-Maturin 1.14.1 wheel and isolated-target sdist runs each passed all forty-nine artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, the complete current CI-equivalent core workspace tests/doctests passed, both Clippy lanes denied warnings, and formatting and diff checks passed.
  - Rotation is cached with display geometry, so direct access stays lazy and does not interpret page content. The task stays unchecked for exhaustive real-fixture rotations, deeper nested page-tree inheritance, mixed page/text rotation, and strict section 10 under `PAGE-022`–`PAGE-024` and `CI-001`–`CI-003`; no raw rotation, inherited value, normalized value, Python type, dimension, serialization field, failure, fixture, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PAGE-005** Match `.mediabox`.
  - Pinned v0.11.10 sorts each source MediaBox, swaps its axes for normalized rotations 90 and 270, and exposes the inverted top-origin result through both `Page.mediabox` and `Page.to_dict([])`. An inherited `/Rotate -90` and explicit `/Rotate 450` therefore produce `(0, 0, 200, 100)` from `[0, 0, 100, 200]`, while explicit `/Rotate 360` produces `(0, 0, 100, 200)`.
  - Red: the exact PAGE-004 wheel omitted the direct property and serialized the unrotated source box for the 90/270-degree cases. Green: fresh exact-Maturin 1.14.1 CPython 3.13 wheel and isolated-target sdist runs each passed all fifty artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, the complete current CI-equivalent core workspace tests/doctests passed, both Clippy lanes denied warnings, and formatting and diff checks passed. The source MediaBox is cached with lazy page geometry, so direct access does not interpret page content.
  - The task stays unchecked because real `page-boxes-example.pdf` still exposes f32-short coordinates instead of preserving source PDF number precision/types, and nonzero, negative, inverted, exhaustive-rotation/inheritance, related bbox/dimension, and strict section 10 cases remain under `SER-009`, `PAGE-010`, `PAGE-011`, `PAGE-022`, `PAGE-023`, and `CI-001`–`CI-003`; no axis, coordinate, value, type, rotation, serialization field, failure, fixture, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PAGE-006** Match `.cropbox`.
  - Pinned v0.11.10 inherits CropBox through nested page trees, sorts reversed coordinates, swaps axes for normalized rotations 90 and 270, inverts against the normalized MediaBox height, and falls back to the compatible MediaBox when CropBox is absent. Inherited `/CropBox [10 20 90 180]` with `/Rotate -90` and explicit reversed `/CropBox [90 180 10 20]` with `/Rotate 450` both produce `(20, 10, 180, 90)` through `Page.cropbox` and `Page.to_dict([])`; a missing CropBox with `/Rotate 360` produces `(0, 0, 100, 200)`.
  - Red: the exact PAGE-005 wheel omitted the direct property, fell back to the MediaBox for the inherited case, and serialized the explicit case as the raw reversed `(90, 180, 10, 20)`. Green: fresh exact-Maturin 1.14.1 CPython 3.13 wheel and isolated-target sdist runs each passed all fifty-one artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, focused parser/core/binding regressions passed, the complete current CI-equivalent core workspace tests/doctests passed, both Clippy lanes denied warnings, and formatting and diff checks passed. The core caches the inherited source CropBox beside lazy page geometry, and the binding derives one rotation-aware top-origin value for direct and serialized access without content interpretation.
  - The task stays unchecked because real `page-boxes-example.pdf` still differs at f32-short coordinates: pinned returns `(14.17323, 42.519679999999994, 581.10236, 856.06299)` while the candidate returns `(14.17323, 42.519653, 581.10236, 856.063)`. Source-number precision/types, nonzero and negative MediaBox origins, exhaustive rotations, deeper inheritance, related bbox/dimension behavior, and strict section 10 remain open under `SER-009`, `PAGE-010`, `PAGE-011`, `PAGE-022`, `PAGE-023`, and `CI-001`–`CI-003`; no axis, coordinate, value, type, inheritance level, rotation, fallback, serialization field, failure, fixture, test, threshold, tolerance, or assertion was omitted or weakened.
- [ ] **PAGE-007** Match `.trimbox`.
- [ ] **PAGE-008** Match `.bleedbox`.
- [ ] **PAGE-009** Match `.artbox`.
- [ ] **PAGE-010** Match `.bbox`, `.width`, and `.height` for rotated and non-zero-origin pages.
- [ ] **PAGE-011** Match handling of negative MediaBox coordinates.
- [ ] **PAGE-012** Match handling of page `UserUnit`.
- [ ] **PAGE-013** Implement `.is_original`, `.root_page`, and derived-page relationships.
- [ ] **PAGE-014** Implement `point2coord(...)`.
- [ ] **PAGE-015** Match `repr(Page)`.
- [ ] **PAGE-016** Implement `.objects` as a cached dictionary keyed by object type.
- [ ] **PAGE-017** Expose `.chars`, `.lines`, `.rects`, `.curves`, `.images`, `.annots`, and `.hyperlinks` as properties, not methods.
- [ ] **PAGE-018** Expose higher-level layout objects when `laparams` requests them.
- [ ] **PAGE-019** Implement `.close()` to flush page caches.
- [ ] **PAGE-020** Implement `.flush_cache()` with compatible property-cache behavior.
- [ ] **PAGE-021** Ensure cached data is not accidentally shared mutably between original and derived pages.
- [ ] **PAGE-022** Verify inherited resources, boxes, and rotation through nested page trees.
- [ ] **PAGE-023** Verify all four page rotations: 0, 90, 180, and 270 degrees.
- [ ] **PAGE-024** Verify mixed-rotation documents and pages containing separately rotated text.
- [ ] **PAGE-025** Add exact geometry tests for points, bounding boxes, width, height, top/bottom, y0/y1, and doctop.

### 8.5 P0 — Object Dictionary Schema and Ordering

#### Common fields

- [ ] **OBJ-001** Add `object_type` to every compatible object dictionary.
- [ ] **OBJ-002** Add 1-based `page_number` to every compatible page object.
- [ ] **OBJ-003** Provide `x0`, `x1`, `y0`, `y1`, `top`, `bottom`, `width`, `height`, and `doctop` where upstream does.
- [ ] **OBJ-004** Match numeric types and preserve integers versus floats where observable.
- [ ] **OBJ-005** Match object ordering within each page and aggregated document objects.
- [ ] **OBJ-006** Match color value shapes for gray, RGB, CMYK, pattern, and unknown color spaces.
- [ ] **OBJ-007** Match missing-value behavior: omitted key versus key with `None`.
- [ ] **OBJ-008** Match serialization of bytes, tuples, nested dictionaries, streams, and PDF literals.
- [ ] **OBJ-009** Add schema snapshots for every object type and fixture class.
- [ ] **OBJ-010** Add a compatibility adapter so Rust-native field names can remain idiomatic without changing Python keys.

#### Characters

- [ ] **OBJ-CHAR-001** Match `text`.
- [ ] **OBJ-CHAR-002** Match `fontname`, including subset prefixes and malformed names.
- [ ] **OBJ-CHAR-003** Match `size`.
- [ ] **OBJ-CHAR-004** Match `adv`.
- [ ] **OBJ-CHAR-005** Match `upright`.
- [ ] **OBJ-CHAR-006** Match `matrix` as a six-value Current Transformation Matrix tuple.
- [ ] **OBJ-CHAR-007** Match `mcid` and `tag`.
- [ ] **OBJ-CHAR-008** Match `ncs`, stroking pattern, and non-stroking pattern fields where emitted upstream.
- [ ] **OBJ-CHAR-009** Match `stroking_color` and `non_stroking_color`.
- [ ] **OBJ-CHAR-010** Do not expose the Rust-only `direction` key in strict compatibility output unless upstream includes it.
- [ ] **OBJ-CHAR-011** Preserve Rust-only `char_code` and parsed-font metadata under a separate extension API.
- [ ] **OBJ-CHAR-012** Match character order before word grouping.

#### Words and text lines

- [ ] **OBJ-WORD-001** Match word keys: `text`, coordinates, `doctop`, `upright`, `height`, `width`, and `direction`.
- [ ] **OBJ-WORD-002** Add requested `extra_attrs` to word dictionaries.
- [ ] **OBJ-WORD-003** Add `chars` only when `return_chars=True`.
- [ ] **OBJ-WORD-004** Match text-line dictionary keys and optional `chars`.
- [ ] **OBJ-WORD-005** Match constituent-character object identity/value behavior.

#### Lines, rectangles, curves, and edges

- [ ] **OBJ-GFX-001** Use `linewidth`, not the incompatible Python-binding key `line_width`.
- [ ] **OBJ-GFX-002** Use `stroking_color` and `non_stroking_color`.
- [ ] **OBJ-GFX-003** Match `stroke`, `fill`, and `evenodd`.
- [ ] **OBJ-GFX-004** Match `pts`.
- [ ] **OBJ-GFX-005** Match full curve `path`, including Bezier control points and commands.
- [ ] **OBJ-GFX-006** Match `dash` as `([dash_array], dash_phase)`.
- [ ] **OBJ-GFX-007** Match `mcid` and `tag`.
- [ ] **OBJ-GFX-008** Match line, rectangle, and curve classification rules.
- [ ] **OBJ-GFX-009** Match edge `orientation` values `h` and `v`.
- [ ] **OBJ-GFX-010** Match edge source/object fields after line, rectangle, and curve conversion.

#### Images

- [ ] **OBJ-IMG-001** Use `srcsize=(width, height)`.
- [ ] **OBJ-IMG-002** Use `colorspace`.
- [ ] **OBJ-IMG-003** Use `bits`.
- [ ] **OBJ-IMG-004** Expose compatible `stream` behavior or a documented proxy with matching serialization.
- [ ] **OBJ-IMG-005** Match `imagemask`.
- [ ] **OBJ-IMG-006** Match `name`.
- [ ] **OBJ-IMG-007** Match `mcid` and `tag`.
- [ ] **OBJ-IMG-008** Keep decoded image bytes, MIME type, filters, and export helpers as Rust extensions.

#### Annotations and hyperlinks

- [ ] **OBJ-ANNOT-001** Match annotation coordinate/common fields.
- [ ] **OBJ-ANNOT-002** Match `uri`, `title`, `contents`, and nested `data`.
- [ ] **OBJ-ANNOT-003** Match hyperlink filtering and URI-action behavior.
- [ ] **OBJ-ANNOT-004** Match malformed annotation decoding and warning behavior.

### 8.6 P0 — Crop, Region, Filter, and Derived-Page Semantics

> Current Rust behavior is incompatible: `crop` uses object centers and rebases coordinates. Python `pdfplumber` keeps intersecting objects, clips partial geometry, and preserves root-page coordinates.

- [ ] **REGION-001** Implement Python-compatible `crop(bbox, relative=False, strict=True)`.
- [ ] **REGION-002** Retain objects with any positive-area intersection for `crop`.
- [ ] **REGION-003** Clip partially intersecting object coordinates to the crop boundary.
- [ ] **REGION-004** Clip curve points/path data consistently or match upstream's exact behavior for each field.
- [ ] **REGION-005** Preserve root-page x/y coordinates instead of rebasing to `(0, 0)`.
- [ ] **REGION-006** Preserve absolute `doctop`.
- [ ] **REGION-007** Set the derived page `.bbox` exactly as upstream does.
- [ ] **REGION-008** Implement `relative=True`.
- [ ] **REGION-009** Implement `strict=True` validation and compatible errors.
- [ ] **REGION-010** Implement `strict=False` behavior for partially out-of-page boxes.
- [ ] **REGION-011** Implement `within_bbox(...)` using full containment.
- [ ] **REGION-012** Implement `outside_bbox(...)` using complete non-intersection and upstream bbox behavior.
- [ ] **REGION-013** Match zero-area, inverted, NaN, infinite, and boundary-touching boxes.
- [ ] **REGION-014** Match nested crop/within/outside behavior.
- [ ] **REGION-015** Preserve original/root page references.
- [ ] **REGION-016** Preserve page number, rotation, media box, crop box, and initial doctop.
- [ ] **REGION-017** Make all text, search, table, serialization, structure, annotation, and visual APIs work on cropped pages where upstream supports them.
- [ ] **REGION-018** Implement `Page.filter(test_function)` and `FilteredPage`.
- [ ] **REGION-019** Preserve upstream behavior when a Python filter callback raises.
- [ ] **REGION-020** Implement `dedupe_chars(tolerance=1, extra_attrs=("fontname", "size"))` on original and derived pages.
- [ ] **REGION-021** Match `FilteredPage.to_image()` limitations and document them.
- [ ] **REGION-022** Add differential fixtures with partially intersecting chars, lines, rectangles, curves, images, and annotations.
- [ ] **REGION-023** Decide whether to retain current rebased-center crop as a Rust-only API under a different explicit name.
- [ ] **REGION-024** Replace current unit tests that assert incompatible center/rebase semantics with separate Rust-extension tests and compatibility tests.

### 8.7 P0 — Text Extraction and Search

#### Character extraction foundation

- [ ] **TEXT-001** Match upstream character count, order, text, and geometry on the full compatibility corpus.
- [ ] **TEXT-002** Match content-stream order before spatial sorting.
- [ ] **TEXT-003** Preserve duplicate characters by default.
- [ ] **TEXT-004** Match whitespace and control-character handling.
- [ ] **TEXT-005** Match null/unmapped glyph representation, including `(cid:N)` cases where upstream cannot decode.
- [ ] **TEXT-006** Define approved-delta policy for cases where Rust decodes more accurately than upstream.
- [ ] **TEXT-007** Match Unicode normalization at document-open time.
- [ ] **TEXT-008** Match marked-content `ActualText` behavior where applicable.
- [ ] **TEXT-009** Match text rendering mode, rise, horizontal scaling, character spacing, and word spacing effects on geometry.
- [ ] **TEXT-010** Match rotated, skewed, mirrored, and negative-scale text geometry.

#### `extract_words`

- [ ] **TEXT-WORD-001** Match `x_tolerance=3`.
- [ ] **TEXT-WORD-002** Match `y_tolerance=3`.
- [ ] **TEXT-WORD-003** Match `x_tolerance_ratio=None`.
- [ ] **TEXT-WORD-004** Match `y_tolerance_ratio=None`.
- [ ] **TEXT-WORD-005** Match `keep_blank_chars=False`.
- [ ] **TEXT-WORD-006** Match `use_text_flow=False`.
- [ ] **TEXT-WORD-007** Match `vertical_ttb` compatibility and deprecation warning.
- [ ] **TEXT-WORD-008** Match `horizontal_ltr` compatibility and deprecation warning.
- [ ] **TEXT-WORD-009** Match `line_dir="ttb"`.
- [ ] **TEXT-WORD-010** Match `char_dir="ltr"`.
- [ ] **TEXT-WORD-011** Match default and explicit `line_dir_rotated`.
- [ ] **TEXT-WORD-012** Match default and explicit `char_dir_rotated`.
- [ ] **TEXT-WORD-013** Validate incompatible direction pairs with compatible errors.
- [ ] **TEXT-WORD-014** Match `extra_attrs=None/list`.
- [ ] **TEXT-WORD-015** Match `split_at_punctuation=False`.
- [ ] **TEXT-WORD-016** Match `split_at_punctuation=True` using Python `string.punctuation`.
- [ ] **TEXT-WORD-017** Match custom punctuation strings.
- [ ] **TEXT-WORD-018** Match `expand_ligatures=True/False`.
- [ ] **TEXT-WORD-019** Match `return_chars=False/True`.
- [ ] **TEXT-WORD-020** Match line clustering and intra-line sorting for `ltr`, `rtl`, `ttb`, and `btt`.
- [ ] **TEXT-WORD-021** Match mixed upright and non-upright grouping.
- [ ] **TEXT-WORD-022** Match dynamic tolerance based on the previous character's size.
- [ ] **TEXT-WORD-023** Match overlap/backtracking rules for duplicate and negatively positioned glyphs.
- [ ] **TEXT-WORD-024** Match combining marks and Arabic diacritics without introducing upstream-incompatible order.
- [ ] **TEXT-WORD-025** Match CJK segmentation without undocumented special-case tolerances.
- [ ] **TEXT-WORD-026** Preserve Rust enhanced direction heuristics behind explicit non-compatibility options.

#### `extract_text` and `TextMap`

- [ ] **TEXT-MAP-001** Implement exact non-layout `extract_text(...)` semantics.
- [ ] **TEXT-MAP-002** Implement exact layout `TextMap` semantics rather than the current block/column approximation.
- [ ] **TEXT-MAP-003** Match `layout=False`.
- [ ] **TEXT-MAP-004** Match `layout=True`.
- [ ] **TEXT-MAP-005** Match `x_density=7.25`.
- [ ] **TEXT-MAP-006** Match `y_density=13`.
- [ ] **TEXT-MAP-007** Match `layout_width`.
- [ ] **TEXT-MAP-008** Match `layout_height`.
- [ ] **TEXT-MAP-009** Match `layout_width_chars`.
- [ ] **TEXT-MAP-010** Match `layout_height_chars`.
- [ ] **TEXT-MAP-011** Match `layout_bbox`.
- [ ] **TEXT-MAP-012** Match `x_shift` and `y_shift`.
- [ ] **TEXT-MAP-013** Match `line_dir_render`.
- [ ] **TEXT-MAP-014** Match `char_dir_render`.
- [ ] **TEXT-MAP-015** Match `presorted` and internal ordering where exposed through utility functions.
- [ ] **TEXT-MAP-016** Match layout-implied whitespace and its mapping to `None`.
- [ ] **TEXT-MAP-017** Match transposition/reversal for all render-direction combinations.
- [ ] **TEXT-MAP-018** Match empty-page behavior.
- [ ] **TEXT-MAP-019** Keep the current Rust column-detection/block-layout algorithm as an explicitly named extension.
- [ ] **TEXT-MAP-020** Remove or isolate incompatible defaults (`x_density=10`, `y_density=10`) from Python compatibility mode.

#### Additional text methods

- [ ] **TEXT-EXTRA-001** Implement `extract_text_simple(x_tolerance=3, y_tolerance=3)`.
- [ ] **TEXT-EXTRA-002** Implement `extract_text_lines(layout=False, strip=True, return_chars=True, **kwargs)`.
- [ ] **TEXT-EXTRA-003** Match text-line whitespace stripping.
- [ ] **TEXT-EXTRA-004** Match text-line char lists and coordinates.
- [ ] **TEXT-EXTRA-005** Implement or expose `get_textmap(...)` with compatible caching where applications use it.
- [ ] **TEXT-EXTRA-006** Match repeated extraction with different options; option-specific results must not poison caches.
- [ ] **TEXT-EXTRA-007** Match behavior on pages containing no characters.

#### Search

- [ ] **TEXT-SEARCH-001** Accept strings and compiled regular-expression patterns.
- [ ] **TEXT-SEARCH-002** Match `regex=True/False`.
- [ ] **TEXT-SEARCH-003** Match `case=True/False`.
- [ ] **TEXT-SEARCH-004** Match `main_group=0` and nonzero groups.
- [ ] **TEXT-SEARCH-005** Match `return_groups=True/False`.
- [ ] **TEXT-SEARCH-006** Match `return_chars=True/False`.
- [ ] **TEXT-SEARCH-007** Pass all layout and word options through search.
- [ ] **TEXT-SEARCH-008** Match bounding boxes from the selected main group.
- [ ] **TEXT-SEARCH-009** Discard zero-length and whitespace-only matches.
- [ ] **TEXT-SEARCH-010** Match errors for incompatible compiled-pattern flags.
- [ ] **TEXT-SEARCH-011** Match capture-group tuple values and `None` entries.
- [ ] **TEXT-SEARCH-012** Match search order across multiline and rotated text.

#### Known text parity defects

- [ ] **TEXT-BUG-218** Fix 90°/270° `extract_text` newline/order artifacts and close issue `#218`.
- [ ] **TEXT-BUG-219** Improve Identity no-embed decoding and word grouping for `pdfjs/noembed-identity-2.pdf`; close issue `#219`.
- [ ] **TEXT-BUG-220** Fix tagged TrueType extraction for `hello_structure.pdf`; close issue `#220`.
- [ ] **TEXT-BUG-221** Recover word grouping for `issue-848.pdf`; close the text portion of issue `#221`.
- [ ] **TEXT-BUG-222** Resolve or explain every residual near-100% char/word delta in issue `#222`.
- [ ] **TEXT-BUG-223A** Add exact rotated-page word-order tests required by table issue `#223`.
- [ ] **TEXT-BUG-285A** Fix vertical cluster ordering that reverses table-cell text in issue `#285`.

### 8.8 P0 — Table Detection and Extraction

#### Settings and validation

- [ ] **TABLE-001** Expose `TableSettings` in Python with compatible constructor and field names.
- [ ] **TABLE-002** Accept a settings dictionary, a `TableSettings` instance, or `None`.
- [ ] **TABLE-003** Match `TableSettings.resolve(...)`.
- [ ] **TABLE-004** Match strategies `lines`, `lines_strict`, `text`, and `explicit`.
- [ ] **TABLE-005** Match independent `vertical_strategy` and `horizontal_strategy`.
- [ ] **TABLE-006** Match `explicit_vertical_lines` as numbers and line/rect/curve-like objects.
- [ ] **TABLE-007** Match `explicit_horizontal_lines` as numbers and line/rect/curve-like objects.
- [ ] **TABLE-008** Validate that explicit strategy receives at least two lines.
- [ ] **TABLE-009** Match `snap_tolerance`, `snap_x_tolerance`, and `snap_y_tolerance` fallback rules.
- [ ] **TABLE-010** Match `join_tolerance`, `join_x_tolerance`, and `join_y_tolerance` fallback rules.
- [ ] **TABLE-011** Match `edge_min_length=3`.
- [ ] **TABLE-012** Match `edge_min_length_prefilter=1`.
- [ ] **TABLE-013** Match `min_words_vertical=3`.
- [ ] **TABLE-014** Match `min_words_horizontal=1`.
- [ ] **TABLE-015** Match `intersection_tolerance`, `intersection_x_tolerance`, and `intersection_y_tolerance`.
- [ ] **TABLE-016** Match `text_tolerance`, `text_x_tolerance`, and `text_y_tolerance`.
- [ ] **TABLE-017** Forward every `text_*` setting to compatible word/text extraction.
- [ ] **TABLE-018** Match non-negative setting validation and unknown-key errors.
- [ ] **TABLE-019** Preserve Rust-only `min_accuracy` under a clearly separate extension setting.
- [ ] **TABLE-020** Preserve Rust-only `duplicate_merged_content` under a clearly separate extension setting.

#### Edge and cell pipeline

- [ ] **TABLE-EDGE-001** Match rectangle-edge and curve-edge derivation.
- [ ] **TABLE-EDGE-002** Match `lines_strict` exclusion of rectangle edges.
- [ ] **TABLE-EDGE-003** Match text-derived vertical alignments using left, right, and center positions.
- [ ] **TABLE-EDGE-004** Match text-derived horizontal top/bottom edge behavior.
- [ ] **TABLE-EDGE-005** Match edge snapping cluster semantics and averaged position.
- [ ] **TABLE-EDGE-006** Match collinear edge joining and endpoint tolerance.
- [ ] **TABLE-EDGE-007** Match edge filtering order before and after joining.
- [ ] **TABLE-EDGE-008** Match edge-intersection tests.
- [ ] **TABLE-EDGE-009** Match smallest-cell reconstruction.
- [ ] **TABLE-EDGE-010** Match contiguous-cell table grouping.
- [ ] **TABLE-EDGE-011** Match table top-to-bottom/left-to-right ordering.
- [ ] **TABLE-EDGE-012** Match behavior for one-cell groups, gaps, merged cells, and irregular grids.
- [ ] **TABLE-EDGE-013** Add property tests for snap, join, intersections, and cell reconstruction invariants.

#### Public classes and methods

- [ ] **TABLE-API-001** Implement `Page.debug_tablefinder(table_settings=None)`.
- [ ] **TABLE-API-002** Implement `Page.find_tables(table_settings=None)`.
- [ ] **TABLE-API-003** Implement `Page.find_table(table_settings=None)`.
- [ ] **TABLE-API-004** Match largest-table selection by cell count and topmost tie-break.
- [ ] **TABLE-API-005** Implement `Page.extract_tables(table_settings=None)`.
- [ ] **TABLE-API-006** Implement `Page.extract_table(table_settings=None)`.
- [ ] **TABLE-API-007** Expose `TableFinder.edges`.
- [ ] **TABLE-API-008** Expose `TableFinder.intersections` with compatible key/value shapes.
- [ ] **TABLE-API-009** Expose `TableFinder.cells`.
- [ ] **TABLE-API-010** Expose `TableFinder.tables`.
- [ ] **TABLE-API-011** Expose `Table.cells` as compatible bounding-box tuples.
- [ ] **TABLE-API-012** Expose `Table.rows` as `Row`/`CellGroup` objects with `.cells` and `.bbox`.
- [ ] **TABLE-API-013** Expose `Table.columns` as `Column`/`CellGroup` objects with `.cells` and `.bbox`.
- [ ] **TABLE-API-014** Match `Table.bbox`.
- [ ] **TABLE-API-015** Implement `Table.extract(**text_kwargs)`.
- [ ] **TABLE-API-016** Match cell inclusion by character midpoint and right/bottom boundary rules.
- [ ] **TABLE-API-017** Match layout-specific cell width, height, and bbox handling.
- [ ] **TABLE-API-018** Match `None` for missing positions caused by spanning cells and `""` for present blank cells.
- [ ] **TABLE-API-019** Keep Rust quality/accuracy/whitespace metrics as extensions without changing upstream row/cell structures.
- [ ] **TABLE-API-020** Make all table APIs work identically on compatible cropped pages.

#### Known table parity defects

- [ ] **TABLE-BUG-221** Recover table quality for `issue-848.pdf`; close the table portion of issue `#221`.
- [ ] **TABLE-BUG-223** Raise rotated NICS table parity to the accepted exact/approved-delta gate and close issue `#223`.
- [ ] **TABLE-BUG-285** Fix rotated table-cell word order without regressing non-rotated fixtures; close issue `#285`.
- [ ] **TABLE-BUG-286** Obtain or create a redistributable minimal reproducer for issue `#286`, add a failing test, and fix table discovery.
- [ ] **TABLE-BUG-ROT-001** Test table detection and extraction at page rotations 0, 90, 180, and 270.
- [ ] **TABLE-BUG-CROP-001** Test tables whose borders and text cross crop boundaries.
- [ ] **TABLE-BUG-DASH-001** Test short dashed rules and `edge_min_length_prefilter`.
- [ ] **TABLE-BUG-MERGE-001** Test horizontal and vertical spanning cells with exact `None` placement.
- [ ] **TABLE-BUG-TEXT-001** Test mixed strategies and `text_*` options against upstream.
- [ ] **TABLE-BUG-ORDER-001** Test multiple equal-sized tables and tie-breaking.

### 8.9 P1 — Graphics, Paths, Colors, and Coordinate Fidelity

- [ ] **GFX-001** Match Current Transformation Matrix composition order for page, text, path, and Form XObject transforms.
- [ ] **GFX-002** Match line-width scaling.
- [ ] **GFX-003** Match line cap, line join, miter limit, and dash data where upstream emits them.
- [ ] **GFX-004** Match path operators `m`, `l`, `c`, `v`, `y`, `h`, and `re`.
- [ ] **GFX-005** Match painting operators `S`, `s`, `f`, `F`, `f*`, `B`, `B*`, `b`, `b*`, and `n`.
- [ ] **GFX-006** Match clipping operators `W` and `W*` where they affect extracted geometry.
- [ ] **GFX-007** Match graphics-state save/restore `q` and `Q`.
- [ ] **GFX-008** Match transformation operator `cm`.
- [ ] **GFX-009** Match external graphics state `gs`.
- [ ] **GFX-010** Match DeviceGray, DeviceRGB, DeviceCMYK, calibrated, ICC-based, indexed, separation, DeviceN, and pattern color representations where upstream succeeds.
- [ ] **GFX-011** Match stroking versus non-stroking color state.
- [ ] **GFX-012** Match rectangle classification versus generic curve classification.
- [ ] **GFX-013** Match degenerate horizontal/vertical lines and zero-area rectangles.
- [ ] **GFX-014** Match Bezier path control points and endpoint ordering.
- [ ] **GFX-015** Match nested Form XObject geometry and resource inheritance.
- [ ] **GFX-016** Detect and safely stop cyclic Form XObject recursion.
- [ ] **GFX-017** Match marked-content tags and MCIDs on paths.
- [ ] **GFX-018** Match shape order relative to text and images.
- [ ] **GFX-019** Match derived `.rect_edges`, `.curve_edges`, and combined `.edges`.
- [ ] **GFX-020** Implement Python `pdfplumber.ctm.CTM` with `scale_x`, `scale_y`, `skew_x`, `skew_y`, `translation_x`, and `translation_y`.
- [ ] **GFX-021** Add coordinate property tests for arbitrary affine transformations.
- [ ] **GFX-022** Add regression tests for non-zero page origins, negative coordinates, oversized boxes, and `UserUnit`.
- [ ] **GFX-023** Define stable float-normalization rules without rounding extraction results prematurely.
- [ ] **GFX-024** Verify high-precision coordinates survive Python conversion and JSON serialization.

### 8.10 P1 — Images

- [ ] **IMAGE-001** Match inline-image discovery.
- [ ] **IMAGE-002** Match Image XObject discovery.
- [ ] **IMAGE-003** Match images nested in Form XObjects.
- [ ] **IMAGE-004** Match transformed, mirrored, clipped, and rotated image geometry.
- [ ] **IMAGE-005** Match source dimensions, color space, bits per component, mask, name, and stream metadata.
- [ ] **IMAGE-006** Match soft-mask and stencil-mask metadata where upstream exposes it.
- [ ] **IMAGE-007** Match image object ordering.
- [ ] **IMAGE-008** Add a compatible stream proxy for serializer behavior.
- [ ] **IMAGE-009** Ensure unsupported or malformed image filters do not prevent unrelated text/geometry extraction unless upstream also fails.
- [ ] **IMAGE-010** Test Flate, DCT/JPEG, JPX/JPEG 2000, CCITT Fax, JBIG2, LZW, RunLength, ASCII85, and ASCIIHex image streams as applicable.
- [ ] **IMAGE-011** Keep native image decoding/export as an extension and test it independently.
- [ ] **IMAGE-012** Do not claim that upstream Python `pdfplumber` reconstructs images; preserve its compatibility surface while documenting Rust extensions.

### 8.11 P1 — Annotations, Hyperlinks, Forms, Bookmarks, Signatures, and Structure

#### Upstream-compatible annotations and structure

- [ ] **SEM-ANNOT-001** Expose `Page.annots` and `PDF.annots`.
- [ ] **SEM-ANNOT-002** Expose `Page.hyperlinks` and `PDF.hyperlinks`.
- [ ] **SEM-ANNOT-003** Match Link URI action extraction.
- [ ] **SEM-ANNOT-004** Match annotation rectangle coordinate conversion under rotation.
- [ ] **SEM-ANNOT-005** Match annotation text decoding and raw data representation.
- [ ] **SEM-STRUCT-001** Implement `PDF.structure_tree`.
- [ ] **SEM-STRUCT-002** Implement `Page.structure_tree`.
- [ ] **SEM-STRUCT-003** Implement `PDFStructTree`.
- [ ] **SEM-STRUCT-004** Implement `PDFStructElement`.
- [ ] **SEM-STRUCT-005** Implement `StructTreeMissing`.
- [ ] **SEM-STRUCT-006** Match fields: type, revision, id, lang, alt text, actual text, title, page number, attributes, MCIDs, and children.
- [ ] **SEM-STRUCT-007** Match RoleMap and ClassMap resolution.
- [ ] **SEM-STRUCT-008** Match ParentTree and page-scoped structure parsing.
- [ ] **SEM-STRUCT-009** Match marked-content references and object references.
- [ ] **SEM-STRUCT-010** Implement `.find(...)` for strings, regular expressions, and callables.
- [ ] **SEM-STRUCT-011** Implement `.find_all(...)`.
- [ ] **SEM-STRUCT-012** Implement `.all_mcids()`.
- [ ] **SEM-STRUCT-013** Implement compact `.to_dict()`.
- [ ] **SEM-STRUCT-014** Match page numbers and MCID ordering in multipage elements.
- [ ] **SEM-STRUCT-015** Match behavior when a PDF has no structure tree.

#### Rust extensions that must remain isolated

- [ ] **SEM-EXT-001** Document forms as a Rust extension unless deep `.doc` compatibility is implemented.
- [ ] **SEM-EXT-002** Document bookmarks/outlines as a Rust extension; Python `pdfplumber` v0.11.10 does not expose the same high-level API.
- [ ] **SEM-EXT-003** Document digital signatures as a Rust extension.
- [ ] **SEM-EXT-004** Document semantic character traversal and HTML export as Rust extensions.
- [ ] **SEM-EXT-005** Ensure extension fields never appear unexpectedly in strict compatibility dictionaries.
- [ ] **SEM-EXT-006** Place extension APIs in an explicit namespace or use clearly non-upstream method names.
- [ ] **SEM-EXT-007** Add deep-compatibility tests if `.doc`, AcroForm internals, or raw PDF object wrappers are promised.

### 8.12 P1 — Visual Debugging and Rendering

- [ ] **DISPLAY-001** Implement `Page.to_image(resolution=72, width=None, height=None, antialias=False, force_mediabox=False)`.
- [ ] **DISPLAY-002** Enforce the upstream mutual-exclusion rules for resolution, width, and height.
- [ ] **DISPLAY-003** Render path inputs and in-memory streams.
- [ ] **DISPLAY-004** Handle passwords during rendering.
- [ ] **DISPLAY-005** Match crop box versus media box behavior.
- [ ] **DISPLAY-006** Match cropped-page image dimensions and coordinate projection.
- [ ] **DISPLAY-007** Implement `PageImage.original`, `annotated`, `scale`, `bbox`, and drawing state.
- [ ] **DISPLAY-008** Implement `PageImage.reset()`.
- [ ] **DISPLAY-009** Implement `PageImage.copy()`.
- [ ] **DISPLAY-010** Implement `PageImage.save(...)`, including format, quantization, colors, bits, DPI, and file-like destinations.
- [ ] **DISPLAY-011** Implement `draw_line` and `draw_lines`.
- [ ] **DISPLAY-012** Implement `draw_vline` and `draw_vlines`.
- [ ] **DISPLAY-013** Implement `draw_hline` and `draw_hlines`.
- [ ] **DISPLAY-014** Implement `draw_rect` and `draw_rects`.
- [ ] **DISPLAY-015** Implement `draw_circle` and `draw_circles`.
- [ ] **DISPLAY-016** Implement `debug_table(...)`.
- [ ] **DISPLAY-017** Implement `debug_tablefinder(...)`.
- [ ] **DISPLAY-018** Implement `outline_words(...)`.
- [ ] **DISPLAY-019** Implement `outline_chars(...)`.
- [ ] **DISPLAY-020** Implement `_repr_png_()` for notebooks.
- [ ] **DISPLAY-021** Implement `show()`.
- [ ] **DISPLAY-022** Match accepted color formats and defaults.
- [ ] **DISPLAY-023** Use pixel-difference tests with a documented rendering tolerance.
- [ ] **DISPLAY-024** Test 0/90/180/270-degree pages and non-default boxes.
- [ ] **DISPLAY-025** Make Pillow and rendering dependencies optional only if import behavior remains compatible.
- [ ] **DISPLAY-026** Retain native SVG debugging as an extension.
- [ ] **DISPLAY-027** Match upstream behavior that filtered-page edits are not reflected by raster rendering.
- [ ] **DISPLAY-028** Convert renderer failures to compatible `MalformedPDFException` behavior.

### 8.13 P1 — Serialization and Container APIs

- [ ] **SER-001** Implement `Container.flush_cache(...)`.
- [ ] **SER-002** Expose properties `rects`, `lines`, `curves`, `images`, `chars`, and optional higher-level layout object lists.
- [ ] **SER-003** Implement `rect_edges`.
- [ ] **SER-004** Implement `curve_edges`.
- [ ] **SER-005** Implement combined `edges`.
- [ ] **SER-006** Implement `horizontal_edges`.
- [ ] **SER-007** Implement `vertical_edges`.
- [ ] **SER-008** Implement `to_dict(object_types=None)`.
- [ ] **SER-009** Implement `to_json(stream=None, object_types=None, precision=None, include_attrs=None, exclude_attrs=None, indent=None)`.
- [ ] **SER-010** Implement `to_csv(stream=None, object_types=None, precision=None, include_attrs=None, exclude_attrs=None)`.
- [ ] **SER-011** Match default object types and object-type filtering.
- [ ] **SER-012** Reject simultaneous `include_attrs` and `exclude_attrs`.
- [ ] **SER-013** Prevent exclusion of required CSV fields.
- [ ] **SER-014** Match CSV column ordering and union of observed attributes.
- [ ] **SER-015** Match float rounding only at serialization time.
- [ ] **SER-016** Match boolean serialization.
- [ ] **SER-017** Match tuple/list/dictionary recursive serialization.
- [ ] **SER-018** Match byte decoding attempts.
- [ ] **SER-019** Match PDF stream base64 representation in JSON.
- [ ] **SER-020** Match Unicode output and `ensure_ascii` behavior.
- [ ] **SER-021** Match writing to paths, text streams, and returned strings where supported.
- [ ] **SER-022** Add exact golden files for JSON and CSV from every object type.
- [ ] **SER-023** Keep native Serde models as an extension and do not substitute their schema for compatibility output.

### 8.14 P1 — Upstream-Compatible Command-Line Interface

- [ ] **CLI-001** Provide an upstream-compatible `pdfplumber` executable accepting an input file without a required subcommand.
- [ ] **CLI-002** Preserve the current richer Rust subcommand interface under a separate executable or explicit mode.
- [ ] **CLI-003** Match `--format csv|json|text`, defaulting to CSV.
- [ ] **CLI-004** Match 1-based `--pages` lists and inclusive ranges.
- [ ] **CLI-005** Match `--types`.
- [ ] **CLI-006** Match `--include-attrs`.
- [ ] **CLI-007** Match `--exclude-attrs`.
- [ ] **CLI-008** Match `--laparams` JSON parsing.
- [ ] **CLI-009** Match `--precision`.
- [ ] **CLI-010** Match `--indent`.
- [ ] **CLI-011** Match `--structure`.
- [ ] **CLI-012** Match `--structure-text`.
- [ ] **CLI-013** Match text output using `extract_text(layout=True)`.
- [ ] **CLI-014** Match default help behavior when invoked without arguments.
- [ ] **CLI-015** Match standard input/file handling where upstream supports it.
- [ ] **CLI-016** Match standard output, standard error, exit codes, and broken-pipe behavior.
- [ ] **CLI-017** Add subprocess differential tests for all flags and combinations.
- [ ] **CLI-018** Add shell-quoting and non-ASCII path tests.
- [ ] **CLI-019** Verify password and repair extension flags do not conflict with compatibility invocation.
- [ ] **CLI-020** Document which executable exposes Rust-only annotations, links, bookmarks, forms, images, search, and SVG debug commands.

### 8.15 P1 — Public Utility and Compatibility Modules

#### `pdfplumber.utils`

- [ ] **UTIL-001** Implement `cluster_list`.
- [ ] **UTIL-002** Implement `make_cluster_dict`.
- [ ] **UTIL-003** Implement `cluster_objects`, including `preserve_order`.
- [ ] **UTIL-004** Implement `to_list`.
- [ ] **UTIL-005** Implement `bbox_to_rect`.
- [ ] **UTIL-006** Implement `calculate_area`.
- [ ] **UTIL-007** Implement `clip_obj`.
- [ ] **UTIL-008** Implement `crop_to_bbox`.
- [ ] **UTIL-009** Implement `curve_to_edges`.
- [ ] **UTIL-010** Implement `filter_edges`.
- [ ] **UTIL-011** Implement `get_bbox_overlap`.
- [ ] **UTIL-012** Implement `intersects_bbox`.
- [ ] **UTIL-013** Implement `line_to_edge`.
- [ ] **UTIL-014** Implement `merge_bboxes`.
- [ ] **UTIL-015** Implement `move_object`.
- [ ] **UTIL-016** Implement `obj_to_bbox`.
- [ ] **UTIL-017** Implement `obj_to_edges`.
- [ ] **UTIL-018** Implement `objects_to_bbox`.
- [ ] **UTIL-019** Implement `objects_to_rect`.
- [ ] **UTIL-020** Implement utility `outside_bbox`.
- [ ] **UTIL-021** Implement `rect_to_edges`.
- [ ] **UTIL-022** Implement `resize_object`.
- [ ] **UTIL-023** Implement `snap_objects`.
- [ ] **UTIL-024** Implement utility `within_bbox`.
- [ ] **UTIL-025** Implement `decode_psl_list`.
- [ ] **UTIL-026** Implement `decode_text`.
- [ ] **UTIL-027** Implement `resolve`.
- [ ] **UTIL-028** Implement `resolve_all`.
- [ ] **UTIL-029** Implement `resolve_and_decode`.
- [ ] **UTIL-030** Implement `chars_to_textmap`.
- [ ] **UTIL-031** Implement `collate_line`.
- [ ] **UTIL-032** Implement utility `dedupe_chars`.
- [ ] **UTIL-033** Implement utility `extract_text`.
- [ ] **UTIL-034** Implement utility `extract_text_simple`.
- [ ] **UTIL-035** Implement utility `extract_words`.
- [ ] **UTIL-036** Export matching default tolerance and density constants.
- [ ] **UTIL-037** Match utility signatures and errors with upstream contract snapshots.

#### `pdfplumber.table`, `convert`, `exceptions`, and related classes

- [ ] **UTIL-TABLE-001** Expose `snap_edges`.
- [ ] **UTIL-TABLE-002** Expose `join_edge_group`.
- [ ] **UTIL-TABLE-003** Expose `merge_edges`.
- [ ] **UTIL-TABLE-004** Expose `words_to_edges_h`.
- [ ] **UTIL-TABLE-005** Expose `words_to_edges_v`.
- [ ] **UTIL-TABLE-006** Expose `edges_to_intersections`.
- [ ] **UTIL-TABLE-007** Expose `intersections_to_cells`.
- [ ] **UTIL-TABLE-008** Expose `cells_to_tables`.
- [ ] **UTIL-TABLE-009** Expose `CellGroup`, `Row`, `Column`, `Table`, `TableSettings`, and `TableFinder`.
- [ ] **UTIL-CONVERT-001** Implement `get_attr_filter`.
- [ ] **UTIL-CONVERT-002** Implement `Serializer`.
- [ ] **UTIL-CONVERT-003** Match `CSV_COLS_REQUIRED` and `CSV_COLS_TO_PREPEND`.
- [ ] **UTIL-EXC-001** Implement `MalformedPDFException`.
- [ ] **UTIL-EXC-002** Implement `PdfminerException`.
  - The isolated Python package now exposes the native exception at the pinned `pdfplumber.utils.exceptions.PdfminerException` identity with `Exception` as its base, without importing `pdfminer.six` at runtime. PDF opening maps only the pinned compatibility boundary into this type; private native diagnostic classes remain available. The task stays unchecked for the wider utility-module surface and strict `CI-001`/`CI-002`/`CI-003` gates.
- [ ] **UTIL-DEBUG-001** Implement or compatibly delegate `set_debug`.
- [ ] **UTIL-DEBUG-002** Document any intentionally unsupported low-level `pdfminer.six` wrapper behavior.

### 8.16 P1 — PDF Parsing, Fonts, Encodings, and Content Streams

> A parser item is complete only when fixtures that Python `pdfplumber` successfully processes produce compatible output. Merely accepting the file is not enough.

#### File structure and streams

- [ ] **PARSE-001** Verify classic cross-reference tables.
- [ ] **PARSE-002** Verify cross-reference streams.
- [ ] **PARSE-003** Verify hybrid-reference files.
- [ ] **PARSE-004** Verify object streams.
- [ ] **PARSE-005** Verify incremental updates and latest-object resolution.
- [ ] **PARSE-006** Verify linearized PDFs.
- [ ] **PARSE-007** Verify inherited page-tree resources and attributes.
- [ ] **PARSE-008** Verify arrays of page content streams.
- [ ] **PARSE-009** Verify indirect stream lengths and malformed stream recovery.
- [ ] **PARSE-010** Verify FlateDecode and predictors.
- [ ] **PARSE-011** Verify LZWDecode and predictors.
- [ ] **PARSE-012** Verify ASCIIHexDecode.
- [ ] **PARSE-013** Verify ASCII85Decode.
- [ ] **PARSE-014** Verify RunLengthDecode.
- [ ] **PARSE-015** Verify content-stream filter chains.
- [ ] **PARSE-016** Match upstream behavior on malformed tokens, comments, whitespace, and unexpected end-of-file.
- [ ] **PARSE-017** Avoid whole-document failure when a recoverable page/object error can be isolated, matching upstream behavior.
- [ ] **PARSE-018** Add deterministic warning/error context with object and page identifiers.

#### Encryption

- [ ] **PARSE-ENC-001** Verify unencrypted PDFs.
- [ ] **PARSE-ENC-002** Verify Standard Security Handler revisions supported by upstream.
- [ ] **PARSE-ENC-003** Verify RC4-encrypted documents where supported.
- [ ] **PARSE-ENC-004** Verify AES-128 documents.
- [ ] **PARSE-ENC-005** Verify AES-256 documents.
- [ ] **PARSE-ENC-006** Verify user and owner passwords.
- [ ] **PARSE-ENC-007** Verify encrypted metadata behavior.
- [ ] **PARSE-ENC-008** Return controlled errors for unsupported public-key encryption.
- [ ] **PARSE-ENC-009** Ensure passwords are never logged or included in panic/error diagnostics.

#### Fonts and character mapping

- [ ] **FONT-001** Verify Standard 14 fonts.
- [ ] **FONT-002** Verify Type 1 fonts.
- [ ] **FONT-003** Verify TrueType fonts.
- [ ] **FONT-004** Verify Type 0 composite fonts.
- [ ] **FONT-005** Verify CIDFontType0 and CIDFontType2.
- [ ] **FONT-006** Verify Compact Font Format (CFF) and OpenType-CFF.
- [ ] **FONT-007** Implement or verify Type 3 fonts.
- [ ] **FONT-008** Match built-in encodings and Differences arrays.
- [ ] **FONT-009** Match glyph-name-to-Unicode conversion.
- [ ] **FONT-010** Match ToUnicode CMaps.
- [ ] **FONT-011** Match Identity-H and Identity-V.
- [ ] **FONT-012** Match predefined Adobe-CNS1, Adobe-GB1, Adobe-Japan1, and Adobe-Korea1 CMaps.
- [ ] **FONT-013** Match CIDToGIDMap handling.
- [ ] **FONT-014** Match horizontal font metrics.
- [ ] **FONT-015** Match vertical metrics and writing mode.
- [ ] **FONT-016** Match missing widths and font descriptor fallbacks.
- [ ] **FONT-017** Match subset font names.
- [ ] **FONT-018** Match ligatures and multi-codepoint mappings.
- [ ] **FONT-019** Match surrogate pairs and supplementary-plane Unicode.
- [ ] **FONT-020** Match combining marks and zero-width glyphs.
- [ ] **FONT-021** Match malformed/missing ToUnicode fallback behavior.
- [ ] **FONT-022** Match no-embed and symbolic-font fallbacks.
- [ ] **FONT-023** Add font-level unit fixtures independent of full PDFs.
- [ ] **FONT-024** Add corpus tests for Latin, Greek, Cyrillic, Arabic, Hebrew, Devanagari, Thai, CJK, and mixed-script pages.
- [ ] **FONT-025** Fix all font-related open-issue fixtures without filename-specific code.

#### Text and graphics operators

- [ ] **OP-001** Verify `BT` and `ET`.
- [ ] **OP-002** Verify `Tf`.
- [ ] **OP-003** Verify `Tm`, `Td`, `TD`, and `T*`.
- [ ] **OP-004** Verify `Tj` and `TJ`.
- [ ] **OP-005** Verify single-quote and double-quote text-showing operators.
- [ ] **OP-006** Verify `Tc`, `Tw`, `Tz`, `TL`, `Tr`, and `Ts`.
- [ ] **OP-007** Verify graphics operators listed in section 8.9.
- [ ] **OP-008** Verify `Do` for Form and Image XObjects.
- [ ] **OP-009** Verify inline images `BI`, `ID`, and `EI`.
- [ ] **OP-010** Verify marked-content operators `BMC`, `BDC`, `EMC`, `MP`, and `DP`.
- [ ] **OP-011** Verify compatibility sections `BX` and `EX`.
- [ ] **OP-012** Safely ignore unknown operators in the same circumstances as upstream.
- [ ] **OP-013** Preserve operator/resource error context without aborting unrelated pages unnecessarily.

### 8.17 P1 — Repair and Malformed-PDF Behavior

- [ ] **REPAIR-001** Implement top-level `pdfplumber.repair(...)`.
- [ ] **REPAIR-002** Accept paths and binary streams.
- [ ] **REPAIR-003** Return `BytesIO` when no output path is supplied.
- [ ] **REPAIR-004** Return `None` after writing an output file.
- [ ] **REPAIR-005** Discover `gs`, `gswin32c`, and `gswin64c` compatibly.
- [ ] **REPAIR-006** Accept `gs_path`.
- [ ] **REPAIR-007** Accept `password`.
- [ ] **REPAIR-008** Accept all repair settings.
- [ ] **REPAIR-009** Match subprocess failure behavior and useful error text.
- [ ] **REPAIR-010** Match `PDF.open(..., repair=True)` integration.
- [ ] **REPAIR-011** Keep any pure-Rust repair path as an extension unless its output is behaviorally equivalent.
- [ ] **REPAIR-012** Add malformed-PDF fixtures for xref, stream length, truncated objects, invalid metadata, and broken CMaps.
- [ ] **REPAIR-013** Never overwrite an input file unintentionally.
- [ ] **REPAIR-014** Use temporary files securely and clean them on all exit paths.

### 8.18 P1 — Robustness, Security, and Fuzzing

- [ ] **ROBUST-001** Add `#![forbid(unsafe_code)]` to first-party crates that do not require unsafe code.
- [ ] **ROBUST-002** Audit dependency unsafe code and document trust boundaries.
- [ ] **ROBUST-003** Ensure every `unwrap`, `expect`, index operation, and assertion reachable from untrusted PDF input is removed or proven safe.
- [ ] **ROBUST-004** Convert parser panics to typed errors.
- [ ] **ROBUST-005** Add fuzz targets for tokenizer and object parser.
- [ ] **ROBUST-006** Add fuzz targets for content-stream interpretation.
- [ ] **ROBUST-007** Add fuzz targets for CMap parsing.
- [ ] **ROBUST-008** Add fuzz targets for TrueType, Type 1, and CFF parsing.
- [ ] **ROBUST-009** Add fuzz targets for text grouping.
- [ ] **ROBUST-010** Add fuzz targets for edge intersection and table reconstruction.
- [ ] **ROBUST-011** Seed fuzzing with the upstream, Rust, PDF.js, PDFBox, and OSS-Fuzz fixture corpora where licenses permit.
- [ ] **ROBUST-012** Add maximum decompressed-stream-size limits.
- [ ] **ROBUST-013** Add maximum object count and recursion depth limits.
- [ ] **ROBUST-014** Add maximum content-operator, glyph, path-segment, image, annotation, and table-intersection limits.
- [ ] **ROBUST-015** Add cycle detection for indirect objects, page trees, Form XObjects, and structure trees.
- [ ] **ROBUST-016** Add timeouts or work budgets for worst-case table geometry.
- [ ] **ROBUST-017** Test decompression bombs and adversarially fragmented edge grids.
- [ ] **ROBUST-018** Test deeply nested arrays/dictionaries and cyclic references.
- [ ] **ROBUST-019** Test huge coordinates, NaN, infinity, denormals, and arithmetic overflow.
- [ ] **ROBUST-020** Ensure malformed UTF-16, glyph names, and CMaps cannot panic.
- [ ] **ROBUST-021** Run AddressSanitizer and UndefinedBehaviorSanitizer where dependencies permit.
- [ ] **ROBUST-022** Run Miri on suitable core algorithms.
- [ ] **ROBUST-023** Add `cargo audit` and dependency-policy checks.
- [ ] **ROBUST-024** Add a security policy and private vulnerability reporting instructions.
- [ ] **ROBUST-025** Ensure logs never include passwords, full document contents, or sensitive metadata by default.
- [ ] **ROBUST-026** Add deterministic resource-limit tests in Rust, Python, and WASM.
- [ ] **ROBUST-027** Add a corpus regression test asserting no first-party panic.
- [ ] **ROBUST-028** Preserve error source chains and page/object context.
- [ ] **ROBUST-029** Ensure Python callback exceptions are propagated without corrupting native state.
- [ ] **ROBUST-030** Verify cancellation/interrupt behavior for long Python operations.

### 8.19 P1 — Continuous Integration and Release Engineering

- [ ] **CI-001** Include `pdfplumber-py` in ordinary CI rather than excluding it.
- [ ] **CI-002** Include `pdfplumber-wasm` in ordinary CI rather than excluding it.
- [ ] **CI-003** Run Rust formatting, Clippy, unit, integration, documentation, and feature-matrix tests.
- [ ] **CI-004** Test the Minimum Supported Rust Version and stable Rust.
- [ ] **CI-005** Test `--no-default-features`, `serde`, `parallel`, and relevant feature combinations.
- [ ] **CI-006** Build and test Python wheels on the declared platform/version matrix.
- [ ] **CI-007** Run Python API-contract and differential tests from installed wheels, not only source trees.
- [ ] **CI-008** Run upstream tests against the installed compatibility package.
- [ ] **CI-009** Build and test the source distribution in a clean environment.
- [ ] **CI-010** Build and test WASM in Node and at least one browser runner.
- [ ] **CI-011** Run compatibility corpus tests with cached, checksum-verified fixtures.
- [ ] **CI-012** Publish parity and benchmark artifacts for pull requests.
- [ ] **CI-013** Prevent release when any P0 gate is incomplete.
- [ ] **CI-014** Validate that all workspace crate versions and dependency versions are publishable together.
- [ ] **CI-015** Replace fixed `sleep 30` registry waits with retry/poll logic.
- [ ] **CI-016** Add release dry-run jobs for crates, wheels, source distribution, and Node package.
- [ ] **CI-017** Smoke-test every produced artifact before publication.
- [ ] **CI-018** Use package-index trusted publishing where available.
- [ ] **CI-019** Generate checksums, a Software Bill of Materials (SBOM), and build provenance.
- [ ] **CI-020** Sign or attest release artifacts.
- [ ] **CI-021** Verify license files are included in every artifact.
- [ ] **CI-022** Verify README install snippets use the current version and correct package names.
- [ ] **CI-023** Add a changelog gate for public API changes.
- [ ] **CI-024** Add a SemVer/API compatibility check for the Rust crates.
- [ ] **CI-025** Add a Python API snapshot diff to release pull requests.
- [ ] **CI-026** Add post-publish installation smoke tests from crates.io, the Python Package Index, and the Node Package Manager registry.
- [ ] **CI-027** Verify `__version__` and package metadata after installation.
- [ ] **CI-028** Ensure the compatibility executable and Rust-extension executable do not overwrite each other unexpectedly.
- [ ] **CI-029** Protect `main` and mark the compatibility, format, lint, and test jobs as required status checks. Until this exists, "CI gated" in section 5 means "a job ran", not "a job had to pass", and every evidence claim at that level is weaker than it reads.

### 8.20 P2 — Performance and Memory

- [ ] **PERF-001** Build a reproducible benchmark corpus covering text-only, graphics-heavy, table-heavy, CJK, RTL, image-heavy, encrypted, malformed, and large PDFs.
- [ ] **PERF-002** Benchmark Python `pdfplumber` v0.11.10 and `pdfplumber-rs` with identical inputs and options.
- [ ] **PERF-003** Measure wall time, CPU time, peak resident memory, allocations, and output size.
- [ ] **PERF-004** Separate parse time, page materialization, text grouping, table detection, serialization, and Python conversion.
- [ ] **PERF-005** Measure cold and warm/cache-enabled runs.
- [ ] **PERF-006** Measure single-page and full-document workloads.
- [ ] **PERF-007** Measure path, bytes, and file-like input overhead.
- [ ] **PERF-008** Measure Python dictionary-conversion overhead.
- [ ] **PERF-009** Release the GIL for long native work and measure multithreaded scaling.
- [ ] **PERF-010** Avoid cloning full page object collections for every property access.
- [ ] **PERF-011** Avoid eager construction of all Python page objects for large documents unless compatibility requires it.
- [ ] **PERF-012** Add bounded caches and cache-clearing memory tests.
- [ ] **PERF-013** Benchmark crop/filter derived pages without unnecessary full-page copies.
- [ ] **PERF-014** Benchmark table intersection complexity on adversarial grids.
- [ ] **PERF-015** Store benchmark baselines and fail on unexplained regressions greater than 10%.
- [ ] **PERF-016** Require compatibility-mode median runtime to be no slower than the target Python implementation on the representative corpus.
- [ ] **PERF-017** Require any published speedup or memory claim to reference a reproducible benchmark artifact.
- [ ] **PERF-018** Remove or qualify unverified "5–20x" and "3–10x" claims until evidence is published.
- [ ] **PERF-019** Benchmark WASM bundle size, startup time, and memory.
- [ ] **PERF-020** Document when parallel page processing changes memory use or output ordering.

### 8.21 P2 — Documentation, Migration, and Ecosystem Quality

- [ ] **DOC-001** Add a versioned compatibility matrix by Python `pdfplumber` release.
- [ ] **DOC-002** Document the exact meaning of "compatible", "extension", and "approved deviation".
- [ ] **DOC-003** Add a migration guide from Python `pdfplumber`.
- [ ] **DOC-004** Add a migration guide from the current pre-parity `pdfplumber-rs` Python binding.
- [ ] **DOC-005** Document 1-based Python page numbers versus zero-based Rust indexes.
- [ ] **DOC-006** Document coordinate systems and page boxes with diagrams.
- [ ] **DOC-007** Document crop semantics and the distinction from the old rebased-center Rust behavior.
- [ ] **DOC-008** Document every text option with compatible examples.
- [ ] **DOC-009** Document every table setting with compatible examples.
- [ ] **DOC-010** Document object dictionary schemas.
- [ ] **DOC-011** Document visual-debugging dependencies and behavior.
- [ ] **DOC-012** Document error types and resource limits.
- [ ] **DOC-013** Document supported encryption and repair behavior.
- [ ] **DOC-014** Document parser/font limitations with fixture references.
- [ ] **DOC-015** Document Rust-only extensions in a separate section.
- [ ] **DOC-016** Ensure README architecture lists all six workspace crates, not only three.
- [ ] **DOC-017** Fix stale crate-version examples in README.
- [ ] **DOC-018** Generate Python API reference documentation from the compatibility shim and type stubs.
- [ ] **DOC-019** Generate Rust API documentation with parity notes where names differ.
- [ ] **DOC-020** Add runnable examples to CI.
- [ ] **DOC-021** Add a troubleshooting guide for tables, fonts, passwords, malformed PDFs, and scanned documents.
- [ ] **DOC-022** Add a reproducible performance methodology document.
- [ ] **DOC-023** Add a contributor guide for creating minimal PDF fixtures.
- [ ] **DOC-024** Add a checklist for sanitizing and licensing user-provided PDFs.
- [ ] **DOC-025** Add release notes that list external contributors and compatibility changes.
- [ ] **DOC-026** Add issue templates that require versions, options, expected upstream output, actual Rust output, and a minimal fixture.
- [ ] **DOC-027** Clearly state that OCR is out of scope and recommend a composable OCR workflow.
- [ ] **DOC-028** Document platform/Python support based on tested artifacts, not classifiers alone.

### 8.22 P3 — Rust-Native Extensions

These items do not block Python parity but should remain well-tested and clearly separated.

- [ ] **EXT-001** Stabilize image extraction/export APIs.
- [ ] **EXT-002** Stabilize bookmarks/outlines APIs.
- [ ] **EXT-003** Stabilize form-field APIs.
- [ ] **EXT-004** Stabilize digital-signature inspection APIs.
- [ ] **EXT-005** Stabilize HTML export.
- [ ] **EXT-006** Stabilize SVG export and table-debug SVG.
- [ ] **EXT-007** Stabilize semantic character/structure traversal.
- [ ] **EXT-008** Stabilize validation and warning APIs.
- [ ] **EXT-009** Stabilize table quality and merged-content normalization.
- [ ] **EXT-010** Stabilize automatic/explicit multi-column reading-order APIs.
- [ ] **EXT-011** Stabilize parallel page processing with deterministic order.
- [ ] **EXT-012** Stabilize the WASM API and TypeScript declarations.
- [ ] **EXT-013** Ensure every extension has Rust, Python-extension, and WASM exposure only where intentionally supported.
- [ ] **EXT-014** Namespace extensions so future upstream additions cannot collide silently.
- [ ] **EXT-015** Document compatibility implications of enabling extensions.

---

## 9. Known Open-Issue Mapping

| Issue | Gap | Primary tasks | Completion rule |
|---|---|---|---|
| `#217` | Stale ignored cross-validation tests | `PARITY-024` | Passing cases are unignored and normal CI remains green |
| `#218` | 90°/270° text newline/order artifact | `TEXT-BUG-218`, `PAGE-023` | All rotation integration tests pass without ignore |
| `#219` | Identity no-embed decoding/word failure | `TEXT-BUG-219`, `FONT-022` | Exact or approved-delta parity for chars and words |
| `#220` | Tagged TrueType extraction gap | `TEXT-BUG-220`, `FONT-003`, `SEM-STRUCT-*` | Fixture reaches compatibility gate and is unignored |
| `#221` | `issue-848` words and tables | `TEXT-BUG-221`, `TABLE-BUG-221` | Word and table outputs meet exact/approved-delta gate |
| `#222` | Residual near-100% deltas | `TEXT-BUG-222`, `PARITY-026` | Every delta is fixed or approved with root cause |
| `#223` | Rotated NICS table parity | `TABLE-BUG-223`, `PAGE-023` | Rotated and non-rotated results pass table gate |
| `#285` | Rotated table cell order | `TEXT-BUG-285A`, `TABLE-BUG-285` | Cell order matches upstream without corpus regression |
| `#286` | Standard table not detected | `TABLE-BUG-286` | Redistributable reproducer exists and passes |

When an issue is closed, add the closing pull request and tests to the Evidence Ledger. Do not remove the row.

---

## 10. Required Test Commands

Agents should run the smallest relevant set first, then the full required tier before checking a task. Update commands as the repository evolves.

### 10.1 Rust baseline

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p pdfplumber --test cross_validation -- --nocapture
```

### 10.2 Python compatibility

```bash
python -m venv .venv-compat
. .venv-compat/bin/activate
python -m pip install --upgrade pip
maturin develop --manifest-path crates/pdfplumber-py/Cargo.toml
python -m pytest -n auto crates/pdfplumber-py/tests
python -m pytest -n auto compat/tests
python -m pytest -n auto compat/upstream-tests
python compat/api_contract.py
python compat/differential.py --all-pages --all-options
```

Use the platform-appropriate activation command on Windows.

### 10.3 WebAssembly

```bash
wasm-pack test --node crates/pdfplumber-wasm
wasm-pack build --target bundler crates/pdfplumber-wasm
```

### 10.4 Robustness

```bash
cargo audit
cargo test --workspace --release
cargo fuzz run tokenizer
cargo fuzz run content_stream
cargo fuzz run cmap
cargo fuzz run font
cargo fuzz run table
```

The exact fuzz target names may change. Keep this section synchronized with the repository.

### 10.5 Release smoke tests

```bash
cargo publish --dry-run -p pdfplumber-core
cargo publish --dry-run -p pdfplumber-parse
cargo publish --dry-run -p pdfplumber
cargo publish --dry-run -p pdfplumber-cli
maturin build --release --manifest-path crates/pdfplumber-py/Cargo.toml
maturin sdist --manifest-path crates/pdfplumber-py/Cargo.toml
```

---

## 11. Milestone Gates

### M0 — Deterministic Baseline

- [ ] Exact upstream version and dependency lock are pinned.
- [ ] API snapshot and all-page differential harness run in CI.
- [ ] Upstream tests are imported and categorized.
- [ ] Approved-delta registry exists.
- [ ] No stale ignored compatibility test remains.

### M1 — Rust Core Semantic Parity

- [ ] Page geometry and object schemas pass the compatibility corpus.
- [ ] Text and word option matrices pass.
- [ ] Table option matrices pass.
- [ ] Crop/filter semantics pass.
- [ ] Open parser/font issues meet exact or approved-delta gates.
- [ ] No first-party panic occurs on the corpus.

### M2 — Python Documented API Replacement

- [ ] `import pdfplumber` exposes the compatible package tree.
- [ ] Upstream README examples run unchanged.
- [ ] `PDF`, `Page`, derived pages, text, tables, search, serialization, and visual debugging pass.
- [ ] Supported wheels and source distribution pass installed-artifact tests.
- [ ] Python page numbering, properties, callbacks, context manager, and exceptions match.

### M3 — Full Public-Surface and CLI Compatibility

- [ ] Public utility modules and classes pass API snapshots.
- [ ] Structure-tree and annotation APIs pass.
- [ ] Command-Line Interface output and flags pass subprocess differential tests.
- [ ] Deep `pdfminer.six` compatibility is implemented or every unresolved surface is explicitly documented and excluded from the claim.
- [ ] No unapproved deviation remains.

### M4 — Production 1.0 Readiness

- [ ] Security, fuzzing, resource limits, and dependency checks are green.
- [ ] Performance and memory baselines are published.
- [ ] All released artifacts are smoke-tested and attested.
- [ ] Documentation and migration guides are complete.
- [ ] Every P0 and P1 task is checked with evidence.
- [ ] Maintainer review explicitly approves the compatibility claim.

---

## 12. Active Work

Agents must claim a task before implementation.

| Task ID | Owner/Agent | Branch/Worktree | Started | Blockers | Notes |
|---|---|---|---|---|---|
| `PARITY-002` | Codex | `feat/golden-provenance` / `../pdfplumber-rs-golden-provenance` | 2026-08-17 | `CI-001`, `CI-002`, `CI-003` | Regenerated the corpus with pinned CPython 3.13.12 and upstream 0.11.10: 98 succeeded, 10 malformed/password inputs failed, and none of those failures retained a stale artifact. All 97 prior artifacts were stamped and `oss-fuzz/5914823472250880.json` was added. Excluding version/provenance metadata, only five 0.0001-point table-bbox values changed across `chelsea_pdta`, `issue-67-example`, and `issue-71-duplicate-chars-2`; text, table counts, and row counts did not change. The task remains unchecked because section 10's all-target/all-feature Clippy command fails on unrelated existing lints and the all-feature workspace test cannot link `pdfplumber-py` as a macOS Rust test binary. No lint was allowed, test skipped, threshold changed, or tolerance widened. |
| `PARITY-005` | Codex | `feat/public-api-snapshot` / `../pdfplumber-rs-api-snapshot` | 2026-08-17 | `CI-001`, `CI-002`, `CI-003` | The pinned v0.11.10 snapshot covers all 20 importable modules, 452 exports, 27 canonically defined classes, and 401 class members, including properties, cached properties, method/data descriptors, call signatures, constants, inherited public members, and the declared-but-missing top-level `set_debug`. Generation and `--check` are byte-deterministic across patch releases in the pinned Python 3.13 series, and CI verifies drift using `.venv-reference`. The task remains unchecked solely because section 10's existing all-target/all-feature failures remain; no lint was allowed, test skipped, threshold changed, or tolerance widened. |
| `PARITY-006` | Codex | `feat/api-call-contracts` / `../pdfplumber-rs-api-contracts` | 2026-08-17 | `PYAPI-002`, `CI-001`, `CI-002`, `CI-003` | Added 12 deterministic behavioral cases spanning positional, keyword, keyword-only, default, invalid-argument, and exception-type behavior. The artifact binds to the pinned target, lock digest, configured interpreter series, and fixture hash; CI executes the reference drift gate. Candidate mode compares only case outcomes and rejects an upstream import, but cannot be exercised until `PYAPI-002` provides an isolated compatibility package. The task remains unchecked; no lint was allowed, test skipped, threshold changed, or tolerance widened. |
| `PARITY-007` | Codex | `feat/all-page-comparison` / `../pdfplumber-rs-all-pages` | 2026-08-17 | `PARITY-012`, `CI-001`, `CI-002`, `CI-003` | Replaced first-page extraction with independently counted, per-page Python/Rust documents; full-corpus reporting retained 80 corpus-relative fixture IDs and compared 288 pages across 79 openable documents with no page-count mismatch. The password fixture remains an explicit failure pending credential-aware differential coverage. The task remains unchecked; no failure was skipped, tolerance widened, threshold lowered, or filename/page-specific workaround added. |
| `PARITY-008` | Codex | `feat/ordered-sequence-comparison` / `../pdfplumber-rs-ordered-sequences` | 2026-08-17 | `TEXT-WORD-020`, `TEXT-BUG-221`, `PARITY-012`, `PARITY-026`, `CI-001`, `CI-002`, `CI-003` | Replaced greedy character, word, line, and rectangle matching with index-sensitive comparison while retaining the existing identity and coordinate rules. The full report exposed word-sequence differences in `issue-1279-example.pdf`, `issue-848.pdf`, and `pr-136-example.pdf`; character-box sequences are exactly equal on 222/288 pages and word sequences on 243/288. The password fixture remains an explicit failure. The task stays unchecked; no tolerance or threshold changed, no test was skipped, and no fixture-specific branch was added. |
| `PARITY-009` | Codex | `feat/exact-dictionary-comparison` / `../pdfplumber-rs-exact-dictionaries` | 2026-08-17 | `OBJ-001`–`OBJ-008`, `OBJ-CHAR-004`–`OBJ-CHAR-010`, `OBJ-WORD-001`, `PARITY-012`, `PARITY-026`, `CI-001`, `CI-002`, `CI-003` | Added recursive exact comparison for dictionary key sets and spelling, runtime value types, nested containers, and explicit `None` placement before value-level normalization or tolerance. The full report found 0/442,036 structurally matching non-empty characters and 0/75,480 non-empty words; table structure matched on all 288 compared pages. The task stays unchecked for those exposed schema gaps and existing strict-tier blockers; no key was normalized away, `None` conflated with `""`, tolerance or threshold changed, test skipped, or fixture-specific branch added. |
| `PARITY-010` | Codex | `feat/multi-api-parity-comparison` / `../pdfplumber-rs-multi-api-comparison` | 2026-08-17 | `TEXT-MAP-002`, `TEXT-EXTRA-001`–`TEXT-EXTRA-004`, `TEXT-SEARCH-001`–`TEXT-SEARCH-012`, `OBJ-WORD-001`–`OBJ-WORD-005`, `OBJ-ANNOT-001`–`OBJ-ANNOT-004`, `SEM-STRUCT-001`–`SEM-STRUCT-015`, `PARITY-012`, `PARITY-026`, `CI-001`–`CI-003` | The report now calls and compares all ten requested APIs per page. The full pinned run retained 80 entries, compared 271 pages in 76 documents, preserved four reference-side failures, and marked all 271 missing Rust simple-text results unsupported; process status is nonzero for unsupported APIs. Exact results remain intentionally visible rather than thresholded: page text 265/271, layout 4/271, simple text 0/271, text lines 4/271, words 4/271, search 4/271, tables 269/271, annotations 236/271, hyperlinks 246/271, and structure trees 188/271. The task stays unchecked; no API, fixture, page, mismatch, threshold, tolerance, or dictionary field was skipped or weakened. |
| `PARITY-011` | Codex | `feat/option-matrix-runner` / `../pdfplumber-rs-option-matrix` | 2026-08-17 | `CI-001`, `CI-002`, `CI-003` | Added 161 pinned-upstream cases covering 111 API-scoped text and 50 table contracts. Every case produced a complete reference result; structural tests reject missing options, non-default values, results, fixture hashes, or provenance, and CI checks byte-for-byte regeneration across patch releases in the pinned Python 3.13 series. The task stays unchecked solely because the pre-existing section 10 all-target/all-feature gates remain red; no option, result, warning, threshold, tolerance, fixture, or page was skipped or weakened. |
| `PARITY-012` | Codex | `test/error-behavior-parity` / `../pdfplumber-rs-error-behavior` | 2026-08-17 | `PYAPI-002`, `CI-001`, `CI-002`, `CI-003` | Added 23 pinned-upstream cases across exceptions, warnings, malformed metadata, passwords, repair, closed resources, and invalid bounding boxes. The contract preserves exact exception type/message/arguments and failure phase, warning and logger output, return/resource state, redacted invocations, and fixture/inline-resource hashes. CI rejects reference drift across patch releases in the pinned Python 3.13 series; candidate mode compares every ordered outcome and rejects an upstream import, but cannot run until `PYAPI-002` supplies the isolated compatibility package. The task stays unchecked; no outcome, phase, warning, resource state, threshold, tolerance, or fixture was skipped or weakened. |
| `PARITY-013` | Codex | `test/serialization-differential` / `../pdfplumber-rs-serialization-differential` | 2026-08-17 | `PYAPI-002`, `CI-001`, `CI-002`, `CI-003` | Added 28 exact-output cases: 15 JSON and 13 CSV across `PDF`/`Page`, two materially different fixtures, return and `StringIO` paths, precision 0/3, indentation, include/exclude filters and their exact failures, and character-only/empty object selections. The 758,879-byte artifact retains every raw byte and SHA-256 plus target/environment/fixture provenance; CI rejects drift across patch releases in the pinned Python 3.13 series, while candidate mode refuses upstream self-comparison. The task stays unchecked until `PYAPI-002` supplies a candidate and strict section 10 gates pass; nothing was skipped or normalized. |
| `PARITY-014` | Codex | `test/upstream-suite-runner` / `../pdfplumber-rs-upstream-suite` | 2026-08-17 | `PYAPI-002`, `PARITY-015`, `CI-001`, `CI-002`, `CI-003` | Added exact source-bundle, hash-locked tool, installed-candidate-origin, xdist result, external-command, and failure-classification contracts. The reference baseline collected 171 tests: 165 passed and six repair tests remained visible failures because `gs` is absent. No candidate run or unsupported entry was fabricated while `PYAPI-002` is open. The task remains unchecked, and no test, failure, threshold, or tolerance was weakened. |
| `PARITY-015` | Codex | `test/unsupported-task-links` / `../pdfplumber-rs-unsupported-task-links` | 2026-08-17 | `CI-001`, `CI-002`, `CI-003` | Added section-8-scoped task-state validation to upstream-suite preflight. Synthetic checked, unchecked, and unknown links prove the gate; the current empty manifest passes without fabricating a compatibility gap. The task remains unchecked for strict section 10 failures, and pytest selection/outcomes are unchanged. |
| `PARITY-016` | Codex | `test/approved-delta-gate` / `../pdfplumber-rs-approved-deltas` | 2026-08-17 | `PARITY-017`, `PARITY-018`, `PARITY-023`, `PARITY-026`, `CI-001`, `CI-002`, `CI-003` | Added an empty target-bound approval registry, complete-entry schema, type-preserving result digests, and exact approved/unregistered/stale evaluation. Full corpus: 80 entries, 76 documents, 271 pages, 1,486 unregistered, 0 approved/stale, 0 digest failures, exit 1. CI validates the registry and gate contracts; the task stays unchecked until the all-page report itself is a CI execution gate and section 10 passes. Current Rust 1.94 Clippy also exposes six pre-existing excluded-scope lints. |
| `PARITY-017` | Codex | `test/machine-readable-parity-report` / `../pdfplumber-rs-machine-report` | 2026-08-17 | `PYAPI-002`, `PARITY-018`, `PARITY-023`, `PARITY-026`, `CI-001`, `CI-002`, `CI-003` | Added deterministic schema-v1 JSON with target/environment state, 80 fixture records, 271 pages, 2,981 page-API records, exact delta dispositions, and all 161 option cases. The candidate option producer and comparison reject upstream self-comparison, incomplete/mutated identities, and invalid content digests. All option cases remain explicitly blocked by `PYAPI-002`, the corpus remains red, and strict section 10 is not green, so the task stays unchecked. |
| `PARITY-018` | Codex | `test/human-readable-parity-summary` / `../pdfplumber-rs-human-summary` | 2026-08-17 | `PARITY-023`, `PARITY-026`, `CI-001`, `CI-002`, `CI-003` | Added deterministic Markdown derived from the complete machine model, including stable first fixture/page/API selection, exact object index and JSON-safe values, bounded text context, signed coordinate deltas, and explicit unchanged-coordinate evidence. Full corpus: 80 fixtures, 271 pages, 2,981 API results, 1,486 first-difference records, four existing failures, and 1,486 unregistered deltas. The task stays unchecked until artifacts are CI-gated and section 10 passes. |
| `PARITY-019` | Codex | `test/prd-evidence-linter` / `../pdfplumber-rs-prd-linter` | 2026-08-17 | `CI-001`, `CI-002`, `CI-003` | Added a deterministic, fenced-code-aware linter for unique section-8 task definitions and section-13 evidence coverage. The standalone gate reports 721 tasks, two checked tasks, and 19 evidence rows and now runs in compatibility CI. The task stays unchecked because the existing strict section 10 failures remain; no task state, evidence, test, threshold, tolerance, fixture, or ignore was weakened. |
Remove the active row after merge and add a permanent Evidence Ledger row.

---

## 13. Evidence Ledger

No checklist item outside the source-level inventory may be marked `[x]` without an evidence row.

| Task ID | Date | Agent | Commit or PR | Tests / artifacts | Notes |
|---|---|---|---|---|---|
| `AUDIT-BASELINE` | 2026-08-16 | GPT-5.6 Pro | Rust `da0663ce`; Python `v0.11.10` | Source/API audit used to create this PRD | Source-presence checks are not parity completion |
| `PARITY-001` | 2026-08-16 | Claude Opus 5 | PR #291 | `compat/requirements-golden.txt` pins 9 packages with 435 SHA-256 hashes covering every published file. `bash scripts/setup_golden_venv.sh` installs it with `pip install --require-hashes` and resolves `pdfplumber 0.11.10`. Corrupting the hashes of one locked package makes that install fail with `THESE PACKAGES DO NOT MATCH THE HASHES`, so the lock is a gate rather than a record. `python -m unittest discover -s compat/tests -t .` (36 tests) asserts every requirement is `==`-pinned, hashed, unique, and sorted. Runs on every pull request in the `Compatibility harness` CI job. | `main` has no branch protection, so no CI job is enforced yet; see `CI-029`. Regenerate the lock with `python3 scripts/lock_golden_env.py`; `--check` fails when the committed lock is stale. Markers are derived from the dependency graph, so `cffi` and `pycparser` stay PyPy-gated and `typing_extensions` stays gated to Python < 3.11 |
| `PARITY-003` | 2026-08-16 | Claude Opus 5 | PR #291 | `scripts/setup_golden_venv.sh` now rebuilds `.venv-reference` from the lock and then runs `scripts/verify_compat_env.py --reference --expect-root`, which fails on a version mismatch, a compiled module, or an import from outside the environment. Verified locally end to end; CI runs the same script on the pinned interpreter. | Replaces `pip install pdfplumber`, which resolved against whatever PyPI served that day. The interpreter is pinned too, since `pdfminer.six` and Pillow behave differently across Python releases |
| `PARITY-002` | 2026-08-17 | Codex | PR #292 | Red: `python3 -m unittest compat.tests.test_golden_artifacts -v` → `FAILED (failures=1)`, 97 artifacts missing provenance. Generation: `PDFPLUMBER_RS_REFERENCE_PYTHON="$(uv python find 3.13)" PDFPLUMBER_RS_REQUIRE_PINNED_PYTHON=1 bash scripts/setup_golden_venv.sh` → reference environment OK, pdfplumber 0.11.10; `.venv-reference/bin/python scripts/generate_golden.py` → `98 succeeded, 10 failed`. Green: `python3 -m unittest discover -s compat/tests -t . -v` → 37 tests, OK; `cargo test -p pdfplumber --test cross_validation -- --nocapture` → 107 passed, 2 ignored; `cargo test -p pdfplumber --tests --quiet` → 130 passed; `cargo check --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm`, `cargo clippy --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm -- -D warnings`, `cargo test --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm --quiet`, `cargo fmt --all -- --check` → exit 0. | The checklist stays open: `cargo clippy --workspace --all-targets --all-features -- -D warnings` fails on pre-existing all-target lints, while `PYO3_PYTHON="$PWD/.venv-reference/bin/python" cargo test --workspace --all-features --quiet` fails at the macOS `pdfplumber-py` link step with undefined Python symbols. These are tracked by `CI-001`, `CI-002`, and `CI-003`; no intentional deviation was added to section 14. |
| `PARITY-005` | 2026-08-17 | Codex | PR #293 | Initial red: `python3 -m unittest compat.tests.test_api_snapshot -v` → `FAILED (failures=1)`, missing `compat/snapshots/pdfplumber-v0.11.10-api.json`. CI red after retargeting to `main`: run `32010389900`, `.venv-reference/bin/python scripts/generate_api_snapshot.py --check` → exit 1 because the committed patch version was `3.13.12` and CI used `3.13.15`. Regression red: `python3 -m unittest compat.tests.test_api_snapshot -v` → `FAILED (failures=1)`, `'3.13.12' != '3.13'`. Generation: `.venv-reference/bin/python scripts/generate_api_snapshot.py` → wrote the snapshot from pinned pdfplumber 0.11.10. Green: the focused test → 1 test, OK; `.venv-reference/bin/python scripts/generate_api_snapshot.py --check` → snapshot current; `python3 -m unittest discover -s compat/tests -t . -v` → 38 tests, OK; `cargo test -p pdfplumber --test cross_validation -- --nocapture` → 100 passed, 9 ignored; `cargo check --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm`, `cargo clippy --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm -- -D warnings`, `cargo test --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm --quiet`, `cargo fmt --all -- --check`, and `git diff --check` → exit 0. | Snapshot: 20 modules, 452 exports, 27 canonical classes, 401 members; `pdfplumber.set_debug` is recorded as declared in `__all__` but missing upstream. The environment field now records the configured Python `3.13` series instead of an unstable patch release. The checklist stays open: strict all-target/all-feature Clippy fails on existing lints and ambient Python 3.14, while the pinned-Python all-feature test fails at the existing macOS `pdfplumber-py` undefined-symbol link step. Tracked by `CI-001`, `CI-002`, and `CI-003`; section 14 is unchanged because there was no intentional deviation. |
| `PARITY-006` | 2026-08-17 | Codex | PR #294 | Initial red: `python3 -m unittest compat.tests.test_api_contract -v` → `FAILED (failures=1)`, missing `compat/contracts/pdfplumber-v0.11.10-calls.json`. CI red after retargeting to `main`: run `32012081574`, `.venv-reference/bin/python compat/api_contract.py` → exit 1 because the committed patch version was `3.13.12` and CI used `3.13.15`. Regression red: `python3 -m unittest compat.tests.test_api_contract -v` → `FAILED (failures=1)`, `'3.13.12' != '3.13'`. Generation: `.venv-reference/bin/python compat/api_contract.py --write-reference` → wrote 12 cases from pinned pdfplumber 0.11.10. Green: the focused test → 1 test, OK; `.venv-reference/bin/python compat/api_contract.py --reference` → contract current; forced `--candidate` under the reference venv → rejected upstream self-comparison; `python3 -m unittest discover -s compat/tests -t . -v` → 39 tests, OK; `cargo test -p pdfplumber --test cross_validation -- --nocapture` → 100 passed, 9 ignored; `cargo check --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm`, `cargo clippy --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm -- -D warnings`, `cargo test --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm --quiet`, `cargo fmt --all -- --check`, and `git diff --check` → exit 0. | Covers six required call categories with exact return projections, processed defaults, binding messages, and exception classes. The environment field records the configured Python `3.13` series instead of an unstable patch release. Candidate comparison remains unexecuted pending `PYAPI-002`. Strict Clippy still fails on existing all-target lints/ambient Python 3.14, and pinned-Python all-feature tests still fail at the existing macOS `pdfplumber-py` undefined-symbol link step. Tracked by `CI-001`/`CI-002`/`CI-003`; section 14 is unchanged because there was no intentional deviation. |
| `PARITY-007` | 2026-08-17 | Codex | PR #295 | Red: `.venv-reference/bin/python -m unittest compat.tests.test_parity_report_pages -v` → `ERROR`, `KeyError: 'page_count'` on the 7-page fixture; after duplicate-name discovery the same focused tier → `ERROR`, missing `fixture_id`. Green: `.venv-reference/bin/python -m unittest compat.tests.parity_report_pages_test -v` → 4 tests, OK; `python3 -m unittest discover -s compat/tests -t . -v` → 39 tests, OK. End to end: `scripts/parity_report.py --only rotated_pages.pdf --json ...` → pages 1–4 reported and JSON retained `[1, 2, 3, 4]`; `--only pdffill-demo.pdf --json ...` → both corpus-relative fixture IDs retained with pages 1–7. Full corpus: `.venv-reference/bin/python scripts/parity_report.py --repo "$PWD" --fixtures "$PWD" --json ...` → 80 entries, 79 compared documents, 288 compared pages, 0 page-count mismatches, exit 1 for the password fixture. `cargo test -p pdfplumber --test cross_validation -- --nocapture` → 100 passed, 9 ignored. `cargo check --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm`, excluded-scope Clippy with `-D warnings`, `cargo test --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm --quiet`, `cargo fmt --all -- --check`, and `git diff --check` → exit 0. | The report now preserves page boundaries, checks Python/Rust page counts independently, emits per-page rows/JSON, retains duplicate basenames by relative path, and returns nonzero instead of hiding incomplete processing. `password-example.pdf` remains `python_failed` pending `PARITY-012`; it was not treated as success. Strict all-target/all-feature Clippy exits 101 on ambient Python 3.14 versus PyO3 0.24.2 and existing all-target lints; pinned-Python all-feature tests exit 101 at the existing macOS undefined-symbol link step. Tracked by `CI-001`/`CI-002`/`CI-003`; section 14 is unchanged because there was no intentional deviation. |
| `PARITY-008` | 2026-08-17 | Codex | PR #296 | Red: `cargo test -p pdfplumber --test cross_validation ordered_sequence_matching -- --nocapture` → exit 101 with two `E0425` errors because `match_ordered_sequence` did not exist; `.venv-reference/bin/python -m unittest compat.tests.ordered_sequence_test -v` → 2 errors, missing `order_equal`. Green: the same Rust command → 2 passed; `.venv-reference/bin/python -m unittest compat.tests.parity_report_pages_test compat.tests.ordered_sequence_test -v` → 6 tests, OK; `python3 -m unittest discover -s compat/tests -t . -v` → 39 tests, OK. `cargo test -p pdfplumber --test cross_validation -- --nocapture` → 102 passed, 9 ignored while its diagnostic summary retained 3 failures and 10 errors. Full corpus: `.venv-reference/bin/python scripts/parity_report.py --repo "$PWD" --fixtures "$PWD" --json ...` → 80 entries, 79 documents, 288 pages, 222 exact character-box sequences, 243 exact word sequences, 0 page-count mismatches, exit 1 only for `password-example.pdf`. `cargo check --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm`, excluded-scope Clippy with `-D warnings`, `cargo test --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm --quiet`, `cargo fmt --all -- --check`, and `git diff --check` → exit 0. | Positional matching preserves the existing object identity and coordinate predicates, counts additions on either side, and exposes order differences instead of greedily matching them elsewhere. The checklist stays open for the visible text/password/exact-gate gaps and section 10 failures. Strict all-target/all-feature Clippy exits 101 on ambient Python 3.14 versus PyO3 0.24.2 plus existing all-target lints; pinned-Python all-feature tests exit 101 at the existing macOS undefined-symbol link step. Tracked by `TEXT-WORD-020`, `TEXT-BUG-221`, `PARITY-012`, `PARITY-026`, and `CI-001`/`CI-002`/`CI-003`; section 14 is unchanged because there was no intentional deviation. |
| `PARITY-009` | 2026-08-17 | Codex | PR #297 | Red: `.venv-reference/bin/python -m unittest compat.tests.dictionary_structure_test -v` → `FAILED (failures=1, errors=5)`: the recursive comparator was absent, full upstream dictionaries were projected to five fields, and table `None` was normalized to `""`. Green: `.venv-reference/bin/python -m unittest compat.tests.parity_report_pages_test compat.tests.ordered_sequence_test compat.tests.dictionary_structure_test -v` → 13 tests, OK; `python3 -m unittest discover -s compat/tests -t . -v` → 39 tests, OK. Differential sample: `scripts/parity_report.py --only basic_text.pdf --json ...` → exit 0; 0/258 character and 0/48 word dictionaries matched structurally, tables matched structurally. Full corpus: `.venv-reference/bin/python scripts/parity_report.py --repo "$PWD" --fixtures "$PWD" --json ...` → 80 entries, 79 documents, 288 pages, 0/442,036 structurally matching character objects, 0/75,480 word objects, four equal empty-object pages for each, 288/288 structurally equal table pages, 0 page-count mismatches, and exit 1 only for `password-example.pdf`. `cargo test -p pdfplumber --test cross_validation -- --nocapture` → 102 passed, 9 ignored with diagnostic 85 PASS/3 FAIL/10 ERROR. Current CI-equivalent check, excluded-scope Clippy with `-D warnings`, workspace tests, formatting, and diff checks → exit 0. | The comparator keeps exact key spelling, distinguishes `bool`/`int`/`float`, dictionary/list/tuple nesting, omitted keys, and positional `None`; scalar values continue through their dedicated comparators. `basic_text.pdf` demonstrates upstream/Rust key counts of 22/11 for characters and 10/8 for words. The checklist stays open for `OBJ-001`–`OBJ-008`, `OBJ-CHAR-004`–`OBJ-CHAR-010`, `OBJ-WORD-001`, `PARITY-012`, `PARITY-026`, and strict-tier failures. All-target/all-feature Clippy exits 101 on ambient Python 3.14 versus PyO3 0.24.2 plus existing lints; pinned-Python all-feature tests exit 101 at the existing macOS undefined-symbol link step. Section 14 is unchanged because there was no intentional deviation. |
| `PARITY-010` | 2026-08-17 | Codex | PR #298 | Red: `.venv-reference/bin/python -m unittest compat.tests.test_multi_api_report -v` → one failure and two errors because the report omitted nine requested API results, still required the old `text` key, and had no unsupported-value comparator; `cargo test -p pdfplumber-cli --test compat_snapshot_cmd -- --nocapture` → exit 101 because `compat-snapshot` was unrecognized; the later process-status test first failed `0 != 1` because an unsupported candidate API was treated as a successful report. Green: the Python focused tier → 4 tests, OK; the Rust focused tier → 2 passed; `.venv-reference/bin/python -m unittest discover -s compat/tests -t . -v` → 43 tests, OK; `cargo test -p pdfplumber-cli --all-targets --quiet` → exit 0; `cargo test -p pdfplumber --test cross_validation -- --nocapture` → 102 passed, 9 ignored. Post-open workflow audit: ambient `python3 -m unittest discover -s compat/tests -t . -v` initially errored because the report imported the not-yet-installed reference package during discovery; after deferring that import until the verified executable path, the same command → 43 tests, OK, and `.venv-reference/bin/python -m unittest compat.tests.parity_report_pages_test compat.tests.ordered_sequence_test compat.tests.dictionary_structure_test compat.tests.test_multi_api_report -v` → 17 tests, OK. Differential sample: `scripts/parity_report.py --only basic_text.pdf --json ...` → page text/tables exact, one unsupported API, exit 1. Full corpus: `.venv-reference/bin/python scripts/parity_report.py --repo "$PWD" --fixtures "$PWD" --json /tmp/pdfplumber-parity010-full.json` → 80 entries, 76 documents, 271 pages, no page-status omissions, 4 reference failures, and exact page counts of 265 page text, 4 layout text, 0 simple text with 271 unsupported failures, 4 text lines, 4 words, 4 search, 269 tables, 236 annotations, 246 hyperlinks, and 188 structure trees. Current CI-equivalent workspace check, excluded-scope Clippy with `-D warnings`, workspace tests, formatting, and diff checks → exit 0. | Full-run failures remain visible: both `annotations-unicode-issues.pdf` copies fail with truncated UTF-16 annotation data, `federal-register-2020-17221.pdf` reaches Python's recursion limit in its structure tree, and `password-example.pdf` remains credential-gated. The checklist stays open for the surfaced text/word/search/annotation/structure gaps, `PARITY-012`, `PARITY-026`, and strict-tier failures. All-target/all-feature Clippy exits 101 on ambient Python 3.14 versus PyO3 0.24.2 plus existing lints; pinned-Python all-feature tests exit 101 at the existing macOS `pdfplumber-py` undefined-symbol link step. No tolerance, threshold, schema, API, page, or fixture was weakened or skipped; section 14 is unchanged because there was no intentional deviation. |
| `PARITY-011` | 2026-08-17 | Codex | PR #299 | Initial red: `python3 -m unittest compat.tests.test_option_matrix -v` → exit 1, `ImportError` because `compat.harness.option_matrix` did not exist. Post-merge regression red: `python3 -m unittest compat.tests.test_option_matrix.OptionMatrixSnapshotTests.test_committed_snapshot_is_complete_and_traceable -v` → `FAILED (failures=1)`, `'3.13.12' != '3.13'`. Generation: `.venv-reference/bin/python scripts/generate_option_matrix.py` → wrote `compat/snapshots/pdfplumber-v0.11.10-option-matrix.json` with 161 successful cases. Green: `python3 -m unittest compat.tests.test_option_matrix -v` → 2 tests, OK; `.venv-reference/bin/python scripts/generate_option_matrix.py --check` → snapshot current, 161 cases; `python3 -m unittest discover -s compat/tests -t . -v` → 45 tests, OK; pinned all-page differential tier → 17 tests, OK; `cargo test -p pdfplumber --test cross_validation -- --nocapture` → 102 passed, 9 ignored. Current CI-equivalent workspace check, excluded-scope Clippy with `-D warnings`, workspace tests, formatting, and diff checks → exit 0. | The snapshot contains 111 API-scoped text and 50 table contracts, all `status=ok`; 46 content-addressed output values retain every normalized result without duplicating identical multi-API behavior, alongside reproducible target/environment/fixture provenance. The environment field records the configured Python `3.13` series instead of an unstable patch release. The checklist stays open: all-target/all-feature Clippy exits 101 on existing lints; ambient all-feature tests exit 101 because Python 3.14 exceeds PyO3 0.24.2's maximum 3.13; forcing pinned Python 3.13 reaches the existing macOS `pdfplumber-py` undefined-symbol link failure. Tracked by `CI-001`/`CI-002`/`CI-003`; no intentional deviation was added to section 14. |
| `PARITY-012` | 2026-08-17 | Codex | PR #300 | Initial red: `python3 -m unittest compat.tests.test_error_contract -v` → exit 1, `ImportError` because `compat.harness.error_contract` did not exist. Review regression: the same focused command → one failure because `0x80` in the exact Unicode decode message had been over-normalized as an address. Post-merge regression red: `python3 -m unittest compat.tests.test_error_contract.ErrorBehaviorContractTests.test_committed_contract_is_complete_and_traceable -v` → `FAILED (failures=1)`, `'3.13.12' != '3.13'`. Generation: `.venv-reference/bin/python compat/error_contract.py --write-reference` → wrote the v0.11.10 contract with 23 cases. Green: `python3 -m unittest compat.tests.test_error_contract -v` → 2 tests, OK; `.venv-reference/bin/python compat/error_contract.py --reference` → contract current; `.venv-reference/bin/python compat/error_contract.py --candidate` → exit 1 because it correctly rejected the upstream reference import; `python3 -m unittest discover -s compat/tests -t . -v` → 47 tests, OK; pinned all-page differential tier → 17 tests, OK; `cargo test -p pdfplumber --test cross_validation -- --nocapture` → 102 passed, 9 ignored. Current CI-equivalent workspace check, excluded-scope Clippy with `-D warnings`, workspace tests, formatting, and diff checks → exit 0. | The contract covers 3 closed-resource, 2 general-exception, 6 bounding-box, 2 malformed-metadata, 3 password, 4 repair, and 3 warning cases with exact ordered outcomes. The environment field records the configured Python `3.13` series instead of an unstable patch release. It stays unchecked because `PYAPI-002` has not supplied a candidate package. Strict all-target/all-feature Clippy exits 101 on ambient Python 3.14 versus PyO3 0.24.2 plus existing lints; ambient all-feature tests exit 101 at the Python-version guard; forcing pinned Python reaches the existing macOS undefined-CPython-symbol link failure. Section 14 is unchanged because there was no intentional deviation. |
| `PARITY-013` | 2026-08-17 | Codex | PR #301 | Initial red: `python3 -m unittest compat.tests.test_serialization_contract -v` → exit 1, `ImportError` because `compat.harness.serialization_contract` did not exist. The first generated-reference run then exposed the exact empty-selection CSV header (`object_type` plus ten prepended geometry fields), and the two-fixture contract test subsequently failed with one failure/three errors until the text-only cases and resource hash were implemented. Post-merge regression red: `python3 -m unittest compat.tests.test_serialization_contract.SerializationContractTests.test_committed_contract_is_complete_and_traceable -v` → `FAILED (failures=1)`, `'3.13.12' != '3.13'`. Generation: `.venv-reference/bin/python compat/serialization_contract.py --write-reference` → wrote 28 cases and 758,879 bytes. Green: `python3 -m unittest compat.tests.test_serialization_contract -v` → 4 tests, OK; `.venv-reference/bin/python compat/serialization_contract.py --reference` → contract current; candidate mode → exit 1 after correctly rejecting the upstream reference import; `python3 -m unittest discover -s compat/tests -t . -v` → 51 tests, OK; inherited pinned differential tier → 17 tests, OK. Current CI-equivalent workspace check, excluded-scope Clippy with `-D warnings`, workspace tests, formatting, and diff checks → exit 0. | Full raw strings cover 15 JSON and 13 CSV cases across mixed character/rectangle/image and text-only fixtures. The environment field records the configured Python `3.13` series instead of an unstable patch release. The task stays unchecked because `PYAPI-002` has not supplied a candidate. Strict all-target/all-feature Clippy exits 101 on ambient Python 3.14 versus PyO3 0.24.2 plus existing lints; ambient all-feature tests exit 101 at the version guard; forcing pinned Python reaches the existing macOS undefined-CPython-symbol link failure. Section 14 is unchanged because there was no intentional deviation. |
| `PARITY-014` | 2026-08-17 | Codex | PR #302 | Red: `python3 -m unittest compat.tests.test_upstream_suite -v` → exit 1, `ImportError` because `compat.harness.upstream_suite` did not exist. The first pinned reference run then exposed one undeclared `examples/pdfs` dependency; a new source-bundle contract failed with `AttributeError: SourceConfig has no attribute suite_paths` before the general 102-file bundle was implemented. Source/tool proof: `uv pip compile --python-version 3.10 --generate-hashes compat/requirements-upstream-tests.in --output-file compat/requirements-upstream-tests.txt` → hash-locked 16-package closure; `python3 scripts/setup_upstream_suite.py --source /tmp/pdfplumber-upstream-suite.LwSXE6` and `--check` → exact commit `7d4f2f5`, test tree `2bb743a`, 102 files, SHA-256 `fbf6ab39...f1248`; hash-required CPython 3.13 dry-run resolved all 16 packages. Green: focused suite contracts → 5 tests, OK; all compatibility tests → 56 tests, OK; inherited pinned differential tier → 17 tests, OK. Direct pinned-reference xdist plumbing collected 171 tests and recorded six failures; the suite itself reported 165 passed and six repair failures because `gs` is absent. `python3 scripts/run_upstream_suite.py` → exit 1 for missing `.venv-candidate`; forcing `.venv-reference` → exit 1 after rejecting upstream self-comparison. Current CI-equivalent workspace check, excluded-scope Clippy with `-D warnings`, workspace tests, formatting, syntax, and diff checks → exit 0. | The task stays unchecked: no installed candidate exists, no candidate failures were observed or entered in `upstream-unsupported.toml`, and local Ghostscript is missing. Strict all-target/all-feature Clippy exits 101 on ambient Python 3.14 plus existing lints; ambient all-feature tests exit 101 at the version guard; pinned-Python all-feature tests exit 101 at the existing macOS undefined-CPython-symbol link step. No upstream node was skipped, deselected, marked xfail, classified as passing, or hidden by tolerance/threshold changes; section 14 is unchanged because there was no intentional deviation. |
| `PARITY-015` | 2026-08-17 | Codex | PR #303 | Red: `python3 -m unittest compat.tests.test_upstream_suite.UpstreamSuiteContractTests.test_unsupported_entries_require_existing_unchecked_prd_tasks -v` → exit 1, `AttributeError` because `validate_unsupported_task_links` did not exist. The first implementation run then correctly rejected the incomplete synthetic document until its section 8/9 boundaries were explicit. Green: focused upstream-suite contracts → 6 tests, OK; `python3 scripts/setup_upstream_suite.py --check` → exact 102-file source bundle current; actual empty-manifest runner preflight reached the separate missing `.venv-candidate` failure; all compatibility tests → 57 tests, OK; rebuilt hash-pinned reference environment and inherited differential tier → 17 tests, OK. Current CI-equivalent workspace check, excluded-scope Clippy with `-D warnings`, workspace tests, formatting, syntax, and diff checks → exit 0. | Synthetic entries prove unchecked `PYAPI-002` passes and checked `PARITY-001`/unknown `UNKNOWN-999` fail before pytest. The task stays unchecked because strict all-target/all-feature Clippy exits 101 on ambient Python 3.14 plus existing lints, ambient all-feature tests exit 101 at the version guard, and pinned-Python all-feature tests exit 101 at the existing macOS undefined-CPython-symbol link step. No manifest entry, pytest result, test selection, threshold, or tolerance changed; section 14 is unchanged because there was no intentional deviation. |
| `PARITY-016` | 2026-08-17 | Codex | PR #304 | Red: `python3 -m unittest compat.tests.test_approved_deltas -v` → exit 1, `ImportError` because `compat.harness.approved_deltas` did not exist; the new report behavior test then failed `0 != 1` because an unregistered text mismatch returned success. The first full run exposed unsupported `pdfminer.psparser.PSLiteral`; focused contracts separately failed until stable PDFMiner primitive and resolved `pdfplumber.page.Page` identities were encoded. Green: approved-delta contracts → 6 tests, OK; multi-API report contracts → 6 tests, OK; `python3 scripts/check_approved_deltas.py` → registry OK, zero entries, exact target; all compatibility tests → 65 tests, OK; inherited pinned differential tier → 19 tests, OK. Pinned reference pre-scan → 2,981 API/page values, four existing reference failures, zero digest failures. `basic_text.pdf` → five unregistered differences, exit 1; two annotation fixture copies → 12 unregistered, zero digest failures, exit 1. Full pinned run → 80 entries, 76 documents, 271 pages, four processing failures, 1,486 unregistered, 0 approved, 0 stale, 0 digest failures, exit 1. Workspace check and 130 excluded-scope tests pass; Rust 1.94 excluded-scope Clippy exits 101 on six pre-existing lints. | Exact registration tests prove a matching result pair can pass, while changed results are unregistered and leave the prior entry stale. CI runs registry validation and these behavioral gates, but the expensive full report is not yet a CI execution step (`PARITY-023`/`PARITY-026`), so the task remains unchecked. Strict all-target/all-feature Clippy and ambient tests exit 101 on Python 3.14 versus PyO3 0.24.2 plus the same pre-existing lints; pinned-Python all-feature tests reach the existing macOS undefined-CPython-symbol link failure. No delta was fabricated or approved, and no output, failure, threshold, tolerance, fixture, page, or API was omitted; section 14 is unchanged because there was no intentional deviation. |
| `PARITY-017` | 2026-08-17 | Codex | PR #305 | Red: `python3 -m unittest compat.tests.test_machine_report -v` → exit 1, `ImportError` because `compat.harness.machine_report` did not exist; the JSON integration contract then failed `0 != 1` because `--json` still wrote the legacy dictionary and returned success. Candidate generation first exited 2 on unknown `--candidate-output`; the real sample then exposed `unsupported_in_rust` counted as different, whose regression test failed `0 != 1`. Content-address and ambient-discovery contracts separately failed before result SHA-256 verification and lazy candidate-package import were added. Green: machine/multi-API contracts → 10 tests, OK; ambient and pinned compatibility discovery → 70 tests, OK; committed option matrix → 161 cases current; inherited pinned differential tier → 20 tests, OK. The pinned reference is correctly rejected as a candidate. `basic_text.pdf` → schema v1, one fixture/page, 11 API records (5 equal, 5 different, 1 unsupported), 5 unregistered dispositions, 161 blocked option records, exit 1. Full pinned run → schema v1, 80 fixtures, 76 documents, four processing failures, 271 pages, 2,981 API records, 161 option records, 1,486 unregistered, artifact SHA-256 `8b097e60...87a86`, exit 1. Workspace check and the full excluded-scope workspace test command pass; the latter includes 106 accuracy tests passed/1 ignored and 102 cross-validation tests passed/9 ignored. Formatting passes; excluded-scope Rust 1.94 Clippy exits 101 on six pre-existing lints. | Every compared page has all 11 API keys and every option record has its ID/API/fixture/page/options/reference/candidate/comparison fields; `jq -e` returns true. The task stays unchecked because all 161 candidate option cases are explicitly blocked by `PYAPI-002`, real corpus failures remain, and strict all-target/all-feature Clippy/ambient tests fail on Python 3.14 plus existing lints while pinned-Python all-feature tests reach the existing macOS CPython-link failure. No legacy result was silently dropped, no candidate result was fabricated, and no threshold, tolerance, fixture, page, API, option, or failure was weakened; section 14 is unchanged because there was no intentional deviation. |
| `PARITY-018` | 2026-08-17 | Codex | PR #306 | Red: `python3 -m unittest compat.tests.test_human_summary -v` → exit 1, `ImportError` because `compat.harness.human_summary` did not exist. After the renderer contract passed, the comparison contract failed with `KeyError: first_difference`; CLI integration then exited 2 because `--summary` was unknown. A full-corpus review exposed that equal common coordinates were omitted, and the added assertion failed until unchanged coordinates became explicit. Green: focused human/machine/multi-API contracts → 15 tests, OK; ambient and pinned compatibility discovery → 75 tests each, OK; inherited plus human differential tier → 25 tests, OK; option matrix → 161 cases current; approved-delta registry → 0 entries, exact target. Two `basic_text.pdf` runs produced byte-identical JSON/Markdown, five unregistered deltas, and exit 1. Full pinned command → status failed, 80 fixtures, 76 documents, four explicit processing failures, 271 pages, 2,981 API records, 161 blocked options, 1,486 first-difference records, and 1,486 unregistered deltas. Machine JSON: 57,877,531 bytes, SHA-256 `27c16cd...e66462`; final Markdown: 1,323 bytes, SHA-256 `0b35ae79...bea08`. Workspace check and excluded-scope workspace tests pass; accuracy is 106 passed/1 ignored and cross-validation is 102 passed/9 ignored. | The summary deterministically selects `crates/pdfplumber/tests/fixtures/pdfs/150109DSP-Milw-505-90D.pdf`, page 1, `chars`, index 0; it shows equal text, explicit unchanged common coordinates, exact type-safe objects, and the unregistered disposition. Excluded-scope Rust 1.94 Clippy exits 101 on 14 pre-existing diagnostics. Strict all-target/all-feature Clippy and ambient tests exit 101 on Python 3.14 versus PyO3 0.24.2 maximum 3.13; pinned-Python all-feature tests reach the existing macOS undefined-CPython-symbol link failure. The task stays unchecked because real failures remain and artifact CI execution/upload is pending. No failure, object, coordinate, text, API, option, fixture, page, threshold, tolerance, or ignore was weakened; section 14 is unchanged because there was no intentional deviation. |
| `PARITY-019` | 2026-08-17 | Codex | PR #307 | Red: `python3 -m unittest compat.tests.test_prd_linter -v` → exit 1, `ImportError` because `compat.harness.prd_linter` did not exist. Fenced-example contracts then produced one error (false duplicate at lines 5 and 7) and one failure (fenced evidence incorrectly accepted). Green: `python3 -m unittest compat.tests.test_prd_linter -v` → 8 tests, OK; `python3 -m unittest discover -s compat/tests -t . -v` and the pinned equivalent → 83 tests each, OK; `python3 scripts/check_prd.py` → 721 tasks, 2 checked, 19 evidence rows; `bash scripts/setup_golden_venv.sh` → pinned Python 3.13.12 and pdfplumber 0.11.10 verified; API, 161-case option, error, serialization, call, and empty approved-delta checks are current. `cargo check --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm`, current CI Clippy with `-D warnings`, the excluded workspace test command, `cargo fmt --all -- --check`, and `git diff --check` → exit 0. `cargo test -p pdfplumber --test cross_validation -- --nocapture` → 102 passed, 9 ignored, exit 0. | The linter is section-scoped, line-reporting, deterministic, and fenced-code-aware; compatibility CI invokes it before the reference-environment build. The task stays unchecked: excluded all-target Rust 1.94 Clippy exits 101 on 14 existing diagnostics; strict all-target/all-feature Clippy and ambient all-feature tests exit 101 on Python 3.14 versus PyO3 0.24.2 maximum 3.13; pinned-Python all-feature tests exit 101 at the existing macOS undefined-CPython-symbol link step. No task state, evidence row, test, threshold, tolerance, fixture, or ignore was weakened; section 14 is unchanged because there was no intentional deviation. |
| `PARITY-020` | 2026-08-17 | Codex | PR #308 | Red: `python3 -m unittest compat.tests.test_fixture_licenses -v` → exit 1, `ImportError` because `compat.harness.fixture_licenses` did not exist. Follow-up red audit: `python3 -m unittest compat.tests.test_fixture_licenses.FixtureLicenseContractTests.test_rejects_unpinned_private_or_restricted_sources -v` → exit 1 with two `FixtureMetadataError not raised` failures because invented `not-a-real-spdx-id` and `LicenseRef-Proprietary` values passed the syntax-only check; after adding the reviewed redistribution allowlist, the same command → one test, OK. Source-byte audit against immutable official revisions → pdfplumber v0.11.10: 80 matched/0 mismatched/0 missing; pdf.js: 15/0/0; PDFBox: 10/0/0; Poppler: all three SHA-256 values matched. Green: the focused contract → 8 tests, OK; `python3 scripts/check_fixture_licenses.py` → `Fixture metadata OK: 142 PDFs, 5 sources`; `python3 scripts/check_prd.py` → 721 tasks, 2 checked, 20 evidence rows; ambient and pinned `python -m unittest discover -s compat/tests -t . -v` → 91 tests each, OK. `bash scripts/download_test_fixtures.sh` → 108 skipped, 0 failed, audit OK; `bash tests/fixtures/download_fixtures.sh` → all 19 legacy checksums OK, audit OK. `cargo check --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm`, current CI Clippy with `-D warnings`, `cargo fmt --all -- --check`, shell syntax, Python compile, and `git diff --check` → exit 0. `cargo test -p pdfplumber --test cross_validation -- --nocapture` → 102 passed, 9 ignored. | The registry covers 116 externally sourced paths and 26 project-generated paths; nine duplicate blobs remain explicit path entries. Exact revision URLs expose MIT, Apache-2.0, and GPL-2.0 license texts, and the offline policy rejects SPDX-shaped identifiers that have not received that review. No PDF changed in this PR. The unchanged excluded workspace test command was terminated by the tool's 15-minute ceiling after reporting accuracy 106 passed/1 ignored, cross-validation 102 passed/9 ignored, and all emitted binaries green, so it is not claimed as a completed pass. The task stays unchecked: excluded all-target Clippy exits 101 on 14 existing diagnostics; strict Clippy additionally hits ambient Python 3.14 above PyO3 0.24.2's maximum 3.13; ambient all-feature tests hit the same guard; pinned-Python all-feature tests hit the existing macOS undefined-CPython-symbol link failure. No threshold, tolerance, fixture, test, or license requirement was weakened; section 14 is unchanged because there was no intentional deviation. |
| `PARITY-021` | 2026-08-17 | Codex | PR #309 | Red: `python3 -m unittest compat.tests.test_upstream_fixture_corpus -v` → exit 1, `ImportError` because `compat.harness.upstream_fixture_corpus` did not exist; after the general importer was added, the same command remained red until the committed `tests/pdfs` tree existed. Source import: `python3 scripts/import_upstream_fixtures.py` → 81 files, SHA-256 `74b8ab0bc2dc561010b48a5488bca3aeaf1ab5b495267f8b34795b3a41f5a175`, verified commit `7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62`. Green: corpus contracts → 3 tests, OK; corpus plus provenance contracts → 11 tests, OK; `python3 scripts/import_upstream_fixtures.py --check` → 81 PDFs and the same fingerprint; `python3 scripts/check_fixture_licenses.py` → `Fixture metadata OK: 223 PDFs, 5 sources`; ambient and pinned `python -m unittest discover -s compat/tests -t . -v` → 94 tests each, OK. The reference venv rebuild verified Python 3.13.12 and pdfplumber 0.11.10. `cargo check --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm`, current CI Clippy with `-D warnings`, `cargo test --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm`, `cargo fmt --all -- --check`, Python compile, shell syntax, and diff checks → exit 0; the workspace tier includes accuracy 106 passed/1 ignored and cross-validation 102 passed/9 ignored. | All 81 exact upstream paths are committed beneath their original `tests/pdfs` hierarchy; 19 retain `from-oss-fuzz/load`, `empty.pdf` remains zero bytes, and no upstream PDF byte was changed. `.gitattributes` marks PDFs binary so Git does not reinterpret required xref spacing. The task stays unchecked: excluded all-target Clippy exits 101 on 14 existing diagnostics; strict all-feature Clippy additionally hits ambient Python 3.14 above PyO3 0.24.2's maximum 3.13; ambient all-feature tests hit the same guard; pinned-Python all-feature tests hit the existing macOS undefined-CPython-symbol link failure. No threshold, tolerance, fixture, path, assertion, or test was weakened; section 14 is unchanged because there was no intentional deviation. |
| `PARITY-022` | 2026-08-17 | Codex | PR #310 | Red: `python3 -m unittest compat.tests.test_corpus_index -v` → exit 1, `ImportError` because `compat.harness.corpus_index` did not exist. Green: `python3 -m unittest compat.tests.test_corpus_index compat.tests.test_fixture_licenses -v` → 12 tests, OK; `python3 scripts/check_corpus_index.py` → `Corpus index OK: 223 PDFs (external-parser=28, project-generated=26, rust-regression=88, upstream-v0.11.10=81)`; `python3 scripts/check_fixture_licenses.py` → `Fixture metadata OK: 223 PDFs, 5 sources`; ambient and pinned `python -m unittest discover -s compat/tests -t . -v` → 98 tests each, OK. `bash scripts/setup_golden_venv.sh` rebuilt the hash-pinned environment and verified pdfplumber 0.11.10. `python3 scripts/import_upstream_fixtures.py --check`, `python3 scripts/check_prd.py`, Python compile, `cargo check --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm`, current CI Clippy with `-D warnings`, `cargo test --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm`, `cargo test -p pdfplumber --test cross_validation -- --nocapture`, `cargo fmt --all -- --check`, and `git diff --check` → exit 0; workspace/cross-validation results include accuracy 106 passed/1 ignored and cross-validation 102 passed/9 ignored. | The schema-2 registry contains exactly one collection for every path and remains the only PDF inventory, avoiding a second drift-prone manifest. No PDF bytes changed. The task stays unchecked: pinned-Python `cargo clippy --workspace --all-targets --all-features -- -D warnings` exits 101 on 14 pre-existing diagnostics, and pinned-Python `cargo test --workspace --all-features` exits 101 at the existing macOS `pdfplumber-py` undefined-CPython-symbol link step. No fixture, collection, test, threshold, tolerance, or assertion was omitted or weakened; section 14 is unchanged because there was no intentional deviation. |
| `PYAPI-001` | 2026-08-17 | Codex | PR #316 | Red: `PYO3_PYTHON=/Users/yhkwon/.local/bin/python3.13 cargo test -p pdfplumber-py native_extension_uses_private_submodule_identity -- --nocapture` → exit 101 with `left: "pdfplumber"`, `right: "_native"`. Green: the same focused command → one passed; full binding crate → 50 passed. Maturin 1.14.1 release build produced `pdfplumber_rs-0.2.0-cp313-cp313-macosx_11_0_arm64.whl` and `pdfplumber_rs-0.2.0.tar.gz`; isolated wheel and sdist installs each ran `python -m unittest discover -s crates/pdfplumber-py/tests -p 'test_native_layout.py' -v` → three passed. The wheel contains `pdfplumber/__init__.py`, `_native.cpython-313-darwin.so`, and `_native.pyi`; installed identities are `pdfplumber._native` for the module and native exceptions. `bash scripts/setup_golden_venv.sh` → pinned pdfplumber 0.11.10; ambient and pinned compatibility discovery → 98 tests each, OK. Current CI workspace check and excluded-scope Clippy with `-D warnings` → exit 0; cross-validation → 102 passed, nine ignored. | The task stays unchecked: pinned-Python strict all-target/all-feature Clippy exits 101 on 14 pre-existing diagnostics, and the strict all-feature workspace test exits 101 at the existing macOS undefined-CPython-symbol link step. `PYAPI-002` remains deliberately undone: `pdfplumber/__init__.py` exposes only the private extension, not the upstream-compatible facade. No threshold, tolerance, test, import-origin check, fixture, or assertion was weakened, and section 14 is unchanged because there was no intentional deviation. |
| `PYAPI-003` | 2026-08-17 | Codex | PR #317 | Pinned reference: `.venv-reference/bin/python` verified pdfplumber 0.11.10 and showed `pdfplumber.open == PDF.open`, `__qualname__ = PDF.open`, then opened the same generated fixture used by the candidate contract. Red: after building and installing the pre-change wheel, `python -m unittest crates/pdfplumber-py/tests/test_native_layout.py -v` → three passed and one error, `AttributeError: module 'pdfplumber' has no attribute 'open'`, exit 1. Green: fresh wheel and sdist installs each ran the same command → four passed, including alias equality, real fixture opening, exported return type, and a non-empty page list. Full binding crate → 50 passed; ambient and pinned compatibility discovery → 98 passed each; current CI workspace check and excluded-scope Clippy → exit 0; cross-validation → 102 passed, nine ignored. | The task stays unchecked: pinned-Python strict all-target/all-feature Clippy exits 101 on the same 14 pre-existing diagnostics, and the strict all-feature workspace test exits 101 at the existing macOS undefined-CPython-symbol link step. `PYAPI-004`/`PYAPI-008` remain deliberately undone: `__all__`, the full top-level export policy, exact signature, and classmethod descriptor behavior were not changed. No threshold, tolerance, fixture, test, import-origin check, or assertion was weakened; section 14 is unchanged because there was no intentional deviation. |
| `PARITY-004` | 2026-08-20 | Codex | PR #318 | Red: `python3 -m unittest compat.tests.test_candidate_setup compat.tests.test_environment -v` → exit 1 with one missing-module import error and four `verify_candidate(..., expected_root=...)` type errors. Green: the same focused command → 16 tests, OK. Exact Maturin 1.14.1 built the CPython 3.13 wheel; `python3 scripts/setup_candidate_venv.py --python "$(uv python find 3.13)" --wheel <local-wheel>` installed it with `--no-deps`, verified both package/native origins under `.venv-candidate`, and the installed native-layout suite → four tests, OK. Candidate call/error/serialization commands each reached exact comparison and exited 1; option generation wrote all 161 cases as explicit errors and exited 1. | Isolation is now operational rather than fabricated: the upstream package lacks the required private native extension, a candidate outside the configured root is rejected, and the current package exposes real compatibility failures. The task stays unchecked for those failures and `CI-001`/`CI-002`/`CI-003`; no dependency, case, outcome, test, threshold, tolerance, or assertion was skipped or weakened. |
| `PDF-002` | 2026-08-20 | Codex | PR #319 | Pinned reference v0.11.10 opens the same fixture from both `str` and `pathlib.Path`. Red: the pre-change CPython 3.13 wheel ran `python -m unittest discover -s crates/pdfplumber-py/tests -p 'test_native_layout.py' -v` → four passed and one error, `TypeError: argument 'path': 'PosixPath' object cannot be converted to 'PyString'`. Green: after extracting the binding argument through Python's filesystem-path protocol, a fresh exact-Maturin 1.14.1 wheel ran the same installed-artifact command → five passed. Candidate call-contract output no longer contains that PosixPath conversion failure. | `PDF.open` and top-level `open` remain the same callable, and existing string-path behavior stays covered. The task remains unchecked for the strict section 10 `CI-001`/`CI-002`/`CI-003` blockers. Stream inputs, ownership, lifecycle, options, modules, tests, thresholds, tolerances, and assertions were not broadened, skipped, or weakened. |
| `PDF-003` | 2026-08-20 | Codex | PR #320 | Pinned reference v0.11.10 opened the same fixture from an existing `BufferedReader`, rewound a nonzero starting position, retained caller ownership, and left the stream at EOF. Red: the pre-change CPython 3.13 wheel ran the installed native-layout suite → five passed and two subtest errors; `BufferedReader` was rejected as an invalid filesystem path. Green: the fresh exact-Maturin 1.14.1 wheel ran the same suite → six passed, including real page access and exact open/position state. | Candidate contracts no longer contain the prior binary-file path-conversion error and expose downstream lifecycle/exception gaps instead. The task stays unchecked for `CI-001`/`CI-002`/`CI-003`; close semantics, retained public stream state, tests, thresholds, tolerances, and assertions were not skipped or weakened. |
| `PDF-004` | 2026-08-20 | Codex | PR #320 | Pinned reference v0.11.10 opened `BytesIO` from offsets 0, 10, and EOF, retained caller ownership, and left the stream at EOF. The same red installed-wheel run rejected `BytesIO` as an invalid filesystem path; the green wheel rewound a nonzero offset, opened the real fixture, returned populated pages, and left the stream open at EOF. Candidate call/error/serialization commands reached exact comparison and exited 1 without the old BytesIO/BufferedReader conversion errors. | The remaining failures stay explicit, including missing lifecycle and serialization options. The task stays unchecked for those gaps and `CI-001`/`CI-002`/`CI-003`; no failure, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-005` | 2026-08-20 | Codex | PR #321 | Pinned reference v0.11.10 retained external `BytesIO` and `BufferedReader` identity and left both open after `PDF.close()`. Red: the pre-change installed-wheel suite → six passed and three errors, beginning with missing `.stream`; green: the exact-Maturin 1.14.1 wheel ran nine artifact tests with `ResourceWarning` promoted to error, all OK. External `.stream` is the exact caller object and remains open across repeated close and context exit. | The task stays unchecked for strict `CI-001`/`CI-002`/`CI-003`; lifecycle differences outside the tested contract remain explicit. No owned/external state, failure, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-006` | 2026-08-20 | Codex | PR #321 | Pinned path input exposed an open `BufferedReader`, then closed it on the first idempotent `close()` while returning `None` on both calls. The same red run lacked `.stream`; the green wheel retains a real owned Python file, closes it without a resource warning, and preserves repeated-close behavior. | Internally owned resources are now observable rather than simulated. The task remains unchecked for `CI-001`/`CI-002`/`CI-003`; no cleanup warning or error was suppressed. |
| `PDF-007` | 2026-08-20 | Codex | PR #321 | Red artifact evidence included `TypeError: 'builtins.PDF' object does not support the context manager protocol`. Green verifies `__enter__` identity, owned-stream close on exit, and external-stream preservation; candidate call-contract output no longer contains the missing-context-manager error and still exits 1 for remaining exact differences. | `__exit__` returns `None` and therefore does not suppress exceptions. The task stays unchecked for strict section 10 blockers; no exception path, test, threshold, tolerance, or assertion was weakened. |
| `PDF-008` | 2026-08-20 | Codex | PR #321 | Pinned reference retained one page, metadata, and previously accessed page content after close and allowed a second close. Green installed-wheel evidence likewise retains page count and real first-page character content after owned resource cleanup while remaining idempotent. | The broader error contract still exits 1 on independent binding-shape and exception-type gaps, which remain visible. The task stays unchecked for those gaps and `CI-001`/`CI-002`/`CI-003`; no after-close outcome was skipped or rewritten. |
| `PDF-009` | 2026-08-20 | Codex | PR #321 | Pinned path state was `pathlib.Path`, owned `BufferedReader`, `password=None`; pinned external state was the identical input stream, `path=None`, `password=None`. Green artifact tests assert those same types, values, identities, and closed-state transitions. | Password input itself remains `PDF-013`; ownership is private implementation state rather than a new public compatibility claim. This task stays unchecked for `PDF-013` and `CI-001`/`CI-002`/`CI-003`, with no sentinel substitution or assertion weakening. |
| `PDF-010` | 2026-08-20 | Codex | PR #322 | Pinned reference v0.11.10 on the 5-page fixture showed `None → [1,2,3,4,5]`, `[] → []`, `[2,1] → [1,2]`, duplicate `[1,1] → [1]`, zero/negative/out-of-range and `[\"1\"]` omitted, `[True] → [1]`, and lazy `TypeError` for scalar `1`. Initial red: the pre-change lifecycle wheel ran nine tests and one error, `PDF.open() got an unexpected keyword argument 'pages'`; strengthened red: the integer-vector wheel errored on `[\"1\"]` during `open`. Green: a fresh exact-Maturin 1.14.1 wheel ran all ten artifact tests with `ResourceWarning` promoted to error, including text identity for document-ordered pages 3 and 5 and exact Python membership edge cases. | Candidate serialization comparison no longer contains the `pages=` keyword error and now reaches the independent missing `to_csv`/`to_json` gaps, remaining exit 1. The task stays unchecked for those gaps and `CI-001`/`CI-002`/`CI-003`; no selection, failure, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-011` | 2026-08-20 | Codex | PR #323 | Pinned v0.11.10 loaded `(3, 5)` as original page numbers `[3, 5]`; first-character offsets `doctop - top` were `0` and `841.89`, the height of selected page 3. Red: the exact preceding page-selection wheel ran eleven artifact tests with one failure, returning `[2, 4]`. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all eleven tests with `ResourceWarning` promoted to error, asserting `[3, 5]` and selected-view offsets for both characters and derived words. | The core rebases only character `doctop`; page-local `top` remains unchanged and word geometry derives the same offset. `.initial_doctop` remains `PAGE-003`. The task stays unchecked for strict `CI-001`/`CI-002`/`CI-003`; no page, coordinate, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PAGE-001` | 2026-08-20 | Codex | PR #323 | The same installed-artifact red/green changed public selected page numbers from `[2, 4]` to the pinned `[3, 5]`; the binding unit expectation changed from native zero to public one for the first page. | Translation occurs only in `PyPage.page_number`; strict section 10 and broader page attributes remain open, so the task stays unchecked. |
| `PAGE-002` | 2026-08-20 | Codex | PR #323 | Existing Rust extraction remains zero-based, and the focused core rebase test preserves native page number `4` while changing only character document-top coordinates. The Python getter alone adds one, with binding and installed-wheel tests green. | No Rust index, parser lookup, selection membership, or core page ordering was changed. The task stays unchecked for strict `CI-001`/`CI-002`/`CI-003`. |
| `PDF-012` | 2026-08-20 | Codex | PR #324 | Pinned v0.11.10 accepts `None`, dicts, and general mappings for seven `LAParams` keys; `{}` enables higher-level objects, `line_margin=0.75` is retained, scalar `1` and unknown keys raise `TypeError`, and `boxes_flow=2.0` raises `ValueError`. Initial red: the preceding geometry wheel errored because `laparams` was an unexpected keyword. Strengthened red: the first dict-only wheel rejected `MappingProxyType`. Green: a fresh exact-Maturin 1.14.1 wheel ran all twelve artifact tests with `ResourceWarning` promoted to error, including every known key, immutable mappings, real text extraction, and exact failure classes/messages. | DEC-004 selects native implementation and forbids a hidden runtime `pdfminer.six` fallback. The mapping is retained for future native layout analysis, but higher-level object production and compatible public `LAParams` state remain `PAGE-018`; this task stays unchecked for that gap and strict `CI-001`/`CI-002`/`CI-003`. No option, failure, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-013` | 2026-08-20 | Codex | PR #325 | Pinned v0.11.10 opened the real four-page `password-example.pdf` from both path and `BytesIO` with `password="test"`, retained `.password`, left the caller stream open, and rejected absent/empty/wrong values. Red: the preceding exact wheel ran twelve tests and errored because `password` was an unexpected keyword. The first binding implementation authenticated but exposed zero pages, and a focused core regression reproduced `0 != 4`; switching from load-then-decrypt to lopdf's password-aware reader made all five core password tests pass. Green: a fresh exact-Maturin 1.14.1 wheel ran all thirteen artifact tests with `ResourceWarning` promoted to error, including four populated pages, real text, path/stream state, EOF position, and external ownership on both success and failure. | The supplied password is retained but never logged. Missing and wrong candidate failures deliberately remain the native `PdfPasswordRequired`/`PdfInvalidPassword` classes pending exact `PDF-014` exception unification; this task stays unchecked for that gap and strict `CI-001`/`CI-002`/`CI-003`. No credential, failure, page, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-014` | 2026-08-20 | Codex | PR #326 | Pinned v0.11.10 kept missing-path and directory failures as `FileNotFoundError`/`IsADirectoryError`; empty and garbage streams raised `pdfplumber.utils.exceptions.PdfminerException` with `No /Root object! - Is this really a PDF?`; closed streams used the same public type with their I/O message; missing, empty, and wrong passwords used that type with an empty message. Mutating the real encrypted fixture from `/V 2` to unsupported `/V 9` produced the same type with a non-empty encryption diagnostic. Red: the exact preceding wheel ran fourteen artifact tests with one import error because `pdfplumber.utils` did not exist; its direct outcomes were private `PdfParseError`, `PdfPasswordRequired`, and `PdfInvalidPassword`, and unsupported encryption was incorrectly collapsed into password failure. Green: structured lopdf variant matching plus the compatibility exception boundary made the focused parser regression pass and a fresh exact-Maturin 1.14.1 CPython 3.13 wheel run all fourteen artifact tests with `ResourceWarning` promoted to error. | Built-in path I/O remains unwrapped, caller-owned streams remain open after every failure, and the unsupported-encryption check is an in-memory same-length mutation of the pinned fixture. The task stays unchecked for strict `CI-001`/`CI-002`/`CI-003`; no failure category, input, credential, test, threshold, tolerance, or assertion was omitted or weakened. |
| `UTIL-EXC-002` | 2026-08-20 | Codex | PR #326 | The same red wheel failed to import `pdfplumber.utils.exceptions`; the green wheel imports `PdfminerException`, reports the exact module identity, derives it directly from `Exception` rather than `RuntimeError`, and exercises it through malformed, closed-stream, password, and unsupported-encryption paths. | The public module aliases a native exception and does not import upstream `pdfplumber` or `pdfminer.six`. Wider utility compatibility and strict section 10 remain open, so the task stays unchecked. |
| `PDF-015` | 2026-08-20 | Codex | PR #327 | Pinned v0.11.10 returned the same normal metadata for `strict_metadata=False` and `True`; on the deterministic cyclic `/Info` document, permissive mode logged a warning and returned while strict mode raised `RecursionError("maximum recursion depth exceeded")`, leaving the external stream open in both cases. Red: the exact preceding wheel ran fifteen artifact tests with one error because `strict_metadata` was an unexpected keyword. Green: the raw metadata graph validator's focused parser test passed, and a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all fifteen artifact tests with `ResourceWarning` promoted to error. | Strict validation follows general nested metadata references and does not special-case bytes, object IDs, or keys. Permissive-mode key/value/warning parity remains visible under `PDF-021`; this task stays unchecked for that gap and strict `CI-001`/`CI-002`/`CI-003`, with no outcome, stream state, test, threshold, tolerance, or assertion omitted or weakened. |
| `PDF-016` | 2026-08-20 | Codex | PR #328 | Pinned v0.11.10 retained `None`, `NFC`, `NFD`, `NFKC`, and `NFKD` on `.unicode_norm`; actual fixture characters distinguished composed `é`, decomposed `e` plus combining acute, unchanged `ﬁ`, and compatibility-normalized `fi`. It accepted `"nfc"`, `""`, `1`, `True`, and `b"NFC"` at open time, then raised exact `ValueError("invalid normalization form")` or `TypeError("normalize() argument 1 must be str, not <type>")` during character materialization. Red: the exact PDF-015 wheel ran sixteen artifact tests with one error because `.unicode_norm` was absent. Green: after catching and removing an intermediate empty-probe panic, a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all sixteen artifact tests with `ResourceWarning` promoted to error. | The implementation uses Python's standard-library validator for exact public failures and the existing general Rust normalization engine for extracted characters; it does not special-case fixture positions or code points. PDF-022 later moved validation from `.pages` to first content access; strict section 10 remains open. No form, error, input, fixture, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-017` | 2026-08-20 | Codex | PR #329 | Pinned v0.11.10 and the candidate both used a deterministic temporary `gs` executable that consumed path or stream input and emitted the selected real fixture as repaired output. Path, offset-10 `BytesIO`, and password-protected inputs returned populated documents backed by a distinct owned `BytesIO` with `.path=None`; external sources remained open at EOF and repaired streams closed with the PDF. With `PATH` empty, `repair=False` still opened normally and `repair=True` raised the exact two-line Ghostscript installation exception. Red: the exact PDF-016 wheel ran seventeen artifact tests with one error because `repair` was an unexpected keyword. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel installed the packaged `pdfplumber.repair` helper and ran all seventeen artifact tests with `ResourceWarning` promoted to error. | `PDF.open` delegates to the same Ghostscript process contract instead of substituting the behaviorally different pure-Rust repair extension forbidden by `REPAIR-011`. Explicit `gs_path`, presets, top-level `repair`, subprocess failure variants, and strict section 10 remain open under `PDF-018`, `PDF-019`, `REPAIR-001`–`REPAIR-011`, and `CI-001`/`CI-002`/`CI-003`. No executable lookup, offset, byte stream, credential, output, failure, resource state, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-018` | 2026-08-20 | Codex | PR #330 | Pinned v0.11.10 and the candidate accepted an explicit deterministic Ghostscript executable as both `str` and `pathlib.Path` while `PATH` was empty, produced a populated document from the same real output fixture, and accepted an arbitrary `gs_path` object without evaluating it when `repair=False`. A missing `Path` matched direct process execution's platform-native `FileNotFoundError`, including errno, `.filename` representation, and exact message. Red: the exact PDF-017 wheel ran eighteen artifact tests with three errors because `gs_path` was an unexpected keyword. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all eighteen artifact tests with `ResourceWarning` promoted to error; the exact public signature plus missing, duplicate, unexpected-keyword, positional, and keyword call shapes were separately verified. | The binding now collects its growing upstream-compatible option list through a general positional/keyword parser instead of suppressing Clippy's argument-count lint. Presets and broader repair failures remain visible under `PDF-019`, `REPAIR-001`–`REPAIR-011`, and strict section 10. No path type, executable lookup state, error field, output, resource state, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-019` | 2026-08-20 | Codex | PR #331 | Pinned v0.11.10 and the candidate forwarded `default`, `prepress`, `printer`, `ebook`, and `screen` exactly once as `-dPDFSETTINGS=/...` to the same explicit Ghostscript subprocess and real output fixture. When `repair=False`, an arbitrary setting remained unevaluated and no repair process ran. Red: the exact PDF-018 wheel ran nineteen artifact tests with six errors because `repair_setting` was unexpected. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all nineteen artifact tests with `ResourceWarning` promoted to error, and the exact public signature plus ninth positional argument were separately verified. | The option is forwarded without a fixture- or preset-specific native branch, matching the pinned Python helper. Broader repair failures and the top-level repair API remain visible under `REPAIR-001`–`REPAIR-011` and strict section 10. No preset, subprocess argument, disabled path, output, resource state, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-020` | 2026-08-20 | Codex | PR #332 | Pinned v0.11.10 retained `True`, `False`, `None`, `0`, `1`, empty/nonempty strings, and an opaque sentinel by identity and defaulted to the identical `True` object. The real malformed-annotation fixture opened as one page for every value. Red: the exact PDF-019 wheel ran twenty artifact tests with nine errors: eight calls rejected `raise_unicode_errors` and the omitted form lacked the property. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all twenty artifact tests with `ResourceWarning` promoted to error, and exact signature plus tenth positional argument probes passed. | Pinned `.annots` access separately proved truthy `UnicodeDecodeError` versus false `UserWarning` plus raw contents. Candidate `Page.annots` is not public yet, so that behavior remains explicit under `PAGE-017`, `OBJ-ANNOT-004`, `SEM-ANNOT-001`, and `SEM-ANNOT-005`; the task and strict section 10 stay unchecked. No value, identity, default, fixture, error, warning, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-021` | 2026-08-20 | Codex | PR #333 | Pinned v0.11.10 established exact source key spelling and order, PDFDocEncoding bullet and UTF-16 text decoding, names, integers, reals, booleans, null array members, indirect strings, nested lists and dictionaries, and omission of dictionary-valued nulls. The real `issue-316-example.pdf` additionally proves list-valued change dictionaries and integer `SPDF`. Red: the exact PDF-020 wheel ran twenty-two artifact tests with two failures because metadata used eight normalized lowercase fields and emitted no cycle warning. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all twenty-two tests with `ResourceWarning` promoted to error; two focused raw-metadata parser tests passed. All 60 corpus PDFs opened by both implementations had exactly equal ordered metadata keys, recursively tagged runtime types, and values. Both `metadata.non_strict_cycle` and `metadata.strict_cycle` exactly equal the committed 0.11.10 error contract, including logger, message, phase, key, retained `pdfminer.pdftypes.PDFObjRef` type, exception, and external stream state. | A backend-independent ordered metadata value model keeps the idiomatic fixed Rust metadata API intact while the Python adapter exposes the compatibility dictionary. The other 21 corpus inputs remain parser/open gaps rather than omitted metadata comparisons. The complete error contract remains red on independently tracked page-bbox, resource-state, annotation, and repair surfaces, and strict section 10 remains open, so this task stays unchecked. No common-open key, value type, container, reference, warning, failure, stream state, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-022` | 2026-08-20 | Codex | PR #334 | Pinned v0.11.10 returned one mutable cached `.pages` list, identical `Page` objects, original selected page numbers `[3, 5]`, and selected initial document-top offsets `[0, 841.89]`. It enumerated pages and exposed page number/width for `"nfc"`, `""`, `1`, `True`, and `b"NFC"`, then raised the exact `ValueError` or `TypeError` from `.chars`, lines, rectangles, images, and text extraction. A membership error on page two left page one in the stable partial cache without reevaluating selection. Red: the exact PDF-021 wheel ran the three focused artifact tests with two identity/cache failures and five enumeration errors. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel passed all three focused tests and all twenty-four artifact tests with `ResourceWarning` promoted to error; the binding's fifty unit tests also passed. | `Pdf` caches page display geometry during document setup; the binding installs the Python list before selection, creates stable handles, and interprets each selected content stream once on first content access. List mutation and partial failure state are intentionally observable because upstream returns its cached list. Close/flush invalidation and broader repeated-property identity remain explicit under `PDF-023`, `PDF-024`, and `PDF-032`; strict section 10 stays open. No identity, mutation, selection, geometry, failure phase, error, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-023` | 2026-08-20 | Codex | PR #335 | Pinned v0.11.10 discarded the caller-mutated cached page list on every `close()`, returned new page handles afterward, retained already materialized content through an old handle, closed only its owned stream, and returned `None` on repeated calls. With scalar `pages=1`, close raised the exact `TypeError("argument of type 'int' is not iterable")` before closing the owned stream. Red: the exact PDF-022 wheel failed both focused close tests by retaining the original cache and closing without materializing the invalid selection. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all twenty-five artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, and binding Clippy passed with warnings denied. | `close()` takes the list cache and reconstructs lightweight page handles before resource cleanup; it does not broaden into the separate container or page cache APIs. `PDF-024`, `PAGE-019`, `PAGE-020`, and strict section 10 remain open. No cache identity, list mutation, old-page state, repeated call, failure phase, ownership state, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-024` | 2026-08-20 | Codex | PR #336 | Pinned v0.11.10 invalidated `_pages` for omitted, explicit-`None`, and `["_pages"]` inputs while preserving the stream and old page content. Empty, unknown, and string iterables left the cache identical. Non-iterable `1` and item `1` produced their exact `TypeError` messages; `["_pages", 1]` retained the first deletion before raising. Red: the exact PDF-023 wheel raised `AttributeError` throughout the focused contract because `flush_cache` was absent. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all twenty-six artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, binding Clippy passed with warnings denied, and the packaged native type stub declares the new method. | The implementation consumes arbitrary iterables in order and shares one page-cache invalidator with `close()`. Other document caches, page caches, general container inheritance, repeated-property identity, and strict section 10 remain explicit under `PDF-025`, `PAGE-020`, `SER-001`, `PDF-032`, and `CI-001`–`CI-003`. No property form, error, partial effect, cache identity, old-page state, stream state, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-025` | 2026-08-20 | Codex | PR #337 | Pinned v0.11.10 and the candidate grouped only present page object types in document order and cached the same mutable dictionary and lists. Real fixture counts matched exactly: `basic_text.pdf` had 258 chars; `table-curves-example.pdf` had 1,992 chars, 208 rects, and 33 curves; `inline-image.pdf` had 22 chars and one image; and `empty-page.pdf` had no keys. Selected pages 3 and 5 of `long_document.pdf` had exactly 1,386 chars and two lines. Flushing `_pages` preserved the mutated object cache, flushing `_objects` preserved page identity and rebuilt the aggregate, and the default invalidated both. Red: the exact PDF-024 wheel raised five `AttributeError`s across two focused tests. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all twenty-eight artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, binding Clippy passed with warnings denied, and the packaged native type stub declares `.objects`. | Object aggregation uses the public page object lists without hiding their current schema differences. Page-object identity, broader page properties, complete object schemas, repeated-property identity, serialization, and strict section 10 remain explicit under `PAGE-016`, `PAGE-018`, `OBJ-001`–`OBJ-008`, `PDF-032`, and `CI-001`–`CI-003`, so the task stays unchecked. No object type, order, count, mutation, cache interaction, fixture, failure, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-026` | 2026-08-20 | Codex | PR #338 (based on PR #337) | Pinned v0.11.10 and the candidate returned fresh document/page annotation lists for the real `issue-463-example.pdf`, with exact page counts `[2, 4, 0]`, aggregate count six, page-number sequence `[1, 1, 2, 2, 2, 2]`, `object_type="annot"`, and ordered fifteen-key dictionary shape; `issue-598-example.pdf` retained its exact `http://www.ck12.org` URI. A page-2-only view retained four annotations with original page number 2 and selected-view `doctop == top`. Caller mutation of one document list did not affect the next access. Red: the exact PDF-025 wheel raised two `AttributeError`s because `Page.annots` was absent. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran both focused tests and all thirty artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, binding Clippy passed with warnings denied, and the packaged native type stub declares document and page `.annots`. | The adapter exposes the complete public key order while retaining visible backend gaps: raw `data` is not reconstructed, malformed text decoding and warnings differ, rotation remains unverified, and some common values still differ. Those gaps remain explicit under `PAGE-017`, `OBJ-ANNOT-001`–`OBJ-ANNOT-004`, `SEM-ANNOT-001`, `SEM-ANNOT-004`, `SEM-ANNOT-005`, and strict section 10, so the task stays unchecked. No page, annotation, order, key, identity, selection, coordinate, URI, failure, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-027` | 2026-08-20 | Codex | PR #339 (based on PR #338) | Pinned v0.11.10 and the candidate returned fresh page and document hyperlink lists with exact real-fixture results: `issue-982-example.pdf` had page counts `[0, 0, 0, 0, 34, 0, 0, 0]`, aggregate count 34, and only page number 5; selecting pages 2 and 5 retained `[0, 34]`. `pdffill-demo.pdf` had counts `[1, 1, 1, 4, 2, 7, 1]` and exact page-number sequence `[1, 2, 3, 4, 4, 4, 4, 5, 5, 6, 6, 6, 6, 6, 6, 6, 7]`; `issue-463-example.pdf` returned a fresh empty list. Red: the exact PDF-026 wheel raised two `AttributeError`s because `Page.hyperlinks` was absent. The first candidate included internal destinations, and filtering URI-shaped strings still misclassified named `/GoTo` targets such as `glo:CNN`, yielding `[0, 5, 10, 6, 34, 0, 0, 0]`. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran all thirty-two artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, core URI/GoTo regression tests passed, Clippy denied warnings, formatting and diff checks passed, and the native stub declares both properties. | The backend now preserves existing all-target Rust hyperlinks while exposing a separate exact `/URI` action view; no scheme whitelist or fixture-specific branch remains. Complete annotation `data`, rotation, malformed text decoding/warnings, broader common values, and strict section 10 stay explicit under `PAGE-017`, `OBJ-ANNOT-001`–`OBJ-ANNOT-004`, `SEM-ANNOT-002`–`SEM-ANNOT-005`, and `CI-001`–`CI-003`, so the task stays unchecked. No action type, target, page, link, order, key, identity, selection, coordinate, failure, fixture, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-028` | 2026-08-20 | Codex | PR #340 (based on PR #339) | Pinned v0.11.10 and the candidate returned fresh compact structure-tree lists for the real `image_structure.pdf`. The document tree exactly retained a `Document` root, two `P` children with MCIDs 0 and 1, and a `Figure` child with the exact two-paragraph alt text, MCID 2, and 1-based page number 1; the page tree retained the same hierarchy and values while omitting document-only page numbers. Untagged `basic_text.pdf` returned fresh empty lists at both scopes. Red: the exact PDF-027 wheel raised two `AttributeError`s because `PDF.structure_tree` was absent. Green: a fresh exact-Maturin 1.14.1 CPython 3.13 wheel ran both focused tests and all thirty-four artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, eight parser and fourteen real-fixture structure tests passed, Clippy denied warnings, formatting and diff checks passed, and the native stub declares both properties. | The document caches the native hierarchy once and page materialization filters that cache instead of reparsing `/StructTreeRoot`. Revision, ID, title, attributes, RoleMap/ClassMap, ParentTree, MCR/OBJR, selected/multipage edge behavior, find helpers, and strict section 10 remain explicit under `SEM-STRUCT-003`–`SEM-STRUCT-015` and `CI-001`–`CI-003`, so the task stays unchecked. No node, field, page number, MCID, order, hierarchy, empty result, fixture, failure, test, threshold, tolerance, or assertion was omitted or weakened. |

| `PDF-029` | 2026-08-20 | Codex | PR #341 (based on PR #340) | Pinned v0.11.10 and the candidate returned fresh `metadata`/`pages` document dictionaries and ordered page dictionaries. `basic_text.pdf` exactly retained its metadata, eight geometry keys, 258 default chars, empty annotations, direct page output, and explicit char/annotation filtering. Selected pages 3 and 5 of `long_document.pdf` retained original numbers, exact initial doctops `[0, 841.89]`, and generator consumption only on the first page. Four invalid inputs matched exact upstream exception types and messages. Red: the exact PDF-028 wheel raised two `AttributeError`s because `PDF.to_dict` was absent. The first candidate exposed backend f32 box values such as `595.280029296875` and broke equality between selected doctop and public page height; the final compatibility conversion restored the backend number's shortest decimal. Green: fresh exact-Maturin 1.14.1 CPython 3.13 wheel and sdist runs passed all thirty-six artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, all pinned contracts were current, and isolated workspace check, tests, doctests, formatting, and Clippy with warnings denied passed. | Six-fixture projection was exact for `basic_text.pdf`, selected `long_document.pdf`, and `table-curves-example.pdf`. Rotated boxes, non-default/non-zero-origin boxes, `issue-1181.pdf` object-key order, complete object schemas, higher-level layout types, serialization, repeated identity, and strict section 10 remain explicit under `PAGE-004`–`PAGE-018`, `OBJ-001`–`OBJ-010`, `SER-008`–`SER-023`, `PDF-030`–`PDF-032`, and `CI-001`–`CI-003`, so the task stays unchecked. No object type, key, order, number, page, filter, failure, fixture, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-030` | 2026-08-20 | Codex | PR #342 | Pinned v0.11.10 established document/page JSON parameter order, compact and indented returns, stream writes returning `None`, recursive precision and bool/list/tuple/dict/bytes handling, PDF-stream base64 conversion, required `object_type` filtering, and exact validation and call-shape failures. Red: the exact PDF-029 wheel lacked both methods and `pdfplumber.convert`. Green: fresh exact-Maturin 1.14.1 CPython 3.13 wheel and sdist runs passed all thirty-nine artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, binding and core Clippy denied warnings, and the complete core workspace test/doctest command passed. | The exact 15-case JSON differential remains 2 green and 13 red on existing object schemas, geometry values/types, font names, and precision. A 60-document/253-page audit rejected an integer heuristic after 1,604 type mismatches, leaving raw number provenance explicit. Derived-page containers, `PSLiteral`, complete image streams, every-object golden output, and strict section 10 remain open under `PAGE-004`–`PAGE-020`, `OBJ-001`–`OBJ-010`, `SER-009`–`SER-023`, `PDF-031`, `PDF-032`, and `CI-001`–`CI-003`, so the task stays unchecked. No value, type, key, order, stream state, error, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-031` | 2026-08-20 | Codex | PR #343 | Pinned v0.11.10 established document/page CSV parameter order, return and `StringIO` forms, CRLF bytes, required/prepended/sorted column ordering, object selection, serialization-time precision, filters, page-number rows, and exact validation and call-shape failures. Red: the exact PDF-030 wheel raised eleven `AttributeError`s because both methods were absent; strengthened red then exposed a blank page-number field. Green: fresh exact-Maturin 1.14.1 CPython 3.13 wheel and sdist runs passed all forty-one artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, binding and core Clippy denied warnings, and the complete core workspace test/doctest command passed. | The exact 13-case CSV differential is 4 green and nine red on existing object-field naming/schema, geometry values/types, font names, and image streams. Derived-page containers, repeated object-type selection, complete schemas/streams, and strict section 10 remain open under `PAGE-004`–`PAGE-020`, `OBJ-001`–`OBJ-010`, `SER-011`–`SER-023`, `PDF-032`, and `CI-001`–`CI-003`, so the task stays unchecked. No byte, column, row, value, type, stream state, error, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-032` | 2026-08-20 | Codex | PR #344 | Pinned CPython 3.13 and pdfplumber v0.11.10 returned `true` for all fifteen tested identity/mutation flags: stable stream/path/password/metadata/pages/objects, fresh annotations/hyperlinks/structure trees, metadata reuse by `to_dict()`, and metadata identity plus mutation persistence after flush and close. Red: the exact PDF-031 wheel failed nine of those guarantees. Green: fresh exact-Maturin 1.14.1 wheel and sdist runs each passed all forty-two artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, binding and core Clippy denied warnings, and the complete core workspace tests/doctests passed. | Candidate audits retain the open baseline without hiding it: call contracts are 1/12 exact, error contracts 2/23, serialization 6/28, and option cases 5/161 successful. Page-property cache identity, broader API/schema behavior, and strict section 10 remain open under `PAGE-016`, `PAGE-020`, and `CI-001`–`CI-003`, so the task stays unchecked. No identity, mutation, cache interaction, existing mismatch, fixture, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-033` | 2026-08-20 | Codex | PR #345 | Pinned CPython 3.13 and pdfplumber v0.11.10 established the exact `pdfplumber.pdf.PDF` module, qualname, default `repr()`/`str()`, ordered five-item shared `cached_properties` list, original `pages_to_parse` object identity, and path/external stream ownership flags. Red: the exact PDF-032 wheel reported `builtins.PDF` and omitted all three attributes. Green: fresh exact-Maturin 1.14.1 wheel and sdist runs each passed all forty-three artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, binding and core Clippy denied warnings, and the complete core workspace tests/doctests passed. | The module-qualified type identity restores diagnostics without pretending that `pdfplumber.pdf` is import-compatible or that native dictionaries are pdfminer objects. Module files and `.doc`, `.rsrcmgr`, and `.laparams` remain open under `PYAPI-005`, `PYAPI-006`, `PDF-036`, and strict `CI-001`–`CI-003`, so the task stays unchecked. No identity, attribute, ordering, ownership state, existing gap, fixture, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-034` | 2026-08-20 | Codex | PR #346 | Pinned CPython 3.13 and pdfplumber v0.11.10 established exact zero-byte rejection, real zero-page empty surfaces and serialization, invalid-selection behavior, and a truncation acceptance set. Across every 1–100-byte suffix removal from `basic_text.pdf`, both final implementations accept only 1–11 and 20–21. Red: the exact PDF-033 wheel rejected removals 5, 10, and 20. A first candidate accepted the partial-`startxref` band 12–19, and the strengthened differential forced those cases back to rejection. Green: fresh exact-Maturin 1.14.1 wheel and sdist runs each passed all forty-five artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, binding and core Clippy denied warnings, and the complete core workspace tests/doctests passed. | Recovery composes with incorrect-offset repair but requires a traditional xref table, a closed trailer dictionary, and either no terminal token or a complete `startxref` token with a truncated numeric/EOF suffix. Incomplete trailers and partial tokens stay rejected. Exact severe-truncation messages and the broader malformed-object corpus remain open under `PDF-014`, `REPAIR-012`, and strict `CI-001`–`CI-003`, so the task stays unchecked. No acceptance boundary, malformed case, failure type, stream state, serialization value, existing gap, fixture, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PDF-035` | 2026-08-21 | Codex | PR #347 | Pinned CPython 3.13 and pdfplumber v0.11.10 expose none of `bookmarks`, `form_fields`, `signatures`, `validate`, `extract_images`, or `rust` on `PDF`. Red against the exact PDF-034 wheel: the namespace contract failed because direct `PDF.bookmarks` existed, and the data contract errored because `.rust` was absent. Green: one deterministic PDF exercised a 0-based outline destination, text and signed form fields, signature metadata, a catalog validation warning, and a one-byte image. Fresh exact-Maturin 1.14.1 wheel and isolated-target sdist runs each passed all forty-seven artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, binding and core Clippy denied warnings, and the complete core workspace tests/doctests passed. | All five extension families are reachable only through `document.rust`; the upstream-facing `PDF.bookmarks` method was removed. The first sdist install reused the shared Cargo target and reproduced stale PDF-034 code in five truncation cases, so it was rejected; rebuilding the same sdist with a fresh explicit target and no pip cache passed 47/47. Extension stabilization and strict section 10 remain open under `EXT-001`–`EXT-004`, `EXT-008`, `EXT-013`–`EXT-015`, and `CI-001`–`CI-003`, so the task stays unchecked. No namespace, method family, field, byte, index, validation issue, failure, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PAGE-003` | 2026-08-21 | Codex | PR #348 | Pinned CPython 3.13 and pdfplumber v0.11.10 returned full-view `.initial_doctop` values `[0, 841.89, 1683.78, 2525.67, 3367.56]` and selected-view values `[0, 841.89]` for original pages 3 and 5, with exact Python types `int` for the zero sentinel and `float` thereafter. Red against the exact PDF-035 wheel: the property was absent and `to_dict()` emitted `0.0`. Green: fresh exact-Maturin 1.14.1 wheel and isolated-target sdist runs each passed all forty-eight artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, binding and core Clippy denied warnings, formatting and diff checks passed, and the complete current CI-equivalent core workspace tests/doctests passed. | The binding reuses the existing selected/full cumulative-height calculation and converts only exact zero to the upstream integer sentinel in direct and serialized forms. It does not guess source numeric types from integer-looking `f64` values. Raw PDF-number provenance for later integer-valued page heights and strict section 10 remain open under `SER-009`, `CI-001`, `CI-002`, and `CI-003`, so the task stays unchecked. No page, view, value, type, serialization byte, fixture, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PAGE-004` | 2026-08-21 | Codex | PR #349 | Pinned CPython 3.13 and pdfplumber v0.11.10 returned normalized integer rotations `270`, `90`, and `0` for inherited `/Rotate -90`, explicit `/Rotate 450`, and explicit `/Rotate 360`, respectively, through both `Page.rotation` and `Page.to_dict([])`. Red against the exact PAGE-003 wheel: the direct property was absent and serialization returned the raw values. Green: fresh exact-Maturin 1.14.1 wheel and isolated-target sdist runs each passed all forty-nine artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, both focused core rotation tests and the complete current CI-equivalent core workspace tests/doctests passed, both Clippy lanes denied warnings, and formatting and diff checks passed. | The core caches inherited rotations normalized modulo 360 beside page display geometry, and extracted `Page` values now retain the same normalized value. The binding exposes that cache without forcing content interpretation and serializes it consistently. Exhaustive real-fixture rotations, deeper nested inheritance, mixed page/text rotation, and strict section 10 remain open under `PAGE-022`–`PAGE-024` and `CI-001`–`CI-003`, so the task stays unchecked. No raw rotation, inherited value, normalized value, Python type, dimension, serialization field, failure, fixture, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PAGE-005` | 2026-08-21 | Codex | PR #350 | Pinned CPython 3.13 and pdfplumber v0.11.10 sorted the source MediaBox, swapped axes for normalized rotations 90/270, inverted to top-origin coordinates, and returned the same tuple through `Page.mediabox` and `Page.to_dict([])`. Inherited `/Rotate -90` and explicit `/Rotate 450` produced `(0, 0, 200, 100)` from `[0, 0, 100, 200]`; explicit `/Rotate 360` produced `(0, 0, 100, 200)`. Red against the exact PAGE-004 wheel: the direct property was absent and serialization retained the unrotated source box. Green: fresh exact-Maturin 1.14.1 wheel and isolated-target sdist runs each passed all fifty artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, the complete current CI-equivalent core workspace tests/doctests passed, both Clippy lanes denied warnings, and formatting and diff checks passed. | The core caches the source MediaBox beside lazy page geometry, and the binding derives one rotation-aware top-origin value for direct and serialized access without content interpretation. Real `page-boxes-example.pdf` still exposes f32-short coordinates instead of source PDF number precision/types. Nonzero, negative, inverted, exhaustive-rotation/inheritance, related bbox/dimension, and strict section 10 cases remain open under `SER-009`, `PAGE-010`, `PAGE-011`, `PAGE-022`, `PAGE-023`, and `CI-001`–`CI-003`, so the task stays unchecked. No axis, coordinate, value, type, rotation, serialization field, failure, fixture, test, threshold, tolerance, or assertion was omitted or weakened. |
| `PAGE-006` | 2026-08-21 | Codex | PR #351 | Pinned CPython 3.13 and pdfplumber v0.11.10 inherited CropBox through nested page trees, sorted reversed coordinates, swapped axes for normalized rotations 90/270, inverted against normalized MediaBox height, and used compatible MediaBox fallback. Inherited `/CropBox [10 20 90 180]` with `/Rotate -90` and explicit reversed `/CropBox [90 180 10 20]` with `/Rotate 450` both produced `(20, 10, 180, 90)` through the direct property and `to_dict([])`; missing CropBox with `/Rotate 360` produced `(0, 0, 100, 200)`. Red against the exact PAGE-005 wheel: the direct property was absent, the inherited case fell back to MediaBox, and the explicit case serialized raw reversed coordinates. Green: fresh exact-Maturin 1.14.1 wheel and isolated-target sdist runs each passed all fifty-one artifact tests with `ResourceWarning` promoted to error; binding tests were 50/50, compatibility-harness tests were 103/103, focused parser/core/binding regressions passed, the complete current CI-equivalent core workspace tests/doctests passed, both Clippy lanes denied warnings, and formatting and diff checks passed. | The core caches inherited source CropBox values beside lazy page geometry, and the binding derives one rotation-aware top-origin value for direct and serialized access without interpreting content. Real `page-boxes-example.pdf` still differs at f32-short coordinates: pinned `(14.17323, 42.519679999999994, 581.10236, 856.06299)` versus candidate `(14.17323, 42.519653, 581.10236, 856.063)`. Source-number precision/types, nonzero and negative origins, exhaustive rotations, deeper inheritance, related bbox/dimension behavior, and strict section 10 remain open under `SER-009`, `PAGE-010`, `PAGE-011`, `PAGE-022`, `PAGE-023`, and `CI-001`–`CI-003`, so the task stays unchecked. No axis, coordinate, value, type, inheritance level, rotation, fallback, serialization field, failure, fixture, test, threshold, tolerance, or assertion was omitted or weakened. |

---

## 14. Decision Log

| Decision ID | Status | Decision | Rationale |
|---|---|---|---|
| `DEC-001` | Proposed | Use a pure-Python compatibility package over a native module named `pdfplumber`, with native code at `pdfplumber._native` | Required for submodules, properties, callbacks, context managers, and exact Python signatures |
| `DEC-002` | Proposed | Keep Python compatibility semantics in adapters when changing the idiomatic Rust API would be a breaking change | Allows parity without unnecessary Rust SemVer breakage |
| `DEC-003` | Proposed | Strict compatibility output excludes Rust-only fields and methods | Prevents schema drift and future upstream-name collisions |
| `DEC-004` | Accepted | Reimplement `laparams` and higher-level layout objects natively; do not load `pdfminer.six` as a runtime fallback | Preserves `PYAPI-020` and isolated-wheel provenance. PDF-012 accepts, validates, and retains the pinned mapping contract; PAGE-018 remains responsible for native higher-level layout objects. |
| `DEC-005` | Open | Decide the supported depth of `.doc` and raw `pdfminer.six` internal compatibility | Some users rely on internals that are not practical native class-for-class replacements |
| `DEC-006` | Proposed | Better-than-upstream decoding is allowed only through approved deltas or an explicit enhanced mode | Silent output improvement can still break a drop-in consumer |
| `DEC-007` | Proposed | Preserve the current rebased-center crop only under an explicit Rust-extension API | Current behavior conflicts with Python `pdfplumber` crop semantics |
| `DEC-008` | Proposed | Keep the rich Rust subcommand CLI under a separate executable/mode and make `pdfplumber` upstream-compatible | Avoids sacrificing useful Rust functionality while restoring drop-in behavior |
| `DEC-009` | Accepted | Agents may autonomously merge focused PRs after exact-head CI, DCO, final-diff, and mergeability checks all pass | Completes the requested PR lifecycle while preserving red gates, stacked-PR ordering, and post-merge verification |

Agents may change a proposed decision only in a focused design pull request with tests and an updated rationale.

---

## 15. How to Add Newly Discovered Work

1. Add a stable identifier under the most relevant section.
2. Use the existing priority unless the gap blocks a higher-level gate.
3. State observable behavior, not an implementation guess.
4. Reference the failing fixture, upstream test, issue, or API snapshot.
5. Add dependencies when the task cannot be completed independently.
6. Keep the task unchecked.
7. Add it to the open-issue mapping when applicable.
8. Never renumber existing tasks.

A good task is independently testable and small enough for one focused pull request. Split tasks that require unrelated parser, Python, and documentation changes.

---

## 16. Final Compatibility Claim Template

Do not publish this claim until M4 is complete:

> `pdfplumber-rs` implements the documented Python `pdfplumber` v0.11.10 API and behavior for machine-generated PDFs. Compatibility is continuously verified against the upstream API snapshot, upstream tests, and a versioned differential corpus. Intentional deviations are listed in the public deviation registry. Rust-native extensions are isolated from strict compatibility output.

Until then, describe the project as a Rust port or compatibility work in progress, not a complete drop-in replacement.
