# Parser and font limitations

This guide describes what the current `0.3.x` source does when it loads PDF
structure, recovers content-stream tokens, selects font metrics, and maps
character codes to Unicode. It is a troubleshooting and evidence guide, not a
claim that every parser or font feature is compatible.

The measurements below use runtime source commit
`a735f12a7fff05be0025ed95af2c668f154e8283`. DOC-014 changes only the Product
Requirements Document (PRD), documentation, and its documentation contract; it
does not change that runtime source. The reference is pinned CPython 3.13.12
with `pdfplumber==0.11.10` and `pdfminer.six==20260107`; the candidate is an exact
Maturin 1.14.1 wheel for `pdfplumber-rs==0.3.0` on the same interpreter.

## How to read this guide

The fixture table is an inventory, not a support matrix. Its labels have narrow
meanings:

- **Open only** means both implementations reached page extraction. It says
  nothing about compatible output.
- **Exact text** means the installed-artifact probe compared ordered character
  text directly, without a tolerance or `(cid:N)` wildcard. Font names and
  orientation are named separately when they were also exact.
- **Bounded metric** means a repository accuracy test passed a tolerance or
  threshold. It is useful regression evidence but not exact compatibility.
- **Known residual** names a retained difference. It must not be hidden by a
  threshold or promoted to a support claim.

Do not infer support from a successful open, a non-panicking extraction, or a
passing percentage threshold. The PRD rule is stricter: a parser or font task is
complete only after every required output, failure, and full test tier passes.

## Structural parser boundary

`lopdf` is the structural backend. It owns PDF object loading, page discovery,
indirect-object resolution, encryption primitives, and low-level stream
decompression. The facade then interprets page resources and content into
`pdfplumber-rs` models. `lopdf` being able to parse a construct does not prove
that the facade interprets it like pinned Python.

A successful `Pdf::open_*` call proves only that structural loading completed.
It does not prove that every page can be interpreted, that a font can be mapped,
or that the extracted objects match pinned Python.

Current loading has three deliberately narrow repairs:

1. The backend may strip a leading preamble before the first structural load.
2. On load failure it can correct a bad `startxref` or restore a narrowly
   truncated terminal `startxref`/`%%EOF` suffix.
3. Passwordless opening separately attempts the empty user password; the
   complete credential boundary is in the
   [encryption and repair guide](encryption-and-repair.md).

If the repaired byte sequence still fails `lopdf::Document::load_mem`, other
structural failures remain fatal. This path does not rebuild arbitrary object
graphs, scan every object without a cross-reference table, or apply the separate
native repair API. In particular, filter and file-structure tasks remain
unverified even when a fixture opens.

Source anchors:

- [`lopdf_backend.rs`](../crates/pdfplumber-parse/src/lopdf_backend.rs) contains
  preamble handling, the `startxref` retry, page discovery, and stream access.
- [`pdf.rs`](../crates/pdfplumber/src/pdf.rs) owns input budgets, page
  orchestration, and error context.
- [`lopdf` reference record](../references/lopdf.md) identifies the resolved
  backend version and its structural—not text-semantic—scope.

## Content-stream recovery and warnings

Once structure and page content are available, the parser uses a lenient token
path. `tokenize_lenient` preserves operators parsed before and after a malformed
token. On a token error it resumes one byte after the failed token start and
clears the partial operand stack, preventing stale operands from attaching to a
later operator. It reports the skipped byte offset as a warning.

That recovery can preserve useful later text, but it can also discard the
operands that surrounded the malformed region. A recovered page is not
automatically compatible with pinned Python output. Unknown operators, missing
resources, damaged inline images, and decompression failures can fail or recover
at different phases from `pdfminer.six`.

At the facade, warning collection is opt-in through
`ExtractOptions::collect_warnings`. During interpretation, missing fonts and
missing metrics can fall back and continue extraction. When collection is enabled, warnings can carry a
page index, operator index, font resource name, and stable warning code.
`Page::warnings()` is currently a Rust-only diagnostic surface; it is not a
Python-compatible logging or warning contract.

Source anchors:

- [`tokenizer.rs`](../crates/pdfplumber-parse/src/tokenizer.rs) defines strict
  and lenient tokenization.
