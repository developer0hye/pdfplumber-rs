# Rust example-target references

Rechecked: 2026-08-27.

## Cargo example targets

Source: [Cargo Targets](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#examples)

- Files in a package's `examples` directory are executable example targets.
- `[[example]].required-features` prevents a feature-dependent example from
  being selected without the API it demonstrates.

Source: [`cargo check`](https://doc.rust-lang.org/cargo/commands/cargo-check.html)

- `--examples` selects every example target.
- `--all-features` enables the optional APIs required by the Serde and parallel
  examples.

## Application to DX-010

The facade package keeps ordinary examples auto-discovered and declares only
the two feature-specific targets explicitly. Continuous Integration runs
`cargo check -p pdfplumber --examples --all-features` on current stable Rust,
so every task program is compiler-checked through the public facade.
