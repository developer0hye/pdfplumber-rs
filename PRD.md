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
7. Do not start another task until the current task is committed and its branch state is clear.

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
  - Unchecked because the candidate environment is declared and guarded but not yet constructed; it depends on `PYAPI-002` shipping an importable `pdfplumber` package.
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
- [ ] **PARITY-010** Compare page text, layout text, simple text, text lines, words, search results, tables, annotations, hyperlinks, and structure trees.
- [ ] **PARITY-011** Add an option-matrix runner that generates reference output for non-default values of every documented text and table option.
- [ ] **PARITY-012** Add differential tests for exceptions, warnings, malformed metadata, passwords, repair, closed resources, and invalid bounding boxes.
- [ ] **PARITY-013** Add exact JSON and CSV differential tests, including precision and include/exclude attribute behavior.
- [ ] **PARITY-014** Run the upstream Python test suite against the compatibility package; maintain a machine-readable list of temporarily unsupported tests.
- [ ] **PARITY-015** Require every temporary unsupported upstream test to reference an unchecked task in this document.
- [ ] **PARITY-016** Create `compat/approved_deltas.toml` and fail CI on unregistered output differences.
- [ ] **PARITY-017** Generate a machine-readable parity report with per-API, per-option, per-fixture, and per-page results.
- [ ] **PARITY-018** Generate a human-readable summary artifact that shows the first differing object and a compact coordinate/text diff.
- [ ] **PARITY-019** Add a PRD linter that rejects duplicate task identifiers and checked tasks without an Evidence Ledger row.
- [ ] **PARITY-020** Add fixture provenance and license metadata; do not commit private or redistribution-restricted PDFs.
- [ ] **PARITY-021** Import the upstream v0.11.10 PDF fixture corpus and preserve its directory names for traceability.
- [ ] **PARITY-022** Add the Rust repository's issue fixtures and external parser fixtures to one indexed corpus manifest.
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
- [ ] **PDF-003** Accept an already-open binary file object.
- [ ] **PDF-004** Accept `io.BytesIO` and equivalent seekable binary streams.
- [ ] **PDF-005** Preserve external-stream ownership: closing `PDF` must not close a caller-owned stream.
- [ ] **PDF-006** Close internally opened streams when `PDF.close()` is called.
- [ ] **PDF-007** Implement `PDF.__enter__` and `PDF.__exit__`.
- [ ] **PDF-008** Match behavior when operations are attempted after close.
- [ ] **PDF-009** Expose compatible `.stream`, `.path`, `.password`, and ownership state where public behavior depends on them.
- [ ] **PDF-010** Implement `pages=` selection using 1-based page numbers and preserve upstream ordering and validation.
- [ ] **PDF-011** Match page-number and `doctop` behavior when only selected pages are loaded.
- [ ] **PDF-012** Implement `laparams=` acceptance and define the native-versus-fallback strategy.
- [ ] **PDF-013** Implement `password=` for paths and streams.
- [ ] **PDF-014** Distinguish password-required, invalid-password, malformed-PDF, I/O, and unsupported-encryption failures compatibly.
- [ ] **PDF-015** Implement `strict_metadata=`.
- [ ] **PDF-016** Implement `unicode_norm=` with `NFC`, `NFD`, `NFKC`, and `NFKD`.
- [ ] **PDF-017** Implement `repair=`.
- [ ] **PDF-018** Implement `gs_path=`.
- [ ] **PDF-019** Implement `repair_setting=` values `default`, `prepress`, `printer`, `ebook`, and `screen`.
- [ ] **PDF-020** Implement `raise_unicode_errors=`.
- [ ] **PDF-021** Match metadata dictionary keys, decoded values, list-valued metadata, warnings, and strict failures.
- [ ] **PDF-022** Make `.pages` lazy/cached in a behaviorally compatible way.
- [ ] **PDF-023** Implement `PDF.close()` and page cache cleanup.
- [ ] **PDF-024** Implement `PDF.flush_cache()` behavior inherited from the container surface.
- [ ] **PDF-025** Implement `.objects` aggregated by object type.
- [ ] **PDF-026** Implement `.annots` aggregated across pages.
- [ ] **PDF-027** Implement `.hyperlinks` aggregated across pages.
- [ ] **PDF-028** Implement `.structure_tree`.
- [ ] **PDF-029** Implement `.to_dict(object_types=None)`.
- [ ] **PDF-030** Implement `.to_json(...)`.
- [ ] **PDF-031** Implement `.to_csv(...)`.
- [ ] **PDF-032** Match repeated-property access and cache identity where observable.
- [ ] **PDF-033** Match `repr(PDF)` and useful diagnostic attributes.
- [ ] **PDF-034** Add empty-document, zero-page, truncated-document, and invalid-page-selection tests.
- [ ] **PDF-035** Expose Rust-only bookmarks, forms, signatures, validation, and image extraction under clearly non-upstream names or namespaces.
- [ ] **PDF-036** Decide and document deep compatibility for `.doc` and other `pdfminer.six` internals; do not silently return unrelated native types.

### 8.4 P0 — Page Geometry, Identity, and Cache Semantics

- [ ] **PAGE-001** Present public `Page.page_number` as 1-based in Python.
- [ ] **PAGE-002** Preserve the idiomatic zero-based Rust page index without leaking it into compatibility mode.
- [ ] **PAGE-003** Implement `.initial_doctop`.
- [ ] **PAGE-004** Match `.rotation`, including normalization and inherited rotation.
- [ ] **PAGE-005** Match `.mediabox`.
- [ ] **PAGE-006** Match `.cropbox`.
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
| `PARITY-004` | unclaimed | — | — | `PYAPI-002` | Reference side is done. The candidate environment cannot be built until an importable `pdfplumber` package ships. |

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

---

## 14. Decision Log

| Decision ID | Status | Decision | Rationale |
|---|---|---|---|
| `DEC-001` | Proposed | Use a pure-Python compatibility package over a native module named `pdfplumber`, with native code at `pdfplumber._native` | Required for submodules, properties, callbacks, context managers, and exact Python signatures |
| `DEC-002` | Proposed | Keep Python compatibility semantics in adapters when changing the idiomatic Rust API would be a breaking change | Allows parity without unnecessary Rust SemVer breakage |
| `DEC-003` | Proposed | Strict compatibility output excludes Rust-only fields and methods | Prevents schema drift and future upstream-name collisions |
| `DEC-004` | Open | Decide whether `laparams` is reimplemented natively or provided through an optional `pdfminer.six` fallback | Higher-level layout object compatibility is substantial |
| `DEC-005` | Open | Decide the supported depth of `.doc` and raw `pdfminer.six` internal compatibility | Some users rely on internals that are not practical native class-for-class replacements |
| `DEC-006` | Proposed | Better-than-upstream decoding is allowed only through approved deltas or an explicit enhanced mode | Silent output improvement can still break a drop-in consumer |
| `DEC-007` | Proposed | Preserve the current rebased-center crop only under an explicit Rust-extension API | Current behavior conflicts with Python `pdfplumber` crop semantics |
| `DEC-008` | Proposed | Keep the rich Rust subcommand CLI under a separate executable/mode and make `pdfplumber` upstream-compatible | Avoids sacrificing useful Rust functionality while restoring drop-in behavior |

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
