# Rust Serde JSON schema references

Retrieved: 2026-08-27. Used for `DX-006` only; these notes define the upstream
serialization rules that the local compatibility fixtures make explicit.

## Serde enum representations

Source: [Serde enum representations](https://serde.rs/enum-representations.html)

- Serde's default enum representation is externally tagged. Unit, newtype,
  tuple, and struct variants therefore have observably different JSON shapes.
- `ExtractWarningCode` already selects the adjacent `type` and `detail` tags;
  the v1 fixture freezes that deliberate exception as well as default enum
  shapes.

## Serde field attributes

Source: [Serde field attributes](https://serde.rs/field-attrs.html)

- `rename` changes serialized and deserialized field names, while `alias`
  accepts an additional input name. Either can affect compatibility and must
  be reviewed against both fixture directions.
- `default`, `skip`, and conditional skipping change whether missing, null, or
  present values are accepted or emitted. The v1 policy therefore records
  optional values explicitly rather than relying only on round trips.

## Serde JSON conventions

Source: [Structs and enums in JSON](https://serde.rs/json.html)

- Named structs serialize as JSON objects, tuple structs as arrays, and
  newtype structs as their inner value.
- The local fixture compares semantic `serde_json::Value` values. It does not
  claim that JSON member order, whitespace, or numeric spelling is stable.
