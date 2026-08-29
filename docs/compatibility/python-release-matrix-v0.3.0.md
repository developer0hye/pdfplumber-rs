# Python pdfplumber release matrix for pdfplumber-rs v0.3.0

This versioned matrix keeps evidence separate for each exact Python `pdfplumber` release. It does not make a blanket compatibility claim. A result for one row never transfers to another release.

The [compatibility terminology](terms.md) defines the required claim scope and outcome vocabulary. Unlisted releases are also not tested; absence from this table is not compatibility evidence.

## Scope

- Candidate release: `pdfplumber-rs` `0.3.0`.
- Enumerated upstream series: Python `pdfplumber` `0.11.x`.
- Authoritative upstream inventory: [release tags](https://github.com/jsvine/pdfplumber/tags).
- Matrix source: `compat/python-release-matrix-v0.3.0.toml`.

## Release matrix

| Python pdfplumber release | Coverage | Evidence | Boundary |
| --- | --- | --- | --- |
| [`0.11.10`](https://github.com/jsvine/pdfplumber/tree/v0.11.10) | Observed | [machine-readable scorecard](scorecard-v0.3.0.json); `exact=1232; approved_delta=0; unsupported=271; reference_failure=4; candidate_failure=1639; not_tested=146`; SHA-256 `29ccd0579ef347232457405dd4dbca336f3b0f0fc197770445c7294966be3ce3` | Release-specific observations include unsupported, failure, and untested outcomes; inspect the scorecard before any scoped claim. |
| [`0.11.9`](https://github.com/jsvine/pdfplumber/tree/v0.11.9) | Not tested — no release-specific scorecard | No release-specific compatibility scorecard has been executed for Python pdfplumber 0.11.9 with pdfplumber-rs 0.3.0. | No behavior, platform, artifact, or workflow result is inferred. |
| [`0.11.8`](https://github.com/jsvine/pdfplumber/tree/v0.11.8) | Not tested — no release-specific scorecard | No release-specific compatibility scorecard has been executed for Python pdfplumber 0.11.8 with pdfplumber-rs 0.3.0. | No behavior, platform, artifact, or workflow result is inferred. |
| [`0.11.7`](https://github.com/jsvine/pdfplumber/tree/v0.11.7) | Not tested — no release-specific scorecard | No release-specific compatibility scorecard has been executed for Python pdfplumber 0.11.7 with pdfplumber-rs 0.3.0. | No behavior, platform, artifact, or workflow result is inferred. |
| [`0.11.6`](https://github.com/jsvine/pdfplumber/tree/v0.11.6) | Not tested — no release-specific scorecard | No release-specific compatibility scorecard has been executed for Python pdfplumber 0.11.6 with pdfplumber-rs 0.3.0. | No behavior, platform, artifact, or workflow result is inferred. |
| [`0.11.5`](https://github.com/jsvine/pdfplumber/tree/v0.11.5) | Not tested — no release-specific scorecard | No release-specific compatibility scorecard has been executed for Python pdfplumber 0.11.5 with pdfplumber-rs 0.3.0. | No behavior, platform, artifact, or workflow result is inferred. |
| [`0.11.4`](https://github.com/jsvine/pdfplumber/tree/v0.11.4) | Not tested — no release-specific scorecard | No release-specific compatibility scorecard has been executed for Python pdfplumber 0.11.4 with pdfplumber-rs 0.3.0. | No behavior, platform, artifact, or workflow result is inferred. |
| [`0.11.3`](https://github.com/jsvine/pdfplumber/tree/v0.11.3) | Not tested — no release-specific scorecard | No release-specific compatibility scorecard has been executed for Python pdfplumber 0.11.3 with pdfplumber-rs 0.3.0. | No behavior, platform, artifact, or workflow result is inferred. |
| [`0.11.2`](https://github.com/jsvine/pdfplumber/tree/v0.11.2) | Not tested — no release-specific scorecard | No release-specific compatibility scorecard has been executed for Python pdfplumber 0.11.2 with pdfplumber-rs 0.3.0. | No behavior, platform, artifact, or workflow result is inferred. |
| [`0.11.1`](https://github.com/jsvine/pdfplumber/tree/v0.11.1) | Not tested — no release-specific scorecard | No release-specific compatibility scorecard has been executed for Python pdfplumber 0.11.1 with pdfplumber-rs 0.3.0. | No behavior, platform, artifact, or workflow result is inferred. |
| [`0.11.0`](https://github.com/jsvine/pdfplumber/tree/v0.11.0) | Not tested — no release-specific scorecard | No release-specific compatibility scorecard has been executed for Python pdfplumber 0.11.0 with pdfplumber-rs 0.3.0. | No behavior, platform, artifact, or workflow result is inferred. |

## Observed provenance

### Python pdfplumber 0.11.10

- Reference commit: `7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62`.
- Candidate revision: `8c6281d62d5272e2b91dd2082fa08f81d77fe638`.
- Scorecard SHA-256: `29ccd0579ef347232457405dd4dbca336f3b0f0fc197770445c7294966be3ce3`.

Counts are retained evidence, not a release-level success metric. Exact observations do not cancel unsupported behavior, failures, or untested cells.
