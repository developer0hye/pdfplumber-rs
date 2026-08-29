# Compatibility workflows for v0.3.0

This human-readable view is generated from the versioned [machine-readable scorecard](scorecard-v0.3.0.json). It groups parity observations by user workflow; it is not a release-support or readiness claim.

No success percentage is computed. Exact matches, approved deltas, unsupported behavior, reference failures, candidate failures, and untested coverage remain separate counts.
The canonical [compatibility terminology](terms.md) defines the scope of compatible, extension, and approved-deviation claims.

## Provenance

- Candidate: `pdfplumber-rs` `0.3.0` at `8c6281d62d5272e2b91dd2082fa08f81d77fe638`.
- Reference: `pdfplumber` `0.11.10` (`v0.11.10`).
- Indexed corpus: 223 PDFs.
- Machine scorecard SHA-256: `29ccd0579ef347232457405dd4dbca336f3b0f0fc197770445c7294966be3ce3`.

## Run coverage

Run coverage is reported separately from workflow outcomes. A package build or smoke test does not become a parity result.

| Platform and artifact | Scope | Coverage | Reason |
| --- | --- | --- | --- |
| macOS source | api | Observed | — |
| macOS wheel | option | Observed | — |
| Ubuntu Linux source | api | Not tested | CI validates the harness and builds the pinned reference environment, but does not execute the full all-page parity report. |
| Ubuntu Linux wheel | option | Not tested | CI installs and verifies the candidate wheel, but does not execute the candidate option matrix against that installed artifact. |
| Ubuntu Linux sdist | api, option | Not tested | CI installs and checks the source-distribution layout, but does not execute page/API or option parity from the installed artifact. |

## Outcome vocabulary

| Outcome | Meaning |
| --- | --- |
| Exact | Reference and candidate results are structurally equal. |
| Approved delta | The exact observed difference has a reviewed approval entry. |
| Unsupported | The candidate explicitly reports that the API is unsupported. |
| Reference failure | The pinned reference could not produce a comparable result. |
| Candidate failure | The candidate failed or differed without an approved delta. |
| Not tested | No compatibility comparison was executed for this scope. |

## Workflows

### Open

Coverage: **Observed evidence**; this is not a workflow-level pass.

Evidence basis: one derived document outcome per indexed fixture from explicit fixture failures/gaps or the presence of page/API observations; an indexed fixture absent from both remains not tested.

Counted outcomes: 223.

| Outcome | Count |
| --- | ---: |
| Exact | 75 |
| Approved delta | 0 |
| Unsupported | 0 |
| Reference failure | 4 |
| Candidate failure | 0 |
| Not tested | 144 |

### Text

Coverage: **Observed evidence**; this is not a workflow-level pass.

Evidence basis: API and option observations for `chars`, `extract_text`, `extract_text_lines`, `extract_text_simple`, `layout_text`, `page_text`, `simple_text`, `text_lines`, `utils.extract_text`.

Counted outcomes: 1416.

| Outcome | Count |
| --- | ---: |
| Exact | 283 |
| Approved delta | 0 |
| Unsupported | 271 |
| Reference failure | 0 |
| Candidate failure | 862 |
| Not tested | 0 |

### Words

Coverage: **Observed evidence**; this is not a workflow-level pass.

Evidence basis: API and option observations for `extract_words`, `words`.

Counted outcomes: 288.

| Outcome | Count |
| --- | ---: |
| Exact | 4 |
| Approved delta | 0 |
| Unsupported | 0 |
| Reference failure | 0 |
| Candidate failure | 284 |
| Not tested | 0 |

### Crop

Coverage: **Not tested** — The v1 machine scorecard has no crop or bounding-box operation observation; uncropped extraction does not establish crop parity.

Machine observations: 0.

### Search

Coverage: **Observed evidence**; this is not a workflow-level pass.

Evidence basis: API and option observations for `search`.

Counted outcomes: 304.

| Outcome | Count |
| --- | ---: |
| Exact | 4 |
| Approved delta | 0 |
| Unsupported | 0 |
| Reference failure | 0 |
| Candidate failure | 300 |
| Not tested | 0 |

### Tables

Coverage: **Observed evidence**; this is not a workflow-level pass.

Evidence basis: API and option observations for `extract_tables`, `tables`.

Counted outcomes: 321.

| Outcome | Count |
| --- | ---: |
| Exact | 271 |
| Approved delta | 0 |
| Unsupported | 0 |
| Reference failure | 0 |
| Candidate failure | 50 |
| Not tested | 0 |

### Serialization

Coverage: **Not tested** — The v1 machine scorecard has no JSON or CSV observation; separate serialization contracts are not promoted into this parity artifact.

Machine observations: 0.

### Annotations

Coverage: **Observed evidence**; this is not a workflow-level pass.

Evidence basis: API and option observations for `annotations`, `hyperlinks`.

Counted outcomes: 542.

| Outcome | Count |
| --- | ---: |
| Exact | 482 |
| Approved delta | 0 |
| Unsupported | 0 |
| Reference failure | 0 |
| Candidate failure | 60 |
| Not tested | 0 |

### Structure

Coverage: **Observed evidence**; this is not a workflow-level pass.

Evidence basis: API and option observations for `structure_tree`.

Counted outcomes: 271.

| Outcome | Count |
| --- | ---: |
| Exact | 188 |
| Approved delta | 0 |
| Unsupported | 0 |
| Reference failure | 0 |
| Candidate failure | 83 |
| Not tested | 0 |

### Rendering

Coverage: **Not tested** — The v1 machine scorecard has no page-image rendering observation; extraction results do not establish rendering parity.

Machine observations: 0.

### Command-Line Interface

Coverage: **Not tested** — The source-built harness transport is not a product Command-Line Interface parity test; arguments, output, and exit behavior have no v1 machine observation.

Machine observations: 0.
