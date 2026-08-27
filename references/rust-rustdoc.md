# Rust API documentation references

Recorded on 2026-08-27 against `rustc 1.94.1` and Clippy 0.1.94. The links
below are official Rust project documentation and were rechecked as reachable
when this contract was written.

## Sources

- [The `missing_docs` lint](https://doc.rust-lang.org/rustc/lints/listing/allowed-by-default.html#missing-docs)
  reports missing documentation for public items. It is allowed by default, so
  the facade explicitly denies it.
- [The rustdoc lints](https://doc.rust-lang.org/rustdoc/lints.html#broken_intra_doc_links)
  describe `broken_intra_doc_links`, which detects links that cannot be
  resolved or are ambiguous while documentation is built.
- [Clippy `missing_errors_doc`](https://rust-lang.github.io/rust-clippy/stable/index.html#missing_errors_doc)
  reports public `Result`-returning functions without an `# Errors` section.
- [Clippy `missing_panics_doc`](https://rust-lang.github.io/rust-clippy/stable/index.html#missing_panics_doc)
  reports public functions that may panic without a `# Panics` section.

## Applied boundary

`missing_docs` and `broken_intra_doc_links` are crate-level denials because
every stable facade item must render. The two Clippy documentation lints run
with `--no-deps` against the all-feature `pdfplumber` facade: this checks its
public methods without making the separately published low-level crates part
of the `DX-009` stability promise.

The all-feature rustdoc build is necessary because default compilation alone
does not render `serde`- or `parallel`-gated items. Warnings are denied so a
future public item or broken feature-only link cannot leave Continuous
Integration green with incomplete generated documentation.
