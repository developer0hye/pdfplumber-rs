# Migrating from Python pdfplumber

This guide helps you decide whether an application using Python `pdfplumber`
v0.11.10 can move to the `pdfplumber-rs` `0.3.x` alpha. The current package is
not a complete drop-in replacement. Its recorded observations apply only to the
named release and revision: one release is not evidence for another release.

Use the [Python-release compatibility matrix](compatibility/python-release-matrix-v0.3.0.md)
to confirm the exact upstream target, the [workflow scorecard](compatibility/workflows-v0.3.0.md)
to find observations for common workflows, and the
[compatibility terminology](compatibility/terms.md) to interpret each result.
Platform and interpreter evidence is separate; consult the
[Python support policy](python-support.md) and [support matrix](support.md#python).

Treat migration as an application-specific validation exercise. A successful
build or import proves that one installed artifact loads; it does not prove that
your imports, calls, outputs, failures, or lifecycle behavior match upstream.

If the application already uses `pdfplumber-rs` 0.2.0 rather than Python
`pdfplumber`, follow the separate
[pre-parity binding migration guide](pre-parity-python-migration.md). It compares
the project's legacy and current alpha APIs without treating that release-to-release
comparison as upstream compatibility evidence.

Page identity crosses more than one convention. Read the
[page-numbering guide](page-numbering.md) before translating Python list
positions, compatibility page numbers, Rust indexes, or native-extension
destinations.

## 1. Inventory the application

List every Python-facing behavior your application depends on before changing
the environment. Include:

- imports and public exports, including submodules and utility imports;
- call signatures and defaults, including positional, keyword, and keyword-only
  arguments;
- return values, ordering, and runtime types, including nested dictionaries;
- exceptions, warnings, and messages for invalid and malformed inputs;
- caching, mutation, close, and context-manager behavior;
- optional executables and side effects, such as Ghostscript when `repair=True`.

Record the input classes and platforms that matter to you: ordinary and
malformed PDFs, passwords, page selection, crop boxes, text and table settings,
serialization, and any visual-debugging or command-line use. Do not replace this
inventory with a feature-name comparison. Two packages can both expose
`extract_text` while differing in options, ordering, types, or errors.

The top-level import name remains `pdfplumber`, and the current candidate
provides `pdfplumber.open` for filesystem paths. That narrow overlap does not
establish the complete upstream module tree or every method contract. Check each
inventory entry against the scorecards, then add an application regression for
every entry that lacks evidence in your exact scope.

## 2. Build isolated environments

The two distributions write into the same `pdfplumber/` import package. Never
install both distributions in one environment. `pip` tracks their distribution
names separately, so `pip check` can still succeed for a mixed package while
files come from different installations.

Create a CPython 3.13 reference environment containing only the pinned upstream
distribution:

```bash
python3.13 -m venv .venv-pdfplumber-reference
. .venv-pdfplumber-reference/bin/activate
python -m pip install 'pdfplumber==0.11.10'
python -m pip show pdfplumber
python -m pip show pdfplumber-rs
deactivate
```

The second `pip show` command must report that `pdfplumber-rs` is absent. Create
a separate candidate environment containing only the published candidate:

```bash
python3.13 -m venv .venv-pdfplumber-rs
. .venv-pdfplumber-rs/bin/activate
python -m pip install 'pdfplumber-rs==0.3.0'
python -m pip show pdfplumber
python -m pip show pdfplumber-rs
deactivate
```

Here the first `pip show` command must report that the separate `pdfplumber`
distribution is absent. The import package is still named `pdfplumber`. Confirm
the installed artifact rather than trusting the import name alone:

```bash
. .venv-pdfplumber-rs/bin/activate
python -c 'import importlib.metadata as m, pdfplumber; print(m.version("pdfplumber-rs")); print(pdfplumber.__file__)'
deactivate
```

The current-source policy supports exactly CPython 3.13 for the next release.
Published `0.3.0` metadata contains older legacy classifiers; those classifiers
are not execution evidence. Adapt activation commands for Windows, but do not
transfer results between operating systems or architectures.

If either distribution has ever shared an environment with the other, discard
that environment and recreate it. Uninstalling one distribution is not a safe
rollback because either uninstaller can remove shared files recorded by the
other distribution.

## 3. Run the same workload

Run your existing regression command once with the reference interpreter and
once with the candidate interpreter. If the application does not have a
regression suite, create a representative corpus and serialize the observable
results that matter to it.

For every comparison, hold these facts fixed:

- the same PDF bytes and password or repair configuration;
- the same page selection and page order;
- the same positional and keyword arguments, including omitted defaults;
- the same operating-system and architecture scope when that affects behavior;
- the same application code and result-normalization rules.

Keep the reference output and candidate output as separate artifacts. Record the
Python version, operating system, architecture, distribution version, imported
module path, input digest, command, options, exit status, standard error, and
result digest beside each run. Do not compare only text when your application
also depends on coordinates, dictionary keys, runtime types, warnings, or close
behavior.

Use the workflow report as an index, not as a substitute for your tests:

| Application workflow | Application evidence to retain |
| --- | --- |
| Open and lifecycle | Page selection, metadata, errors, ownership, close, cache, and context-manager behavior. |
| Text, words, and search | Exact strings, ordering, coordinates, nested values, types, options, and failure behavior. |
| Tables | Settings, discovered geometry, row and cell order, values, types, and empty results. |
| Objects and annotations | Object families, key order, values, numeric types, page identity, and serialization. |
| Crop and derived pages | Bounding boxes, inclusion rules, rebased coordinates, page numbers, and chained operations. |
| Rendering and Command-Line Interface | Executable dependencies, output bytes, exit status, standard error, and side effects. |

The [text-option guide](text-options.md) gives the complete pinned v0.11.10
keyword catalog, compatible examples, option interactions, and the current
surface matrix. Use it to turn each text, word, text-line, or search call into
an explicit migration decision.
The [table-setting guide](table-settings.md) gives the corresponding complete
table catalog, pipeline interactions, exact failures, and current surface
matrix. Use it to classify every table settings dictionary explicitly.
The [object-dictionary schema guide](object-dictionary-schemas.md) gives the
exact pinned family and key inventory, derived-edge rules, serialization
shapes, and current gaps. Compare ordered keys as well as values and types.
The [visual-debugging guide](visual-debugging.md) gives the exact pinned
PDFium/Pillow render, overlay, crop, save, display, and table-debug behavior,
then separates it from the current SVG extension and absent Python adapter API.
The [error and resource-limit guide](errors-and-resource-limits.md) gives the
exact pinned exception/warning boundary, Rust typed errors and active budgets,
adapter gaps, and host controls needed for untrusted documents.
The [encryption and repair guide](encryption-and-repair.md) gives the pinned and
current algorithm matrix, password and permission boundaries, Ghostscript and
native repair contracts, stream ownership, and adapter-specific gaps.
The [parser and font limitations](parser-and-font-limitations.md) guide gives
the installed-wheel fixture evidence and current parser, font, warning, and
numeric boundaries that still require migration checks.
The [Rust-native extensions](rust-extensions.md) guide separates the explicit
`document.rust` namespace and other current native additions from compatible
Python output, including adapter gaps and unnamespaced collision risks.

An observed result from a source-built command does not automatically cover a
wheel, source distribution, or another platform. Validate the installed artifact
you plan to deploy.

## 4. Interpret every result

Classify each required observation with the repository vocabulary:

| Outcome | Migration meaning |
| --- | --- |
| **Exact** | The recorded upstream and candidate behavior matches in the complete named scope. |
| **Approved delta** | One exact difference matches a reviewed registry entry; inspect its risk and review condition. |
| **Unsupported** | The candidate does not implement the required behavior. |
| **Reference failure** | Upstream did not produce a comparable result for that input and operation. |
| **Candidate failure** | The candidate errored or differed without an applicable approval. |
| **Not tested** | The required comparison did not run. |

Only Exact observations permit an unqualified compatibility claim for their
fully named scope. Unsupported, Reference failure, Candidate failure, and Not
tested are not compatible results. An Approved delta remains a visible
difference and must be named beside a “compatible with approved deviations”
claim. Counts are evidence inventory, not a success rate.

`pdfplumber._native` is a packaging boundary for compiled code, not proof of
Python behavior. `document.rust` is an extension namespace for capabilities that
upstream does not define. Those extensions do not count as parity evidence and
cannot compensate for a failed or untested upstream-compatible operation.

If you deliberately rewrite the application around an unsupported or different
candidate API, record that as an application migration decision. Do not relabel
the difference as upstream compatibility.

## 5. Cut over or roll back

Cut over only when every required inventory entry has an acceptable application
decision, every behavior claimed as compatible is Exact or names its approved
deviation, and the deployed interpreter/platform/artifact matches the evidence.
Use the exact candidate artifact tested during evaluation rather than rebuilding
or silently upgrading it.

During the validation window, keep the reference environment, upstream lock,
input corpus, and reference outputs until the candidate has passed. A
failure, Unsupported requirement, Candidate failure, or Not tested requirement
that matters to the application blocks cutover unless the application is changed
and revalidated explicitly.

To roll back, stop using the candidate environment, discard the candidate
environment, and restore the separately locked reference environment. Do not
install upstream over the candidate environment. Uninstalling one distribution
is not a safe rollback for a shared or previously shared environment.

After any `pdfplumber`, `pdfplumber-rs`, Python, operating-system, architecture,
or application change, rerun the like-for-like evaluation. Existing evidence is
not silently transferable to the new scope.
