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
| `harness/call_contract.py` | Executes argument-binding, default, invalid-call, and exception contracts |
| `harness/approved_deltas.py` | Validates exact intentional differences and rejects unregistered or stale results |
| `harness/machine_report.py` | Builds the versioned per-API/per-option/per-fixture/per-page JSON report |
| `harness/human_summary.py` | Renders the deterministic first-difference Markdown summary from that JSON model |
| `approved_deltas.toml` | Target-bound maintainer approvals for exact fixture/page/API result pairs |
| `harness/upstream_suite.py` | Verifies the pinned upstream test tree and classifies, but never suppresses, failures |
| `upstream-suite.toml` | Exact upstream commit/test-tree fingerprint and test-tool lock digest |
| `harness/upstream_fixture_corpus.py` | Imports and verifies the pinned PDF paths and bytes without flattening directories |
| `upstream-fixtures.toml` | Exact v0.11.10 PDF-corpus source, destination, file count, and tree fingerprint |
| `fixtures/upstream/pdfplumber-v0.11.10/tests/pdfs/` | Committed 81-file upstream PDF tree, including malformed and OSS-Fuzz inputs |
| `upstream-unsupported.toml` | Machine-readable temporary failure classifications |
| `requirements-upstream-tests.txt` | Hash-pinned minimal pytest/xdist/pandas dependency closure |
| `upstream-tests/` | Ignored materialized copy of the verified upstream tests and their external fixture |
| `snapshots/pdfplumber-v0.11.10-api.json` | Committed public API contract for the pinned upstream release |
| `contracts/pdfplumber-v0.11.10-calls.json` | Pinned behavioral outcomes for representative public calls |
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

# Fail if pinned call binding, defaults, validation, or exceptions drift.
.venv-reference/bin/python compat/api_contract.py

# Deliberately replace the reference call contract after reviewing a target change.
.venv-reference/bin/python compat/api_contract.py --write-reference

# In the isolated candidate environment, compare only behavioral case outcomes.
.venv-candidate/bin/python compat/api_contract.py --candidate

# Compare every page, retain complete JSON, and write a compact review summary.
.venv-reference/bin/python scripts/parity_report.py \
  --json parity-report.json --summary parity-summary.md

# Once an isolated compatibility package exists, collect its option outcomes
# and include their exact comparison in the same machine report.
.venv-candidate/bin/python scripts/generate_option_matrix.py \
  --candidate-output candidate-options.json
.venv-reference/bin/python scripts/parity_report.py \
  --candidate-options candidate-options.json --json parity-report.json

# Validate the target binding and complete schema of every approved delta.
python3 scripts/check_approved_deltas.py

# Pinned-upstream behavioral tests for page accounting, object order, and schemas.
.venv-reference/bin/python -m unittest \
  compat.tests.parity_report_pages_test \
  compat.tests.ordered_sequence_test \
  compat.tests.dictionary_structure_test -v

# Harness's own tests (no network, no install).
python3 -m unittest discover -s compat/tests -t .

# Materialize and re-verify the 101-file tests plus their external PDF fixture.
python3 scripts/setup_upstream_suite.py
python3 scripts/setup_upstream_suite.py --check

# Import from the pinned Git checkout, then verify the committed PDF-only tree.
python3 scripts/import_upstream_fixtures.py
python3 scripts/import_upstream_fixtures.py --check

# Install only the hash-locked upstream test tools into the candidate venv.
.venv-candidate/bin/python -m pip install --require-hashes \
  -r compat/requirements-upstream-tests.txt

# The upstream repair tests also require the `gs` executable (Ghostscript).
command -v gs