- [`interpreter.rs`](../crates/pdfplumber-parse/src/interpreter.rs) turns
  operators, resources, fonts, and graphics state into events.
- [`pdf.rs`](../crates/pdfplumber/src/pdf.rs) decorates collected warnings with
  page context.
- [`page.rs`](../crates/pdfplumber/src/page.rs) exposes `Page::warnings()`.

## Font and Unicode resolution

### Font identity and metrics are different decisions

The emitted display name is separate from the normalized name used for metrics
and encoding lookup. The current display-name rules mirror the focused pinned
Python boundary:

- Standard 14 aliases use their canonical AFM names.
- Other simple fonts use the font descriptor's `/FontName`.
- Type 0 display names come from the descendant CIDFont descriptor.
- subset prefixes are retained.
- invalid UTF-8 PDF names retain Python bytes-`repr` spelling.
- Five known CP936 literal names receive the same pinned corrections as
  `pdfplumber`.

An exact `fontname` therefore says where a name came from; it does not prove that
the selected widths, vertical origins, or embedded program are exact.

### Unicode resolution order

For each character, the observable resolution order is:

> ToUnicode CMap → simple-font encoding → legacy CJK encoding → predefined Adobe collection map → Identity or `(cid:N)` fallback

The important boundaries are:

- ToUnicode stream decoding or CMap parsing failure is currently collapsed to
  an absent map because both operations use an optional `.ok()` path. No warning
  distinguishes missing, undecodable, and malformed ToUnicode data.
- an explicit but incomplete ToUnicode CMap is authoritative. After the simple
  and legacy-encoding paths are exhausted, an unmapped code from that map
  becomes `(cid:N)` rather than being invented from a collection table.
- the predefined Adobe-CNS1, Adobe-GB1, Adobe-Japan1, and Adobe-Korea1 tables are
  consulted only when ToUnicode is absent.
- Identity-H or Identity-V does not by itself prove that every CID is a Unicode
  scalar. Identity fallback also considers collection identity, and control
  scalars become `(cid:N)`.
- Simple-font encoding covers WinAnsi, MacRoman, MacExpert, StandardEncoding,
  `/Differences`, selected embedded Type 1 program encodings, Symbol, and
  ZapfDingbats fallbacks. That list is current code, not proof that every
  `FONT-008`, `FONT-009`, or `FONT-022` case is complete.

### Metrics and geometry fallbacks

The metrics path reads PDF widths and descriptors, Standard 14 metrics, embedded
TrueType `hmtx`, CFF widths, CID `/W` and `/DW`, vertical `/W2` and `/DW2`, and
`CIDToGIDMap` where the current implementation reaches them. When unavailable,
missing CID metrics, missing simple-font metrics, and missing font resources use
defaults after an optional warning.

As a consequence, fallback widths can change `adv`, bounding boxes, and word
grouping. Vertical origin or width choices also flow into `size` and the matrix
translation. That is why font-name parity does not prove font-metric or parser
parity.

Source anchors:

- [`cmap.rs`](../crates/pdfplumber-parse/src/cmap.rs) parses embedded ToUnicode
  CMaps.
- [`cid_font.rs`](../crates/pdfplumber-parse/src/cid_font.rs) handles Type 0/CID
  metrics, predefined CMap names, writing mode, and `CIDToGIDMap`.
- [`cjk_encoding.rs`](../crates/pdfplumber-parse/src/cjk_encoding.rs) maps
  supported legacy CJK CMap names to byte decoders.
- [`font_metrics.rs`](../crates/pdfplumber-parse/src/font_metrics.rs),
  [`standard_fonts.rs`](../crates/pdfplumber-parse/src/standard_fonts.rs),
  [`type1.rs`](../crates/pdfplumber-parse/src/type1.rs), and
  [`truetype.rs`](../crates/pdfplumber-parse/src/truetype.rs) provide the
  current metrics and simple-font fallbacks.
- [`pdfminer.six`](../references/pdfminer-six.md),
  [`ttf-parser`](../references/ttf-parser.md),
  [`Apache PDFBox`](../references/pdfbox.md),
  [`adobe-cmap-parser`](../references/adobe-cmap-parser.md),
  [`allsorts`](../references/allsorts.md), and
  [`PDF.js`](../references/pdfjs.md) record the upstream/reference patterns
  used to check this architecture.

