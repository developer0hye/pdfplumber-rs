# pdfplumber-rs Current Implementation Verification Audit

## 1. Audit baseline

- Repository: `developer0hye/pdfplumber-rs`
- Audited branch: `main`
- Audited commit: `da0663ce27f35bfc641055c0cebf8fae97932ac4`
- Commit date: 2026-08-11
- Python compatibility target: `jsvine/pdfplumber` `v0.11.10`
- Audit date: 2026-08-16
- Current GitHub Actions CI run: `31538234423` (`success`)

This audit verifies the 37 checked items in Section 6 of `PRD.md` at four different evidence levels:

1. **Source presence** — the module, type, function, workflow, script, or binding exists.
2. **Required CI coverage** — the current required CI compiles and executes tests for that surface on every pull request or push to `main`.
3. **Behavioral verification** — tests assert meaningful behavior rather than merely compiling, skipping, printing a report, or using permissive thresholds.
4. **Exact compatibility verification** — behavior, defaults, ordering, coordinates, schemas, errors, and lifecycle match Python `pdfplumber` `v0.11.10` under a pinned and reproducible differential suite.

### Execution limitation

The audit environment could not clone GitHub directly because outbound DNS was unavailable. Runtime conclusions therefore use the GitHub Actions logs for the exact audited commit, while source and compatibility conclusions use the pinned repository files and the pinned upstream source. This is strong evidence of the repository's current CI state, but it is not a second independent machine reproduction.

---

## 2. Executive verdict

### 2.1 What is valid

All **37 of 37** checked statements are valid as **source-inventory statements**. The referenced surface exists in the audited source tree.

### 2.2 What is not valid

The current source is **not in a state where those 37 items can be interpreted as 100% implemented, 100% regression-gated, or 100% compatible with Python `pdfplumber`**.

Audit classification:

| Classification | Count | Meaning |
|---|---:|---|
| `VERIFIED-SURFACE` | 4 | Crate-level surface exists and is compiled/tested by current required CI. This verifies architecture, not Python compatibility. |
| `SOURCE-ONLY` | 11 | Surface exists, but current required CI does not build or test it. |
| `HISTORICAL-ONLY` | 1 | Workflow exists and an older tagged release succeeded, but current `main` artifacts are not continuously verified. |
| `PARTIAL` / `INCOMPATIBLE` | 16 | Native implementation and tests exist, but exact behavior is incomplete, permissively tested, or known to differ from Python. |
| `INFRA-ONLY` | 5 | Compatibility tooling or fixtures exist, but they do not constitute a strict compatibility gate. |

**Strict drop-in replacement completion: `0 / 37`.**

The `PRD.md` disclaimer is therefore correct: the current `[x]` marks prove source observation only. However, a single checkbox remains easy for Claude, Codex, or a human reviewer to misread as completion and should be replaced with multi-level evidence fields.

---

## 3. Current required CI: what it actually proves

The current CI executes:

```text
cargo fmt --all -- --check
cargo clippy --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm -- -D warnings
cargo check --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm
cargo test --workspace --exclude pdfplumber-py --exclude pdfplumber-wasm
```

Consequences:

- `pdfplumber-py` is excluded from `cargo check`, `cargo clippy`, and `cargo test`.
- `pdfplumber-wasm` is excluded from `cargo check`, `cargo clippy`, and `cargo test`.
- Optional `serde` and `parallel` features are not enabled because CI runs default features only.
- `serde_roundtrip.rs` is guarded by `#![cfg(feature = "serde")]`; the current default-feature CI therefore executes zero tests from that file.
- There is no required wheel build, wheel install, Python import, Python differential test, `stubtest`, WebAssembly build, Node test, browser test, or published-artifact smoke test on normal pull requests.

A green current CI run proves the default-feature native Rust workspace, excluding the Python and WebAssembly crates, is green. It does not prove the complete workspace or the replacement contract.

---

## 4. Strictness defects in the existing parity tests

### 4.1 Broad thresholds are diagnostic, not compatibility gates

The main cross-validation suite accepts:

- `95%` character and word rates for many Python fixtures.
- `90%` table-cell rate for selected table fixtures.
- `80%` character and word rates for external fixtures.
- Special thresholds as low as:
  - `30% / 30%`
  - `15% / 5%`
  - `50% / 5%`
  - `50% / 0%`

