# Task-oriented Rust examples

Every example below is a complete fallible program compiled with all features
on stable Rust and Rust 1.85 in Continuous Integration. Replace the sample path
with your own local PDF. Arguments after `--` belong to the example.

## Extraction and inspection

```bash
cargo run -p pdfplumber --example extract_text -- document.pdf
cargo run -p pdfplumber --example extract_words -- document.pdf
cargo run -p pdfplumber --example extract_table -- document.pdf
cargo run -p pdfplumber --example inspect_geometry -- document.pdf
cargo run -p pdfplumber --example inspect_metadata -- document.pdf
```

Word and geometry coordinates are displayed page-space points with a top-left
origin. Table cells are printed in row order, and absent metadata fields are
shown as `<unset>`.

## Password and malformed-input handling

```bash
cargo run -p pdfplumber --example open_encrypted -- encrypted.pdf password
cargo run -p pdfplumber --example handle_malformed -- input.pdf
```

The password example passes the credential as bytes to
`Pdf::open_path_with_password` and never prints it. The malformed-input example
recognizes the stable `PdfErrorKind::Parse` category; other failures still
propagate to the caller. Command-line arguments can be visible to other local
processes, so production applications should obtain secrets through a protected
input mechanism.

## Optional features

```bash
cargo run -p pdfplumber --example serialize_words --features serde -- document.pdf
cargo run -p pdfplumber --example parallel_batch --features parallel -- document.pdf
```

`serialize_words` prints the external `serde-json-v1` schema identifier beside
the direct JSON representation; the identifier is not embedded automatically.
`parallel_batch` uses `Pdf::pages_parallel` and consumes the returned vector in
zero-based page-index order. The parallel feature is unavailable on WebAssembly.

The source programs live in
[`crates/pdfplumber/examples`](../crates/pdfplumber/examples), and the
[public API guide](rust-api.md) defines the stable types they exercise.
