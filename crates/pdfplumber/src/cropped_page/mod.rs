//! CroppedPage: a spatially filtered view of a PDF page.
//!
//! Provides [`CroppedPage`] which contains filtered objects from a [`Page`]
//! based on a bounding box criterion (crop, within_bbox, or outside_bbox).
//! Coordinates are adjusted relative to the crop origin.

use pdfplumber_core::{
    BBox, Char, ColumnMode, Curve, DedupeOptions, Edge, Image, Line, Orientation, PageObject, Rect,
    Table, TableFinder, TableSettings, TextDirection, TextLine, TextOptions, Word, WordExtractor,
    WordOptions, blocks_to_text, cluster_lines_into_blocks, cluster_words_into_lines, dedupe_chars,
    derive_edges, detect_columns, extract_text_for_cells, normalize_table_columns,
    sort_blocks_column_order, sort_blocks_reading_order, split_lines_at_columns, words_to_text,
};

/// A spatially filtered view of a PDF page.
///
/// Created by [`crate::Page::crop`], [`crate::Page::within_bbox`], or [`crate::Page::outside_bbox`].
/// Contains only the objects matching the spatial filter criterion, with
/// coordinates adjusted relative to the crop origin.
pub struct CroppedPage {
    /// Page width (crop width).
    width: f64,
    /// Page height (crop height).
    height: f64,
    /// Filtered and coordinate-adjusted characters.
    chars: Vec<Char>,
    /// Filtered and coordinate-adjusted lines.
    lines: Vec<Line>,
    /// Filtered and coordinate-adjusted rectangles.
    rects: Vec<Rect>,
    /// Filtered and coordinate-adjusted curves.
    curves: Vec<Curve>,
    /// Filtered and coordinate-adjusted images.
    images: Vec<Image>,
}

impl CroppedPage {
    /// Returns the crop bounding box (dimensions of the cropped region).
    pub fn bbox(&self) -> BBox {
        BBox::new(0.0, 0.0, self.width, self.height)
    }

    /// Returns the width of the cropped region.
    pub fn width(&self) -> f64 {
        self.width
    }

    /// Returns the height of the cropped region.
    pub fn height(&self) -> f64 {
        self.height
    }

    /// Returns the characters in the cropped region.
    pub fn chars(&self) -> &[Char] {
        &self.chars
    }

