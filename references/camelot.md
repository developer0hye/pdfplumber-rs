# Camelot (Python)

- **URL:** https://github.com/camelot-dev/camelot
- **Stars:** ~3.6k | **License:** MIT | **Latest:** v1.0.9
- **Status:** Actively maintained

## Table Detection Algorithms

### Lattice Parser (image-based)
1. Convert PDF page to raster image (via pdfium)
2. OpenCV morphological transforms (erosion + dilation) to isolate H/V line segments
3. Detect intersections by pixel-AND of H and V line masks
4. Compute table boundaries by pixel-OR of all line masks
5. Scale detected coordinates from image space → PDF space
6. Detect spanning cells using line segments and intersection points
7. Remove detected edges, re-run to find additional tables

### Stream Parser (text-based)
1. Group words into text rows by y-axis overlap (via PDFMiner)
2. Compute "text edges" — vertical/horizontal lines implied by text alignment
3. Use text edges to guess table areas
4. Guess column count within each area
5. Remove identified edges, re-run until no new tables found

## Key Design Decision

Lattice converts to raster (loses precision, gains robustness against imperfect PDF lines). Stream stays in text-coordinate space.

## Relevance to pdfplumber-rs

- Lattice parser's morphological approach is an alternative to pdfplumber's line-intersection method
- Stream parser's text-edge algorithm is a useful reference for the `"text"` edge strategy
- Clean separation between lattice and stream detection approaches
