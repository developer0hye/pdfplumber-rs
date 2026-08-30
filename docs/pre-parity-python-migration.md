# Migrating from the pre-parity Python binding

This guide is for applications upgrading from the published
`pdfplumber-rs==0.2.0` Python binding to `pdfplumber-rs==0.3.0` or a later
candidate in the current `0.3.x` alpha. It is not a complete drop-in
replacement guide. Remember that one release is not evidence for another
release, so retain the exact installed artifacts and application observations
used for the migration decision.

The exact legacy source is tag `v0.2.0`, commit
`caf412d9307d7d22769b6cd5fb330ad0594ef0bf`. The compatibility audit began at
`da0663ce27f35bfc641055c0cebf8fae97932ac4`, which retained the same pre-parity
Python guide and surface. PyPI lists both releases separately; use the
[0.2.0 release page](https://pypi.org/project/pdfplumber-rs/0.2.0/) and the
[0.3.0 release page](https://pypi.org/project/pdfplumber-rs/0.3.0/) to identify
published artifacts. v0.2.0 was published under `MIT OR Apache-2.0`; v0.3.0
uses `Apache-2.0`.

This is a migration between two `pdfplumber-rs` releases. It is separate from
the [Python migration guide](python-migration.md), which compares Python
`pdfplumber` v0.11.10 with this project's alpha compatibility surface. The
[workflow scorecard](compatibility/workflows-v0.3.0.md) and
[compatibility terminology](compatibility/terms.md) describe that upstream
comparison, while the [Python support policy](python-support.md) describes
current artifact evidence. The scorecard does not compare 0.2.0 with 0.3.0.

## 1. Freeze the 0.2.0 application contract

Before changing application code, capture what the deployed 0.2.0 environment
actually does. Record its Python version, operating system, architecture,
distribution metadata, import path, wheel or source-distribution filename, and
artifact digest. Retain the exact dependency lock and the complete environment
until the candidate has passed.

Inventory every legacy dependency, including:

- imports and exception symbols;
- `PDF.open`, `PDF.open_bytes`, page traversal, and bookmarks;
- callable page and cropped-page object collections;
- zero-based page indexes used in arrays, logs, storage, or user interfaces;
- text, word, table, search, crop, and spatial-filtering arguments;
- metadata and object dictionary schemas, ordering, and runtime types;
- error handling, cache assumptions, mutation, close, and resource ownership;
- performance assumptions copied from the former package guide.

Keep representative ordinary, encrypted, malformed, cropped, rotated, and
table-bearing PDFs. Hash the inputs and retain the complete 0.2.0 output and
failure records. Remember that legacy performance claims are not migration
evidence; measure only after the candidate produces acceptable application
results for the same workload.

## 2. Build isolated release environments

Both releases install the same distribution and import names. The distribution
is `pdfplumber-rs`, and the import package is `pdfplumber`. Do not upgrade the
0.2.0 environment in place. Separate environments preserve a runnable baseline,
make import identity observable, and provide an environment-level rollback.

Create the legacy environment with CPython 3.13:

```bash
python3.13 -m venv .venv-pdfplumber-rs-020
. .venv-pdfplumber-rs-020/bin/activate
python -m pip install 'pdfplumber-rs==0.2.0'
python -m pip show pdfplumber-rs
python -m pip show pdfplumber
python -c 'import importlib.metadata as m, pdfplumber; print(m.version("pdfplumber-rs")); print(pdfplumber.__file__)'
deactivate
```

Create the published candidate environment independently:

```bash
python3.13 -m venv .venv-pdfplumber-rs-030
. .venv-pdfplumber-rs-030/bin/activate
python -m pip install 'pdfplumber-rs==0.3.0'
python -m pip show pdfplumber-rs
python -m pip show pdfplumber
python -c 'import importlib.metadata as m, pdfplumber; print(m.version("pdfplumber-rs")); print(pdfplumber.__file__)'
deactivate
```

In both environments, the separate `pdfplumber` distribution must be absent;
`python -m pip show pdfplumber` must say it was not found. Installing Python
`pdfplumber` beside either release creates the shared-package conflict explained
in the general migration guide.

Published 0.2.0 and 0.3.0 metadata includes historical interpreter classifiers.
The current-source policy supports exactly CPython 3.13 for the next release.
Published metadata is not execution evidence for your deployment: run the
artifact on the exact interpreter, operating system, and architecture you plan
to use. Adapt activation commands for Windows without transferring results
between platforms.

If evaluating a source build newer than 0.3.0, create a third environment and
record its exact source revision and built-artifact digest. Do not label it
0.3.0 or reuse results from the published wheel.

## 3. Rewrite calls deliberately

Make each API rewrite explicit and reviewable. Do not add a global shim that
silently guesses whether it received a 0.2.0 or 0.3.x object.

| 0.2.0 application code | 0.3.x migration | Boundary to retain |
| --- | --- | --- |
| `pdfplumber.PDF.open(path)` | `pdfplumber.open(path)` | Prefer the public compatibility entry point and revalidate arguments and failures. |
| `pdfplumber.PDF.open_bytes(data)` | `pdfplumber.open(BytesIO(data))` | Use the public stream path when the workflow is meant to track Python `pdfplumber`. |
| `page.chars()` | `page.chars` | Object collections changed from methods to cached properties. |
| `page.lines()` | `page.lines` | Revalidate dictionary shape and cache identity. |
| `page.rects()` | `page.rects` | Revalidate dictionary shape and cache identity. |
| `page.curves()` | `page.curves` | Revalidate dictionary shape and cache identity. |
| `page.images()` | `page.images` | This returns page image dictionaries, not extracted pixel bytes. |
| `cropped.chars()` | `cropped.chars` | Cropped-page collections changed to properties too. Apply the same rewrite to `lines`, `rects`, `curves`, and `images`. |
| 0-based `page.page_number` | 1-based `page.page_number` | Convert only at application boundaries that still require a zero-based index. |
| `pdf.bookmarks()` | `pdf.rust.bookmarks()` | Bookmarks are a Rust-only extension with zero-based destinations. |

For byte inputs, prefer the public stream contract:

```python
from io import BytesIO

import pdfplumber

pdf = pdfplumber.open(BytesIO(data))
```

The original native convenience still exists through a private native extension
path:

```python
from pdfplumber import _native

pdf = _native.PDF.open_bytes(data)
```

Use that form only when the application deliberately depends on a native
extension. `pdfplumber._native` is a packaging boundary, and its successful use
does not count as compatibility evidence. Do not treat private exception classes,
type names, or module layout as a stable public facade.

`page.page_number` changed from 0-based to 1-based on the compatibility surface.
Audit database keys, array offsets, filenames, user-visible labels, bookmark
destinations, and page-selection logic separately. The `pdf.rust` namespace
retains Rust conventions, including zero-based destinations and page indexes;
do not apply one blanket increment or decrement to both surfaces. The canonical
[page-numbering guide](page-numbering.md) defines list positions, selection,
derived pages, conversions, and persisted field names for every public surface.

The top-level package exposes `pdfplumber.open`, but the public facade is still
incomplete. Avoid star imports and import only the names your application has
validated. Treat newly available serialization, annotation, structure, layout,
cache, repair, and lifecycle APIs as new contracts rather than assuming they
preserve 0.2.0 behavior.

## 4. Re-baseline observable behavior

Run the legacy and candidate applications against the same PDF bytes, page
scope, options, passwords, platform, and application normalization. Keep raw
records from both environments before adding compatibility wrappers. There is
no automatic translation from a 0.2.0 result into a compatible 0.3.x result.

At minimum, compare:

- document and page counts, selected-page order, metadata, and resource state;
- exact text, word order, coordinates, and search results;
- dictionary keys, key order, nested values, numeric types, and `None` placement;
- exception classes, messages, and arguments for valid and invalid inputs;
- cache identity, mutation, `flush_cache()`, `close()`, and context-manager behavior;
- crop, `within_bbox`, and `outside_bbox` coordinates and inclusion rules;
- table geometry, rows, extracted values, and `accuracy`;
- bookmark values, destinations, and every other `pdf.rust` extension result.

The [crop-semantics guide](crop-semantics.md) is the migration authority for
partial-object clipping, root-preserved versus rebased coordinates, absolute
and relative nested boxes, strict validation, and the current property/method
split. Do not reuse a 0.2.0 crop baseline as Python compatibility evidence.
Use the [text-option guide](text-options.md) to translate every 0.2.0 text,
word, or search argument explicitly; the current compatibility facade accepts
only a subset of the pinned Python options.
Use the [table-setting guide](table-settings.md) to inventory every table
strategy and setting separately; the current compatibility facade's table
methods do not yet accept the pinned settings argument.
Use the [object-dictionary schema guide](object-dictionary-schemas.md) to
compare family presence, exact key order, value shapes, derived edges, and
serialization before accepting any 0.2.0 object baseline.
Use the [visual-debugging guide](visual-debugging.md) to revalidate every image
size, crop, overlay, save, viewer, and Command-Line Interface workflow; the
current 0.3.x Python adapter does not expose the pinned `PageImage` API.
Use the [error and resource-limit guide](errors-and-resource-limits.md) to
revalidate exception classes, warning/log channels, safe reporting, and every
resource assumption; adapter constructors do not expose Rust budgets.
Use the [encryption and repair guide](encryption-and-repair.md) to revalidate
security-handler revisions, password type and failure behavior, permission
policy, repair ownership and rewriting, and every external-executable boundary.

Do not infer that matching method names imply matching output. The 0.3.x work
targets Python `pdfplumber` semantics, so object schemas, numeric types, page
identity, coordinates, failures, and cache behavior can intentionally differ
from the pre-parity binding. Crop and derived-page behavior still requires
workflow-specific evidence; keep a candidate difference visible rather than
relabeling it as compatible.

Classify every application observation:

| Outcome | Migration meaning |
| --- | --- |
| **Exact** | The legacy and candidate observations match for the complete named workflow. |
| **Application-accepted change** | The application deliberately changes its contract, has a regression for the new behavior, and no longer claims legacy equivalence. |
| **Unsupported** | The candidate has no acceptable implementation for a required legacy workflow. |
| **Legacy failure** | 0.2.0 failed before producing a comparable observation. |
| **Candidate failure** | 0.3.0 errored or differed without an accepted application change. |
| **Not tested** | The comparison did not run in the required scope. |

An Application-accepted change is an application decision, not an approved
compatibility delta. The approved-delta registry does not approve differences
from 0.2.0; it governs the separate pinned-upstream compatibility comparison.
Likewise, the scorecard does not compare 0.2.0 with 0.3.0 and cannot replace
this side-by-side application evidence. Outcome counts are an inventory, not a
success rate.

## 5. Cut over or roll back

Cut over only with the exact 0.3.0 artifact tested and a recorded decision for
every required legacy workflow. Exact observations may retain the old
application expectation. Each Application-accepted change must update the
application, tests, runbook, and user-visible contract. Unsupported, Candidate
failure, and Not tested block cutover when the affected workflow is required.

Deploy the candidate as its own immutable environment and route a bounded
workload to it first. During the validation window, keep the complete 0.2.0
environment, artifact, dependency lock, input corpus, outputs, and run command.

To roll back, stop routing work to the 0.3.0 environment, discard the 0.3.0
environment, and restore the separately locked 0.2.0 environment. Do not
reinstall 0.2.0 over the candidate environment. Environment replacement keeps
the import package, native module, stubs, and recorded dependencies coherent.

After changing the application, `pdfplumber-rs` version, Python interpreter,
artifact, operating system, architecture, or input corpus, rerun the migration
evaluation. Evidence from one scope does not silently transfer to another.
