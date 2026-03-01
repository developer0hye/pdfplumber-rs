//! Shared test utilities for cross-validation tests.
//!
//! Provides golden data types, matching logic, and validation functions
//! used by both `cross_validation.rs` and `real_world_cross_validation.rs`.

#![allow(dead_code)]

use serde::Deserialize;
use std::path::PathBuf;

// ─── Golden JSON schema types ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GoldenData {
    pub source: String,
    pub pdfplumber_version: String,
    pub pages: Vec<GoldenPage>,
}

#[derive(Debug, Deserialize)]
pub struct GoldenPage {
    pub page_number: usize,
    pub width: f64,
    pub height: f64,
    pub chars: Vec<GoldenChar>,
    pub words: Vec<GoldenWord>,
    pub text: String,
    pub lines: Vec<GoldenLine>,
    pub rects: Vec<GoldenRect>,
    pub tables: Vec<GoldenTable>,
}

#[derive(Debug, Deserialize)]
pub struct GoldenChar {
    pub text: String,
    pub x0: f64,
    pub top: f64,
    pub x1: f64,
    pub bottom: f64,
    pub fontname: String,
    pub size: f64,
    pub doctop: f64,
    pub upright: bool,
}

#[derive(Debug, Deserialize)]
pub struct GoldenWord {
    pub text: String,
    pub x0: f64,
    pub top: f64,
    pub x1: f64,
    pub bottom: f64,
    pub doctop: f64,
}

