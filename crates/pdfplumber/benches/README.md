# pdfplumber-rs Local Regression Benchmarks

This Criterion suite measures changes within the current Rust implementation on
synthetic fixtures. It does not request materially equivalent outputs from another
library and publishes no cross-project result. A separate
[versioned corpus](../../../docs/benchmarks/corpus-v0.3.0.md) now defines shared
inputs. Its [output-equivalence preflight](../../../docs/benchmarks/equivalence-v0.3.0.md)
rejects mismatched semantics or canonical results before timing. The separate
[component stage suite](../../../docs/benchmarks/stages-v0.3.0.md) excludes
process launch and setup from seven exact-output-gated clocks. Cross-project
measurements remain deferred after `SCORE-003` and `SCORE-004` until `SCORE-005`
through `SCORE-009` satisfy the
[comparison policy](../../../docs/comparison.md).

## Running Benchmarks

```bash
cargo bench --bench extraction
```

HTML reports are generated in `target/criterion/`.

## Benchmark Suite

| Benchmark | Description |
|---|---|
| `pdf_open` | Parse PDF bytes and initialize document |
| `char_extraction` | Extract characters from all pages |
| `word_extraction` | Group characters into words |
| `text_extraction` | Full text extraction pipeline |
| `text_extraction_layout` | Text extraction with layout detection |
| `table_detection_lattice` | Table detection using visible line edges |
| `table_detection_stream` | Table detection using text alignment |
| `edge_computation` | Derive edges from geometric primitives |

## Test PDFs

All PDFs are generated programmatically using lopdf:

| Fixture | Pages | Content |
|---|---|---|
| Simple | 1 | 10 lines of text (~60 chars each) |
| Medium | 10 | 30 lines of text per page |
| Complex | 10 | Header (Courier) + 15 body lines (Helvetica) + 5x4 lattice table per page |
| Lattice table | 1 | 20x5 grid with visible borders and cell text |
| Stream table | 1 | 20x5 text grid (no visible borders) |

## Interpreting Results

- **pdf_open**: Measures lopdf parsing overhead only (no page content processing)
- **char_extraction**: Includes content stream interpretation (the main bottleneck)
- **word_extraction**: char_extraction + word grouping algorithm
- **text_extraction**: word_extraction + line clustering + text assembly
- **table_detection**: Full pipeline including edge computation, intersection finding, cell extraction, and text population
