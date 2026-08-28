# Choosing a PDF extraction library

This page is an evaluation aid, not a benchmark or a claim that one library wins every workload. External project facts were observed on 2026-08-26 from the revision-pinned primary sources below. Features and maintenance state can change, so re-check the linked projects before making a long-lived dependency decision.

## Observed facts

| Project | Public surface observed in its own documentation | Evidence boundary |
|---|---|---|
| `pdfplumber-rs` | A native Rust extraction API for text, words, coordinates, graphics, images, and tables. The workspace also contains Python, Command-Line Interface, and WebAssembly packages. | The project is at `0.3.x` alpha. Its Python `pdfplumber` v0.11.10 compatibility is incomplete, and the readiness of the four surfaces is not uniform. See the [root README](../README.md), [workspace manifest](../Cargo.toml), and [public roadmap](../ROADMAP.md). |
| `pdf_oxide` | A Rust core with extraction, creation, editing, many language bindings, WebAssembly, a Command-Line Interface, and a Model Context Protocol server. Its documentation also publishes its own benchmark and corpus results. | This is a broader toolkit than the current `pdfplumber-rs` adoption target. Its published speed and pass-rate figures are project-reported, not results reproduced by this repository. |
| `pdfsink-rs` | A pure-Rust library and Command-Line Interface exposing text, word, table, layout, image, metadata, serialization, rendering, and `pdfplumber`-inspired geometry APIs. Its repository includes benchmark scripts and reported results. | API resemblance is not exact Python compatibility. Its performance and accuracy figures are project-reported, not results reproduced by this repository. |
| Python `pdfplumber` | A Python library and Command-Line Interface built on `pdfminer.six`, with detailed PDF-object access, customizable text and table extraction, cropping, and visual debugging. | It is the behavior target for this project's Python migration work. Its own documentation says it works best on machine-generated PDFs and does not provide Optical Character Recognition. |
| `pdf-extract` | A Rust library whose public README presents a small in-memory plain-text extraction API. | It is an adjacent choice for narrower text-only needs, not a documented table, geometry, or Python-compatibility substitute. |

The external observations above come from these immutable source snapshots:

| Project | Revision-pinned primary source |
|---|---|
| `pdf_oxide` | [`3be1951b171edb9d69a10f42ef72ee73f52e51bf`](https://github.com/yfedoseev/pdf_oxide/blob/3be1951b171edb9d69a10f42ef72ee73f52e51bf/README.md) |
| `pdfsink-rs` | [`980d9f7b8ec44456f3d54427f4ced747b6eb6154`](https://github.com/clark-labs-inc/pdfsink-rs/blob/980d9f7b8ec44456f3d54427f4ced747b6eb6154/README.md) |
| Python `pdfplumber` | [`4c64b92d5caccd71c645e98e0fabb0c4dba7ff45`](https://github.com/jsvine/pdfplumber/blob/4c64b92d5caccd71c645e98e0fabb0c4dba7ff45/README.md) |
| `pdf-extract` | [`b95bf9f6268772d5088f09b0034e488e64294835`](https://github.com/jrmuizel/pdf-extract/blob/b95bf9f6268772d5088f09b0034e488e64294835/README.md) |

At the observation date, GitHub repository metadata reported all four external repositories as public and not archived. That metadata is time-sensitive; the immutable README links preserve only the feature snapshot.

## Reproducible measurements

No cross-project performance result is currently claimed by `pdfplumber-rs`.

`pdf_oxide` and `pdfsink-rs` publish performance and accuracy figures in their own revision-pinned READMEs. Those figures describe their authors' fixtures, requested outputs, environments, and methods. This project has not independently reproduced them and therefore does not repeat them as a ranking.

The current `pdfplumber-rs` corpus provides compatibility evidence, not a fair speed comparison. It indexes 223 PDFs and pins Python `pdfplumber` v0.11.10, but known parse failures and incomplete API/schema parity remain explicit in the [Evidence Ledger](../PRD.md#13-evidence-ledger).

The versioned [benchmark corpus](benchmarks/corpus-v0.3.0.md) selects ten redistributable, digest-bound inputs spanning the required semantic and size classes. The [output-equivalence preflight](benchmarks/equivalence-v0.3.0.md) then requires two distinct implementations to use the same fixture digest, exact request semantics, canonical output schema, and exact canonical JSON result before a case becomes eligible for timing. Errors, unsupported behavior, and any mismatch are blocked rather than timed.

The [pinned competitor suite](benchmarks/competitors-v0.3.0.md) binds Python `pdfplumber`, `pdf_oxide`, and `pdfsink-rs` to exact Git revisions and the candidate to its exact run head. It exercises only the `document-open` and page-preserving plain-text workloads supported by all four projects on identical corpus bytes. The complete output phase precedes timing, and a Python/candidate/competitor triple is timed only when all three exact canonical outputs match. Its single combined-process sample is local and unpublished, not a performance claim.

The [separated stage suite](benchmarks/stages-v0.3.0.md) replaces that combined scope with independent in-adapter monotonic clocks for document open, page materialization, character extraction, word grouping, table detection, canonical JSON serialization, and installed-candidate PyO3 conversion. Process launch and named prerequisites stay outside each clock. An implementation is timed only when its untimed stage result matches the pinned Python reference exactly and the timed invocation reproduces it. Fused or non-equivalent pinned APIs remain explicit semantic outcomes without a misleading component timing. The result is local and unpublished.

The [resource and artifact suite](benchmarks/metrics-v0.3.0.md) retains wall time as an uninstrumented pass and repeats only exact-output-eligible cases for stage-only process CPU and allocation observations. Peak resident memory is labeled as an adapter process-lifetime high-water mark. Python `tracemalloc` and Rust global-allocator fields remain method-specific rather than being treated as equivalent. Candidate CLI size, WebAssembly runtime bundle size, and fresh-process Node module startup are attributable measurements; the combined competitor adapter binary is excluded. These one-sample results are also local and unpublished.

The [workload-scenario suite](benchmarks/scenarios-v0.3.0.md) gives fresh-process and same-process warmed opens separate identities, times a second identical character access on the same live Python page as a cache hit, compares the first page with all pages on the same 65-page input, and clocks the Rust parallel page workload only after its page-index-ordered output matches pinned Python `pdfplumber`. “Cold” describes empty library/process state; the operating-system filesystem cache is explicitly uncontrolled rather than claimed to be cleared. These one-sample results are also local and unpublished.

The [run-provenance contract](benchmarks/provenance-v0.3.0.md) requires a clean exact source revision and records host hardware and operating system, Python/Rust/build-tool versions, material build flags, dependency-lock and built-artifact digests, fixture hashes, and exact argument arrays. It retains five round-robin raw repetitions for every eligible key and binds minimum, median, arithmetic mean, maximum, sample standard deviation, and relative standard deviation summaries back to those samples. These results remain local and unpublished.

A future cross-project result becomes publishable only after `SCORE-008` and `SCORE-009` retain raw release artifacts and enforce the result-removal policy. The corpus, equivalence policy, and local metric/scenario runs are correctness foundations, not a ranking. The full policy is the [benchmark and comparison contract](../PRD.md#75-benchmark-and-comparison-contract).

## Product interpretation

The guidance in this section is the `pdfplumber-rs` maintainers' interpretation of the observed facts, not a measured winner declaration.

- Choose `pdfplumber-rs` when you need native Rust text/table/geometry extraction and value an explicit path toward Python `pdfplumber` behavior, and you can validate your workflow against an alpha support boundary.
- Choose Python `pdfplumber` when you need its established Python behavior today, especially its visual debugging and mature customizable table workflow.
- Evaluate `pdf_oxide` when broad language bindings, Markdown conversion, PDF creation/editing, or its Command-Line Interface and Model Context Protocol surfaces matter more than a narrow Python `pdfplumber` compatibility contract.
- Evaluate `pdfsink-rs` when you want a pure-Rust, `pdfplumber`-inspired library and Command-Line Interface with a broad extraction and rendering surface; validate its output and published benchmark method against your own documents.
- Evaluate `pdf-extract` when a small Rust plain-text extraction API is sufficient and you do not need the richer table, geometry, or migration surfaces above.

None of these observations makes a non-OCR extractor suitable for image-only PDFs by itself. For scanned documents, add an OCR stage and evaluate the searchable result with representative files.

When updating this page, refresh the observation date and exact source revisions, keep external claims attributed, and do not promote a self-published number into a `pdfplumber-rs` claim without satisfying the benchmark contract.