Examples in current source include:

```text
extra-attrs-example.pdf                 30% chars / 30% words
pr-136-example.pdf                     15% chars / 5% words
pr-138-example.pdf                     15% chars / 5% words
issue-842-example.pdf                  50% chars / 5% words
pdfjs/noembed-identity-2.pdf           50% chars / 0% words
```

A test that allows `0%` word agreement can prevent parse regressions, but cannot verify a Python replacement.

### 4.2 Coordinate comparison is too loose

The current cross-validation coordinate tolerance is `1.0` point. The PRD's compatibility target is `0.05` point before user-requested rounding.

### 4.3 Ignored tests do not assert

`cross_validate_ignored!` expands to an ignored test that only runs validation and prints a summary. It does not assert a threshold even when explicitly included. Current CI leaves nine cross-validation cases ignored. Some ignore reasons are stale, while others cover real extraction failures.

### 4.4 Informational reports cannot fail CI

`cross_validate_all_fixtures_summary` and the summary tooling are explicitly informational and never fail on below-target results. They are useful diagnostics, not release gates.

### 4.5 The standalone parity report is narrow

`scripts/parity_report.py` currently compares only the first page of each PDF and only:

- characters
- words
- page text
- lattice table output

It does not compare the complete Python API, every page, object schemas, annotations, links, structure trees, metadata semantics, exceptions, lifecycle, crop semantics, rendering, serialization, or package behavior.

### 4.6 Golden generation is not reproducibly pinned

`scripts/setup_golden_venv.sh` installs `pdfplumber` without a version pin or hash. Regenerating golden data on a later date can silently change the oracle.

---

## 5. Known current correctness gaps

These are not hypothetical coverage concerns; they are current, tracked behavioral gaps.

1. **90° and 270° text extraction** — three ignored tests produce one-word-per-line output or reversed text instead of stable sentence text.
2. **Rotated table-cell ordering** — a 90° rotated table reads words in the opposite order from Python `pdfplumber`; the tracked fixture's table-cell agreement regressed to `0.522` after the latest cell-order fix.
3. **Rotated table reconstruction** — one tracked rotated National Instant Criminal Background Check System fixture reports only `5.6%` table parity despite `100%` text/object metrics.
4. **Identity no-embed decoding** — a fixture is accepted with a `50%` character threshold and a `0%` word threshold.
5. **Tagged TrueType extraction** — `hello_structure.pdf` remains ignored with a severe character/word gap.
6. **Word/table recovery** — `issue-848.pdf` has full character recovery but historically very low word and table agreement and remains ignored.
7. **Real-world table miss** — an open report dated 2026-08-15 states a relatively standard table detected by Python `pdfplumber` is not detected by `pdfplumber-rs`.

These gaps alone rule out a 100% compatibility verdict for text, fonts, rotation, and tables.

---

## 6. Audit of all 37 checked inventory items

Legend:

- `YES` — observed in pinned source.
- `GATED` — current required CI builds/tests the relevant crate or behavior.
- `PARTIAL` — some tests exist, but exact contract coverage is incomplete or permissive.
- `NO` — no current required gate or exact parity evidence.
- `INCOMPATIBLE` — current behavior is known to differ from the target.
- `N/A` — Python parity is not directly applicable to the architecture-presence statement.

### 6.1 Architecture and distribution

| ID | Checked statement | Source | Required CI | Exact parity/artifact proof | Audit verdict |
|---|---|---:|---:|---:|---|
| INV-001 | `pdfplumber-core` contains backend-independent types and algorithms | YES | GATED | N/A | `VERIFIED-SURFACE` |
| INV-002 | `pdfplumber-parse` contains parsing, font, CMap, and interpreter code | YES | GATED | N/A | `VERIFIED-SURFACE` |
| INV-003 | `pdfplumber` exposes a public Rust facade | YES | GATED | N/A | `VERIFIED-SURFACE` |
| INV-004 | `pdfplumber-cli` exposes multiple subcommands | YES | GATED | NO upstream CLI parity matrix | `VERIFIED-SURFACE`, compatibility unverified |
| INV-005 | `pdfplumber-py` exposes PyO3 classes | YES | NO | NO | `SOURCE-ONLY` |
| INV-006 | `pdfplumber-wasm` provides a WebAssembly surface | YES | NO | NO | `SOURCE-ONLY`; JavaScript test can skip and return success when its fixture is absent |
| INV-007 | Optional `serde` serialization exists | YES | NO | NO | `SOURCE-ONLY`; feature-gated tests are not enabled in required CI |
| INV-008 | Optional Rayon parallel page processing exists | YES | NO | NO | `SOURCE-ONLY`; no required feature-specific race/determinism gate |
| INV-009 | Release automation builds Rust, Python, and Node packages | YES | Tag-only | Historical `v0.2.0` release succeeded | `HISTORICAL-ONLY`; current `main` artifacts are not continuously installed and exercised |

