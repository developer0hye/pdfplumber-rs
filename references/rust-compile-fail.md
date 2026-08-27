# Rust compile-fail documentation references

Rechecked: 2026-08-27.

## rustdoc documentation tests

Source: [rustdoc documentation tests](https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html#compile_fail)

- `compile_fail` requires the fenced Rust program to fail compilation; a
  program that starts compiling makes the documentation test fail.
- `no_run` requires successful compilation without executing the program.
- Rust warns that an intentional failure may become valid in a later compiler,
  so each negative example must describe the current API rule it protects.

## Application to DX-011

The facade pairs each `compile_fail` block with a compiling `no_run`
alternative. The three protected rules are that `Pages` needs explicit
conversion before iterator adapters, borrowed page views cannot escape their
source `Pdf`, and opaque `PdfError` values are classified through `kind()`.
Continuous Integration runs all-feature facade doctests on stable Rust and
Rust 1.85 so both the negative and positive halves remain current.
