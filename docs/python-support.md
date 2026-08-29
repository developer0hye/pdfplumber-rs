# Python support policy

The next published Python distribution will support exactly the interpreters
exercised by required installed-artifact Continuous Integration.
`support-matrix.toml` is the machine-readable source for that policy; the
package metadata checker derives the required job matrix, `Requires-Python`,
and Python classifiers from the same version interval.

## Current boundary

| Claim | Current value |
|---|---|
| Implementation | CPython |
| Supported version | CPython 3.13 |
| Required artifacts | wheel and source distribution |
| `Requires-Python` | `>=3.13,<3.14` |
| Release wheel versions | CPython 3.13 only |

Published version 0.3.0 predates this policy and its immutable PyPI metadata
still advertises Python 3.9 and PyPy. Those legacy classifiers are not support
evidence; the corrected constraints apply to the next release built from the
current source tree.

Each required matrix entry builds both artifacts, checks their embedded core
metadata, installs each into a clean environment, imports the installed
package, and executes the native-layout contract. Building a wheel without
installing it does not create a support claim.

Linux release wheels have a separate artifact-inspection gate. The
[Linux wheel policy](linux-wheels.md) requires exact `manylinux_2_17` /
`manylinux2014` tags, no shared libraries outside the manylinux policy, and no
unsupported instruction-set requirement on x86-64 or AArch64. That inspection
does not replace installed-wheel testing.

macOS release wheels use native installed-artifact jobs on both
[`macos-15-intel` and `macos-15`](macos-wheels.md). Those jobs require the
exact x86-64 or arm64 Mach-O architecture, the Rust macOS 10.12 or 11.0
deployment floor, imports from an isolated wheel installation, and exact text
from a real fixture. Deployment metadata is not described as execution on the
oldest declared operating-system version.

The Windows x86-64 release wheel uses a native installed-artifact job on
[`windows-2025`](windows-wheels.md). It requires the exact AMD64 `PE32+`
machine and direct DLL import allowlist, imports from an isolated CPython 3.13
wheel installation, and extracts exact text from ordinary non-ASCII and
longer-than-260-character paths while the runner reports
`LongPathsEnabled=1`. That evidence is scoped to the named runner and enabled
system policy.

## Explicit exclusions

Python 3.14 is excluded from the current metadata. The workspace pins PyO3
0.24.2, whose normal CPython support ends at 3.13; PyO3 0.25.0 is the first
release line to add Python 3.14 support. Forward-compatibility overrides are
not evidence of package compatibility and are not used by release jobs.

PyPy is not supported. No required job installs or executes either artifact on
PyPy, so the package publishes no PyPy classifier. PyPy-specific markers in
the pinned upstream reference lock describe that separate reference
environment and do not expand candidate support.

## Changing the interval

To add or remove a minor version, update the `python_support` table first. The
metadata checker fails until the project metadata, required installed-artifact
matrix, release wheel interpreters, generated support documents, and embedded
wheel/source-distribution metadata all agree. Adding Python 3.14 also requires
a supported PyO3 upgrade and an exact installed-artifact run; changing a
classifier alone is insufficient.

Primary-source links and the distinction between classifiers and install-time
version constraints are retained in the [reference note](../references/python-support-metadata.md).