# Run every upstream test against the installed candidate package.
python3 scripts/run_upstream_suite.py
```

The API snapshot discovers the package tree recursively and records public
module exports, `__all__`, classes, inherited public descriptors, constants,
and call signatures. It also preserves inconsistencies in the pinned release:
for example, v0.11.10 declares `set_debug` in top-level `__all__` without
defining that attribute. Review snapshot diffs as compatibility changes; do not
edit the generated JSON by hand.

The executable call contract complements the surface snapshot. Its cases invoke
real upstream callables positionally and by keyword, exercise options that are
accepted only through `**kwargs`, preserve processed defaults, and record both
Python binding errors and runtime exception types. It stores the fixture hash
and compact output digests so the artifact is reviewable without weakening the
behavioral comparison.

The parity report identifies fixtures by their corpus-relative paths, so files
with the same basename remain separate. It obtains the Python and Rust page
counts independently, compares each corresponding page, and exits nonzero if a
fixture cannot be processed or either side omits or adds a page.

Observed output differences are matched against `approved_deltas.toml` by exact
fixture ID, page number, API, type-preserving upstream digest, and
type-preserving Rust digest. Wildcards are not supported. An unregistered
difference fails the report, and an approval that is no longer observed is
stale and also fails within the fixture scope being run. Each approval must
record both result summaries, its reason and compatibility risk, the approving
maintainer, a regression test, and an expiration or review condition. The
committed registry starts empty because no deviation has been approved.

`--json` writes schema-versioned deterministic JSON rather than the legacy raw
fixture dictionary. It records the pinned target and reference environment,
then retains explicit outcomes for every fixture, page, and API plus all 161
option-matrix cases. Option results include their fixture/page/API identity,
arguments, resolved options, warnings, logs, and complete referenced output.
The report also records the candidate-environment state and attaches exact
approved or unregistered delta dispositions to the affected API results.
If no isolated candidate option snapshot is supplied, every option case is
recorded as `blocked` by `PYAPI-002` and the command exits nonzero. Candidate
snapshots must contain exactly the same case IDs and identity fields; missing,
extra, or changed cases fail instead of disappearing from the report.

`--summary` renders a deterministic Markdown artifact from the same machine
report model. It includes target and result counts, then selects the first
differing fixture/page/API in corpus and API order. For sequences it records
the exact object index and JSON-safe upstream/Rust objects; for text it records
the first character offset and bounded context. Common object coordinates are
shown with their signed delta when they differ and explicitly marked unchanged
otherwise. The summary is compact
diagnostic evidence, while `--json` remains the complete record of all later
results, blocked cases, processing failures, and delta-gate dispositions.

Parity-report character/word diagnostics and Rust cross-validation
character/word/line/rectangle diagnostics compare objects at the same sequence
positions. An identical object found elsewhere on the page does not count as a
match, and extra objects on either side remain in the denominator.

The Python side preserves complete upstream character and word dictionaries.
The report recursively compares exact key sets and spelling, runtime value
types, list/tuple and dictionary nesting, and explicit `None` positions at the
same object indexes. Scalar values remain under their dedicated exact or
tolerance-aware comparisons. Table cells keep `None` distinct from an empty
string.

The upstream-suite source is accepted only when its Git commit, `tests` Git
tree, 102-file content fingerprint (101 test-tree files plus the PDF referenced
from `examples/`), and test-requirements digest all match
`upstream-suite.toml`. The runner preflights an installed package inside
`.venv-candidate`, records the origin again inside pytest workers, and refuses
the pinned reference package. It also refuses to start if the manifest-declared
Ghostscript executable is absent, because six upstream repair tests require it.
`upstream-unsupported.toml` classifies observed
failures for follow-up; it never deselects, skips, marks xfail, changes, or masks
a test, and the runner preserves pytest's nonzero exit status. Every entry must
reference a task that exists in PRD section 8 and is still unchecked; checked or
unknown task IDs fail runner preflight.

The separately committed upstream PDF corpus contains all 81 `.pdf` paths under
the v0.11.10 `tests/pdfs` tree. The import retains those paths below
`compat/fixtures/upstream/pdfplumber-v0.11.10`, including
`tests/pdfs/from-oss-fuzz/load` and the zero-byte `tests/pdfs/empty.pdf` error
fixture. Both the source checkout and the committed copy must match the pinned
commit's path-and-byte fingerprint; a missing, changed, extra, or flattened PDF
fails verification. The provenance registry records every imported path and
its MIT-licensed immutable source revision.

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
