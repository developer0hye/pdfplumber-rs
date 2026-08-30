# Encryption and repair

This guide records the supported and observed encryption and repair behavior of
the pinned Python reference and the current `pdfplumber-rs` surfaces. Encryption,
permissions, and repair are separate concerns: authenticating a password does
not enforce document permissions, and rewriting a malformed file does not prove
that its content is safe or unchanged.

| Surface | Password-aware open | Repair path | Permission enforcement |
| --- | --- | --- | --- |
| Pinned Python `pdfplumber` | `password=` through pdfminer.six | Ghostscript through `repair=True` or `pdfplumber.repair` | Flags are observable but extraction does not enforce them |
| Current Rust library | Path, bytes, and reader methods | In-process, byte-only lopdf rewrite | Neither exposed nor enforced |
| Current Python adapter | `password=` through the Rust facade | Ghostscript through `repair=True` | Neither exposed nor enforced |
| Current Command-Line Interface | `--password` | Native `--repair`, except the combined flags do not compose | Neither exposed nor enforced |
| Current WebAssembly adapter | Not exposed | Not exposed | Not exposed |

“Supported” below means that the named path is implemented and bounded by the
specific observations and limitations recorded here. It does not mean every PDF
producer, malformed encryption dictionary, crypt filter, or repaired output is
accepted.

## Pinned Python encryption

The pinned environment combines `pdfplumber==0.11.10` with
`pdfminer.six==20260107`. `pdfplumber.PDF` constructs pdfminer.six's
`PDFDocument`, and only the PDF `Standard` security handler is accepted. The
handler registry and accepted revisions are:

| Encryption dictionary | Accepted revision | Content algorithm |
| --- | --- | --- |
| `V=1` or `V=2` | `R=2` or `R=3` | RC4 |
| `V=4` | `R=4` | `V2` (RC4), `AESV2` (AES-128), or `Identity` |
| `V=5` | `R=5` or `R=6` | `AESV3` (AES-256) |

For `V=4` and `V=5`, the selected string and stream filter names must be the
same. `V=0`, `V=3`, unknown `V` values, unsupported revisions, unequal string
and stream filters, and unknown crypt-filter methods fail. Public-key security
handlers and arbitrary plug-in security handlers are outside this contract.

### Password authentication and failures

The reference tries the user password first and then derives and tries the owner
password. The generated probe showed that both user and owner passwords
authenticate in the generated R2-R6 matrix, and that an empty user password
opens without passing `password=`. R2-R4 passwords are encoded as Latin-1; a
Python string that cannot be encoded that way raises `UnicodeEncodeError`.
R5/R6 passwords are UTF-8, limited to 127 bytes, and R6 applies SASLprep.

```python
import pdfplumber

with pdfplumber.open("encrypted.pdf", password="user-pass") as pdf:
    print(pdf.pages[0].extract_text())
```

For a document with a non-empty user password, missing and incorrect non-empty
passwords both become `PdfminerException` with an empty string. The wrapped sole
argument is pdfminer.six's `PDFPasswordIncorrect`, but an application should not
inspect or log that protected detail merely to guess which credential failed.
Unsupported encryption is also wrapped in `PdfminerException`, usually with a
non-empty `PDFEncryptionError` message. Do not classify unsupported encryption
as a wrong password from public exception type alone.

### Permission flags and metadata

The encryption dictionary's `/P` value is decoded so that
`PDF.doc.is_printable`, `PDF.doc.is_modifiable`, and
`PDF.doc.is_extractable` expose `/P` permission flags. These attributes are
pdfminer.six implementation objects under `PDF.doc`, not top-level pdfplumber
compatibility fields.

pdfplumber enumerates `PDFPage.create_pages` directly and does not enforce those
flags; restricted documents therefore remain extractable. Applications that
choose to honor author restrictions must inspect and enforce policy separately;
a successful extraction is not evidence that copying, printing, or modification
was permitted.

