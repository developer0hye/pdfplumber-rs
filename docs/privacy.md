# Privacy and local processing

This statement covers document processing by the released `pdfplumber-rs` runtime
surfaces. It distinguishes project-controlled extraction behavior from the
applications, tools, and services that a user may place around the library. Update
this page and its contract test whenever those boundaries change.

## Local extraction boundary

Across the released Rust, Python, Command-Line Interface, and WebAssembly surfaces,
extraction starts from a file path or bytes supplied by the caller and runs in the
caller's process, except for the optional Python repair path described below. The
project runtime does not upload documents. It does not send document contents, extracted text, or document metadata.
Those values are not sent to the project or a managed extraction service. It does not collect usage telemetry, analytics, or crash reports.

The WebAssembly wrapper receives bytes from its host application and parses those
bytes locally; it does not fetch or upload a document on its own. A browser, server,
or other host application can still choose to transfer the original document or
derived output. Audit that host application separately when evaluating a complete
data flow.

Extraction errors, warnings, and output remain with the calling process or its
configured standard streams. The project does not receive them automatically.

## Optional external executable: Python repair

Ordinary extraction does not launch a document-processing executable. The Rust
`Pdf::open_with_repair` API and the Command-Line Interface `--repair` option use
in-process native repair. Python opening with `repair=False` also launches no repair
process.

Python opening with `repair=True` is the exception: it starts Ghostscript as a child
process. The compatibility facade uses an explicit `gs_path` when supplied;
otherwise it searches for `gs`, `gswin32c`, and `gswin64c`, in that order. The
invocation has these data boundaries:

- A path input is converted to an absolute file path as a child-process argument.
- A file-like input sends its remaining stream bytes through standard input.
- A supplied PDF password is passed as the `-sPDFPassword=...` command-line argument.
  Command arguments may be visible through local process inspection on some operating
  systems.
- The selected repair preset is passed as another command-line argument. Repaired PDF
  bytes are read from standard output. Failure details are read from standard error.

Ghostscript is outside the `pdfplumber-rs` process and is not covered by the runtime
privacy promise above. It inherits the invoking user's environment and permissions;
review the selected executable, its configuration, and its own security and privacy
behavior. If that child-process boundary is unacceptable, do not select
`repair=True`. See the [Python compatibility guide](../crates/pdfplumber-py/README.md)
for the current repair API.

## Boundaries outside the project runtime

This statement does not govern a host application, an OCR or preprocessing tool,
Ghostscript, operating-system logging, or downstream storage. It also does not mean
that installation and documentation are offline: package managers may contact
registries, and documentation links may load remote sites. Configure and audit those
components according to the sensitivity of the documents being processed.

Public issue trackers are not a private reporting channel. Remove document content
and metadata from reproductions unless they are intentionally safe to publish; use a
minimal synthetic fixture whenever possible.