    /// Returns the lines in the cropped region.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// Returns the rectangles in the cropped region.
    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }

    /// Returns the curves in the cropped region.
    pub fn curves(&self) -> &[Curve] {
        &self.curves
    }

    /// Returns the images in the cropped region.
    pub fn images(&self) -> &[Image] {
        &self.images
    }

    /// Compute edges from all geometric primitives in the cropped region.
    pub fn edges(&self) -> Vec<Edge> {
        derive_edges(&self.lines, &self.rects, &self.curves)
    }

    /// Extract words from this cropped page.
    pub fn extract_words(&self, options: &WordOptions) -> Vec<Word> {
        WordExtractor::extract(&self.chars, options)
    }

    /// Extract text from this cropped page.
    pub fn extract_text(&self, options: &TextOptions) -> String {
        let words = self.extract_words(&WordOptions {
            y_tolerance: options.y_tolerance,
            ..WordOptions::default()
        });

        if !options.layout {
            return words_to_text(&words, options.y_tolerance);
        }

        let lines = cluster_words_into_lines(&words, options.y_tolerance);
        let split = split_lines_at_columns(lines, options.x_density);
        let mut blocks = cluster_lines_into_blocks(split, options.y_density);

        match &options.column_mode {
            ColumnMode::None => {
                sort_blocks_reading_order(&mut blocks, options.x_density);
            }
            ColumnMode::Auto => {
                let boundaries =
                    detect_columns(&words, options.min_column_gap, options.max_columns);
                sort_blocks_column_order(&mut blocks, &boundaries);
            }
            ColumnMode::Explicit(boundaries) => {
                sort_blocks_column_order(&mut blocks, boundaries);
            }
        }

        blocks_to_text(&blocks)
    }

    /// Detect tables in the cropped region.
    pub fn find_tables(&self, settings: &TableSettings) -> Vec<Table> {
        let edges = self.edges();
        let words = self.extract_words(&WordOptions::default());

        let finder = TableFinder::new_with_words(edges, words, settings.clone());
        let mut tables = finder.find_tables();

        for table in &mut tables {
            extract_text_for_cells(&mut table.cells, &self.chars);
            for row in &mut table.rows {
                extract_text_for_cells(row, &self.chars);
            }
            for col in &mut table.columns {
                extract_text_for_cells(col, &self.chars);
            }
        }

        // Normalize merged cells: split wide cells into uniform grid columns
        tables = tables
            .into_iter()
            .map(|t| normalize_table_columns(&t))
            .collect();

        tables
    }

    /// Apply a further crop to this cropped page.
    pub fn crop(&self, bbox: BBox) -> CroppedPage {
        filter_and_build(self, bbox, FilterMode::Crop)
    }

    /// Return objects fully contained within the bbox.
    pub fn within_bbox(&self, bbox: BBox) -> CroppedPage {
        filter_and_build(self, bbox, FilterMode::Within)
    }

    /// Return objects fully outside the bbox.
    pub fn outside_bbox(&self, bbox: BBox) -> CroppedPage {
        filter_and_build(self, bbox, FilterMode::Outside)
    }

    /// Return a filtered view retaining only objects that match the predicate.
    ///
    /// Enables composable filtering: `page.filter(f1).filter(f2)`.
    /// See [`crate::Page::filter`] for details.
    pub fn filter<F>(&self, predicate: F) -> CroppedPage
    where
        F: Fn(&PageObject<'_>) -> bool,
    {
        let chars: Vec<Char> = self
            .chars
            .iter()
            .filter(|c| predicate(&PageObject::Char(c)))
            .cloned()
            .collect();
        let lines: Vec<Line> = self
            .lines
            .iter()
            .filter(|l| predicate(&PageObject::Line(l)))
            .cloned()
            .collect();
        let rects: Vec<Rect> = self
            .rects
            .iter()
            .filter(|r| predicate(&PageObject::Rect(r)))
            .cloned()
            .collect();
        let curves: Vec<Curve> = self
            .curves
            .iter()
            .filter(|c| predicate(&PageObject::Curve(c)))
            .cloned()
            .collect();
        let images: Vec<Image> = self
            .images
            .iter()
            .filter(|i| predicate(&PageObject::Image(i)))
            .cloned()
            .collect();
        CroppedPage {
            width: self.width,
            height: self.height,
            chars,
            lines,
            rects,
            curves,
            images,
        }
    }

    /// Remove duplicate overlapping characters, returning a new view.
    ///
    /// Two characters are considered duplicates if their positions overlap
    /// within `tolerance` and the specified `extra_attrs` match. The first
    /// occurrence is kept; subsequent duplicates are discarded.
    pub fn dedupe_chars(&self, options: &DedupeOptions) -> CroppedPage {
        let deduped = dedupe_chars(&self.chars, options);
        CroppedPage {
            width: self.width,
            height: self.height,
            chars: deduped,
            lines: self.lines.clone(),
            rects: self.rects.clone(),
            curves: self.curves.clone(),
            images: self.images.clone(),
        }
    }

    /// Return only horizontal edges.
    pub fn horizontal_edges(&self) -> Vec<Edge> {
        self.edges()
            .into_iter()
            .filter(|e| e.orientation == Orientation::Horizontal)
            .collect()
    }

    /// Return only vertical edges.
    pub fn vertical_edges(&self) -> Vec<Edge> {
        self.edges()
            .into_iter()
            .filter(|e| e.orientation == Orientation::Vertical)
            .collect()
    }

    /// Return text lines whose dominant direction is horizontal (LTR or RTL).
    pub fn text_lines_horizontal(&self, word_options: &WordOptions) -> Vec<TextLine> {
        let words = self.extract_words(word_options);
        let lines = cluster_words_into_lines(words, word_options.y_tolerance);
        lines
            .into_iter()
            .filter(|line| {
                line.words.iter().all(|w| {
                    w.direction == TextDirection::Ltr || w.direction == TextDirection::Rtl
                })
            })
            .collect()
    }

    /// Return text lines whose dominant direction is vertical (TTB or BTT).
    pub fn text_lines_vertical(&self, word_options: &WordOptions) -> Vec<TextLine> {
        let words = self.extract_words(word_options);
        let lines = cluster_words_into_lines(words, word_options.y_tolerance);
        lines
            .into_iter()
            .filter(|line| {
                line.words.iter().all(|w| {
                    w.direction == TextDirection::Ttb || w.direction == TextDirection::Btt
                })
            })
            .collect()
    }
}