```python
import pdfplumber

with pdfplumber.open("restricted.pdf", password="user-pass") as pdf:
    print(pdf.doc.is_extractable)  # The flag may be False.
    print(pdf.pages[0].extract_text())  # pdfplumber still extracts.
```

For revision 4 and later, `EncryptMetadata=false` leaves metadata streams
outside content decryption. It does not make all document metadata public or
safe to log, and it does not change the handling of ordinary encrypted strings
and streams.

## Pinned Python Ghostscript repair

`PDF.open(..., repair=True)` and the public `pdfplumber.repair(...)` helper both
call Ghostscript. This is a subprocess rewrite before normal pdfminer.six
opening, not parser recovery inside pdfplumber.

### Executable and arguments

The accepted preset names are `default`, `prepress`, `printer`, `ebook`, and
`screen`. For resolution, an explicit truthy `gs_path` wins, otherwise discovery
checks `gs`, `gswin32c`, then `gswin64c`. A truthy but nonexistent explicit path
is attempted and produces the operating system's process-launch error; it does
not fall back to discovery.

The base argument sequence is
`-sstdout=%stderr -o - -sDEVICE=pdfwrite -dPDFSETTINGS=/...`; a truthy password
adds `-sPDFPassword=...` to the child-process argument list. That credential can
be visible to local process inspection. An empty password is not appended.

```python
import pdfplumber

with pdfplumber.open(
    "damaged.pdf",
    repair=True,
    gs_path="/reviewed/path/to/gs",
    repair_setting="prepress",
) as pdf:
    print(pdf.pages[0].extract_text())
```

### Input, output, ownership, and failure

For a path, path input is converted to an absolute path argument; a file-like
input is read from its current position and sent through standard input. The
original caller-owned stream is consumed but not closed.

Repaired standard output is held in a `BytesIO`; standard error is held in
memory, and the subprocess has no timeout. On failure, a nonzero status raises
plain `Exception` with decoded standard error. A launch failure such as a
missing explicit executable remains its ordinary `OSError` subtype. If
discovery finds no executable, the helper raises its two-line installation
`Exception`.

```python
from io import BytesIO

import pdfplumber

source = BytesIO(b"%PDF-...")
repaired = pdfplumber.repair(source)
try:
    assert repaired.tell() == 0
finally:
    repaired.close()
```

At the public helper boundary, when `outfile` is supplied, `pdfplumber.repair`
writes the bytes and returns `None`; without `outfile`, it returns a caller-owned
`BytesIO` positioned at zero. `PDF.open(repair=True)` owns and closes the
repaired stream but not an original caller-owned stream. In that integrated
route, `.path` is `None`, the normal password and metadata arguments are applied
after Ghostscript returns, and the repaired bytes are parsed like any other
input.

In either entry point, repair runs before the normal parser and does not make
every malformed input recoverable. Ghostscript can discard, normalize,
re-encode, recompress, reorder, or otherwise alter structures. Retain the
original, validate the output, and compare the application-visible content that
matters.

## Current Rust library

Rust encryption and repair are independent APIs. Password-aware opening uses
lopdf decryption; native repair loads and saves a lopdf document without
Ghostscript.

### Password-aware input family

The public methods are `open_path_with_password`, `open_bytes_with_password`,
and `open_reader_with_password`; the password type is `&[u8]`. The hidden
compatibility aliases remain `open_file_with_password` and
`open_with_password`.

```rust
use pdfplumber::Pdf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pdf = Pdf::open_path_with_password("encrypted.pdf", b"user-pass", None)?;
    println!("{}", pdf.page(0)?.extract_text(&Default::default()));
    Ok(())
}
```

