# Poppler (C++)

- **URL:** https://gitlab.freedesktop.org/poppler/poppler
- **Stars:** N/A (GitLab) | **License:** GPL-2.0+
- **Status:** Actively maintained (~20 years maturity)

## Text Layout: `TextOutputDev.cc`

### Characters → Words
- Groups by font size similarity (`maxWordFontSizeDelta`)
- Baseline alignment (`maxIntraLineDelta`)
- Spacing thresholds (`minWordSpacing`, `maxWordSpacing`)

### Words → Lines/Blocks (`coalesce()`)
- Hardcoded constants for block font size delta, column spacing
- `fixed_pitch` parameter: max word distance and min column spacing
- Two modes: `physLayout` (preserve physical layout) and `rawOrder` (content stream order)

### Column Detection
- Heuristics follow columns and tables for reading order
- Uses column spacing thresholds to identify column boundaries

## Characteristics

- Very mature (~20 years), widely used (Evince, Okular)
- Constants largely hardcoded — less configurable than pdfminer.six
- Strong column detection for multi-column documents

## Relevance to pdfplumber-rs

The `coalesce()` algorithm's approach to column separation is worth studying for multi-column PDF handling. The hardcoded constants represent decades of tuning on real-world documents.
