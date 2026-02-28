/**
 * TypeScript type definitions for pdfplumber-wasm.
 *
 * These types provide rich typing for the objects returned by the WASM
 * bindings. The auto-generated wasm-bindgen types use `any` for complex
 * return values (JsValue); use these interfaces instead for type safety.
 *
 * @example
 * ```typescript
 * import { WasmPdf, WasmPage } from 'pdfplumber-wasm';
 * import type { PdfChar, PdfWord, PdfSearchMatch, PdfMetadata } from 'pdfplumber-wasm';
 *
 * const pdf = WasmPdf.open(pdfBytes);
 * const page: WasmPage = pdf.page(0);
 * const chars: PdfChar[] = page.chars() as PdfChar[];
 * const words: PdfWord[] = page.extractWords() as PdfWord[];
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
 * ```
 */
export class WasmPdf {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;

  /** Open a PDF from raw bytes (Uint8Array). */
  static open(data: Uint8Array): WasmPdf;

  /** Get a page by 0-based index. */
  page(index: number): WasmPage;

  /** Document metadata. */
  readonly metadata: PdfMetadata;

  /** Number of pages in the document. */
  readonly pageCount: number;
}

/**
 * A single PDF page (WASM binding).
 *
 * @example
 * ```typescript
 * const page = pdf.page(0);
 * console.log(page.extractText());
 * const words = page.extractWords() as PdfWord[];
 * ```
 */
export class WasmPage {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;

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

  /** Find tables on the page. Returns table objects with cells and rows. */
  findTables(): PdfTable[];

  /**
   * Extract tables as 2D text arrays.
   * Returns one array per table, each containing rows of cell values.
   */
  extractTables(): PdfTableData[];

  /**
   * Search for a text pattern on the page.
   * @param pattern - Text or regex pattern to search for.
   * @param regex - Whether pattern is a regex (default: true).
   * @param _case - Case-sensitive search (default: true).
   */
  search(pattern: string, regex?: boolean | null, _case?: boolean | null): PdfSearchMatch[];

  /** Page height in points. */
  readonly height: number;

  /** Page index (0-based). */
  readonly pageNumber: number;

  /** Page width in points. */
  readonly width: number;
}
