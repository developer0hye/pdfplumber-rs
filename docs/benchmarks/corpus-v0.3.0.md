# Benchmark corpus 0.3.0

This versioned selection defines inputs; it does not publish a timing or performance result. Every selected PDF is already covered by the [licensed corpus index](../../compat/fixture-provenance.toml), and the manifest binds the selection to exact SHA-256 digests. `pdfplumber-rs-v0.3.0` contains 10 unique PDFs totaling 6,646,998 bytes.

Before any comparison is timed, `SCORE-002` must prove materially equivalent requested outputs and semantics. A failed or unsupported case is reported separately and cannot become a performance win.

## Coverage contract

Required semantic classes: `cjk`, `encrypted`, `graphics-heavy`, `image-heavy`, `malformed`, `right-to-left`, `table-heavy`, `text-only`, `word-geometry`.

`small` means at most 4,096 bytes; `large` means at least 1,048,576 bytes; values between those bounds are `medium`. The recorded page count is part of the digest-bound fixture description; byte classifications are checked directly from the committed files.

| ID | Fixture | Semantic classes | Size | Bytes | Pages | Access | Source |
|---|---|---|---:|---:|---:|---|---|
| `cjk-vertical` | [`crates/pdfplumber/tests/fixtures/pdfs/pdfjs/vertical.pdf`](../../crates/pdfplumber/tests/fixtures/pdfs/pdfjs/vertical.pdf) | `cjk` | `medium` | 6,905 | 3 | none | `pdfjs-upstream` |
| `encrypted-document` | [`compat/fixtures/upstream/pdfplumber-v0.11.10/tests/pdfs/password-example.pdf`](../../compat/fixtures/upstream/pdfplumber-v0.11.10/tests/pdfs/password-example.pdf) | `encrypted` | `medium` | 49,755 | 4 | password `test` | `pdfplumber-upstream` |
| `graphics-heavy-table` | [`compat/fixtures/upstream/pdfplumber-v0.11.10/tests/pdfs/table-curves-example.pdf`](../../compat/fixtures/upstream/pdfplumber-v0.11.10/tests/pdfs/table-curves-example.pdf) | `graphics-heavy` | `medium` | 153,741 | 1 | none | `pdfplumber-upstream` |
| `image-heavy-pages` | [`compat/fixtures/upstream/pdfplumber-v0.11.10/tests/pdfs/pr-136-example.pdf`](../../compat/fixtures/upstream/pdfplumber-v0.11.10/tests/pdfs/pr-136-example.pdf) | `image-heavy` | `large` | 3,388,137 | 6 | none | `pdfplumber-upstream` |
| `large-multipage` | [`compat/fixtures/upstream/pdfplumber-v0.11.10/tests/pdfs/chelsea_pdta.pdf`](../../compat/fixtures/upstream/pdfplumber-v0.11.10/tests/pdfs/chelsea_pdta.pdf) | `graphics-heavy`, `image-heavy` | `large` | 2,944,560 | 65 | none | `pdfplumber-upstream` |
| `recoverable-malformed` | [`compat/fixtures/upstream/pdfplumber-v0.11.10/tests/pdfs/malformed-from-issue-932.pdf`](../../compat/fixtures/upstream/pdfplumber-v0.11.10/tests/pdfs/malformed-from-issue-932.pdf) | `malformed` | `medium` | 61,037 | 1 | none | `pdfplumber-upstream` |
| `right-to-left-arabic` | [`crates/pdfplumber/tests/fixtures/pdfs/pdfjs/ArabicCIDTrueType.pdf`](../../crates/pdfplumber/tests/fixtures/pdfs/pdfjs/ArabicCIDTrueType.pdf) | `right-to-left` | `medium` | 39,370 | 1 | none | `pdfjs-upstream` |
| `small-text` | [`tests/fixtures/generated/basic_text.pdf`](../../tests/fixtures/generated/basic_text.pdf) | `text-only` | `small` | 1,200 | 1 | none | `pdfplumber-rs-generated` |
| `table-heavy` | [`tests/fixtures/generated/table_lattice.pdf`](../../tests/fixtures/generated/table_lattice.pdf) | `table-heavy` | `small` | 1,604 | 1 | none | `pdfplumber-rs-generated` |
| `word-geometry` | [`tests/fixtures/real-world/layout/positioned-text.pdf`](../../tests/fixtures/real-world/layout/positioned-text.pdf) | `word-geometry` | `small` | 689 | 1 | none | `pdfplumber-rs-generated` |

## Selection notes

- `cjk-vertical`: Japanese glyphs in vertical writing mode across three pages.
- `encrypted-document`: RC4-encrypted multipage input with an explicit benchmark password.
- `graphics-heavy-table`: Dense vector table containing rectangles and non-line curves.
- `image-heavy-pages`: Six image-backed pages for image discovery and stream workloads.
- `large-multipage`: A 65-page mixed-content report with dense vector and image objects.
- `recoverable-malformed`: Structurally malformed input that exercises recoverable parsing.
- `right-to-left-arabic`: Arabic right-to-left glyph ordering with an embedded CID TrueType font.
- `small-text`: One text-only page with punctuation, accents, and numeric tokens.
- `table-heavy`: A ruled five-column table with headers, seven rows, and empty cells.
- `word-geometry`: Words positioned at page corners and center with known coordinates.

The workload breadth follows the source-pinned corpus observation in [`references/pdf-oxide.md`](../../references/pdf-oxide.md); no external project's self-published result is copied into this corpus definition.

## Verify

```bash
python3 scripts/check_fixture_licenses.py
python3 scripts/check_corpus_index.py
python3 scripts/generate_benchmark_corpus.py --check
```
