# Downloaded PDF Fixtures

Real-world PDF files used for integration testing.

## Files

All eight files are byte-identical copies from the public pdfplumber v0.11.10
test corpus at commit `7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62`. The source corpus is MIT
licensed; the exact source path, digest, and revision-pinned license evidence
for every copy are in `compat/fixture-provenance.toml`.

| Local file | Upstream file |
|---|---|
| `annotations.pdf` | `tests/pdfs/annotations.pdf` |
| `annotations-rotated-90.pdf` | `tests/pdfs/annotations-rotated-90.pdf` |
| `annotations-rotated-180.pdf` | `tests/pdfs/annotations-rotated-180.pdf` |
| `annotations-rotated-270.pdf` | `tests/pdfs/annotations-rotated-270.pdf` |
| `annotations-unicode-issues.pdf` | `tests/pdfs/annotations-unicode-issues.pdf` |
| `nics-firearm-checks.pdf` | `tests/pdfs/nics-background-checks-2015-11.pdf` |
| `pdffill-demo.pdf` | `tests/pdfs/pdffill-demo.pdf` |
| `scotus-transcript-p1.pdf` | `tests/pdfs/scotus-transcript-p1.pdf` |

## Reproduction

Run `bash tests/fixtures/download_fixtures.sh` from the repository root.
The downloader uses the immutable revision above and finishes by running the
repository-wide fixture metadata audit.