### 6.2 PDF and page processing

| ID | Checked statement | Source | Required CI | Exact Python parity | Audit verdict |
|---|---|---:|---:|---:|---|
| INV-010 | Open from bytes and filesystem paths | YES | PARTIAL | NO | Native APIs are tested; Python file-like objects, `pathlib.Path`, complete options, and ownership semantics are missing |
| INV-011 | Password-protected PDF APIs exist | YES | PARTIAL | NO | Core surface exists; complete Python exposure and encryption compatibility matrix are absent |
| INV-012 | Best-effort repair path exists | YES | PARTIAL | NO | Native repair exists; Python's Ghostscript-based options and behavior are not matched |
| INV-013 | Metadata extraction exists | YES | PARTIAL | NO | Basic metadata exists; strict parsing, warnings, raw key/value behavior, and Python schema are not matched |
| INV-014 | Page boxes, dimensions, rotation, and iteration exist | YES | PARTIAL | INCOMPATIBLE | Rotation text gaps remain; Python binding exposes 0-based `page_number` instead of 1-based |
| INV-015 | Characters, words, lines, blocks, and text extraction exist | YES | PARTIAL | NO | Multiple known text/font/rotation outliers; permissive thresholds and ignored tests remain |
| INV-016 | Lines, rectangles, curves, edges, and image metadata exist | YES | PARTIAL | NO | Native extraction exists; Python object dictionaries and field names are incomplete/incompatible |
| INV-017 | Crop, bbox filters, filter, and dedupe surfaces exist | YES | PARTIAL | INCOMPATIBLE | Current crop uses object centers and rebases coordinates; Python clips intersecting objects and preserves root coordinates |
| INV-018 | Search exists | YES | PARTIAL | NO | Basic regex/literal search exists; full groups, chars, compiled-pattern, layout, and option behavior are not exposed/matched |
| INV-019 | All table strategies and per-axis selection exist | YES | PARTIAL | NO | Strategy code exists; rotated and real-world table misses remain |
| INV-020 | Table cells, rows, columns, merged handling, and metrics exist | YES | PARTIAL | NO | Rotated cell ordering and table-grid parity remain unresolved |
| INV-021 | Annotation, link, form, structure, bookmark, and signature code exists | YES | PARTIAL | NO | Native code exists; complete Python API/schema/ordering differential coverage is absent |
| INV-022 | Image content extraction/export exists | YES | PARTIAL | NO | Native code exists; current Python binding does not expose the complete surface or schema |
| INV-023 | HTML and SVG export/debug surfaces exist | YES | PARTIAL | NO | Native extras exist; they are not equivalent to Python `PageImage` and are not Python-compatibility tested |
| INV-024 | Unicode normalization, BiDi, CJK, glyph lists, and font parsers exist | YES | PARTIAL | NO | Extensive implementation exists, but no-embed, tagged TrueType, and vertical-text gaps remain |
| INV-025 | Validation warnings and resource budgets exist | YES | PARTIAL | NO | Concepts and tests exist; there is no complete public-API panic, timeout, allocation, and malformed-input gate |

### 6.3 Compatibility infrastructure