/// Create a `CroppedPage` from a set of chars and other page data (no coordinate adjustment).
pub(crate) fn from_page_data(
    width: f64,
    height: f64,
    chars: Vec<Char>,
    lines: Vec<Line>,
    rects: Vec<Rect>,
    curves: Vec<Curve>,
    images: Vec<Image>,
) -> CroppedPage {
    CroppedPage {
        width,
        height,
        chars,
        lines,
        rects,
        curves,
        images,
    }
}

/// Filter mode for spatial operations.
#[derive(Debug, Clone, Copy)]
pub(crate) enum FilterMode {
    /// Center of object falls within bbox.
    Crop,
    /// Object fully contained within bbox.
    Within,
    /// Object fully outside bbox (no overlap).
    Outside,
}

/// Returns the center point of a bounding box.
fn bbox_center(x0: f64, top: f64, x1: f64, bottom: f64) -> (f64, f64) {
    ((x0 + x1) / 2.0, (top + bottom) / 2.0)
}

/// Check if a point is within a bbox.
fn point_in_bbox(x: f64, y: f64, bbox: &BBox) -> bool {
    x >= bbox.x0 && x <= bbox.x1 && y >= bbox.top && y <= bbox.bottom
}

/// Check if an object bbox is fully contained within a filter bbox.
fn fully_within(obj_x0: f64, obj_top: f64, obj_x1: f64, obj_bottom: f64, bbox: &BBox) -> bool {
    obj_x0 >= bbox.x0 && obj_x1 <= bbox.x1 && obj_top >= bbox.top && obj_bottom <= bbox.bottom
}

/// Check if an object bbox has no overlap with a filter bbox.
fn fully_outside(obj_x0: f64, obj_top: f64, obj_x1: f64, obj_bottom: f64, bbox: &BBox) -> bool {
    obj_x1 <= bbox.x0 || obj_x0 >= bbox.x1 || obj_bottom <= bbox.top || obj_top >= bbox.bottom
}

/// Check if an object passes the filter.
fn passes_filter(
    obj_x0: f64,
    obj_top: f64,
    obj_x1: f64,
    obj_bottom: f64,
    bbox: &BBox,
    mode: FilterMode,
) -> bool {
    match mode {
        FilterMode::Crop => {
            let (cx, cy) = bbox_center(obj_x0, obj_top, obj_x1, obj_bottom);
            point_in_bbox(cx, cy, bbox)
        }
        FilterMode::Within => fully_within(obj_x0, obj_top, obj_x1, obj_bottom, bbox),
        FilterMode::Outside => fully_outside(obj_x0, obj_top, obj_x1, obj_bottom, bbox),
    }
}

/// Adjust a coordinate by subtracting the crop origin offset.
fn adjust_coord(val: f64, offset: f64) -> f64 {
    val - offset
}

