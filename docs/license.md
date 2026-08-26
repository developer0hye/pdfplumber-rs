# License policy

The current `pdfplumber-rs` source tree and releases from `0.3.0` onward are
licensed under the [Apache License, Version 2.0](../LICENSE), identified by the
SPDX expression `Apache-2.0`.

Releases up to and including `0.2.0` were distributed under
`MIT OR Apache-2.0`. That grant remains valid for those historical versions; it
does not describe the license for the current source tree or later releases.

## Package artifacts

Every Rust crate, Python distribution, and npm package must carry both:

- `Apache-2.0` license metadata in the ecosystem-native package manifest; and
- an exact copy of the repository's canonical [`LICENSE`](../LICENSE) text.

The package-root copies exist because Cargo, Python, and npm packaging tools
operate from different roots. They are not independent license sources:
[`license-policy.toml`](../license-policy.toml) records the canonical digest,
and `scripts/check_package_licenses.py` rejects a missing or divergent copy.
Required Continuous Integration builds and inspects all three artifact
families before changes can merge.

Third-party dependencies, vendored data, and test fixtures retain their own
licenses. This project policy does not relicense those materials; their
notices and provenance remain authoritative.
