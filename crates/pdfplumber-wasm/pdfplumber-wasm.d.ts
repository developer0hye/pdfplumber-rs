/**
 * TypeScript type definitions for pdfplumber-wasm.
 *
 * Full API parity with the Python pdfplumber library and the PyO3 bindings —
 * every method available on Page is available on WasmPage; every method on
 * CroppedPage is available on WasmCroppedPage.
 *
 * @example
 * ```typescript
 * import { WasmPdf, WasmPage, WasmCroppedPage } from 'pdfplumber-wasm';
 * import type { PdfChar, PdfWord, PdfLine, PdfRect, PdfSearchMatch, PdfMetadata } from 'pdfplumber-wasm';
 *
 * const pdf = WasmPdf.open(pdfBytes);
 * const page: WasmPage = pdf.page(0);
 * const chars: PdfChar[] = page.chars() as PdfChar[];
 * const words: PdfWord[] = page.extractWords() as PdfWord[];
 * const header: WasmCroppedPage = page.crop(0, 0, page.width, 80);
 * const headerText: string = header.extractText();
 * ```
 */

// ---- Geometry ----

/** Bounding box with top-left origin coordinates (matching Python pdfplumber). */
export interface BBox {
  /** Left edge (x-coordinate). */
  x0: number;
  /** Top edge (y-coordinate). */
  top: number;
  /** Right edge (x-coordinate). */
  x1: number;
  /** Bottom edge (y-coordinate). */
  bottom: number;
}

// ---- Characters ----

/** A single extracted character with position and font information. */
export interface PdfChar {
  /** The character text (usually a single character). */
  text: string;
  /** Left edge of the character bounding box. */
  x0: number;
  /** Top edge of the character bounding box. */
  top: number;
  /** Right edge of the character bounding box. */
  x1: number;
  /** Bottom edge of the character bounding box. */
  bottom: number;
  /** Font name (e.g., "Helvetica", "TimesNewRoman"). */
  fontname: string;
  /** Font size in points. */
  size: number;
  /** Absolute top position across all pages. */
  doctop: number;
  /** Whether the character is upright (not rotated). */
  upright: boolean;
  /** Text direction: "ltr", "rtl", "ttb", or "btt". */
  direction: string;
}

// ---- Words ----

/** A word extracted from text grouping. */
export interface PdfWord {
  /** The word text. */
  text: string;
  /** Left edge of the word bounding box. */
  x0: number;
  /** Top edge of the word bounding box. */
  top: number;
  /** Right edge of the word bounding box. */
  x1: number;
  /** Bottom edge of the word bounding box. */
  bottom: number;
  /** Absolute top position across all pages. */
  doctop: number;
  /** Text direction. */
  direction: string;
}

// ---- Tables ----

/** A table cell. */
export interface PdfCell {
  /** Cell bounding box. */
  x0: number;
  top: number;
  x1: number;
  bottom: number;
  /** Cell text content, or null if empty. */
  text: string | null;
}

/** A detected table with structure information. */
export interface PdfTable {
  /** Table bounding box. */
  bbox: BBox;
  /** All cells in the table. */
  cells: PdfCell[];
  /** Rows of cells. */
  rows: PdfCell[][];
}

/** Extracted table data as a 2D array of cell text values. */
export type PdfTableData = (string | null)[][];

// ---- Search ----

/** A search match result. */
export interface PdfSearchMatch {
  /** The matched text. */
  text: string;
  /** Left edge of the match bounding box. */
  x0: number;
  /** Top edge of the match bounding box. */
  top: number;
  /** Right edge of the match bounding box. */
  x1: number;
  /** Bottom edge of the match bounding box. */
  bottom: number;
  /** Page number (0-based). */
  page_number: number;
  /** Indices of matched characters. */
  char_indices: number[];
}

// ---- Geometry primitives ----

/** A line path segment on the page. */
export interface PdfLine {
  x0: number;
  top: number;
  x1: number;
  bottom: number;
  line_width: number;
  /** "horizontal" | "vertical" | "diagonal" */
  orientation: string;
}

/** A rectangle on the page. */
export interface PdfRect {
  x0: number;
  top: number;
  x1: number;
  bottom: number;
  line_width: number;
  stroke: boolean;
  fill: boolean;
}

/** A Bézier curve on the page. */
export interface PdfCurve {
  x0: number;
  top: number;
  x1: number;
  bottom: number;
  /** Control points as [x, y] pairs. */
  pts: [number, number][];
  line_width: number;
  stroke: boolean;
  fill: boolean;
}

/** An image on the page. */
export interface PdfImage {
  x0: number;
  top: number;
  x1: number;
  bottom: number;
  width: number;
  height: number;
  name: string;
  src_width: number | null;
  src_height: number | null;
  bits_per_component: number | null;
  color_space: string | null;
}

/** A document outline/bookmark entry. */
export interface PdfBookmark {
  title: string;
  level: number;
  page_number: number | null;
  dest_top: number | null;
}

/** A hyperlink annotation. */
export interface PdfHyperlink {
  x0: number;
  top: number;
  x1: number;
  bottom: number;
  uri: string | null;
}

// ---- Metadata ----

/** Document metadata from the PDF info dictionary. */
export interface PdfMetadata {
  title?: string | null;
  author?: string | null;
  subject?: string | null;
  keywords?: string | null;
  creator?: string | null;
  producer?: string | null;
  creation_date?: string | null;
  mod_date?: string | null;
}

// ---- WASM Classes ----

