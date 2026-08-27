# Rust Serde JSON compatibility policy

The identifier `serde-json-v1` names the direct JSON representation of the
curated `pdfplumber::models` types. The full identifier is available as
`pdfplumber::models::SERDE_JSON_SCHEMA` when the optional `serde` feature is
enabled and has the value `pdfplumber-rs/serde-json-v1`.

## Scope

For every `0.3.x` release, `serde-json-v1` covers values passed directly to
`serde_json::to_value`, `serde_json::to_string`, `serde_json::from_value`, or
`serde_json::from_str` for all types listed in the
[curated data-model contract](rust-data-models.md#curated-families). Every one
of those types implements both `Serialize` and `Deserialize` when the `serde`
feature is enabled.

The policy freezes the JSON object field names and JSON value kinds; collection
nesting; tuple arity; optional-field `null` versus present-value behavior; enum
variant names; and unit, newtype, tuple, struct, and explicitly tagged enum
encodings. A field rename is incompatible. A field type change that changes its
JSON value kind or nesting is incompatible. Adding or removing a serialized
field is incompatible, and an enum variant or enum encoding change is
incompatible. The stable Rust field-type rules in the data-model contract apply
even when two Rust types could happen to produce the same JSON value kind.

JSON object member order is not guaranteed. The policy also does not guarantee
JSON whitespace or numeric spelling. Consumers must treat objects as mappings
rather than depend on the order in which members happen to be emitted.

## Direct values, not an envelope

This policy keeps the existing raw model representation: it does not add an
envelope or inject a schema field into direct `serde_json` output. Applications
that store heterogeneous or long-lived records should store
`pdfplumber-rs/serde-json-v1` adjacent to the model name and serialized value in
their own container. The identifier distinguishes the compatibility contract;
it is not evidence that an arbitrary JSON value carries its own type name.

Only direct `serde_json` values are covered. Other Serde formats are not
covered because binary and self-describing serializers can choose different
representations. The WebAssembly JavaScript surface uses
`serde_wasm_bindgen` and remains a separate distribution contract rather than
silently inheriting the Rust JSON promise. Python `to_json`, Command-Line
Interface output, and upstream-compatibility serialization are also separate
contracts.

## Enforced producer and consumer fixtures

The committed
[`serde-schema-v1.json`](../crates/pdfplumber/tests/fixtures/serde-schema-v1.json)
fixture contains at least one non-default value for every curated model and
every variant of each curated enum. `crates/pdfplumber/tests/serde_schema.rs`
checks both directions:

1. current typed values must serialize to the frozen v1 JSON values; and
2. every frozen v1 JSON value must deserialize to the current type and
   reserialize without semantic JSON drift.

The compatibility harness separately verifies that the fixture inventory is
exactly the curated model inventory and that primary documentation points to
this policy. Together these gates turn field-name, value-kind, nesting, and
enum-encoding changes into review-visible failures instead of silent drift.

The v1 fixture must never be edited in place to make an incompatible change
green. A future incompatible crate line may retain v1 or introduce a new
identifier and fixture, but it must preserve the old fixture as historical
evidence and publish migration notes. Whether that future line continues to
accept v1 input must be stated explicitly; this policy commits the whole
`0.3.x` line to both producing and consuming v1.
