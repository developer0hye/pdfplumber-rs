# Rust error-chain design sources

Checked on 2026-08-27 for DX-007.

## Rust standard library

- [`std::error::Error::source`](https://doc.rust-lang.org/std/error/trait.Error.html#method.source)
  defines access to the lower-level cause across an abstraction boundary. The
  `Error` documentation says a wrapped cause should be returned by `source()`
  or rendered by the outer `Display`, but not both.
- [`std::io::Error`](https://doc.rust-lang.org/std/io/struct.Error.html) implements
  `Error`, exposes its own source, and retains its machine-readable
  [`ErrorKind`](https://doc.rust-lang.org/std/io/enum.ErrorKind.html). Preserving
  the concrete value therefore keeps both downcasting and I/O classification.

## thiserror

- [`thiserror` 2.0.20](https://docs.rs/thiserror/2.0.20/thiserror/) documents
  `#[source]`/`#[from]` source propagation and recommends an opaque public error
  wrapper when its private representation needs to evolve without repeated API
  breaks.

## Applied boundary

`PdfError` uses the opaque-public-type pattern but implements `Error` directly
so its default `Display` and `Debug` can exclude source messages. The original
cause remains available through `std::error::Error::source`; public kind,
context, object ID, and resource-limit types provide machine-readable fields
without exposing parser internals.