The current order means that passwordless opening auto-decrypts a document whose
user password is empty; after that automatic empty-user decryption, any supplied
password is ignored. Separately, a supplied password is ignored for an
unencrypted document. For a document that requires a non-empty credential,
missing credentials return `PdfErrorKind::PasswordRequired`; incorrect
credentials return `PdfErrorKind::InvalidPassword`, while unsupported
encryption structure remains `PdfErrorKind::Parse`. Callers must not turn every
parse failure into a credential retry.

The current dependency source accepts encryption versions 1, 2, 4, and 5 and
the `V2`, `AESV2`, `AESV3`, and `Identity` crypt filters. Generated qpdf 12.3.2
fixtures produced a more precise facade observation on 2026-08-30:

| Probe | User password | Owner password | Missing/wrong non-empty password |
| --- | --- | --- | --- |
| R2 / V1 / RC4 | Text matched source | Opened, but text was empty | Typed password failure |
| R3 / V2 / RC4 | Text matched source | Opened, but text was empty | Typed password failure |
| R4 / V4 / RC4 | Text matched source | Opened, but text was empty | Typed password failure |
| R4 / V4 / AES-128 | Text matched source | Opened, but text was empty | Typed password failure |
| R5 / V5 / AES-256 | Text matched source | Text matched source | Typed password failure |
| R6 / V5 / AES-256 | Text matched source | Text matched source | Typed password failure |

In short, user-password extraction succeeded for R2, R3, R4 with RC4, R4 with
AES-128, and R5/R6 with AES-256, while owner-password extraction succeeded for
R5/R6 but produced empty text for the generated R2-R4 probes. Therefore, do not
rely on legacy owner-password extraction until that residual is fixed. This
observed limitation takes precedence over the broader “user and owner” wording
in the current method comment.

At this boundary, the current facade neither exposes nor enforces document
permission flags. The restricted R6 probe still extracted text with a user
password. Treat permission policy as an application responsibility and keep it
distinct from authentication.

### In-process native repair

`Pdf::open_bytes_with_repair` is byte-only and accepts `RepairOptions`. Its
hidden alias is `Pdf::open_with_repair`. The fields `rebuild_xref`,
`fix_stream_lengths`, and `remove_broken_objects` all default to `true`.

```rust
use pdfplumber::{Pdf, RepairOptions};

fn open_repaired(bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let (pdf, result) = Pdf::open_bytes_with_repair(
        bytes,
        None,
        Some(RepairOptions::default()),
    )?;
    eprintln!("logged fixes: {}", result.log.len());
    println!("pages: {}", pdf.page_count());
    Ok(())
}
```

This path is best effort and deliberately narrow:

- loading with lopdf must succeed before native repair can run;
- saving always writes a fresh cross-reference table; the `rebuild_xref` flag
  does not run a separate marker scanner or add a log entry;
- direct missing or incorrect stream lengths are rewritten; indirect `/Length`
  references are skipped;
- dangling references in arrays, dictionaries, and stream dictionaries are
  recursively replaced with `Null`;
- all option combinations serialize the loaded document, including the
  all-disabled combination;
- `RepairResult::has_repairs()` reports whether the log is non-empty, not
  whether serialization rewrote bytes.

Critically, native repair has no password argument and is not an
encrypted-document repair contract. In the generated encrypted probes, the
rewrite retained encryption and the subsequent passwordless open returned
`PasswordRequired`. Callers must not assume that repair decrypts, authenticates,
or composes with a separate credential.

For its wired byte budget, the original and repaired byte sequences are both
checked against `max_input_bytes`. Other parser time and memory boundaries
remain those in the
[error and resource-limit guide](errors-and-resource-limits.md). Native repair
does not verify semantic equivalence and is not a sanitizer.

## Current adapter matrix

### Python adapter

The Python adapter accepts `password=` on paths and seekable binary streams. It
passes the encoded password to the Rust facade and maps Rust password failures
to the compatibility `PdfminerException`. As in the Rust facade, missing and
wrong non-empty passwords have the pinned empty public message, while
unsupported encryption keeps a non-empty parser diagnostic.

