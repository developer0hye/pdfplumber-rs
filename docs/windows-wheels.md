# Windows wheel compatibility

Every release-candidate Windows wheel is built, installed, executed, and
inspected on a native x64 runner before its Software Bill of Materials,
provenance, and retained artifact group are created. The machine and import
policy is
[`python-windows-wheel-targets.toml`](../python-windows-wheel-targets.toml),
and [`scripts/check_windows_wheel.py`](../scripts/check_windows_wheel.py)
enforces it.

## Current policy

| Target | Native runner | Python / ABI | Wheel platform | PE contract |
|---|---|---|---|---|
| x86-64 | Windows Server 2025 (`windows-2025`) | `cp313` / `cp313` | `win_amd64` | AMD64 machine `8664`, `PE32+` |

The extension's required DLL imports are exact and reviewable. The current
allowlist is `python313.dll`, `vcruntime140.dll`, `kernel32.dll`, `ntdll.dll`,
`bcryptprimitives.dll`, and the five named `api-ms-win-*` synchronization and
Universal C Runtime libraries in the policy file. A missing or additional
import fails the release job until the policy and compatibility impact are
reviewed together.

## Enforced inspection and execution

For the Windows target, the release job:

1. requires the exact `cp313-cp313-win_amd64` filename tag;
2. extracts exactly one private `.pyd` without trusting archive paths;
3. locates the runner's x64 Visual C++ `DUMPBIN` through VSWhere rather than a
   version-specific Visual Studio path;
4. requires `DUMPBIN /DEPENDENTS` and `/HEADERS` to report AMD64 `8664`,
   `PE32+`, and exactly the policy DLL imports;
5. installs the wheel without dependencies into a fresh virtual environment;
6. requires the installed `.pyd` digest to equal the extension extracted from
   the wheel, then imports both `pdfplumber` and `pdfplumber._native` from that
   environment;
7. opens `basic_text.pdf` from a non-ASCII path and from a non-ASCII path with
   more than 260 characters, requiring exact fixture text in both cases.

The long-path case uses a normal Win32 path, not a `\\?\` extended-length
prefix. The job first requires the native runner registry to report
`LongPathsEnabled=1`, as configured by the official hosted-runner image, and
the policy requires a path length of at least 280 characters.

The retained JSON binds the wheel and policy SHA-256 digests, installed and
archive-native module digests, runner and registry state, PE inspection,
interpreter version, fixture and expected-output digests, path-case results,
and extracted-text digests.

## Scope boundary

This job proves installation, import, DLL resolution, and exact fixture
extraction on the named Windows Server 2025 x86-64 hosted runner. It also
proves ordinary non-ASCII and long-path behavior while that host reports
`LongPathsEnabled=1`.
It does not prove behavior when the Windows long-path policy is disabled,
on 32-bit or Arm Windows, on other Windows releases, or for every PDF and
filesystem configuration. The import allowlist describes
this extension's direct PE imports; it is not a security audit of those
libraries or their transitive dependencies.

Primary-source context is recorded in the
[Windows wheel reference note](../references/python-windows-wheels.md).
