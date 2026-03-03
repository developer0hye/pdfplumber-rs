# PdfPig (C#)

- **URL:** https://github.com/UglyToad/PdfPig
- **Stars:** 2.4k | **License:** Apache-2.0
- **Status:** Actively maintained
- **Wiki:** https://github.com/UglyToad/PdfPig/wiki/Document-Layout-Analysis

## Text Layout Algorithms (most diverse of any project)

### Word Extraction: Nearest Neighbour
- Connects each glyph's `EndBaseLine` to closest glyph's `StartBaseLine`
- Manhattan distance (axis-aligned) or Euclidean (rotated text)
- DFS groups connected glyphs into words
- Handles LTR, RTL, and rotated text

### Page Segmentation: Recursive XY Cut
- Top-down: scan horizontally for vertical gaps, then vertically for horizontal gaps
- Uses dominant font dimensions for gap thresholds
- Good for single/multi-column layouts

### Page Segmentation: Docstrum (Document Spectrum)
- Bottom-up: nearest-neighbor analysis of word centroids
- Estimates line spacing, constructs text lines, assembles blocks
- Effective for complex layouts (L-shaped text, rotated paragraphs)

### Reading Order: Unsupervised Detector
- Allen's interval algebra with tolerance parameter
- Establishes reading sequences from spatial relationships + rendering order

## Companion Project

[BobLd/DocumentLayoutAnalysis](https://github.com/BobLd/DocumentLayoutAnalysis) (631 stars) — additional layout analysis resources.

## Relevance to pdfplumber-rs

Best algorithmic diversity for layout analysis. XY Cut and Docstrum are well-documented alternatives to pdfminer.six's LAParams approach. Nearest Neighbour word extraction handles rotated/RTL text better.
