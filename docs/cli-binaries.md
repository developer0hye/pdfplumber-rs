# Prebuilt Command-Line Interface binaries

The release workflow is configured to attach versioned `pdfplumber` archives
to the GitHub Release for the next successful tag. The release is not created
unless all five native builds finish and every archive passes the target and
executable format and runtime smoke gates below.

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
for the complete matrix. [`cli-release-smoke.toml`](../cli-release-smoke.toml)
binds the command, timeout, fixture, and expected output to exact SHA-256 values.
For every target entry, the reusable workflow:

1. routes the job to the declared native runner and requires `rustc` to report
   the same host triple;
2. runs `cargo build --locked --release --package pdfplumber-cli --target
   <target>`;
3. verifies a 64-bit ELF, Mach-O, or PE/COFF header and its exact architecture;
4. packages only `pdfplumber` (`pdfplumber.exe` on Windows), the CLI README,
   and the Apache-2.0 license beneath one versioned archive root; and
5. extracts that archive and executes its binary on the target operating system
   against `tests/fixtures/generated/basic_text.pdf`, requiring exit code zero,
   empty standard error, and exact standard output bound by SHA-256; and
6. retains the archive for the GitHub Release only after verification.

The matrix uses `fail-fast: false` so every platform reports its result, while
the release job depends on the complete matrix and therefore cannot publish a
partial set.

## Archive names and installation

Assets use `pdfplumber-cli-<version>-<target>.tar.gz` on Linux and macOS and
`pdfplumber-cli-<version>-<target>.zip` on Windows. Each archive contains a
same-named root directory. Extract the archive, move `pdfplumber` or
`pdfplumber.exe` to a directory on `PATH`, and then inspect `pdfplumber --help`.

The smoke gate proves one deterministic JSON text-extraction path on the exact
fixture bytes. It does not prove every subcommand, PDF class, or older operating
system version. The [release integrity gate](release-integrity.md) binds every
native archive to `SHA256SUMS`, a group-scoped SPDX document, GitHub Actions
build provenance, and signed SBOM evidence. Verify both attestation predicates
before execution; transport from the canonical release page is not provenance
by itself.
