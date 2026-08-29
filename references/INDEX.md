# Reference Projects Index

Projects organized by domain. Read the specific file matching your current problem.

## Direct Upstream

| Project | Language | File | Use When |
|---------|----------|------|----------|
| [pdfplumber](https://github.com/jsvine/pdfplumber) | Python | [pdfplumber.md](pdfplumber.md) | API design, table detection pipeline, overall architecture |
| [pdfminer.six](https://github.com/pdfminer/pdfminer.six) | Python | [pdfminer-six.md](pdfminer-six.md) | Text layout (LAParams), font metrics, CMap handling, char extraction |

## Rust PDF Ecosystem

| Project | Stars | File | Use When |
|---------|-------|------|----------|
| [lopdf](https://github.com/J-F-Liu/lopdf) | 2.1k | [lopdf.md](lopdf.md) | PDF object access, content stream decoding, and supported input readers (current backend) |
| [pdf-rs](https://github.com/pdf-rs/pdf) | 1.6k | [pdf-rs.md](pdf-rs.md) | Typed PDF object model, derive-macro patterns |
| [pdf-extract](https://github.com/jrmuizel/pdf-extract) | 571 | [pdf-extract.md](pdf-extract.md) | CMap/CFF/Type1 parsing crate ecosystem on lopdf |
| [pdf_oxide](https://github.com/yfedoseev/pdf_oxide) | 975 | [pdf-oxide.md](pdf-oxide.md) | High-level Rust facade design, competitive analysis, or self-published benchmark methodology |
| [pdfsink-rs](https://github.com/clark-labs-inc/pdfsink-rs) | — | [pdfsink-rs.md](pdfsink-rs.md) | Materially equivalent text/table options or pinned cross-project benchmark adapters |
| [pdfium-render](https://github.com/ajrcarey/pdfium-render) | — | [pdfium-render.md](pdfium-render.md) | Borrowed page-collection views, direct indexed selection, and lazy iteration |

## Rust API Design

| Source | File | Use When |
|---|---|---|
| Cargo and the Rust Reference | [rust-public-data-models.md](rust-public-data-models.md) | Reviewing the curated data-model boundary, public-field compatibility, or exhaustive enums |
| Serde | [rust-serde-schema.md](rust-serde-schema.md) | Reviewing curated JSON field, value-shape, or enum-encoding compatibility |
| Rust standard library and thiserror | [rust-errors.md](rust-errors.md) | Reviewing public error kinds, source chains, safe formatting, or context propagation |
| Rust standard library, Rayon, and PyO3 | [rust-concurrency.md](rust-concurrency.md) | Reviewing `Send`/`Sync`, shared resource budgets, parallel page ordering, thread pools, or Python GIL boundaries |
| rustc, rustdoc, and Clippy | [rust-rustdoc.md](rust-rustdoc.md) | Reviewing missing public docs, link validation, or fallible and panicking API documentation gates |
| Cargo | [rust-examples.md](rust-examples.md) | Reviewing compiled example targets and feature-specific example gates |
| Cargo | [cargo-features.md](cargo-features.md) | Reviewing additive feature flags, defaults, unification, and representative combination gates |
| rustdoc | [rust-compile-fail.md](rust-compile-fail.md) | Reviewing intentional compilation failures and their positive alternatives |
| cargo-semver-checks and Cargo | [rust-semver-checks.md](rust-semver-checks.md) | Reviewing release API compatibility, baselines, approved breaks, or migration-note gates |
| Rust Reference and Cargo | [rust-deprecation.md](rust-deprecation.md) | Reviewing deprecation annotations, support windows, removal gates, or safety exceptions |
| Rust and Cargo | [rust-toolchain-policy.md](rust-toolchain-policy.md) | Reviewing the rolling stable toolchain policy or dependency compiler requirements |
| Rust API Guidelines, standard library, Rust Reference, and RFC 1105 | [rust-api-design.md](rust-api-design.md) | Reviewing ownership, allocations, iterators, determinism, errors, extension traits, or future compatibility |
| Cargo | [rust-ttfv.md](rust-ttfv.md) | Reproducing or reviewing clean-project Rust time-to-first-value measurements and cache isolation |
| Cargo | [cargo-packaging.md](cargo-packaging.md) | Reviewing verified crate archives, publish dry runs, coordinated workspace releases, or exact-commit package provenance |
| Rust, GitHub Actions, and Typst | [rust-cli-binaries.md](rust-cli-binaries.md) | Reviewing native target matrices, versioned CLI archives, or release-asset build gates |
| GitHub Actions and Anchore Syft | [release-artifact-integrity.md](release-artifact-integrity.md) | Reviewing release checksums, SPDX SBOMs, build provenance, Sigstore attestations, or verification commands |
| Node.js, TypeScript, Vite, Playwright, and wasm-pack | [wasm-package-testing.md](wasm-package-testing.md) | Reviewing prepublication Node and maintained-browser package installation, type-checking, or execution gates |
| crates.io, PyPI, npm, GitHub Actions, and Node.js | [trusted-publishing.md](trusted-publishing.md) | Reviewing registry OpenID Connect bindings, short-lived publisher credentials, job permissions, or client-version floors |
| Python Packaging User Guide, PyO3, and CPython | [python-support-metadata.md](python-support-metadata.md) | Reviewing `Requires-Python`, version and implementation classifiers, tested artifact matrices, or Python 3.14 exclusions |
| PyPA auditwheel, PEP 600, and Python Packaging User Guide | [python-linux-wheels.md](python-linux-wheels.md) | Reviewing Linux wheel platform tags, allowed system libraries, external shared-library audits, or instruction-set compatibility |
| GitHub Actions, Rust, Apple, and Maturin | [python-macos-wheels.md](python-macos-wheels.md) | Reviewing native macOS wheel runners, deployment targets, Mach-O architectures, or installed-wheel execution |
| GitHub Actions, Microsoft, and CPython | [python-windows-wheels.md](python-windows-wheels.md) | Reviewing Windows PE imports, native wheel installation, or non-ASCII and long-path execution |
| Cargo, PyPI, npm, and GitHub | [release-recovery.md](release-recovery.md) | Containing registry lag, partial publication, compromised credentials, or incorrect release claims |
| Development Containers, Docker, and Rust Official Images | [rust-dev-containers.md](rust-dev-containers.md) | Reproducing the pinned Rust contributor environment or updating its image digest |

## Benchmark Design

| Source | File | Use When |
|---|---|---|
| [MLPerf Inference](https://github.com/mlcommons/inference_policies) | [mlperf-inference.md](mlperf-inference.md) | Separating output accuracy or equivalence validation from performance measurement |

## Font Parsing

| Project | Language | File | Use When |
|---------|----------|------|----------|
| [ttf-parser](https://github.com/harfbuzz/ttf-parser) | Rust | [ttf-parser.md](ttf-parser.md) | TrueType hmtx/head/hhea/maxp table parsing |
| [allsorts](https://github.com/yeslogic/allsorts) | Rust | [allsorts.md](allsorts.md) | CFF parsing (Top DICT, Private DICT, CharStrings) |
| [Apache PDFBox](https://github.com/apache/pdfbox) | Java | [pdfbox.md](pdfbox.md) | Font class hierarchy, width resolution, CID fonts |
| [pdf.js](https://github.com/mozilla/pdf.js) | JS | [pdfjs.md](pdfjs.md) | CFF width extraction, CMap binary format |
| [adobe-cmap-parser](https://github.com/jrmuizel/adobe-cmap-parser) | Rust | [adobe-cmap-parser.md](adobe-cmap-parser.md) | CMap file parsing for CJK fonts |

## Table Detection

| Project | Language | File | Use When |
|---------|----------|------|----------|
| [Camelot](https://github.com/camelot-dev/camelot) | Python | [camelot.md](camelot.md) | Lattice (OpenCV morphology) and stream parser algorithms |
| [tabula-java](https://github.com/tabulapdf/tabula-java) | Java | [tabula-java.md](tabula-java.md) | Ruling class design, spreadsheet cell extraction |

## Text Layout & Reading Order

| Project | Language | File | Use When |
|---------|----------|------|----------|
| [PdfPig](https://github.com/UglyToad/PdfPig) | C# | [pdfpig.md](pdfpig.md) | Multiple layout algorithms (XY Cut, Docstrum, Nearest Neighbour) |
| [Poppler](https://gitlab.freedesktop.org/poppler/poppler) | C++ | [poppler.md](poppler.md) | Column detection, TextOutputDev coalesce algorithm |
| [MuPDF](https://github.com/ArtifexSoftware/mupdf) | C | [mupdf.md](mupdf.md) | Stream-order text hierarchy (Block→Line→Span→Char) |

## Utility Crates (Rust)

| Crate | Use When |
|-------|----------|
| [unicode-bidi](https://github.com/servo/unicode-bidi) | BiDi text (UAX #9) |
| [jieba-rs](https://github.com/messense/jieba-rs) | Chinese word segmentation |
| [unicode-segmentation](https://crates.io/crates/unicode-segmentation) | Unicode word boundaries (UAX #29) |

## Product Documentation

| Project | Language | File | Use When |
|---------|----------|------|----------|
| [Polars](https://github.com/pola-rs/polars) | Rust/Python | [polars.md](polars.md) | Keeping product or performance claims adjacent to their evidence |
| [reqwest](https://github.com/seanmonstar/reqwest) | Rust | [reqwest.md](reqwest.md) | Writing a concise, complete, fallible Rust quick start |
| [wpt.fyi and EARL](https://github.com/web-platform-tests/wpt.fyi) | Go / standard | [compatibility-scorecards.md](compatibility-scorecards.md) | Designing machine-readable compatibility runs, identities, provenance, and explicit untested outcomes |
