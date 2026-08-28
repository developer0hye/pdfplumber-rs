# Pinned competitor suite 0.3.0

Suite `pdfplumber-rs-competitors-v0.3.0` binds four implementations to corpus `pdfplumber-rs-v0.3.0` and equivalence policy `pdfplumber-rs-equivalence-v0.3.0`.

| ID | Project | Revision | License | Overlapping workloads |
|---|---|---|---|---|
| `pdf-oxide` | [pdf_oxide](https://github.com/yfedoseev/pdf_oxide) | `3be1951b171edb9d69a10f42ef72ee73f52e51bf` | MIT OR Apache-2.0 | `document-open`, `text` |
| `pdfplumber-python` | [Python pdfplumber](https://github.com/jsvine/pdfplumber) | `7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62` | MIT | `document-open`, `text` |
| `pdfplumber-rs` | [pdfplumber-rs](https://github.com/developer0hye/pdfplumber-rs) | resolved exact repository head at run time | MIT | `document-open`, `text` |
| `pdfsink-rs` | [pdfsink-rs](https://github.com/clark-labs-inc/pdfsink-rs) | `980d9f7b8ec44456f3d54427f4ced747b6eb6154` | MIT | `document-open`, `text` |

## Execution contract

Every implementation receives the same repository-owned fixture bytes, SHA-256 identity, fixture password metadata, page selection, and plain-text options. The adapters expose only `document-open` and page-preserving `text`, the two materially equivalent workloads available across all four pins.

The complete output phase finishes before the timing phase starts. A timing triple requires pinned Python `pdfplumber`, the exact `pdfplumber-rs` run head, and one competitor to succeed and match exact canonical JSON. Errors, unsupported cases, and output differences remain in the local run and have no timing entry.

```console
python3 scripts/run_competitor_benchmarks.py --check
python3 scripts/run_competitor_benchmarks.py --build
python3 scripts/run_competitor_benchmarks.py --run --output /tmp/pdfplumber-rs-competitors.json
```

A SCORE-003 run is deliberately local and unpublished. It records one combined process wall-time sample only after equivalence; it is not a ranking or a publishable benchmark. SCORE-004 through SCORE-007 add separated clocks, resource metrics, explicit workload state, complete run metadata, five raw repetitions, and statistical summaries. Retained release artifacts remain required by SCORE-008.