| ID | Checked statement | Source | Required CI | Strict gate | Audit verdict |
|---|---|---:|---:|---:|---|
| INV-026 | Golden-data generation scripts exist | YES | NO | NO | `INFRA-ONLY`; upstream install is not pinned reproducibly |
| INV-027 | Python-versus-Rust parity report exists | YES | NO | NO | `INFRA-ONLY`; first page and a narrow output subset only |
| INV-028 | Cross-validation compares chars, words, lines, rects, and tables | YES | GATED | NO | `PARTIAL`; loose thresholds, ignored non-asserting cases, and informational summaries remain |
| INV-029 | Multiple real-world/upstream fixtures exist | YES | PARTIAL | NO | `INFRA-ONLY`; corpus presence does not prove coverage or exactness |
| INV-030 | Open issues track extraction outliers | YES | N/A | N/A | `INFRA-ONLY`; this verifies issue tracking and simultaneously proves known incompleteness |

### 6.4 Python binding

| ID | Checked statement | Source | Required CI | Exact Python compatibility | Audit verdict |
|---|---|---:|---:|---:|---|
| INV-031 | `PDF.open(path)` and `PDF.open_bytes(data)` exist | YES | NO | INCOMPATIBLE | `SOURCE-ONLY`; no top-level `pdfplumber.open`, incomplete inputs/options/password/repair surface |
| INV-032 | `PDF.pages` and `PDF.metadata` exist | YES | NO | NO | `SOURCE-ONLY`; eager reconstruction, lifecycle, cache, and metadata semantics are unverified |
| INV-033 | Basic page dimensions and page number exist | YES | NO | INCOMPATIBLE | `SOURCE-ONLY`; public page number is 0-based rather than Python's 1-based numbering |
| INV-034 | Basic char/text/word/table/geometry/crop/search methods exist | YES | NO | INCOMPATIBLE | `SOURCE-ONLY`; Python properties are exposed as methods, signatures are narrow, schemas differ, and crop semantics conflict |
| INV-035 | Basic table bbox, rows, extraction, and accuracy exist | YES | NO | NO | `SOURCE-ONLY`; full cells/columns/settings/debug behavior is not matched or tested from Python |
| INV-036 | Python exception subclasses exist | YES | NO | NO | `SOURCE-ONLY`; no differential exception type/message/trigger matrix |
| INV-037 | Type stub and Python package metadata exist | YES | NO | NO | `SOURCE-ONLY`; no wheel import test, `stubtest`, static type test, or stub/runtime consistency gate |

---

## 7. Python drop-in compatibility blockers confirmed from source

The following current behaviors prevent a drop-in replacement even before document extraction accuracy is considered:

| Area | Current binding | Python `pdfplumber` target |
|---|---|---|
| Top-level open | `pdfplumber.PDF.open(...)` only | `pdfplumber.open(...)` and `PDF.open(...)` |
| Package layout | Single native extension module | Python package with public modules such as `page`, `table`, `utils`, `display`, `structure`, `repair`, and `ctm` |
| Page number | 0-based | 1-based |
| Object access | `chars()`, `lines()`, `rects()`, etc. | `.chars`, `.lines`, `.rects`, etc. properties |
| Open inputs | String path and bytes | String, `pathlib.Path`, binary stream, and `BytesIO` |
| Open options | Minimal | Pages, layout parameters, password, strict metadata, Unicode normalization, repair controls, Unicode-error policy |
| Lifecycle | No compatible context manager/close/cache model | `with`, `close`, owned/external stream handling, cache flushing |
| Object schemas | Small Rust-oriented subsets | Full Python object dictionaries and established field names |
| Crop | Center inclusion and coordinate rebasing | Intersection clipping and root-coordinate preservation |
| Visual debugging | SVG-oriented native API | `PageImage` raster rendering and drawing API |

These are deterministic API incompatibilities, not statistical extraction differences.

---

## 8. What “100% verifiable” should mean

No finite test suite can prove correctness for every possible or malformed PDF. A realistic 100% target must mean:

> **100% of the declared public compatibility contract and 100% of the pinned regression corpus are enforced by deterministic required checks.**

A checked completion item should require all applicable evidence below.

### 8.1 Source evidence

- Public symbol exists.
- Signature and default values are documented.
- No placeholder, silent skip, unimplemented branch, or unsupported-option fallback remains.

### 8.2 Build evidence

- Default Rust features compile and test.
- `serde` feature compiles and all round-trip tests execute.
- `parallel` feature compiles and determinism/thread-safety tests execute.
- `wasm32-unknown-unknown` builds.
- Python extension builds for every supported interpreter and platform.

### 8.3 Differential evidence

