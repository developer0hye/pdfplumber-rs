# Rust feature policy

Cargo features are additive capabilities. Enabling one may add APIs, trait
implementations, diagnostics, or an execution path, but it must not remove an
existing API or silently select different extraction semantics. Runtime options
such as `TextOptions`, `WordOptions`, and `TableSettings` control extraction
semantics.

## Public facade features

| Feature | Default | Adds | Does not change |
|---|---:|---|---|
| `std` | Yes | Filesystem-path constructors such as `Pdf::open_path` | Byte and reader parsing or extraction output |
| `serde` | No | Serde traits for the curated models through `pdfplumber-core/serde` | Model values or the extraction algorithms that produce them |
| `parallel` | No | `Pdf::pages_parallel` through Rayon | Sequential extraction or page-index result order |

The default `std` feature serves the primary filesystem workflow. It enables
`Pdf::open_path` and the matching password method. `Pdf::open_reader` and
`Pdf::open_bytes` remain available without default features. The crate still
uses the Rust standard library when `std` is disabled; this is not a `no_std`
contract. The feature name is retained for compatibility and gates only the
filesystem-path API family.

## Choosing a combination

Ordinary native applications use the defaults:

```toml
[dependencies]
pdfplumber = "0.3"
```

Add only the integrations the application consumes:

```toml
[dependencies]
pdfplumber = { version = "0.3", features = ["serde", "parallel"] }
```

WebAssembly and other byte-only consumers disable the path feature explicitly:

```toml
[dependencies]
pdfplumber = { version = "0.3", default-features = false, features = ["serde"] }
```

No public features are mutually exclusive. Because Cargo unifies dependency
features across a build graph, every feature must remain safe when another
dependency enables it.

## Workspace and packaging flags

The facade forwards `serde` to `pdfplumber-core/serde`; ordinary applications
should still depend only on `pdfplumber`. Advanced parser users can enable
`pdfplumber-parse/tracing`, which compiles diagnostic events without changing
parser results. The facade deliberately does not re-export this parser-internal
flag. The `pdfplumber-py/extension-module` flag changes PyO3 linkage for Python
packaging, not extraction behavior. The WebAssembly package deliberately uses
`default-features = false` plus `serde`, while the Command-Line Interface enables
`serde` for structured output. These package-specific choices are verified by
their parser, wheel, source-distribution, WebAssembly, and Command-Line Interface
jobs.

## Verification matrix

Continuous Integration runs the same semantic regression against two materially
different generated fixtures under these facade configurations:

| Configuration | Contract |
|---|---|
| No defaults | Byte-based text, geometry, and table extraction |
| Defaults | The same extraction plus path-versus-byte identity |
| No defaults + `serde` | The same extraction plus model serialization |
| No defaults + `parallel` | The same extraction plus sequential/parallel page identity |
| All features | Union safety for every public feature |

The regression fingerprints exact text, characters, words, graphics, images,
page geometry, and detected tables. Separate Serde schema and concurrency tests
retain their deeper feature-specific contracts. Parser tests are also rerun
with `pdfplumber-parse/tracing` enabled.

The design follows the official Cargo feature rules recorded in the
[source note](../references/cargo-features.md).
