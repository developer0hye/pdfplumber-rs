# Examples by outcome

Choose the result you want, then run the maintained source example linked from
that outcome. This index covers every executable example under
`crates/*/examples`; the links point to source so that the behavior can be
reviewed before it is reused.

Maturity labels come from [`support-matrix.toml`](../support-matrix.toml), the
checked source for public surface status.
An example does not raise a surface's maturity or imply parity with Python
`pdfplumber`. Rust is currently alpha and WebAssembly is experimental.

Rust examples are compiled in Continuous Integration with all features on the
current stable Rust toolchain. The commands below accept a local PDF after
`--`; replace `document.pdf` with your path. See the
[Rust-specific execution notes](rust-examples.md) for output and safety details.

## Extract content

| Example | Surface | Maturity | Run |
|---|---|---|---|
| [`extract_text.rs`](../crates/pdfplumber/examples/extract_text.rs) | Rust | `alpha` | `cargo run -p pdfplumber --example extract_text -- document.pdf` |
| [`extract_chars.rs`](../crates/pdfplumber/examples/extract_chars.rs) | Rust | `alpha` | `cargo run -p pdfplumber --example extract_chars -- document.pdf` |
| [`extract_words.rs`](../crates/pdfplumber/examples/extract_words.rs) | Rust | `alpha` | `cargo run -p pdfplumber --example extract_words -- document.pdf` |
| [`extract_table.rs`](../crates/pdfplumber/examples/extract_table.rs) | Rust | `alpha` | `cargo run -p pdfplumber --example extract_table -- document.pdf` |

## Inspect document details

| Example | Surface | Maturity | Run |
|---|---|---|---|
| [`inspect_geometry.rs`](../crates/pdfplumber/examples/inspect_geometry.rs) | Rust | `alpha` | `cargo run -p pdfplumber --example inspect_geometry -- document.pdf` |
| [`inspect_metadata.rs`](../crates/pdfplumber/examples/inspect_metadata.rs) | Rust | `alpha` | `cargo run -p pdfplumber --example inspect_metadata -- document.pdf` |

## Handle protected or invalid input

| Example | Surface | Maturity | Run |
|---|---|---|---|
| [`open_encrypted.rs`](../crates/pdfplumber/examples/open_encrypted.rs) | Rust | `alpha` | `cargo run -p pdfplumber --example open_encrypted -- encrypted.pdf password` |
| [`handle_malformed.rs`](../crates/pdfplumber/examples/handle_malformed.rs) | Rust | `alpha` | `cargo run -p pdfplumber --example handle_malformed -- input.pdf` |

Passwords supplied on a command line can be visible to other local processes.
Production applications should obtain secrets through a protected input
mechanism.

## Produce repeatable automation output

| Example | Surface | Maturity | Run |
|---|---|---|---|
| [`serialize_words.rs`](../crates/pdfplumber/examples/serialize_words.rs) | Rust | `alpha` | `cargo run -p pdfplumber --example serialize_words --features serde -- document.pdf` |
| [`parallel_batch.rs`](../crates/pdfplumber/examples/parallel_batch.rs) | Rust | `alpha` | `cargo run -p pdfplumber --example parallel_batch --features parallel -- document.pdf` |

The serialization example identifies the external `serde-json-v1` schema. The
parallel example returns results in zero-based page-index order; its feature is
not available on WebAssembly.

## Explore local browser extraction

| Example | Surface | Maturity | Run |
|---|---|---|---|
| [`browser-demo.html`](../crates/pdfplumber-wasm/examples/browser-demo.html) | WebAssembly | `experimental` | `wasm-pack build --target web crates/pdfplumber-wasm`, then serve `crates/pdfplumber-wasm` over HTTP and open the demo |

The WebAssembly browser demo is experimental. It is a source-level exploration,
not a maintained Vite application, and does not complete `ECOSYS-006`. Package
build and test gates do not execute this HTML file end to end. Review the
[WebAssembly package testing guide](wasm-package-testing.md) before making any
browser-support claim.
