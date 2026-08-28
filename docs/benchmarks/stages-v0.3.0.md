# Separated benchmark stages 0.3.0

Suite `pdfplumber-rs-stages-v0.3.0` separates seven component clocks while retaining corpus `pdfplumber-rs-v0.3.0` and pinned semantic reference `pdfplumber-python`.

| Stage | Fixtures | Semantic implementations | Timed implementations | Setup outside clock | Timed operation |
|---|---:|---|---|---|---|
| `document-open` | 10 | `pdfplumber-python`, `pdfplumber-rs`, `pdf-oxide`, `pdfsink-rs` | `pdfplumber-python`, `pdfplumber-rs`, `pdf-oxide`, `pdfsink-rs` | `fixture-digest-verified` | `open-path-and-parse-document` |
| `page-materialization` | 10 | `pdfplumber-python`, `pdfplumber-rs`, `pdf-oxide`, `pdfsink-rs` | `pdfplumber-python`, `pdfplumber-rs`, `pdf-oxide` | `document-open` | `materialize-ordered-page-handles` |
| `character-extraction` | 1 | `pdfplumber-python`, `pdfplumber-rs`, `pdf-oxide`, `pdfsink-rs` | `pdfplumber-python`, `pdfplumber-rs`, `pdf-oxide` | `document-open`, `ordered-page-handles-ready` | `extract-ordered-page-characters` |
| `word-grouping` | 1 | `pdfplumber-python`, `pdfplumber-rs`, `pdf-oxide`, `pdfsink-rs` | `pdfplumber-python`, `pdfplumber-rs`, `pdfsink-rs` | `document-open`, `ordered-page-characters-ready` | `group-characters-into-words` |
| `table-detection` | 1 | `pdfplumber-python`, `pdfplumber-rs`, `pdf-oxide`, `pdfsink-rs` | `pdfplumber-python`, `pdfplumber-rs`, `pdfsink-rs` | `document-open`, `ordered-page-characters-and-graphics-ready` | `detect-and-populate-tables` |
| `serialization` | 1 | `pdfplumber-python`, `pdfplumber-rs`, `pdf-oxide`, `pdfsink-rs` | `pdfplumber-python`, `pdfplumber-rs`, `pdf-oxide`, `pdfsink-rs` | `canonical-page-text-value-ready` | `canonical-json-utf8-encoding` |
| `language-boundary-conversion` | 1 | `pdfplumber-python`, `pdfplumber-rs-python` | `pdfplumber-rs-python` | `document-open`, `ordered-page-handles-ready`, `native-page-cache-warm` | `native-to-python-char-dicts` |

## Validity boundary

Every adapter first emits an untimed canonical stage result. A clock is retained only when that result exactly matches the semantic reference, and the timed invocation reproduces the same result. Adapter process launch and the listed setup operations are outside the monotonic clock.

The pinned APIs do not expose every cost as an independently comparable component. `pdfsink-rs` eagerly materializes pages and characters during document open, while the pinned `pdf_oxide` word and table entry points repeat extraction work. Those implementations still emit semantic outcomes but are excluded from the affected component clocks instead of being timed under a misleading label.

Language-boundary conversion is candidate-specific: the installed PyO3 page cache is warmed outside the clock, then only native character to Python dictionary conversion is timed. Python `pdfplumber` supplies the untimed canonical output but has no equivalent native-language boundary clock.

```console
python3 scripts/run_stage_benchmarks.py --check
python3 scripts/run_stage_benchmarks.py --run --output /tmp/pdfplumber-rs-stages.json
```

SCORE-004 component results are not published independently. Wall time is the only component metric here; the separate SCORE-005 resource and artifact suite preserves that uninstrumented pass. SCORE-006 and SCORE-007 add execution scenarios, complete environment metadata, five raw repetitions, and statistics. SCORE-008 publishes only the complete exact-tag result bundle.
