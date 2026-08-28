# Output-equivalence preflight 0.3.0

Policy `pdfplumber-rs-equivalence-v0.3.0` is the correctness gate for benchmark corpus `pdfplumber-rs-v0.3.0`. It compares untimed canonical-output records before any performance command is allowed to run.

A case is eligible for timing only when both implementations are distinct, select the same indexed fixture and digest, use the exact declared semantic request and output schema, succeed, and produce the same canonical JSON bytes after object-key sorting. Array order, JSON number types, strings, null placement, and nested values remain exact.

Errors, unsupported workloads, missing fields, extra timing fields, non-finite numbers, fixture drift, request drift, schema drift, and output drift reject the case. A rejected case may be reported as incompatible, but it cannot contribute a timing result.

## Workload contracts

| Workload | Canonical output | Applicable fixtures | Exact request |
|---|---|---|---|
| `document-open` | `page-count-v1` | all corpus fixtures | `{"operation":"open","page_selection":"all","password_source":"fixture-metadata","repair":"disabled"}` |
| `graphics` | `page-graphics-v1` | `graphics-heavy` | `{"coordinate_system":"top-origin-points","page_selection":"all","preserve_path_order":true,"primitives":["curve","line","rect"]}` |
| `images` | `page-images-v1` | `image-heavy` | `{"coordinate_system":"top-origin-points","include_stream_data":true,"page_selection":"all","preserve_page_order":true}` |
| `tables` | `page-tables-v1` | `table-heavy` | `{"horizontal_strategy":"lines","intersection_tolerance_points":3.0,"join_tolerance_points":3.0,"min_words_horizontal":1,"min_words_vertical":3,"page_selection":"all","preserve_cell_nulls":true,"snap_tolerance_points":3.0,"vertical_strategy":"lines"}` |
| `text` | `page-text-v1` | `cjk`, `right-to-left`, `text-only` | `{"layout":false,"normalization":"none","page_selection":"all","preserve_page_boundaries":true}` |
| `words` | `page-words-v1` | `word-geometry` | `{"coordinate_system":"top-origin-points","expand_ligatures":true,"keep_blank_chars":false,"page_selection":"all","preserve_word_order":true,"use_text_flow":false,"x_tolerance_points":3.0,"y_tolerance_points":3.0}` |

The preflight follows the accuracy-before-performance separation in [`references/mlperf-inference.md`](../../references/mlperf-inference.md): a reference output contract is established independently from the performance phase. This project uses exact canonical equality rather than a quality threshold because benchmark wins must never weaken PDF semantics.

## Record protocol

Each JSON record contains only `schema_version`, `implementation`, `fixture`, `workload`, `request`, and `outcome`. A successful outcome contains `value`; unsupported and error outcomes contain diagnostics and are always ineligible. The decision contains output digests and rejection reasons, but no measured duration.

```bash
python3 scripts/check_benchmark_equivalence.py --check
python3 scripts/check_benchmark_equivalence.py \
  --reference reference-output.json \
  --candidate candidate-output.json
```

The comparison command exits `0` only for an eligible case, `1` for a well-formed rejected case, and `2` for malformed policy or record input. It does not run or accept a benchmark duration.
