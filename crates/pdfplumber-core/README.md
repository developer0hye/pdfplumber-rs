# pdfplumber-core

`pdfplumber-core` provides the backend-independent data types and extraction
algorithms used by [`pdfplumber-rs`](https://github.com/developer0hye/pdfplumber-rs).
It includes geometry, text and word grouping, shapes, images, search, table
detection, serialization models, validation, and diagnostic rendering types.

Most applications should depend on the high-level
[`pdfplumber`](https://crates.io/crates/pdfplumber) crate instead. Direct use of
`pdfplumber-core` is intended for advanced integrations that provide their own
PDF parser or consume the lower-level algorithms. The `0.4.x` API is alpha and
may change before its stability contract is complete.

API documentation is available on
[`docs.rs/pdfplumber-core`](https://docs.rs/pdfplumber-core). Feature and
maturity boundaries are recorded in the repository's
[support matrix](https://github.com/developer0hye/pdfplumber-rs/blob/main/docs/support.md).

Licensed under [Apache-2.0](https://github.com/developer0hye/pdfplumber-rs/blob/main/LICENSE).
