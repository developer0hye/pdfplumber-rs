# Compatibility harness

The pinned environment every Python `pdfplumber` compatibility claim is measured
against. See `PRD.md` section 8.1 for the tasks this implements.

## Why this exists

Golden data and parity reports are only evidence if the environment that
produced them can be rebuilt. Previously `scripts/setup_golden_venv.sh` ran
`pip install pdfplumber`, so the baseline was whatever PyPI happened to serve
that day — and `pdfplumber` itself only requires `Pillow>=12.2.0` and
`pypdfium2>=5.9.0`, both of which affect extraction output.

Everything here exists to make that reproducible.

## Layout

| Path | Purpose |
|---|---|
| `upstream.toml` | The pinned target: upstream version, tag, commit, and reference interpreter |
| `requirements-golden.txt` | Hash-pinned lock for the whole dependency closure (generated) |
| `harness/upstream.py` | Reads the pinned target |
| `harness/lockfile.py` | Parses the lock and computes its digest |
| `harness/provenance.py` | Builds the provenance block stamped into golden artifacts |
| `harness/environment.py` | Guards against importing the wrong `pdfplumber` |
| `harness/api_snapshot.py` | Reflects the complete upstream module/export/class surface deterministically |
| `snapshots/pdfplumber-v0.11.10-api.json` | Committed public API contract for the pinned upstream release |
| `tests/` | Structural gates on all of the above |

## Usage

```bash
# Build the pinned reference environment (rebuilt from scratch each time).
bash scripts/setup_golden_venv.sh

# Confirm the interpreter has the upstream package, not this project's binding.
.venv-reference/bin/python scripts/verify_compat_env.py --reference

# Regenerate golden data.
.venv-reference/bin/python scripts/generate_golden.py

# Regenerate the pinned upstream public API snapshot.
.venv-reference/bin/python scripts/generate_api_snapshot.py

# Fail if the committed API snapshot has drifted.
.venv-reference/bin/python scripts/generate_api_snapshot.py --check

# Harness's own tests (no network, no install).
python3 -m unittest discover -s compat/tests -t .
```

The API snapshot discovers the package tree recursively and records public
module exports, `__all__`, classes, inherited public descriptors, constants,
and call signatures. It also preserves inconsistencies in the pinned release:
for example, v0.11.10 declares `set_debug` in top-level `__all__` without
defining that attribute. Review snapshot diffs as compatibility changes; do not
edit the generated JSON by hand.

The harness needs Python 3.11+ for `tomllib`. The *reference* interpreter is
pinned separately in `upstream.toml`; `PDFPLUMBER_RS_REFERENCE_PYTHON` overrides
it locally, with a warning, when the pinned version is not installed.

## Changing the pinned target

A target move is a deliberate change, not a refresh (PARITY-030):

1. Edit `[target]` in `upstream.toml`.
2. Regenerate the lock: `python3 scripts/lock_golden_env.py` (needs network).
3. Rebuild the environment and regenerate every golden artifact.
4. Review the resulting data diff — it is the behavioral difference between the
   two upstream releases, and each change needs an explanation or an approved
   delta.
5. Record the move in the PRD Decision Log and Evidence Ledger.

`python3 scripts/lock_golden_env.py --check` fails when the committed lock no
longer matches a fresh resolve.

## Two environments, deliberately

Upstream `pdfplumber` and this project's Python binding are both imported as
`pdfplumber`, so they live in separate virtual environments (`.venv-reference`
and `.venv-candidate`) rather than being separated by `sys.path` order.

A runner that imports the wrong one compares an implementation against itself
and reports flawless parity. `harness/environment.py` makes that a loud error:
the reference must be the pinned pure-Python upstream release, and the candidate
must not be.