/**
 * A PDF document opened for extraction (WASM binding).
 *
 * @example
 * ```typescript
 * const response = await fetch('document.pdf');
 * const bytes = new Uint8Array(await response.arrayBuffer());
 * const pdf = WasmPdf.open(bytes);
 * console.log(`Pages: ${pdf.pageCount}`);
 * const toc = pdf.bookmarks() as PdfBookmark[];
 * ```
 */
export class WasmPdf {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;

  /** Open a PDF from raw bytes (Uint8Array). Throws on invalid PDF. */
  static open(data: Uint8Array): WasmPdf;

  /** Get a page by 0-based index. Throws if index is out of range. */
  page(index: number): WasmPage;

  /** Document metadata. */
  readonly metadata: PdfMetadata;

  /** Number of pages in the document. */
  readonly pageCount: number;

  /** Document bookmarks (outline / table of contents). */
  bookmarks(): PdfBookmark[];
}

/**
 * A single PDF page (WASM binding).
 *
 * Exposes full extraction API: text, words, characters, tables, geometry
 * (lines, rects, curves, images), annotations, hyperlinks, and spatial
 * cropping operations.
 *
 * @example
 * ```typescript
 * const page = pdf.page(0);
 * console.log(page.extractText());
 * const words = page.extractWords() as PdfWord[];
 * // Extract only the header region
 * const header = page.crop(0, 0, page.width, 80);
 * const headerText = header.extractText();
 * ```
 */
export class WasmPage {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;

  // ---- Text extraction ----

  /** Return all characters as an array of PdfChar objects. */
  chars(): PdfChar[];

  /**
   * Extract text from the page.
   * @param layout - When true, detects multi-column layouts. Defaults to false.
   */
  extractText(layout?: boolean | null): string;

  /**
   * Extract words from the page.
   * @param x_tolerance - Horizontal tolerance for word grouping (default: 3).
   * @param y_tolerance - Vertical tolerance for word grouping (default: 3).
   */
  extractWords(x_tolerance?: number | null, y_tolerance?: number | null): PdfWord[];

  // ---- Table extraction ----

  /** Find tables on the page. Returns table objects with cells and rows. */
  findTables(): PdfTable[];

  /**
   * Extract tables as 2D text arrays.
   * Returns one array per table, each containing rows of cell values.
   */
  extractTables(): PdfTableData[];

  // ---- Search ----

  /**
   * Search for a text pattern on the page.
   * @param pattern - Text or regex pattern to search for.
   * @param regex - Whether pattern is a regex (default: true).
   * @param _case - Case-sensitive search (default: true).
   */
  search(pattern: string, regex?: boolean | null, _case?: boolean | null): PdfSearchMatch[];

  // ---- Geometry ----

  /** Return lines on this page. */
  lines(): PdfLine[];

  /** Return rectangles on this page. */
  rects(): PdfRect[];

  /** Return Bézier curves on this page. */
  curves(): PdfCurve[];

  /** Return images on this page. */
  images(): PdfImage[];

  /** Return annotations (highlights, notes, etc.). */
  annots(): unknown[];

  /** Return hyperlinks on this page. */
  hyperlinks(): PdfHyperlink[];

  // ---- Spatial filtering ----

  /**
   * Crop this page to a bounding box.
   * Returns a WasmCroppedPage with only objects intersecting the bbox.
   * Coordinates are in page points (origin at top-left).
   */
  crop(x0: number, top: number, x1: number, bottom: number): WasmCroppedPage;

  /**
   * Return a view containing only objects **fully within** the bbox.
   */
  within_bbox(x0: number, top: number, x1: number, bottom: number): WasmCroppedPage;

  /**
   * Return a view containing only objects **outside** the bbox.
   */
  outside_bbox(x0: number, top: number, x1: number, bottom: number): WasmCroppedPage;

  // ---- Page properties ----

  /** Page height in points. */
  readonly height: number;

  /** Page index (0-based). */
  readonly pageNumber: number;

  /** Page width in points. */
  readonly width: number;

  /** Page rotation in degrees (0, 90, 180, or 270). */
  readonly rotation: number;

  /** Page bounding box. */
  readonly bbox: BBox;

  /** MediaBox as defined in the PDF dictionary. */
  readonly mediaBox: BBox;
}

/**
 * A spatially-filtered view of a PDF page (WASM binding).
 *
 * Produced by `WasmPage.crop()`, `.within_bbox()`, or `.outside_bbox()`.
 * Supports the same extraction methods as `WasmPage` plus further cropping.
 *
 * @example
 * ```typescript
 * // Extract text from just the header region
 * const header = page.crop(0, 0, page.width, 80);
 * const headerText = header.extractText();
 *
 * // Extract tables from the body region only
 * const body = page.crop(0, 80, page.width, page.height - 40);
 * const tables = body.extractTables();
 * ```
 */
export class WasmCroppedPage {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;

  // ---- Dimensions ----
  readonly width: number;
  readonly height: number;

  // ---- Text extraction ----
  chars(): PdfChar[];
  extractText(layout?: boolean | null): string;
  extractWords(x_tolerance?: number | null, y_tolerance?: number | null): PdfWord[];

  // ---- Table extraction ----
  findTables(): PdfTable[];
  extractTables(): PdfTableData[];

  // ---- Geometry ----
  lines(): PdfLine[];
  rects(): PdfRect[];
  curves(): PdfCurve[];
  images(): PdfImage[];

  // ---- Further spatial filtering ----
  crop(x0: number, top: number, x1: number, bottom: number): WasmCroppedPage;
  within_bbox(x0: number, top: number, x1: number, bottom: number): WasmCroppedPage;
  outside_bbox(x0: number, top: number, x1: number, bottom: number): WasmCroppedPage;
}
