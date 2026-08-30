# pdfplumber-parse

`pdfplumber-parse` is the PDF parsing and content-stream interpretation layer
used by [`pdfplumber-rs`](https://github.com/developer0hye/pdfplumber-rs). It
provides the backend contract, the default `lopdf` backend, page geometry,
content handlers, text state, font metrics, character maps, and lenient token
recovery used by the high-level facade.

Most applications should depend on the high-level
[`pdfplumber`](https://crates.io/crates/pdfplumber) crate instead. Direct use of
`pdfplumber-parse` is intended for advanced parser and backend integrations.
The `0.4.x` API is alpha and may change before its stability contract is
complete.

API documentation is available on
[`docs.rs/pdfplumber-parse`](https://docs.rs/pdfplumber-parse). Parser and font
limitations are documented in the repository's
[source-bound guide](https://github.com/developer0hye/pdfplumber-rs/blob/main/docs/parser-font-limitations.md).

Licensed under [Apache-2.0](https://github.com/developer0hye/pdfplumber-rs/blob/main/LICENSE).
