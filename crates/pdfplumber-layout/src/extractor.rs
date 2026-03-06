//! Core extraction engine: `&Page` → `PageLayout`.
//!
//! For each page this module:
//! 1. Computes the body-baseline font size (modal bucket across all chars).
//! 2. Clusters chars → words → lines → [`TextBlock`]s.
//! 3. Detects column boundaries and sorts blocks in true reading order
//!    (column 1 top-to-bottom, then column 2, etc.).
//! 4. Classifies each [`TextBlock`] as [`LayoutBlock::Heading`] or
//!    [`LayoutBlock::Paragraph`], with list-item detection on paragraphs.
//! 5. Detects [`LayoutBlock::Table`]s via the built-in lattice/stream finder.
//! 6. Detects [`LayoutBlock::Figure`]s from image XObjects and rect clusters.
//! 7. Suppresses blocks that fall inside known header/footer zones.
//! 8. Marks paragraphs as captions when they follow a figure bbox.

use pdfplumber::Page;
use pdfplumber_core::{
    BBox, Char, ColumnMode, TableSettings, TextBlock, WordOptions,
    blocks_to_text, cluster_lines_into_blocks, cluster_words_into_lines,
    detect_columns, sort_blocks_reading_order,
};

use crate::classifier::{compute_body_baseline, is_heading_candidate, mean_font_size};
use crate::figures::{Figure, detect_figures_from_images, detect_figures_from_rects,
    merge_overlapping_figures};
use crate::headings::{Heading, HeadingLevel};
use crate::lists::parse_list_prefix;
use crate::{LayoutBlock, LayoutTable};
use crate::paragraphs::{Paragraph, looks_like_caption};

/// Options controlling layout extraction behaviour.
#[derive(Debug, Clone)]
pub struct LayoutOptions {
    /// Vertical tolerance for clustering words into lines (points). Default: 3.0.
    pub y_tolerance: f64,
    /// Maximum vertical gap to group lines into the same block (points). Default: 12.0.
    pub y_density: f64,
    /// Column detection mode. Default: [`ColumnMode::Auto`].
    ///
    /// `Auto` detects column boundaries from word x-positions.
    /// `None` uses simple top-to-bottom sort (faster, wrong for multi-column PDFs).
    /// `Explicit(vec![300.0])` uses the given x-coordinates as column separators.
    pub column_mode: ColumnMode,
    /// Minimum horizontal gap (points) to consider as a column separator. Default: 20.0.
    pub min_column_gap: f64,
    /// Whether to run the table finder. Default: true.
    pub detect_tables: bool,
    /// Whether to run figure detection. Default: true.
    pub detect_figures: bool,
    /// Optional header zone: blocks whose bbox falls entirely within this y-range
    /// (0..header_zone_bottom from page top) are suppressed. Default: None.
    ///
    /// Set by [`crate::document::Document`] after cross-page region detection.
    pub header_zone_bottom: Option<f64>,
    /// Optional footer zone: blocks whose bbox falls entirely above this y-value
    /// (footer_zone_top..page_height) are suppressed. Default: None.
    pub footer_zone_top: Option<f64>,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            y_tolerance: 3.0,
            y_density: 12.0,
            column_mode: ColumnMode::Auto,
            min_column_gap: 20.0,
            detect_tables: true,
            detect_figures: true,
            header_zone_bottom: None,
            footer_zone_top: None,
        }
    }
}

/// All layout blocks extracted from a single page, in reading order.
#[derive(Debug, Clone)]
pub struct PageLayout {
    /// Page number (0-based).
    pub page_number: usize,
    /// Page width in points.
    pub width: f64,
    /// Page height in points.
    pub height: f64,
    /// All semantic blocks sorted in reading order (column-aware).
    pub blocks: Vec<LayoutBlock>,
}

impl PageLayout {
    /// Iterate headings on this page.
    pub fn headings(&self) -> impl Iterator<Item = &Heading> {
        self.blocks.iter().filter_map(|b| {
            if let LayoutBlock::Heading(h) = b { Some(h) } else { None }
        })
    }

    /// Iterate paragraphs on this page.
    pub fn paragraphs(&self) -> impl Iterator<Item = &Paragraph> {
        self.blocks.iter().filter_map(|b| {
            if let LayoutBlock::Paragraph(p) = b { Some(p) } else { None }
        })
    }

