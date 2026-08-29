# macOS wheel compatibility

Every release-candidate macOS wheel is built, installed, executed, and inspected
on a native runner before its Software Bill of Materials, provenance, and
retained artifact group are created. The machine policy is
[`python-macos-wheel-targets.toml`](../python-macos-wheel-targets.toml), and
[`scripts/check_macos_wheel.py`](../scripts/check_macos_wheel.py) enforces it.

## Current policy

| Target | Native runner | Python / ABI | Wheel platform | Mach-O deployment target |
|---|---|---|---|---|
| x86-64 | macOS 15 Intel (`macos-15-intel`) | `cp313` / `cp313` | `macosx_10_12_x86_64` | macOS 10.12 |
| Apple Silicon | macOS 15 arm64 (`macos-15`) | `cp313` / `cp313` | `macosx_11_0_arm64` | macOS 11.0 |

The workflow exports the exact `MACOSX_DEPLOYMENT_TARGET` before invoking
Maturin 1.14.1. It does not cross-build one architecture and treat that build
as native execution: the Intel wheel runs on the native Intel host, and the
arm64 wheel runs on Apple Silicon.

## Enforced inspection and execution

For each target, the release job:

1. requires the exact CPython, ABI, deployment, and architecture filename tag;
2. extracts the private native extension without trusting archive paths;
3. requires `lipo -archs` to report exactly the policy architecture;
4. reads `LC_VERSION_MIN_MACOSX` or `LC_BUILD_VERSION` with `otool -l` and
   requires the exact 10.12 or 11.0 deployment target;
5. installs the wheel without dependencies into a fresh virtual environment;
6. imports both `pdfplumber` and `pdfplumber._native` from that environment;
7. opens `basic_text.pdf` and requires exact text matching the checked expected
   output.

The checker retains deterministic JSON containing the wheel and policy
SHA-256 digests, native module digest, runner identity, Mach-O result, installed
distribution version, interpreter version, fixture and expected-output
digests, and extracted-text digest.

## Scope boundary

The native jobs prove that each artifact installs and executes on the named
macOS 15 architecture, and the Mach-O metadata proves the declared deployment
floor embedded in the extension. This does not prove execution on the minimum
operating-system release itself; specifically, it does not prove execution on the minimum operating-system release. macOS 10.12 and macOS 11.0 machines are
not part of the runner matrix. It also does not cover Windows runtime
dependencies or Linux installation behavior, which remain separate platform
tasks.

Primary-source context is recorded in the
[macOS wheel reference note](../references/python-macos-wheels.md).