Python `repair=True` uses the bundled Ghostscript helper, not Rust native
repair. Its lookup, arguments, ownership, preset, password, and failure behavior
follow the pinned integrated open contract described above. However, the
current package does not export the upstream public `pdfplumber.repair(...)`
helper: `pdfplumber.__all__` contains only `_native`, and the bundled
`pdfplumber.repair` module defines the private `_repair` helper only. This remains
open under the public module and `REPAIR-001` tasks.

### Command-Line Interface

In the current parser, all extraction commands accept `--password` and
`--repair`; `validate` accepts only `--password`. The extraction set is `text`,
`chars`, `words`, `tables`, `info`, `annots`, `links`, `bookmarks`, `forms`,
`debug`, `search`, `images`, and the hidden compatibility snapshot command.

The two flags are not compositional: when both `--password` and `--repair` are
present, the CLI performs password-only opening and skips repair. It emits no
warning that repair was skipped. With `--repair` alone, the CLI reads the file
into memory, applies default native repair, and reports non-empty repair-log
entries to standard error. An encrypted input still requires a password after a
native rewrite, so repair-only opening fails.

```console
$ pdfplumber text encrypted.pdf --password user-pass
$ pdfplumber text damaged.pdf --repair
# This authenticates but does not repair:
$ pdfplumber text encrypted-damaged.pdf --password user-pass --repair
```

Because the CLI has no secret-input channel, the CLI password is a command-line
argument and may be visible to shell history and local process inspection.
Prefer a protected wrapper or another surface when that exposure is
unacceptable.

### WebAssembly

At this surface, the WebAssembly adapter exposes neither password-aware opening
nor repair. `WasmPdf.open` calls `Pdf::open_bytes(data, None)`, so a
non-empty-password document becomes a JavaScript error derived from Rust's safe
outer message. A host application must decrypt or repair elsewhere, then make
an explicit trust decision before passing bytes into WebAssembly.

## Security and operations

The central operational rule is that a password is not a sandbox. PDF
permission bits are advisory in these surfaces, decrypted content can still be
malicious or resource-intensive, and successful authentication says nothing
about a file's safety. Likewise, repair is a lossy rewrite boundary, not proof
that content, signatures, attachments, forms, metadata, structure,
accessibility, or rendering stayed equivalent.

For untrusted files:

1. Keep the original immutable and store repaired output separately.
2. Resolve `gs_path` to a reviewed executable rather than trusting a mutable
   search path in a privileged service.
3. Always run untrusted parsing and Ghostscript with host-enforced CPU, memory,
   file, and time limits. The Python Ghostscript helper has no timeout, and the
   native parser cannot interrupt every allocation through an option.
4. Restrict the child process's filesystem, environment, network, and output
   permissions according to the host platform.
5. Apply an input byte limit before invoking a subprocess or parser, then bound
   repaired output and downstream extracted output separately.
6. Validate the repaired PDF and compare required text, page count, metadata,
   attachments, signatures, forms, and visual output before replacing anything.
7. In ordinary telemetry, do not log passwords, decrypted bytes, original
   paths, Ghostscript arguments, or protected source chains. Use safe request
   identifiers and stable error categories in ordinary logs.
8. Enforce document permission policy explicitly if the application promises to
   honor it; neither authentication nor extraction does so automatically.

See the [privacy statement](privacy.md) for the complete local-process and
Ghostscript data boundary.

## Validation and provenance

Pinned source identity:

- [`pdfplumber/pdf.py`](https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/pdf.py),
  Git object `f3d2dc69906c1c5b946916f80dce661b5f00f32f` from official tag
  `v0.11.10` at `7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62`;
- [`pdfplumber/repair.py`](https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/repair.py),
  Git object `2e4df9aaf0c034077e4ef68b5c776c975fa1eed4` from the same tag;
