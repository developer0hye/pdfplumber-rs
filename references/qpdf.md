# qpdf (C++)

- **URL:** https://github.com/qpdf/qpdf
- **Documentation:** https://qpdf.readthedocs.io/en/stable/
- **License:** Apache-2.0
- **Validated version:** 12.3.2

## Encryption fixture generation

The official command-line interface can create 40-, 128-, and 256-bit encrypted
PDFs with separate user and owner passwords. `--use-aes`, `--force-V4`, and
`--force-R5` make the content algorithm and encryption dictionary revision
explicit for compatibility probes. RC4 and R5 are weak or deprecated; current
qpdf requires explicit test-only acknowledgement for weak output.

`--show-encryption` reports revision, permission bits, and stream/string/file
encryption methods. The [command options](https://qpdf.readthedocs.io/en/stable/qpdf-options.html),
[encryption design](https://qpdf.readthedocs.io/en/stable/design.html), and
[weak-cryptography policy](https://qpdf.readthedocs.io/en/stable/weak-crypto.html)
are the primary references.

## Relevance to pdfplumber-rs

Use qpdf only to generate and inspect controlled encryption fixtures. It is not
a runtime dependency and its successful decryption is not evidence that the
Rust facade, Python reference, or adapters produce equivalent extracted output.
