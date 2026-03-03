# Reference Projects Index

Projects organized by domain. Read the specific file matching your current problem.

## Direct Upstream

| Project | Language | File | Use When |
|---------|----------|------|----------|
| [pdfplumber](https://github.com/jsvine/pdfplumber) | Python | [pdfplumber.md](pdfplumber.md) | API design, table detection pipeline, overall architecture |
| [pdfminer.six](https://github.com/pdfminer/pdfminer.six) | Python | [pdfminer-six.md](pdfminer-six.md) | Text layout (LAParams), font metrics, CMap handling, char extraction |

## Rust PDF Ecosystem

| Project | Stars | File | Use When |
|---------|-------|------|----------|
| [lopdf](https://github.com/J-F-Liu/lopdf) | 2.1k | [lopdf.md](lopdf.md) | PDF object access, content stream decoding (current backend) |
| [pdf-rs](https://github.com/pdf-rs/pdf) | 1.6k | [pdf-rs.md](pdf-rs.md) | Typed PDF object model, derive-macro patterns |
| [pdf-extract](https://github.com/jrmuizel/pdf-extract) | 571 | [pdf-extract.md](pdf-extract.md) | CMap/CFF/Type1 parsing crate ecosystem on lopdf |
| [pdf_oxide](https://github.com/yfedoseev/pdf_oxide) | 197 | [pdf-oxide.md](pdf-oxide.md) | Performance benchmarking, competitive analysis |

## Font Parsing

| Project | Language | File | Use When |
|---------|----------|------|----------|
| [ttf-parser](https://github.com/harfbuzz/ttf-parser) | Rust | [ttf-parser.md](ttf-parser.md) | TrueType hmtx/head/hhea/maxp table parsing |
| [allsorts](https://github.com/yeslogic/allsorts) | Rust | [allsorts.md](allsorts.md) | CFF parsing (Top DICT, Private DICT, CharStrings) |
| [Apache PDFBox](https://github.com/apache/pdfbox) | Java | [pdfbox.md](pdfbox.md) | Font class hierarchy, width resolution, CID fonts |
| [pdf.js](https://github.com/mozilla/pdf.js) | JS | [pdfjs.md](pdfjs.md) | CFF width extraction, CMap binary format |
| [adobe-cmap-parser](https://github.com/jrmuizel/adobe-cmap-parser) | Rust | [adobe-cmap-parser.md](adobe-cmap-parser.md) | CMap file parsing for CJK fonts |

## Table Detection

| Project | Language | File | Use When |
|---------|----------|------|----------|
| [Camelot](https://github.com/camelot-dev/camelot) | Python | [camelot.md](camelot.md) | Lattice (OpenCV morphology) and stream parser algorithms |
| [tabula-java](https://github.com/tabulapdf/tabula-java) | Java | [tabula-java.md](tabula-java.md) | Ruling class design, spreadsheet cell extraction |

## Text Layout & Reading Order

| Project | Language | File | Use When |
|---------|----------|------|----------|
| [PdfPig](https://github.com/UglyToad/PdfPig) | C# | [pdfpig.md](pdfpig.md) | Multiple layout algorithms (XY Cut, Docstrum, Nearest Neighbour) |
| [Poppler](https://gitlab.freedesktop.org/poppler/poppler) | C++ | [poppler.md](poppler.md) | Column detection, TextOutputDev coalesce algorithm |
| [MuPDF](https://github.com/ArtifexSoftware/mupdf) | C | [mupdf.md](mupdf.md) | Stream-order text hierarchy (Block→Line→Span→Char) |

## Utility Crates (Rust)

| Crate | Use When |
|-------|----------|
| [unicode-bidi](https://github.com/servo/unicode-bidi) | BiDi text (UAX #9) |
| [jieba-rs](https://github.com/messense/jieba-rs) | Chinese word segmentation |
| [unicode-segmentation](https://crates.io/crates/unicode-segmentation) | Unicode word boundaries (UAX #29) |