    /// Iterate tables on this page.
    pub fn tables(&self) -> impl Iterator<Item = &LayoutTable> {
        self.blocks.iter().filter_map(|b| {
            if let LayoutBlock::Table(t) = b { Some(t) } else { None }
        })
    }

    /// Iterate figures on this page.
    pub fn figures(&self) -> impl Iterator<Item = &Figure> {
        self.blocks.iter().filter_map(|b| {
            if let LayoutBlock::Figure(f) = b { Some(f) } else { None }
        })
    }
}

/// Extract a [`PageLayout`] from a single page.
pub fn extract_page_layout(page: &Page, opts: &LayoutOptions) -> PageLayout {
    let page_number = page.page_number();
    let chars = page.chars();

    // 1. Body baseline.
    let body_baseline = compute_body_baseline(chars);

    // 2. Words → lines → TextBlocks.
    let word_opts = WordOptions {
        x_tolerance: 3.0,
        y_tolerance: opts.y_tolerance,
        keep_blank_chars: false,
        use_text_flow: false,
        text_direction: pdfplumber_core::TextDirection::Ltr,
        expand_ligatures: true,
    };
    let words = page.extract_words(&word_opts);
    let lines = cluster_words_into_lines(&words, opts.y_tolerance);
    let mut text_blocks: Vec<TextBlock> = cluster_lines_into_blocks(lines, opts.y_density);

    // 3. Column-aware reading order sort.
    match &opts.column_mode {
        ColumnMode::None => {
            sort_blocks_reading_order(&mut text_blocks, opts.min_column_gap);
        }
        ColumnMode::Auto => {
            let boundaries = detect_columns(&words, opts.min_column_gap, 6);
            sort_in_column_order(&mut text_blocks, &boundaries);
        }
        ColumnMode::Explicit(boundaries) => {
            sort_in_column_order(&mut text_blocks, boundaries);
        }
    }

    // 4. Tables.
    let tables = if opts.detect_tables {
        page.find_tables(&TableSettings::default())
    } else {
        vec![]
    };
    let table_bboxes: Vec<BBox> = tables.iter().map(|t| t.bbox).collect();

    // 5. Figures.
    let mut figures: Vec<Figure> = if opts.detect_figures {
        let from_images = detect_figures_from_images(page.images(), page_number);
        let from_rects = detect_figures_from_rects(page.rects(), page_number, 10.0);
        merge_overlapping_figures([from_images, from_rects].concat())
    } else {
        vec![]
    };
    // Drop figures inside tables.
    figures.retain(|fig| {
        !table_bboxes
            .iter()
            .any(|tb| bbox_overlap_fraction(fig.bbox, *tb) > 0.5)
    });
    let figure_bboxes: Vec<BBox> = figures.iter().map(|f| f.bbox).collect();

    // 6. Classify text blocks.
    let mut blocks: Vec<LayoutBlock> = Vec::new();

    for tb in &text_blocks {
        // Suppress header/footer zones.
        if let Some(hz) = opts.header_zone_bottom {
            if tb.bbox.bottom <= hz {
                continue;
            }
        }
        if let Some(fz) = opts.footer_zone_top {
            if tb.bbox.top >= fz {
                continue;
            }
        }

        // Skip blocks substantially inside a table.
        if table_bboxes
            .iter()
            .any(|tb_bbox| bbox_overlap_fraction(tb.bbox, *tb_bbox) > 0.7)
        {
            continue;
        }

        let text = blocks_to_text(std::slice::from_ref(tb)).trim().to_string();
        if text.is_empty() {
            continue;
        }

        // Chars inside this block bbox for font profiling.
        let block_chars: Vec<Char> = chars
            .iter()
            .filter(|c| {
                let cy = (c.bbox.top + c.bbox.bottom) / 2.0;
                c.bbox.x0 >= tb.bbox.x0 - 1.0
                    && c.bbox.x1 <= tb.bbox.x1 + 1.0
                    && cy >= tb.bbox.top - 1.0
                    && cy <= tb.bbox.bottom + 1.0
            })
            .cloned()
            .collect();

        let block_mean_size = if block_chars.is_empty() {
            body_baseline
        } else {
            mean_font_size(&block_chars)
        };
        let fontname = dominant_fontname(&block_chars);

        if is_heading_candidate(&block_chars, text.len(), body_baseline) {
            let size_ratio = if body_baseline > 0.0 {
                block_mean_size / body_baseline
            } else {
                1.0
            };
            blocks.push(LayoutBlock::Heading(Heading {
                text,
                bbox: tb.bbox,
                page_number,
                level: HeadingLevel::from_size_ratio(size_ratio),
                font_size: block_mean_size,
                fontname,
            }));
        } else {
            // Caption detection.
            let is_caption = looks_like_caption(&text)
                || figure_bboxes.iter().any(|fb| {
                    let x_overlap = fb.x0.max(tb.bbox.x0) < fb.x1.min(tb.bbox.x1);
                    let below = tb.bbox.top >= fb.bottom && (tb.bbox.top - fb.bottom) < 24.0;
                    x_overlap && below && text.len() < 200
                });

            // List item: mark the paragraph if it starts with a bullet or ordinal prefix.
            let is_list_item = parse_list_prefix(&text).is_some();

            blocks.push(LayoutBlock::Paragraph(Paragraph {
                text,
                bbox: tb.bbox,
                page_number,
                line_count: tb.lines.len(),
                font_size: block_mean_size,
                fontname,
                is_caption,
                is_list_item,
            }));
        }
    }

    // 7. Table blocks.
    for table in &tables {
        let col_count = table.rows.first().map(|r| r.len()).unwrap_or(0);
        let row_count = table.rows.len();
        let cells: Vec<Vec<Option<String>>> = table
            .rows
            .iter()
            .map(|row| row.iter().map(|cell| cell.text.clone()).collect())
            .collect();
        blocks.push(LayoutBlock::Table(LayoutTable {
            bbox: table.bbox,
            page_number,
            rows: row_count,
            cols: col_count,
            cells,
        }));
    }

    // 8. Figure blocks.
    for fig in figures {
        blocks.push(LayoutBlock::Figure(fig));
    }

    // 9. Final sort of all blocks: top → bottom, tie-break left → right.
    //    This merges text, tables, and figures into a single reading-order stream.
    blocks.sort_by(|a, b| {
        a.bbox()
            .top
            .partial_cmp(&b.bbox().top)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.bbox()
                    .x0
                    .partial_cmp(&b.bbox().x0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    PageLayout {
        page_number,
        width: page.width(),
        height: page.height(),
        blocks,
    }
}

// ---------------------------------------------------------------------------
// Column-aware reading order
// ---------------------------------------------------------------------------

/// Sort text blocks in column-aware reading order.
///
/// Given column boundary x-coordinates, assigns each block to the leftmost
/// column whose right edge is greater than the block's left edge (x0).
/// Blocks within each column are sorted top-to-bottom. Columns are emitted
/// left-to-right.
fn sort_in_column_order(blocks: &mut Vec<TextBlock>, column_boundaries: &[f64]) {
    if column_boundaries.is_empty() {
        // No columns detected — simple top-to-bottom sort.
        sort_blocks_reading_order(blocks, 20.0);
        return;
    }

    // Build column "lanes": each lane is bounded by [left_x, right_x).
    // boundaries are x-coordinates of column separators.
    // For 2 columns and one boundary at x=306: lane 0 = [0, 306), lane 1 = [306, ∞).
    let assign_column = |bbox: &BBox| -> usize {
        let mid = (bbox.x0 + bbox.x1) / 2.0;
        let mut col = 0;
        for &boundary in column_boundaries {
            if mid >= boundary {
                col += 1;
            }
        }
        col
    };

    // Group blocks by column.
    let num_cols = column_boundaries.len() + 1;
    let mut columns: Vec<Vec<TextBlock>> = vec![Vec::new(); num_cols];
    for block in blocks.drain(..) {
        let col = assign_column(&block.bbox);
        columns[col.min(num_cols - 1)].push(block);
    }

    // Sort each column top-to-bottom, then emit columns left-to-right.
    for col in &mut columns {
        col.sort_by(|a, b| {
            a.bbox.top
                .partial_cmp(&b.bbox.top)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        blocks.extend(col.drain(..));
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Fraction of `inner` area that overlaps `outer`.
fn bbox_overlap_fraction(inner: BBox, outer: BBox) -> f64 {
    let ix0 = inner.x0.max(outer.x0);
    let iy0 = inner.top.max(outer.top);
    let ix1 = inner.x1.min(outer.x1);
    let iy1 = inner.bottom.min(outer.bottom);
    if ix1 <= ix0 || iy1 <= iy0 {
        return 0.0;
    }
    let inter = (ix1 - ix0) * (iy1 - iy0);
    let area = (inner.x1 - inner.x0) * (inner.bottom - inner.top);
    if area <= 0.0 { 0.0 } else { inter / area }
}

/// Return the most common fontname among chars, or empty string.
fn dominant_fontname(chars: &[Char]) -> String {
    use std::collections::HashMap;
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for c in chars {
        *counts.entry(c.fontname.as_str()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, v)| *v)
        .map(|(k, _)| k.to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::BBox;

    fn make_tb(x0: f64, top: f64, x1: f64, bottom: f64) -> TextBlock {
        TextBlock {
            lines: vec![],
            bbox: BBox { x0, top, x1, bottom },
        }
    }

    #[test]
    fn bbox_overlap_no_overlap() {
        let a = BBox { x0: 0.0, top: 0.0, x1: 10.0, bottom: 10.0 };
        let b = BBox { x0: 20.0, top: 20.0, x1: 30.0, bottom: 30.0 };
        assert_eq!(bbox_overlap_fraction(a, b), 0.0);
    }

    #[test]
    fn bbox_overlap_full() {
        let a = BBox { x0: 0.0, top: 0.0, x1: 10.0, bottom: 10.0 };
        assert!((bbox_overlap_fraction(a, a) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn bbox_overlap_half() {
        let inner = BBox { x0: 0.0, top: 0.0, x1: 10.0, bottom: 10.0 };
        let outer = BBox { x0: 5.0, top: 0.0, x1: 15.0, bottom: 10.0 };
        let frac = bbox_overlap_fraction(inner, outer);
        assert!((frac - 0.5).abs() < 1e-9, "got {frac}");
    }

    #[test]
    fn dominant_fontname_empty() {
        assert_eq!(dominant_fontname(&[]), "");
    }

    #[test]
    fn sort_in_column_order_single_column() {
        let mut blocks = vec![
            make_tb(72.0, 200.0, 400.0, 220.0),
            make_tb(72.0, 100.0, 400.0, 120.0),
            make_tb(72.0, 150.0, 400.0, 170.0),
        ];
        sort_in_column_order(&mut blocks, &[]);
        // With no boundaries, should be top-to-bottom.
        assert!(blocks[0].bbox.top <= blocks[1].bbox.top);
        assert!(blocks[1].bbox.top <= blocks[2].bbox.top);
    }

    #[test]
    fn sort_in_column_order_two_columns() {
        // Two columns split at x=300.
        // Left column blocks at top=100, 200. Right column blocks at top=50, 150.
        // Expected order: left col first (100, 200), then right col (50, 150).
        let mut blocks = vec![
            make_tb(310.0, 50.0, 550.0, 70.0),   // right col, top=50
            make_tb(72.0, 100.0, 280.0, 120.0),   // left col, top=100
            make_tb(310.0, 150.0, 550.0, 170.0),  // right col, top=150
            make_tb(72.0, 200.0, 280.0, 220.0),   // left col, top=200
        ];
        sort_in_column_order(&mut blocks, &[300.0]);
        // Left column blocks come first (top=100 then top=200)
        assert_eq!(blocks[0].bbox.top, 100.0);
        assert_eq!(blocks[1].bbox.top, 200.0);
        // Then right column (top=50 then top=150)
        assert_eq!(blocks[2].bbox.top, 50.0);
        assert_eq!(blocks[3].bbox.top, 150.0);
    }

    #[test]
    fn layout_options_default_auto_columns() {
        let opts = LayoutOptions::default();
        assert!(matches!(opts.column_mode, ColumnMode::Auto));
        assert!(opts.detect_tables);
        assert!(opts.detect_figures);
        assert!(opts.header_zone_bottom.is_none());
        assert!(opts.footer_zone_top.is_none());
    }

    #[test]
    fn page_layout_accessors_empty() {
        let layout = PageLayout {
            page_number: 0,
            width: 612.0,
            height: 792.0,
            blocks: vec![],
        };
        assert_eq!(layout.headings().count(), 0);
        assert_eq!(layout.paragraphs().count(), 0);
        assert_eq!(layout.tables().count(), 0);
        assert_eq!(layout.figures().count(), 0);
    }
}
