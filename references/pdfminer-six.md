# pdfminer.six (Python)

- **URL:** https://github.com/pdfminer/pdfminer.six
- **Stars:** ~6.9k | **License:** MIT | **Latest:** 20260107
- **Status:** Actively maintained

## Text Layout Pipeline (`layout.py`)

3-stage pipeline controlled by `LAParams`:

### 1. Characters → Lines
- `char_margin` (default 2.0): max horizontal distance (relative to char width) to group chars
- `line_overlap` (default 0.5): min vertical overlap (relative to char height) to be on same line
- Output: `LTTextLineHorizontal` / `LTTextLineVertical`

### 2. Lines → Text Boxes
- `line_margin` (default 0.5): max vertical distance between lines
- Lines grouped via `Plane` spatial index; must match in height and alignment
- Output: `LTTextBoxHorizontal` / `LTTextBoxVertical`

### 3. Text Boxes → Reading Order
- Hierarchical agglomerative clustering using "unused space" distance metric
- `boxes_flow` (default 0.5, range -1.0 to +1.0): sort key formula:
  `(1 - boxes_flow) * x0 - (1 + boxes_flow) * (y0 + y1)`

## Font Metrics (`pdffont.py`, `cmapdb.py`)

- Font types: Type1, TrueType, Type3, CID fonts
- Width from `/Widths`, `/FirstChar`, `/LastChar` arrays
- CID `/W` entries: `[start [w1 w2 ...]]` or `[start end w]`; `/DW2` for vertical
- CMapDB caches CMaps by name; PDFResourceManager caches fonts by object ID
- Descriptors: `/Ascent`, `/Descent`, `/FontBBox`

## Relevance to pdfplumber-rs

pdfplumber-rs must match pdfminer.six behavior for Python compatibility. Key files:
- `layout.py` — LAParams pipeline (target behavior for text grouping)
- `pdffont.py` — Font width extraction (target behavior for char bbox)
- `cmapdb.py` — CMap loading and CID mapping
