# Rust CLI binary release sources

Sources used for the five-target prebuilt Command-Line Interface design.

- [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
  lists standard native Linux x86-64/AArch64, macOS Intel/Apple Silicon, and
  Windows x86-64 labels for public repositories.
- [Rust platform support](https://doc.rust-lang.org/rustc/platform-support.html)
  defines the tier guarantees for the selected target triples. The separate
  [Apple target page](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html)
  records the macOS floors and x86-64 tier.
- [`cargo build`](https://doc.rust-lang.org/cargo/commands/cargo-build.html)
  defines `--target` and the target-specific release output directory.
- [Typst's release workflow](https://github.com/typst/typst/blob/main/.github/workflows/release.yml)
  informed the target-keyed matrix, non-Windows archive versus Windows ZIP,
  inclusion of public README/license files, and complete per-target reporting.

This repository additionally requires a native host match, validates the
executable header and architecture, generates the matrix from one checked TOML
policy, and blocks GitHub Release creation on the complete matrix. Runtime
fixture smoke tests and integrity/provenance assets remain separate tasks.