/// Trait for types that provide page-like data for filtering.
pub(crate) trait PageData {
    fn chars_data(&self) -> &[Char];
    fn lines_data(&self) -> &[Line];
    fn rects_data(&self) -> &[Rect];
    fn curves_data(&self) -> &[Curve];
    fn images_data(&self) -> &[Image];
}

impl PageData for CroppedPage {
    fn chars_data(&self) -> &[Char] {
        &self.chars
    }
    fn lines_data(&self) -> &[Line] {
        &self.lines
    }
    fn rects_data(&self) -> &[Rect] {
        &self.rects
    }
    fn curves_data(&self) -> &[Curve] {
        &self.curves
    }
    fn images_data(&self) -> &[Image] {
        &self.images
    }
}

/// Build a CroppedPage by filtering and coordinate-adjusting objects from source data.
pub(crate) fn filter_and_build(source: &dyn PageData, bbox: BBox, mode: FilterMode) -> CroppedPage {
    let dx = bbox.x0;
    let dy = bbox.top;

    let chars: Vec<Char> = source
        .chars_data()
        .iter()
        .filter(|c| passes_filter(c.bbox.x0, c.bbox.top, c.bbox.x1, c.bbox.bottom, &bbox, mode))
        .map(|c| {
            let mut ch = c.clone();
            ch.bbox = BBox::new(
                adjust_coord(ch.bbox.x0, dx),
                adjust_coord(ch.bbox.top, dy),
                adjust_coord(ch.bbox.x1, dx),
                adjust_coord(ch.bbox.bottom, dy),
            );
            ch.doctop = adjust_coord(ch.doctop, dy);
            ch
        })
        .collect();

    let lines: Vec<Line> = source
        .lines_data()
        .iter()
        .filter(|l| passes_filter(l.x0, l.top, l.x1, l.bottom, &bbox, mode))
        .map(|l| {
            let mut ln = l.clone();
            ln.x0 = adjust_coord(ln.x0, dx);
            ln.top = adjust_coord(ln.top, dy);
            ln.x1 = adjust_coord(ln.x1, dx);
            ln.bottom = adjust_coord(ln.bottom, dy);
            ln
        })
        .collect();

    let rects: Vec<Rect> = source
        .rects_data()
        .iter()
        .filter(|r| passes_filter(r.x0, r.top, r.x1, r.bottom, &bbox, mode))
        .map(|r| {
            let mut rc = r.clone();
            rc.x0 = adjust_coord(rc.x0, dx);
            rc.top = adjust_coord(rc.top, dy);
            rc.x1 = adjust_coord(rc.x1, dx);
            rc.bottom = adjust_coord(rc.bottom, dy);
            rc
        })
        .collect();

    let curves: Vec<Curve> = source
        .curves_data()
        .iter()
        .filter(|c| passes_filter(c.x0, c.top, c.x1, c.bottom, &bbox, mode))
        .map(|c| {
            let mut cv = c.clone();
            cv.x0 = adjust_coord(cv.x0, dx);
            cv.top = adjust_coord(cv.top, dy);
            cv.x1 = adjust_coord(cv.x1, dx);
            cv.bottom = adjust_coord(cv.bottom, dy);
            cv.pts = cv.pts.iter().map(|(px, py)| (px - dx, py - dy)).collect();
            cv
        })
        .collect();

    let images: Vec<Image> = source
        .images_data()
        .iter()
        .filter(|i| passes_filter(i.x0, i.top, i.x1, i.bottom, &bbox, mode))
        .map(|i| {
            let mut im = i.clone();
            im.x0 = adjust_coord(im.x0, dx);
            im.top = adjust_coord(im.top, dy);
            im.x1 = adjust_coord(im.x1, dx);
            im.bottom = adjust_coord(im.bottom, dy);
            im
        })
        .collect();

    CroppedPage {
        width: bbox.width(),
        height: bbox.height(),
        chars,
        lines,
        rects,
        curves,
        images,
    }
}


#[cfg(test)]
mod tests;
