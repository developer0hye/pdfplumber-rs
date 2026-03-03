# adobe-cmap-parser (Rust)

- **URL:** https://github.com/jrmuizel/adobe-cmap-parser
- **Stars:** 5 | **Downloads:** 690k+ on crates.io | **License:** MIT
- **Status:** Maintained (last update Sep 2024)

## What It Does

Parses Adobe CMap files — character code to CID mappings.

## Capabilities

- Codespace range parsing
- CID range mappings (`begincidrange`/`endcidrange`)
- Individual CID mappings (`begincidchar`/`endcidchar`)
- Handles Identity-H/V and CJK character collections

## Relevance to pdfplumber-rs

Small, focused crate for CMap parsing. Could be:
1. Used directly as a dependency for CMap handling
2. Referenced for correctness validation of pdfplumber-rs's own CMap parser (`cmap.rs`)
3. Compared against for handling edge cases in CJK font support