## Fixture evidence

### Full indexed scan

The installed CPython 3.13 scan attempted all 223 indexed entries in
[`compat/fixture-provenance.toml`](../compat/fixture-provenance.toml). The scan
found that both environments completed 182 common-success fixtures across 587 pages. Across
those pages, 875,366 ordered characters matched exactly for page count,
character count, text, `fontname`, and `upright`.

Failures remain part of the result:

- the reference recorded 21 explicit failures and the candidate recorded 41;
- 20 indexed entries were candidate-only failures; no entry was a
  reference-only failure;
- all 21 shared failures differed in phase, exception type, or arguments; and
- the 20 candidate-only entries are two retained copies of ten OSS-Fuzz source
  PDFs, once in the pinned upstream collection and once in the Rust regression
  collection. They are distinct indexed entries with identical source content,
  not twenty unique malformed documents.

The candidate-only failures occur during structural open, primarily as invalid
cross-reference start values or an invalid trailer. Pinned Python extracts the
ten underlying documents. They remain parser compatibility failures under
`PDF-014`, `PARSE-001`–`PARSE-018`, and the exact error tasks.

### Focused font/parser scan

The focused result shows that all 28 licensed `external-parser` fixtures opened in both environments. The
exact installed-wheel probe covered 30 pages, 2,576 ordered characters, and 433
ordered word strings. Within that scope, all text, `fontname`, `upright`, and ordered word strings
matched exactly.

The remaining numeric observations were:

- 2,568 of 2,576 sizes matched exactly; the eight residuals had maximum
  absolute difference `2.842170943040401e-14`;
- 2,565 of 2,576 advances matched exactly; the eleven residuals had maximum
  absolute difference `1.2161865114990178e-07`;
- 2,547 of 2,576 matrix tuples matched exactly; and
- 15,427 of 15,456 matrix components matched exactly; all 29 residual
  components were the `e` translation. For matrices, the maximum matrix-component difference
  was `1.2161865470261546e-07`.

Separately, all 28 focused accuracy cases reported character and word F1
`1.000`. That result remains **Bounded metric** evidence:
`cross_validation` uses ordered ratios and fixture-specific floors, including a
wildcard for pinned `(cid:N)` text, while `accuracy_benchmark` uses
nearest-neighbor F1 with a two-point coordinate tolerance. For interpretation,
neither threshold is exact compatibility evidence. The direct installed-wheel
probe is the source of the exact text, name, orientation, and numeric counts
above.

Reproduction commands:

```console
cargo test -p pdfplumber --test cross_validation cv_pdfjs_ -- --nocapture
cargo test -p pdfplumber --test cross_validation cv_pdfbox_ -- --nocapture
cargo test -p pdfplumber --test cross_validation cv_poppler_ -- --nocapture
```

```console
cargo test -p pdfplumber --test accuracy_benchmark accuracy_pdfjs_ -- --nocapture
cargo test -p pdfplumber --test accuracy_benchmark accuracy_pdfbox_ -- --nocapture
cargo test -p pdfplumber --test accuracy_benchmark accuracy_poppler_ -- --nocapture
```

The checked implementations are
[`crates/pdfplumber/tests/cross_validation.rs`](../crates/pdfplumber/tests/cross_validation.rs)
and
[`crates/pdfplumber/tests/accuracy_benchmark.rs`](../crates/pdfplumber/tests/accuracy_benchmark.rs).

### Licensed external fixture inventory

Every digest below is checked against
[`compat/fixture-provenance.toml`](../compat/fixture-provenance.toml). “Exact” in
the observation column covers text, `fontname`, `size`, `adv`, `upright`, matrix,
and ordered word strings unless a residual is named.