- Upstream version and dependency lock are pinned.
- Every public Python symbol has an API manifest test.
- Signatures, defaults, property/method shape, return types, dictionary keys, ordering, exceptions, warnings, and lifecycle are compared.
- Every fixture page is compared, not only page zero.
- Character/word/object counts and text are exact unless an explicitly registered delta exists.
- Coordinate delta is at most `0.05` point.
- Table bounding boxes, cells, rows, columns, `None` placement, text, and ordering are exact.
- No compatibility test uses a threshold below the declared release requirement.
- No required compatibility case is ignored or informational-only.

### 8.4 Artifact evidence

- Build wheel, install into a clean environment, import, and execute smoke/differential tests.
- Build source distribution, install it, and repeat smoke tests.
- Build the Node Package Manager package, import it in Node and a browser-like runner, and execute real fixture tests.
- Exercise exactly the artifacts that would be published.

### 8.5 Reliability evidence

- Continuous fuzzing or scheduled fuzzing covers parser and extraction entry points.
- Time, object-count, decompression, recursion, and image-byte limits have adversarial tests.
- Panic paths are audited; public methods do not rely on unchecked floating-point `partial_cmp(...).unwrap()` or Python conversion `unwrap()` calls.
- Sanitizer, Miri where applicable, and malformed-corpus jobs run separately from fast pull-request tests.

---

## 9. Required CI changes before a 100% verification claim

At minimum, add required jobs equivalent to:

```text
# Native Rust baseline
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets

# Explicit feature gates
cargo test -p pdfplumber-core --features serde --test serde_roundtrip
cargo test -p pdfplumber --features parallel
cargo test -p pdfplumber --no-default-features

# Python binding
maturin build --manifest-path crates/pdfplumber-py/Cargo.toml
pip install <built-wheel>
python -m pytest tests/python_compat
python -m mypy tests/python_typing
python -m stubtest pdfplumber

# WebAssembly
wasm-pack build --target nodejs crates/pdfplumber-wasm
node crates/pdfplumber-wasm/tests/test.mjs
wasm-pack test --node crates/pdfplumber-wasm

# Strict differential gate
python scripts/parity_report.py \
  --upstream-version 0.11.10 \
  --all-pages \
  --all-apis \
  --coordinate-tolerance 0.05 \
  --fail-on-difference

# Published artifact smoke tests
pip install <wheel-or-sdist>
python tests/artifact_smoke.py
npm install <packed-tarball>
node tests/artifact_smoke.mjs
```

The exact command layout may need platform-specific jobs; the requirements, not these literal commands, are normative.

---

## 10. Recommended PRD checkbox model

Do not use one checkbox to represent both source presence and completion. Replace each broad item with evidence subchecks:

```markdown
- Feature: Python-compatible `Page.crop`
  - [x] Source surface exists.
  - [x] Native unit tests exist.
  - [ ] Required CI executes the tests on every change.
  - [ ] Python `v0.11.10` differential tests pass exactly.
  - [ ] Wheel-installed tests pass on the supported matrix.
  - Evidence: `<test path>`, `<CI run>`, `<parity report>`, `<commit>`
```

Recommended status vocabulary:

- `OBSERVED` — source exists.
- `NATIVE-TESTED` — native behavior is regression-gated.
- `PARITY-VERIFIED` — pinned upstream differential contract passes.
- `ARTIFACT-VERIFIED` — installed publishable artifacts pass.
- `COMPLETE` — all applicable levels pass and no registered blocker remains.

Under this model, all 37 current inventory items may remain `OBSERVED`; none should be marked `COMPLETE` as a Python `pdfplumber` replacement.

---

## 11. Final determination

The repository has a substantial and well-tested native Rust implementation. The existing architecture, parsing core, extraction algorithms, fixtures, and regression suite are meaningful engineering progress.

Nevertheless, the audited commit is **not 100% verifiable as a Python `pdfplumber` replacement** because:

- major shipped surfaces are excluded from required CI;
- optional features are not enabled in required CI;
- compatibility tests use permissive and sometimes near-zero thresholds;
- ignored tests do not assert;
- known text, font, rotation, and table gaps remain;
- deterministic Python API incompatibilities remain;
- compatibility tooling is not pinned or comprehensive;
- built release artifacts are not continuously installed and exercised.

**Final status: source inventory verified; replacement completion not verified.**
