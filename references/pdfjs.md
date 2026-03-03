# pdf.js (JavaScript)

- **URL:** https://github.com/mozilla/pdf.js
- **Stars:** ~52.9k | **License:** Apache-2.0
- **Status:** Very actively maintained (Mozilla)

## Font Parsing

### CFF (`src/core/cff_parser.js`)
- CharString state machine extracts width per glyph
- Width = `nominalWidthX + charstring_width_operand`, or `defaultWidthX` if no operand
- Known bug: negative width differences from nominalWidthX historically mishandled ([#12541](https://github.com/mozilla/pdf.js/issues/12541))

### CMap (`src/core/cmap.js`)
- Predefined binary CMap (`.bcmap`) format for CJK
- Has had issues with malformed CMap data in ToUnicode streams ([#4875](https://github.com/mozilla/pdf.js/issues/4875))

### Fonts (`src/core/fonts.js`)
- Font loading, type detection, metrics aggregation
- Width comparison: calculates both PDF-declared and font-program widths, flags mismatches

## Relevance to pdfplumber-rs

- CFF width extraction logic is clearly documented in `cff_parser.js`
- Width mismatch detection pattern (PDF vs font program) useful for debugging
- Binary CMap format could be a future optimization for CJK loading speed
