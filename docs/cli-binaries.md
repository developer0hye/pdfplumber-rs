# Prebuilt Command-Line Interface binaries

The release workflow is configured to attach versioned `pdfplumber` archives
to the GitHub Release for the next successful tag. The release is not created
unless all five native builds finish and every archive passes the target and
executable format gates below.

## Target matrix

| Target | Native runner | Archive | Platform boundary |
|---|---|---|---|
| `x86_64-unknown-linux-gnu` | Ubuntu 22.04 x86-64 | `.tar.gz` | Linux GNU; built against the Ubuntu 22.04 environment |
| `aarch64-unknown-linux-gnu` | Ubuntu 22.04 AArch64 | `.tar.gz` | Linux GNU; built against the Ubuntu 22.04 environment |
| `x86_64-apple-darwin` | macOS 15 Intel | `.tar.gz` | Rust tier 2; macOS 10.12 is the Rust target floor |
| `aarch64-apple-darwin` | macOS 15 Apple Silicon | `.tar.gz` | Rust tier 1; macOS 11 is the Rust target floor |
| `x86_64-pc-windows-msvc` | Windows 2025 x86-64 | `.zip` | Rust tier 1; Windows 10 or Windows Server 2016 is the Rust target floor |

The Linux archives use the GNU C library and are not static musl binaries. A
build on Ubuntu 22.04 does not prove compatibility with an older GNU C library.
The macOS floors come from the Rust target contract; the workflow does not
raise them with `MACOSX_DEPLOYMENT_TARGET`.

## Release gate

[`cli-release-targets.toml`](../cli-release-targets.toml) is the checked source
for the complete matrix. For every entry, the reusable workflow:

1. routes the job to the declared native runner and requires `rustc` to report
   the same host triple;
2. runs `cargo build --locked --release --package pdfplumber-cli --target
   <target>`;
3. verifies a 64-bit ELF, Mach-O, or PE/COFF header and its exact architecture;
4. packages only `pdfplumber` (`pdfplumber.exe` on Windows), the CLI README,
   and the Apache-2.0 license beneath one versioned archive root; and
5. retains the archive for the GitHub Release only after verification.

The matrix uses `fail-fast: false` so every platform reports its result, while
the release job depends on the complete matrix and therefore cannot publish a
partial set.

## Archive names and installation

Assets use `pdfplumber-cli-<version>-<target>.tar.gz` on Linux and macOS and
`pdfplumber-cli-<version>-<target>.zip` on Windows. Each archive contains a
same-named root directory. Extract the archive, move `pdfplumber` or
`pdfplumber.exe` to a directory on `PATH`, and then inspect `pdfplumber --help`.

These archives are build-gated but not runtime-smoke-tested. [DIST-004](../PRD.md#824-p1--distribution-and-installation)
remains responsible for executing every target archive against a real PDF and
asserting exact output. [DIST-005](../PRD.md#824-p1--distribution-and-installation)
remains responsible for SHA-256 checksums, Software Bill of Materials,
provenance, and attestations. Until that integrity work lands, download only
from the canonical release page and do not treat transport alone as artifact
provenance.