| Fixture | Source | SHA-256 | Current observation |
|---|---|---|---|
| `crates/pdfplumber/tests/fixtures/pdfs/pdfbox/BidiSample.pdf` | PDFBox | `dd4947a7b825c2827729065cfff07115f0742d2c7bdcb048628097c62d1acbb3` | **Exact text**; 163/163 names, sizes, advances, and orientations; matrix 162/163 (**Known residual**, `e` only); 26/26 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfbox/FC60_Times.pdf` | PDFBox | `1616a1fc70fb70812f9865e4e6648736b218ab0d44775380adaf668e4300cc0d` | Exact: 8/8 characters and 1/1 word. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfbox/hello3.pdf` | PDFBox | `2b4aa9b63f3eed9a5e544b10bc677fcf70e1baaa62651e786a9d22d5dec860b2` | Exact: 18/18 characters and 3/3 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfbox/pdfbox-3127-vfont-reduced.pdf` | PDFBox | `e915930e95fef0727acc1ea2c674640d39ba2f820f4487e59f0ceb96a986762d` | Exact: 730/730 vertical-font characters and 115/115 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfbox/pdfbox-3833-japanese-reduced.pdf` | PDFBox | `69eeb7845a36925556ec8c67847864752e4421bfcb4d810b415f4f716d0ce0bd` | Exact: 3/3 characters and 1/1 word. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfbox/pdfbox-4322-empty-tounicode-reduced.pdf` | PDFBox | `c3b4a465b7664677f13aa3079ef4e5223eac9a1cf82719a863d07034388d2798` | Exact: 6/6 empty-ToUnicode characters and 1/1 word. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfbox/pdfbox-4531-bidi-ligature-1.pdf` | PDFBox | `05449b36c6b147652ef3c0b5404ae84b9d8c4352a43f96ec27cef8714f99b13f` | Exact: 6/6 characters and 2/2 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfbox/pdfbox-4531-bidi-ligature-2.pdf` | PDFBox | `cf4d80ac8466d07678de7651d63fda747a34b600c57fb4a8eb8faf9d612465ae` | Exact: 6/6 characters and 2/2 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfbox/pdfbox-5350-korean-reduced.pdf` | PDFBox | `ad13a85449c62b7665d552e3702be867de213efb0defd8742a1d5a3d748fbfc9` | Exact: 6/6 characters and 1/1 word. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfbox/pdfbox-5747-surrogate-diacritic-reduced.pdf` | PDFBox | `4e822ba5255923364baf5fdc14141a6cb6b860a5f4f86261043e7e08c7fce382` | Exact: 2/2 supplementary/diacritic characters and 2/2 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/ArabicCIDTrueType.pdf` | PDF.js | `1cad1de912ba29f89a6d8b08bc5b0f84382874ffedab2c8f9e05ef608c265bb1` | **Exact text**; 76/76 names, sizes, advances, and orientations; matrix 75/76 (**Known residual**, `e` only); 12/12 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/cid_cff.pdf` | PDF.js | `d894d356411217414fd9040d3d038612e1f4a1598936b2385a76458dd6d64d38` | Exact: 27/27 CID CFF characters and 3/3 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/issue14117.pdf` | PDF.js | `52537869a32f0bc04a6f676b9ded451cca0c744024fb36491d051494d96d4548` | **Exact text**; 1,213/1,213 names, sizes, advances, and orientations; matrix 1,212/1,213 (**Known residual**, `e` only); 213/213 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/issue3521.pdf` | PDF.js | `5ab6217d6634589fb9a2c4c8780c6aed02b498bb0a60ad9419f9e13a2e1bfe2d` | Exact: 7/7 characters and 1/1 word. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/issue4875.pdf` | PDF.js | `af7509fa6526257e37ea0603844dae7ee0e2ca7aa1eee027ffa71b64c4fd2add` | **Exact text**, name, size, and orientation for 3/3; advance 0/3 and matrix 1/3 (**Known residual**, `e` only); 1/1 word. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/issue7696.pdf` | PDF.js | `2666c86e2c0fa1f3cc3eb7efab30eed8165edfea915989295ce232e56dbe14a7` | Exact: 8/8 incomplete-ToUnicode characters and 1/1 word. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/issue8570.pdf` | PDF.js | `86b6c3c00fa7c49164c5331680bf0e19be3728845256332ddb7fdd52d45895ab` | **Exact text**, name, size, and orientation for 49/49; advance 45/49 and matrix 27/49 (**Known residual**, `e` only); 3/3 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/issue9262_reduced.pdf` | PDF.js | `d05dc91e7ab4dc33bfc7a383afbfa1512b69231c75f1f12247dd26d71d6b697f` | Exact: 8/8 characters and 1/1 word. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/noembed-eucjp.pdf` | PDF.js | `cadd5b9a27e2de9a11a0cf29b20724cb0f63643387be5e690de35ca35922e45c` | Exact: 5/5 EUC-JP fallback characters and 1/1 word. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/noembed-identity-2.pdf` | PDF.js | `457ee3ddbb36b2c42ac3f7bad8d595694d606a31cf0e925cff53c22969eb731f` | Exact: 12/12 Identity characters and 1/1 word. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/noembed-identity.pdf` | PDF.js | `b9bdcc78758415331a3f94c0d05a45fd605c462f907da38ec1e79be1fe37517b` | **Exact text**, name, size, and orientation for 12/12; advance 10/12 and matrix 11/12 (**Known residual**, `e` only); 2/2 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/noembed-jis7.pdf` | PDF.js | `812493361351a703b3a1c34830115df762024229aac8e41d9a16b8a075761691` | **Exact text**, name, size, and orientation for 12/12; advance 10/12 and matrix 11/12 (**Known residual**, `e` only); 2/2 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/noembed-sjis.pdf` | PDF.js | `13fd4811beaf0390d313847e23839db237d1fb65efc839f6d9f534ea52311c95` | Exact: 5/5 Shift-JIS fallback characters and 1/1 word. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/text_clip_cff_cid.pdf` | PDF.js | `81aaf48f55c659d29d7a533cd913bd9d028ffad183b46580a2122eebe5187506` | Exact: 6/6 CID CFF characters and 1/1 word. |
| `crates/pdfplumber/tests/fixtures/pdfs/pdfjs/vertical.pdf` | PDF.js | `514511143db12309893fb69cdf98c76f2361c20da394f79fdff2c119ed7a4393` | **Exact text**, name, advance, orientation, and matrix for 8/8; size 0/8 with only `2.842170943040401e-14` maximum (**Known residual**); 8/8 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/poppler/deseret.pdf` | Poppler | `15557c958a539f771ea22dd10518edf5c92a417f970c94a6dd5da76fc2ebc1eb` | Exact: 11/11 supplementary-plane characters and 2/2 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/poppler/pdf20-utf8-test.pdf` | Poppler | `066cac85038db27cb85b923eb2fbdffb4c76e94bf2f4d21cc952af94b40e71fe` | Exact: 107/107 PDF 2.0 UTF-8 characters and 19/19 words. |
| `crates/pdfplumber/tests/fixtures/pdfs/poppler/russian.pdf` | Poppler | `cf7f3725933ec39e1ae18b70ac49b022a26bad915b6886959fda45767b6a2a97` | Exact: 59/59 Cyrillic characters and 7/7 words. |

### Focused project fixtures

These generated fixtures are useful smoke cases but do not replace externally
sourced PDFs:

| Fixture | SHA-256 | Current observation |
|---|---|---|
| `tests/fixtures/generated/cjk_mixed.pdf` | `2aba4e4ed72eee108a52f8d0f99a76dbe955a3d9ea599128fbe88d994270be69` | Exact text/name/size/advance/orientation for 183/183 and 20/20 words; matrix 169/183 (`e` only). |
| `tests/fixtures/generated/multi_font.pdf` | `bac0e75ec84c051606d11b07f8e4f76ccf70eac8e8c3561e08bb08a6791d4dbc` | Exact text/name/size/advance/orientation for 262/262 and 42/42 words; matrix 183/262 (`e` only). |
| `tests/fixtures/real-world/fonts-encoding/special-characters.pdf` | `73a831cf51899a32a6ae7d129343c8cd034d42ba9082456c3703f41ddb6a340f` | Exact across all measured fields for 92/92 characters and 12/12 words. |
| `tests/fixtures/real-world/fonts-encoding/standard-14-fonts.pdf` | `d636f684c6d3890d9179081bb4aa6246049b4287a36595403ef7d29fc5001e4f` | Exact across all measured fields for 52/52 characters and 9/9 words. |

## Surface matrix

The core extraction path is shared, but diagnostics and data shapes differ at
the adapters:

| Surface | Current parser/font boundary |
|---|---|
| Rust | `Page::warnings()` when collection is enabled; typed `PdfError` context; native character fields and options. |
| Python | no public parser-warning collection surface; opening and page failures map through the compatibility exception boundary; character dictionaries expose the measured fields. |
| Command-Line Interface | no structured parser-warning output; failures go to standard error; JSON/CSV/text output cannot prove Python object parity. |
| WebAssembly | no parser-warning output; errors become `JsError`; only the wrapper's current extraction and serialized fields are exposed. |

The Command-Line Interface and WebAssembly surfaces do not expose a separate
font-selection policy. They consume the same Rust facade, then narrow or
serialize its results. A Rust fixture result therefore cannot be promoted to an
adapter contract without exercising that installed adapter.

## Troubleshooting workflow

When a document opens but text is wrong:

1. Record the exact input SHA-256, page number, surface, package version, and
   interpreter or Rust toolchain. Do not diagnose by filename alone.
2. Always reproduce against pinned CPython 3.13, `pdfplumber==0.11.10`, and
   `pdfminer.six==20260107` in a separate environment. Never co-install the two
   Python distributions.
3. Compare ordered character count, `text`, `fontname`, `size`, `adv`,
   `upright`, `matrix`, and word strings before applying a tolerance.
4. Inspect `/Subtype`, `/BaseFont`, `/FontDescriptor`, `/Encoding`,
   `/ToUnicode`, `/DescendantFonts`, `/CIDSystemInfo`, `/W`, `/DW`, `/W2`,
   `/DW2`, and `/CIDToGIDMap`. Preserve indirect references and raw name bytes.
5. In Rust, rerun with `collect_warnings=true` and retain warning codes plus page,
   operator, and font context. Treat default-metrics warnings as possible
   geometry corruption, not harmless noise.
6. If `(cid:N)` appears, determine whether ToUnicode was absent, malformed,
   incomplete, or explicitly Identity before changing the fallback order.
7. Reduce a licensed fixture without deleting the object, CMap, width, writing
   mode, or malformed bytes that cause the difference. Add the original source,
   license, source path, digest, and reduced-fixture relationship to the
   provenance registry.

For a structural failure, record whether it occurs during open or page
interpretation. A broad "cannot parse" report loses the phase distinction shown
by the current 21 shared failures and makes upstream comparison ambiguous.

## Validation and provenance

The guide is locked by
[`test_parser_font_limitations_docs.py`](../compat/tests/test_parser_font_limitations_docs.py).
That contract verifies every external path and digest directly from the
provenance registry, the four focused project fixtures, source anchors,
navigation, measured boundaries, surface limitations, and the unchanged task
state.

Primary behavior references are pinned through the local reference records:

- `pdfplumber` v0.11.10 commit
  [`7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62`](https://github.com/jsvine/pdfplumber/tree/7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62)
- `pdfminer.six` 20260107 commit
  [`9e1243c4ad000bf9bbe60e81fc8dde2fccc0ed3b`](https://github.com/pdfminer/pdfminer.six/tree/9e1243c4ad000bf9bbe60e81fc8dde2fccc0ed3b)
- `lopdf` 0.44.0 source commit
  [`8c454dd93d9c37e608c552a2b304d1d31d1cb2e1`](https://github.com/J-F-Liu/lopdf/tree/8c454dd93d9c37e608c552a2b304d1d31d1cb2e1)

The exact documentation contract is:

```console
python3 -m unittest compat.tests.test_parser_font_limitations_docs -v
```

## Claim boundary

DOC-014 changes no runtime behavior. It documents current evidence and known
limitations; it does not approve a compatibility deviation and does not change
the generated support matrix or readiness scorecard.

All `PARSE-*`, `FONT-*`, malformed-input, object, text, serialization, and
strict section 10 tasks remain independently open. A future runtime fix must
start from its own failing upstream differential, preserve every fixture and
failure, pass its required tiers, and receive its own evidence row before any
of those tasks can be checked.