#[derive(Debug, Deserialize)]
pub struct GoldenLine {
    pub x0: f64,
    pub top: f64,
    pub x1: f64,
    pub bottom: f64,
    pub linewidth: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct GoldenRect {
    pub x0: f64,
    pub top: f64,
    pub x1: f64,
    pub bottom: f64,
    pub linewidth: Option<f64>,
    pub stroke: bool,
    pub fill: bool,
}

#[derive(Debug, Deserialize)]
pub struct GoldenTable {
    pub bbox: GoldenBBox,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct GoldenBBox {
    pub x0: f64,
    pub top: f64,
    pub x1: f64,
    pub bottom: f64,
}

// ─── Tolerance and threshold constants ──────────────────────────────────────

/// Coordinate tolerance in points (±1.0pt).
pub const COORD_TOLERANCE: f64 = 1.0;

/// Font size tolerance in points (±0.5pt).
pub const FONT_SIZE_TOLERANCE: f64 = 0.5;

/// Minimum char match rate (PRD: 95%).
pub const CHAR_THRESHOLD: f64 = 0.95;

/// Minimum word match rate (PRD: 95%).
pub const WORD_THRESHOLD: f64 = 0.95;

/// Minimum lattice table cell accuracy (PRD: 90%).
pub const TABLE_THRESHOLD: f64 = 0.90;

// ─── Helpers ────────────────────────────────────────────────────────────────

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

pub fn load_golden(pdf_name: &str) -> GoldenData {
    let json_name = pdf_name.replace(".pdf", ".json");
    let path = fixtures_dir().join("golden").join(&json_name);
    let data = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read golden file {}: {}", path.display(), e));
    serde_json::from_str(&data)
        .unwrap_or_else(|e| panic!("Failed to parse golden JSON {}: {}", path.display(), e))
}

pub fn open_pdf(pdf_name: &str) -> pdfplumber::Pdf {
    let path = fixtures_dir().join("pdfs").join(pdf_name);
    pdfplumber::Pdf::open_file(&path, None)
        .unwrap_or_else(|e| panic!("Failed to open PDF {}: {}", path.display(), e))
}

pub fn try_open_pdf(pdf_name: &str) -> Result<pdfplumber::Pdf, String> {
    let path = fixtures_dir().join("pdfs").join(pdf_name);
    pdfplumber::Pdf::open_file(&path, None).map_err(|e| format!("{}", e))
}

pub fn coords_match(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

// ─── Char matching ──────────────────────────────────────────────────────────

pub fn char_matches(rust_char: &pdfplumber::Char, golden: &GoldenChar) -> bool {
    rust_char.text == golden.text
        && coords_match(rust_char.bbox.x0, golden.x0, COORD_TOLERANCE)
        && coords_match(rust_char.bbox.top, golden.top, COORD_TOLERANCE)
        && coords_match(rust_char.bbox.x1, golden.x1, COORD_TOLERANCE)
        && coords_match(rust_char.bbox.bottom, golden.bottom, COORD_TOLERANCE)
}

/// Greedy best-match: for each golden char, find the best matching Rust char.
pub fn match_chars(rust_chars: &[pdfplumber::Char], golden_chars: &[GoldenChar]) -> (usize, usize) {
    let total = golden_chars.len();
    if total == 0 {
        return (0, 0);
    }
    let mut used = vec![false; rust_chars.len()];
    let mut matched = 0;
    for golden in golden_chars {
        for (i, rc) in rust_chars.iter().enumerate() {
            if !used[i] && char_matches(rc, golden) {
                used[i] = true;
                matched += 1;
                break;
            }
        }
    }
    (matched, total)
}

// ─── Word matching ──────────────────────────────────────────────────────────

pub fn word_matches(rust_word: &pdfplumber::Word, golden: &GoldenWord) -> bool {
    rust_word.text == golden.text
        && coords_match(rust_word.bbox.x0, golden.x0, COORD_TOLERANCE)
        && coords_match(rust_word.bbox.top, golden.top, COORD_TOLERANCE)
        && coords_match(rust_word.bbox.x1, golden.x1, COORD_TOLERANCE)
        && coords_match(rust_word.bbox.bottom, golden.bottom, COORD_TOLERANCE)
}

pub fn match_words(rust_words: &[pdfplumber::Word], golden_words: &[GoldenWord]) -> (usize, usize) {
    let total = golden_words.len();
    if total == 0 {
        return (0, 0);
    }
    let mut used = vec![false; rust_words.len()];
    let mut matched = 0;
    for golden in golden_words {
        for (i, rw) in rust_words.iter().enumerate() {
            if !used[i] && word_matches(rw, golden) {
                used[i] = true;
                matched += 1;
                break;
            }
        }
    }
    (matched, total)
}

// ─── Table matching ─────────────────────────────────────────────────────────

pub fn match_table_cells(
    rust_table: &pdfplumber::Table,
    golden_table: &GoldenTable,
) -> (usize, usize) {
    let mut total = 0;
    let mut matched = 0;
    for (row_idx, golden_row) in golden_table.rows.iter().enumerate() {
        for (col_idx, golden_cell) in golden_row.iter().enumerate() {
            total += 1;
            if let Some(rust_row) = rust_table.rows.get(row_idx) {
                if let Some(rust_cell) = rust_row.get(col_idx) {
                    let rust_text = rust_cell.text.as_deref().unwrap_or("").trim();
                    let golden_text = golden_cell.trim();
                    if rust_text == golden_text {
                        matched += 1;
                    }
                }
            }
        }
    }
    (matched, total)
}

pub fn find_best_table<'a>(
    rust_tables: &'a [pdfplumber::Table],
    golden_table: &GoldenTable,
) -> Option<&'a pdfplumber::Table> {
    if rust_tables.is_empty() {
        return None;
    }
    rust_tables.iter().min_by(|a, b| {
        let dist_a = bbox_distance(&a.bbox, &golden_table.bbox);
        let dist_b = bbox_distance(&b.bbox, &golden_table.bbox);
        dist_a
            .partial_cmp(&dist_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

pub fn bbox_distance(rust_bbox: &pdfplumber::BBox, golden_bbox: &GoldenBBox) -> f64 {
    let dx0 = rust_bbox.x0 - golden_bbox.x0;
    let dtop = rust_bbox.top - golden_bbox.top;
    let dx1 = rust_bbox.x1 - golden_bbox.x1;
    let dbottom = rust_bbox.bottom - golden_bbox.bottom;
    (dx0 * dx0 + dtop * dtop + dx1 * dx1 + dbottom * dbottom).sqrt()
}

// ─── Line matching ──────────────────────────────────────────────────────────

pub fn line_matches(rust_line: &pdfplumber::Line, golden: &GoldenLine) -> bool {
    coords_match(rust_line.x0, golden.x0, COORD_TOLERANCE)
        && coords_match(rust_line.top, golden.top, COORD_TOLERANCE)
        && coords_match(rust_line.x1, golden.x1, COORD_TOLERANCE)
        && coords_match(rust_line.bottom, golden.bottom, COORD_TOLERANCE)
}

pub fn match_lines(rust_lines: &[pdfplumber::Line], golden_lines: &[GoldenLine]) -> (usize, usize) {
    let total = golden_lines.len();
    if total == 0 {
        return (0, 0);
    }
    let mut used = vec![false; rust_lines.len()];
    let mut matched = 0;
    for golden in golden_lines {
        for (i, rl) in rust_lines.iter().enumerate() {
            if !used[i] && line_matches(rl, golden) {
                used[i] = true;
                matched += 1;
                break;
            }
        }
    }
    (matched, total)
}

// ─── Rect matching ──────────────────────────────────────────────────────────

pub fn rect_matches(rust_rect: &pdfplumber::Rect, golden: &GoldenRect) -> bool {
    coords_match(rust_rect.x0, golden.x0, COORD_TOLERANCE)
        && coords_match(rust_rect.top, golden.top, COORD_TOLERANCE)
        && coords_match(rust_rect.x1, golden.x1, COORD_TOLERANCE)
        && coords_match(rust_rect.bottom, golden.bottom, COORD_TOLERANCE)
}

pub fn match_rects(rust_rects: &[pdfplumber::Rect], golden_rects: &[GoldenRect]) -> (usize, usize) {
    let total = golden_rects.len();
    if total == 0 {
        return (0, 0);
    }
    let mut used = vec![false; rust_rects.len()];
    let mut matched = 0;
    for golden in golden_rects {
        for (i, rr) in rust_rects.iter().enumerate() {
            if !used[i] && rect_matches(rr, golden) {
                used[i] = true;
                matched += 1;
                break;
            }
        }
    }
    (matched, total)
}

// ─── Per-page and per-PDF validation ────────────────────────────────────────

#[derive(Debug)]
pub struct PageResult {
    pub page_number: usize,
    pub char_matched: usize,
    pub char_total: usize,
    pub word_matched: usize,
    pub word_total: usize,
    pub line_matched: usize,
    pub line_total: usize,
    pub rect_matched: usize,
    pub rect_total: usize,
    pub table_cell_matched: usize,
    pub table_cell_total: usize,
}

impl PageResult {
    pub fn char_rate(&self) -> f64 {
        rate(self.char_matched, self.char_total)
    }
    pub fn word_rate(&self) -> f64 {
        rate(self.word_matched, self.word_total)
    }
    pub fn line_rate(&self) -> f64 {
        rate(self.line_matched, self.line_total)
    }
    pub fn rect_rate(&self) -> f64 {
        rate(self.rect_matched, self.rect_total)
    }
    pub fn table_rate(&self) -> f64 {
        rate(self.table_cell_matched, self.table_cell_total)
    }
}

pub fn rate(matched: usize, total: usize) -> f64 {
    if total == 0 {
        1.0
    } else {
        matched as f64 / total as f64
    }
}

pub fn validate_page(pdf_name: &str, page: &pdfplumber::Page, golden: &GoldenPage) -> PageResult {
    let rust_chars = page.chars();
    let rust_words = page.extract_words(&pdfplumber::WordOptions::default());
    let rust_lines = page.lines();
    let rust_rects = page.rects();
    let rust_tables = page.find_tables(&pdfplumber::TableSettings::default());

    let (char_matched, char_total) = match_chars(rust_chars, &golden.chars);
    let (word_matched, word_total) = match_words(&rust_words, &golden.words);
    let (line_matched, line_total) = match_lines(rust_lines, &golden.lines);
    let (rect_matched, rect_total) = match_rects(rust_rects, &golden.rects);

    let mut table_cell_matched = 0;
    let mut table_cell_total = 0;
    for golden_table in &golden.tables {
        if let Some(rust_table) = find_best_table(&rust_tables, golden_table) {
            let (m, t) = match_table_cells(rust_table, golden_table);
            table_cell_matched += m;
            table_cell_total += t;
        } else {
            for row in &golden_table.rows {
                table_cell_total += row.len();
            }
        }
    }

    let result = PageResult {
        page_number: golden.page_number,
        char_matched,
        char_total,
        word_matched,
        word_total,
        line_matched,
        line_total,
        rect_matched,
        rect_total,
        table_cell_matched,
        table_cell_total,
    };

    eprintln!(
        "[{}] page {}: chars={}/{} ({:.1}%) words={}/{} ({:.1}%) \
         lines={}/{} ({:.1}%) rects={}/{} ({:.1}%) tables={}/{} ({:.1}%)",
        pdf_name,
        result.page_number,
        result.char_matched,
        result.char_total,
        result.char_rate() * 100.0,
        result.word_matched,
        result.word_total,
        result.word_rate() * 100.0,
        result.line_matched,
        result.line_total,
        result.line_rate() * 100.0,
        result.rect_matched,
        result.rect_total,
        result.rect_rate() * 100.0,
        result.table_cell_matched,
        result.table_cell_total,
        result.table_rate() * 100.0,
    );

    result
}

#[derive(Debug)]
pub struct PdfResult {
    pub pdf_name: String,
    pub pages: Vec<PageResult>,
    pub parse_error: Option<String>,
}

impl PdfResult {
    pub fn total_char_rate(&self) -> f64 {
        let matched: usize = self.pages.iter().map(|p| p.char_matched).sum();
        let total: usize = self.pages.iter().map(|p| p.char_total).sum();
        rate(matched, total)
    }
    pub fn total_word_rate(&self) -> f64 {
        let matched: usize = self.pages.iter().map(|p| p.word_matched).sum();
        let total: usize = self.pages.iter().map(|p| p.word_total).sum();
        rate(matched, total)
    }
    pub fn total_line_rate(&self) -> f64 {
        let matched: usize = self.pages.iter().map(|p| p.line_matched).sum();
        let total: usize = self.pages.iter().map(|p| p.line_total).sum();
        rate(matched, total)
    }
    pub fn total_rect_rate(&self) -> f64 {
        let matched: usize = self.pages.iter().map(|p| p.rect_matched).sum();
        let total: usize = self.pages.iter().map(|p| p.rect_total).sum();
        rate(matched, total)
    }
    pub fn total_table_rate(&self) -> f64 {
        let matched: usize = self.pages.iter().map(|p| p.table_cell_matched).sum();
        let total: usize = self.pages.iter().map(|p| p.table_cell_total).sum();
        rate(matched, total)
    }

    pub fn print_summary(&self) {
        if let Some(ref err) = self.parse_error {
            eprintln!("\n=== {} === PARSE ERROR: {}", self.pdf_name, err);
            return;
        }
        eprintln!(
            "\n=== {} ===\n  chars: {:.1}%  words: {:.1}%  lines: {:.1}%  rects: {:.1}%  tables: {:.1}%",
            self.pdf_name,
            self.total_char_rate() * 100.0,
            self.total_word_rate() * 100.0,
            self.total_line_rate() * 100.0,
            self.total_rect_rate() * 100.0,
            self.total_table_rate() * 100.0,
        );
    }
}

pub fn validate_pdf(pdf_name: &str) -> PdfResult {
    eprintln!("\n--- Validating: {} ---", pdf_name);
    let golden = load_golden(pdf_name);
    let pdf = open_pdf(pdf_name);

    eprintln!(
        "Golden from pdfplumber v{}, {} pages",
        golden.pdfplumber_version,
        golden.pages.len()
    );

    let mut page_results = Vec::new();

    for golden_page in &golden.pages {
        match pdf.page(golden_page.page_number) {
            Ok(page) => {
                let width_ok = coords_match(page.width(), golden_page.width, COORD_TOLERANCE);
                let height_ok = coords_match(page.height(), golden_page.height, COORD_TOLERANCE);
                if !width_ok || !height_ok {
                    eprintln!(
                        "  WARNING: page {} dimensions differ: \
                         rust=({:.1}, {:.1}) golden=({:.1}, {:.1})",
                        golden_page.page_number,
                        page.width(),
                        page.height(),
                        golden_page.width,
                        golden_page.height,
                    );
                }
                page_results.push(validate_page(pdf_name, &page, golden_page));
            }
            Err(e) => {
                let msg = format!("page {} error: {}", golden_page.page_number, e);
                eprintln!("  ERROR: {}", msg);
                return PdfResult {
                    pdf_name: pdf_name.to_string(),
                    pages: page_results,
                    parse_error: Some(msg),
                };
            }
        }
    }

    let result = PdfResult {
        pdf_name: pdf_name.to_string(),
        pages: page_results,
        parse_error: None,
    };
    result.print_summary();
    result
}

/// Validate a PDF, returning PdfResult with parse_error set on failure instead of panicking.
pub fn try_validate_pdf(pdf_name: &str) -> PdfResult {
    eprintln!("\n--- Validating: {} ---", pdf_name);

    let golden_path = fixtures_dir()
        .join("golden")
        .join(pdf_name.replace(".pdf", ".json"));
    let golden_data = match std::fs::read_to_string(&golden_path) {
        Ok(data) => data,
        Err(e) => {
            return PdfResult {
                pdf_name: pdf_name.to_string(),
                pages: vec![],
                parse_error: Some(format!("golden file not found: {}", e)),
            };
        }
    };
    let golden: GoldenData = match serde_json::from_str(&golden_data) {
        Ok(g) => g,
        Err(e) => {
            return PdfResult {
                pdf_name: pdf_name.to_string(),
                pages: vec![],
                parse_error: Some(format!("golden parse error: {}", e)),
            };
        }
    };

    let pdf = match try_open_pdf(pdf_name) {
        Ok(p) => p,
        Err(e) => {
            return PdfResult {
                pdf_name: pdf_name.to_string(),
                pages: vec![],
                parse_error: Some(format!("PDF open error: {}", e)),
            };
        }
    };

    eprintln!(
        "Golden from pdfplumber v{}, {} pages",
        golden.pdfplumber_version,
        golden.pages.len()
    );

    let mut page_results = Vec::new();

    for golden_page in &golden.pages {
        match pdf.page(golden_page.page_number) {
            Ok(page) => {
                page_results.push(validate_page(pdf_name, &page, golden_page));
            }
            Err(e) => {
                let msg = format!("page {} error: {}", golden_page.page_number, e);
                eprintln!("  ERROR: {}", msg);
                return PdfResult {
                    pdf_name: pdf_name.to_string(),
                    pages: page_results,
                    parse_error: Some(msg),
                };
            }
        }
    }

    let result = PdfResult {
        pdf_name: pdf_name.to_string(),
        pages: page_results,
        parse_error: None,
    };
    result.print_summary();
    result
}