- [`pdfminer/pdfdocument.py`](https://github.com/pdfminer/pdfminer.six/blob/20260107/pdfminer/pdfdocument.py),
  Git object `9287d0c7d64b6192139ee3645f6119784fa14d03` from official tag
  `20260107` at `9e1243c4ad000bf9bbe60e81fc8dde2fccc0ed3b`;
- [`pdfminer/pdfpage.py`](https://github.com/pdfminer/pdfminer.six/blob/20260107/pdfminer/pdfpage.py),
  Git object `8643a06d4a278c67f0421decfee3551ac686f7d6` from the same tag.

The pinned files were byte-identical to the installed CPython 3.13 reference
environment. Current behavior is anchored in:

- [`pdfplumber/src/pdf.rs`](../crates/pdfplumber/src/pdf.rs);
- [`pdfplumber-core/src/repair.rs`](../crates/pdfplumber-core/src/repair.rs);
- [`pdfplumber-parse/src/lopdf_backend.rs`](../crates/pdfplumber-parse/src/lopdf_backend.rs);
- [`pdfplumber-py/src/lib.rs`](../crates/pdfplumber-py/src/lib.rs);
- [`pdfplumber-py/python/pdfplumber/repair.py`](../crates/pdfplumber-py/python/pdfplumber/repair.py);
- [`pdfplumber-cli/src/shared.rs`](../crates/pdfplumber-cli/src/shared.rs);
- [`pdfplumber-cli/src/cli.rs`](../crates/pdfplumber-cli/src/cli.rs);
- [`pdfplumber-wasm/src/lib.rs`](../crates/pdfplumber-wasm/src/lib.rs).

The 2026-08-30 matrix used qpdf 12.3.2, the repository's positioned-text
fixture, synthetic credentials, pinned CPython 3.13, an exact current Maturin
1.14.1 wheel, and the current CLI. Weak RC4 and R5 output was generated only for
compatibility testing, not as a deployment recommendation. Representative
generation commands were:

```console
$ qpdf --allow-weak-crypto --encrypt user-pass owner-pass 40 -- input.pdf r2-v1-rc4.pdf
$ qpdf --allow-weak-crypto --encrypt user-pass owner-pass 128 --use-aes=y -- input.pdf r4-v4-aes128.pdf
$ qpdf --encrypt user-pass owner-pass 256 --force-R5 -- input.pdf r5-v5-aes256.pdf
$ qpdf --encrypt user-pass owner-pass 256 -- input.pdf r6-v5-aes256.pdf
$ qpdf --password=user-pass --show-encryption r6-v5-aes256.pdf
```

The [qpdf reference record](../references/qpdf.md) links the official option and
encryption documentation. The matrix compared page count and exact extracted
text for missing, wrong, user, and owner credential cases. A restricted R6
fixture set extraction, printing, and modification permissions off and verified
that both reference and current extraction still returned text.

Focused repository checks are:

```console
$ python -m unittest compat.tests.test_encryption_repair_docs
$ cargo test -p pdfplumber --test repair_integration
$ cargo test -p pdfplumber-cli shared::tests
$ PYO3_PYTHON="$(uv python find 3.13)" cargo test -p pdfplumber-py
```

## Claim boundary

For scope, encryption and repair documentation is not compatibility evidence and
does not approve a compatibility deviation. DOC-013 changes no runtime behavior.
The generated probe is bounded evidence for the named versions, inputs, and
date; it is not an exhaustive cryptographic certification or malformed-input
corpus.

`PARITY-012`, `CLI-019`, `PDF-014`, `PDF-017` through `PDF-019`,
`PARSE-ENC-008`, `REPAIR-001` through `REPAIR-011`, parser and malformed-input
robustness, public Python module compatibility, fuzzing, security, performance,
timeout, packaging, and strict compatibility gates remain independent and open
according to the PRD. No open task is completed merely because its present
boundary is documented here.
