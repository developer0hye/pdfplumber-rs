# Benchmark Workload Scenarios v0.3.0

SCORE-006 distinguishes process and library-cache state, page scope, and bounded parallel page work before repetitions or statistical summaries are added.

| Scenario | Fixtures | State | Page scope | Concurrency | Timed implementations | Timed operation |
|---|---|---|---|---|---|---|
| `cold-document-open` | `cjk-vertical`, `encrypted-document`, `graphics-heavy-table`, `image-heavy-pages`, `large-multipage`, `recoverable-malformed`, `right-to-left-arabic`, `small-text`, `table-heavy`, `word-geometry` | `fresh-adapter-process` / `library-state-empty` | `none` | `serial` (1) | `pdfplumber-python`, `pdfplumber-rs`, `pdf-oxide`, `pdfsink-rs` | `open-path-and-parse-document` |
| `warm-document-open` | `cjk-vertical`, `encrypted-document`, `graphics-heavy-table`, `image-heavy-pages`, `large-multipage`, `recoverable-malformed`, `right-to-left-arabic`, `small-text`, `table-heavy`, `word-geometry` | `reused-adapter-process` / `prior-identical-open-completed` | `none` | `serial` (1) | `pdfplumber-python`, `pdfplumber-rs`, `pdf-oxide`, `pdfsink-rs` | `open-path-and-parse-document` |
| `cache-hit-characters` | `small-text`, `word-geometry` | `reused-adapter-process` / `first-identical-access-completed` | `first-page` | `serial` (1) | `pdfplumber-python`, `pdfplumber-rs-python` | `second-page-chars-access` |
| `single-page-text` | `large-multipage` | `fresh-adapter-process` / `document-open-page-cache-empty` | `first-page` | `serial` (1) | `pdfplumber-python`, `pdfplumber-rs`, `pdf-oxide`, `pdfsink-rs` | `extract-first-page-plain-text` |
| `full-document-text` | `large-multipage` | `fresh-adapter-process` / `document-open-page-cache-empty` | `all-pages` | `serial` (1) | `pdfplumber-python`, `pdfplumber-rs`, `pdf-oxide`, `pdfsink-rs` | `extract-all-pages-plain-text` |
| `parallel-page-batch-text` | `large-multipage` | `fresh-adapter-process` / `document-open-page-cache-empty` | `all-pages` | `bounded-rayon-thread-pool` (4) | `pdfplumber-rs` | `extract-all-pages-plain-text-in-parallel` |

## State boundary

`cold-document-open` starts in a fresh adapter process with empty library state. It does not claim a cold operating-system page cache; filesystem cache state is `uncontrolled-recorded`. `warm-document-open` performs and closes one identical open in the same process before the clock. `cache-hit-characters` times the second identical character-property access on the same live page.

## Equivalence and timing

Every implementation first emits an untimed canonical result. Timing is allowed only when that exact fixture, request, and output match pinned Python `pdfplumber`; timed invocations must reproduce the same output. Process launch and all listed setup operations remain outside the clock.

The parallel scenario uses a four-worker Rayon pool and preserves page-index order. Pinned Python `pdfplumber` provides its untimed semantic reference, while only the parallel Rust implementation is clocked.

```console
python3 scripts/run_benchmark_scenarios.py --check
python3 scripts/run_benchmark_scenarios.py --run --output /tmp/pdfplumber-rs-scenarios.json
```

SCORE-006 results remain local and unpublished. Complete environment capture, repetitions, statistical summaries, retained release artifacts, and result-removal policy remain open under SCORE-007 through SCORE-009.
