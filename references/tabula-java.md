# tabula-java (Java)

- **URL:** https://github.com/tabulapdf/tabula-java
- **Stars:** ~2k | **License:** MIT
- **Status:** Maintained (powers Tabula GUI app)

## Detection Algorithms

### SpreadsheetDetectionAlgorithm (Lattice)
- Finds tables using intersecting ruling lines
- Merges ruling lines, finds intersection points, constructs cells

### NurminenDetectionAlgorithm
- Based on Anssi Nurminen's master's thesis on table detection
- Text-block analysis + ruling-line heuristics to guess table regions
- Delegates to SpreadsheetExtractionAlgorithm for cell extraction

## Extraction Algorithms

### SpreadsheetExtractionAlgorithm (Ruled tables)
- `findCells()` takes H + V `Ruling` objects
- Finds intersections via `Ruling.findIntersections()`
- Builds cell grid from crossing points

### BasicExtractionAlgorithm (Stream mode)
- No ruling lines — groups text by proximity

## Key Design: `Ruling` Class
- First-class objects with merge and intersection methods
- Clean OOP design for line manipulation

## Key Reference Files
- `src/main/java/technology/tabula/extractors/SpreadsheetExtractionAlgorithm.java`
- `src/main/java/technology/tabula/detectors/NurminenDetectionAlgorithm.java`
- `src/main/java/technology/tabula/Ruling.java`

## Relevance to pdfplumber-rs

The `Ruling` class design and `findCells()` method offer a well-structured Java implementation of lattice table detection, potentially cleaner to reference than Python code.
