# Rust errors and diagnostics

The high-level `pdfplumber` crate returns the opaque `PdfError` type. Classify
it with `PdfError::kind()` and the non-exhaustive `PdfErrorKind` enum instead of
matching implementation-specific parser messages. `PdfErrorKind::code()` gives
a stable uppercase code for logs, metrics, and retry policy.
The cross-surface [error and resource-limit guide](errors-and-resource-limits.md)
adds pinned Python behavior, complete Rust budget enforcement status, adapter
translations, and host-level controls for untrusted inputs.
The [encryption and repair guide](encryption-and-repair.md) adds the exact
password-required, invalid-password, unsupported-encryption, Ghostscript, and
native-repair boundaries that determine recovery policy.
The [parser and font limitations](parser-and-font-limitations.md) guide records
which structural and content failures remain fatal, which fallbacks continue,
and which warnings are available only through the Rust facade.

## Safe default output

`Display` gives a concise description and a next action. `Debug` gives the
kind, safe context, typed resource-limit fields, and whether a source exists.
Neither `Display` nor `Debug` includes an input path, an underlying source
message, or document content. This makes ordinary `?`, `{error}`, and
`{error:?}` reporting safe by default.

The source chain is an explicit diagnostic boundary. Calling
`std::error::Error::source` reveals the underlying cause preserved across the
facade boundary, including a downcastable `std::io::Error` for direct I/O
failures. Source-chain inspection is opt-in and may reveal sensitive parser or
operating-system details, so applications must send it only to an appropriately
protected diagnostic sink.

The outer `PdfError` renders only its own message. This follows the standard
library rule that a wrapped underlying cause should be available through
`Error::source` or rendered by the outer `Display`, but not both.

## Typed fields

Use these public values instead of parsing text:

- `PdfErrorKind` identifies parse, I/O, font, interpreter, resource-limit,
  password, and other categories. Match it with a wildcard because it is
  non-exhaustive.
- `PdfErrorContext` carries a library-owned operation label and any available
  location. A page index and object ID identify the location; every page index
  is zero-based, while an object ID contains the PDF indirect-object number and
  generation.
- `PdfResourceLimit` carries the limit name, configured limit, and observed
  value. It is present only when the kind is `ResourceLimit`.

Known page context is added at the high-level facade. Loading an unavailable
page reports the requested index. Once the backend has resolved a page, later
geometry, content-stream, annotation, hyperlink, image, and form-field failures
also report that page's indirect object ID. Document-wide operations have no
page or object context unless the backend can identify one.

## Handling pattern

Applications should branch on `error.kind()`, inspect `error.context()` or
`error.resource_limit()` when relevant, show `error` to the user, and reserve
`error.source()` for protected diagnostics. Password errors are actionable
without echoing the supplied password. Resource-limit errors identify whether
the caller should raise a configured limit or use a smaller input.

Replacing the old public string-payload enum variants with this opaque type is
an intentional alpha API change. Migrate `match PdfError::ParseError(_)` to
`error.kind() == PdfErrorKind::Parse`, and migrate resource-limit destructuring
to `error.resource_limit()`.
