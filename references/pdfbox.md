# Apache PDFBox (Java)

- **URL:** https://github.com/apache/pdfbox
- **Stars:** ~3k | **License:** Apache-2.0
- **Status:** Actively maintained (Apache Foundation)

## Font Architecture

### Class Hierarchy
- `PDFont` → `PDSimpleFont` (Type1, TrueType, Type3) and `PDType0Font` (composite)
- Separate **FontBox** sub-library for low-level font parsing

### Width Resolution Order
1. PDF `/Widths` array
2. Embedded font program (hmtx for TrueType, CharStrings for CFF)

### TrueType (`fontbox/.../ttf/`)
- `TTFParser`: parses mandatory tables `head`, `hhea`, `maxp`, `hmtx`
- `getAdvanceWidth(gid)`, `getAdvanceHeight(gid)`

### CFF (`fontbox/.../cff/`)
- `CFFFont` (abstract) → `CFFCIDFont`, `CFFType1Font`
- Top DICT, Private DICT, CharStrings
- `getWidth(String name)` returns advance width from CharString data

### CMap Support
- Predefined Adobe character collections (Japan1, GB1, CNS1, Korea1)
- CMap caching and Identity-H/V handling

## Key Reference Files

- `fontbox/src/main/java/org/apache/fontbox/ttf/TTFParser.java`
- `fontbox/src/main/java/org/apache/fontbox/cff/`
- `pdfbox/src/main/java/org/apache/pdfbox/pdmodel/font/`

## Relevance to pdfplumber-rs

Comprehensive Java implementation of font class hierarchy and width resolution. Useful reference for CID font handling and the Type0→CIDFont delegation pattern.
