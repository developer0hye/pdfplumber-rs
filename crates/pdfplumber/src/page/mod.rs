//! Page type for accessing extracted content from a PDF page.

use std::collections::HashMap;

use pdfplumber_core::{
    Annotation, BBox, Char, ColumnMode, Curve, DedupeOptions, Edge, ExportedImage, ExtractWarning,
    FormField, HtmlOptions, HtmlRenderer, Hyperlink, Image, ImageExportOptions, Line, PageObject,
    PageRegions, Rect, SearchMatch, SearchOptions, StructElement, Table, TableFinder,
    TableSettings, TextOptions, Word, WordExtractor, WordOptions, blocks_to_text,
    cluster_lines_into_blocks, cluster_words_into_lines, dedupe_chars, derive_edges,
    detect_columns, duplicate_merged_content_in_table, export_image_set,
    extract_text_for_cells_with_options, normalize_table_columns, search_chars,
    sort_blocks_column_order, sort_blocks_reading_order, split_lines_at_columns, words_to_text,
};

use crate::cropped_page::{CroppedPage, FilterMode, PageData, filter_and_build, from_page_data};

mod helpers;
use helpers::{collect_chars_by_structure_order, collect_elements};

#[cfg(test)]
mod tests;

/// A single page from a PDF document.
///
/// Provides access to characters, words, lines, rects, curves, and edges
/// extracted from the page. Constructed internally by the PDF parsing pipeline.
pub struct Page {
    /// Page index (0-based).
    page_number: usize,
    /// Page width in points.
    width: f64,
    /// Page height in points.
    height: f64,
    /// Page rotation in degrees (0, 90, 180, or 270).
    rotation: i32,
    /// MediaBox — boundaries of the physical page.
    media_box: BBox,
    /// CropBox — visible region of the page (None = same as MediaBox).
    crop_box: Option<BBox>,
    /// TrimBox — intended finished page dimensions after trimming.
    trim_box: Option<BBox>,
    /// BleedBox — clipping region for production output.
    bleed_box: Option<BBox>,
    /// ArtBox — extent of meaningful content.
    art_box: Option<BBox>,
    /// Characters extracted from this page.
    chars: Vec<Char>,
    /// Lines extracted from painted paths.
    lines: Vec<Line>,
    /// Rectangles extracted from painted paths.
    rects: Vec<Rect>,
    /// Curves extracted from painted paths.
    curves: Vec<Curve>,
    /// Images extracted from Do operator (Image XObjects).
    images: Vec<Image>,
    /// Annotations extracted from the page's /Annots array.
    annotations: Vec<Annotation>,
    /// Hyperlinks extracted from Link annotations with resolved URIs.
    hyperlinks: Vec<Hyperlink>,
    /// Form fields belonging to this page (from document AcroForm, filtered by page).
    form_fields: Vec<FormField>,
    /// Structure tree elements for this page (from document StructTreeRoot, filtered by page).
    structure_tree: Option<Vec<StructElement>>,
    /// Non-fatal warnings collected during extraction.
    warnings: Vec<ExtractWarning>,
}

impl Page {
    /// Create a new page with the given metadata and characters.
    pub fn new(page_number: usize, width: f64, height: f64, chars: Vec<Char>) -> Self {
        let media_box = BBox::new(0.0, 0.0, width, height);
        Self {
            page_number,
            width,
            height,
            rotation: 0,
            media_box,
            crop_box: None,
            trim_box: None,
            bleed_box: None,
            art_box: None,
            chars,
            lines: Vec::new(),
            rects: Vec::new(),
            curves: Vec::new(),
            images: Vec::new(),
            annotations: Vec::new(),
            hyperlinks: Vec::new(),
            form_fields: Vec::new(),
            structure_tree: None,
            warnings: Vec::new(),
        }
    }

    /// Create a new page with characters and geometry.
    pub fn with_geometry(
        page_number: usize,
        width: f64,
        height: f64,
        chars: Vec<Char>,
        lines: Vec<Line>,
        rects: Vec<Rect>,
        curves: Vec<Curve>,
    ) -> Self {
        let media_box = BBox::new(0.0, 0.0, width, height);
        Self {
            page_number,
            width,
            height,
            rotation: 0,
            media_box,
            crop_box: None,
            trim_box: None,
            bleed_box: None,
            art_box: None,
            chars,
            lines,
            rects,
            curves,
            images: Vec::new(),
            annotations: Vec::new(),
            hyperlinks: Vec::new(),
            form_fields: Vec::new(),
            structure_tree: None,
            warnings: Vec::new(),
        }
    }

    /// Create a new page with characters, geometry, and images.
    #[allow(clippy::too_many_arguments)]
    pub fn with_geometry_and_images(
        page_number: usize,
        width: f64,
        height: f64,
        chars: Vec<Char>,
        lines: Vec<Line>,
        rects: Vec<Rect>,
        curves: Vec<Curve>,
        images: Vec<Image>,
    ) -> Self {
        let media_box = BBox::new(0.0, 0.0, width, height);
        Self {
            page_number,
            width,
            height,
            rotation: 0,
            media_box,
            crop_box: None,
            trim_box: None,
            bleed_box: None,
            art_box: None,
            chars,
            lines,
            rects,
            curves,
            images,
            annotations: Vec::new(),
            hyperlinks: Vec::new(),
            form_fields: Vec::new(),
            structure_tree: None,
            warnings: Vec::new(),
        }
    }

    /// Create a page from PDF extraction results.
    ///
    /// Used internally by [`Pdf::page()`](crate::Pdf::page) to construct pages
    /// from content stream interpretation.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_extraction(
        page_number: usize,
        width: f64,
        height: f64,
        rotation: i32,
        media_box: BBox,
        crop_box: Option<BBox>,
        trim_box: Option<BBox>,
        bleed_box: Option<BBox>,
        art_box: Option<BBox>,
        chars: Vec<Char>,
        lines: Vec<Line>,
        rects: Vec<Rect>,
        curves: Vec<Curve>,
        images: Vec<Image>,
        annotations: Vec<Annotation>,
        hyperlinks: Vec<Hyperlink>,
        form_fields: Vec<FormField>,
        structure_tree: Option<Vec<StructElement>>,
        warnings: Vec<ExtractWarning>,
    ) -> Self {
        Self {
            page_number,
            width,
            height,
            rotation,
            media_box,
            crop_box,
            trim_box,
            bleed_box,
            art_box,
            chars,
            lines,
            rects,
            curves,
            images,
            annotations,
            hyperlinks,
            form_fields,
            structure_tree,
            warnings,
        }
    }

    /// Returns the page index (0-based).
    pub fn page_number(&self) -> usize {
        self.page_number
    }

    /// Returns the page width in points.
    pub fn width(&self) -> f64 {
        self.width
    }

    /// Returns the page height in points.
    pub fn height(&self) -> f64 {
        self.height
    }

    /// Returns the page rotation in degrees (0, 90, 180, or 270).
    pub fn rotation(&self) -> i32 {
        self.rotation
    }

    /// Returns the page bounding box: `(0, 0, width, height)`.
    pub fn bbox(&self) -> BBox {
        BBox::new(0.0, 0.0, self.width, self.height)
    }

    /// Returns the page MediaBox (boundaries of the physical page).
    pub fn media_box(&self) -> BBox {
        self.media_box
    }

    /// Returns the page CropBox, if explicitly set.
    pub fn crop_box(&self) -> Option<BBox> {
        self.crop_box
    }

    /// Returns the page TrimBox, if set.
    ///
    /// TrimBox defines the intended dimensions of the finished page
    /// after trimming. Important for print production workflows.
    pub fn trim_box(&self) -> Option<BBox> {
        self.trim_box
    }

    /// Returns the page BleedBox, if set.
    ///
    /// BleedBox defines the region to which page contents should be
    /// clipped when output in a production environment.
    pub fn bleed_box(&self) -> Option<BBox> {
        self.bleed_box
    }

    /// Returns the page ArtBox, if set.
    ///
    /// ArtBox defines the extent of the page's meaningful content
    /// as intended by the page's creator.
    pub fn art_box(&self) -> Option<BBox> {
        self.art_box
    }

    /// Returns the characters extracted from this page.
    pub fn chars(&self) -> &[Char] {
        &self.chars
    }

    /// Returns the lines extracted from this page.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// Returns the rectangles extracted from this page.
    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }

    /// Returns the curves extracted from this page.
    pub fn curves(&self) -> &[Curve] {
        &self.curves
    }

    /// Returns the images extracted from this page.
    pub fn images(&self) -> &[Image] {
        &self.images
    }

    /// Export images with deterministic filenames.
    ///
    /// Produces [`ExportedImage`] entries for each image on this page that
    /// has data populated (requires `extract_image_data: true` in
    /// [`ExtractOptions`]). Images without data are skipped.
    ///
    /// Page number in filenames is 1-indexed.
    pub fn export_images(&self, options: &ImageExportOptions) -> Vec<ExportedImage> {
        export_image_set(&self.images, self.page_number + 1, options)
    }

    /// Returns the annotations extracted from this page.
    ///
    /// Annotations include text notes, links, highlights, stamps, and other
    /// interactive elements defined in the page's /Annots array.
    pub fn annots(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Returns the hyperlinks extracted from this page.
    ///
    /// Hyperlinks are Link annotations with resolved URI targets.
    /// Each hyperlink has a bounding box and a URI string.
    pub fn hyperlinks(&self) -> &[Hyperlink] {
        &self.hyperlinks
    }

    /// Returns the form fields belonging to this page.
    ///
    /// Form fields are extracted from the document's AcroForm dictionary
    /// and filtered to this page based on the field's `/P` reference.
    pub fn form_fields(&self) -> &[FormField] {
        &self.form_fields
    }

    /// Returns the structure tree elements for this page, if the PDF is tagged.
    ///
    /// Tagged PDFs (ISO 32000-1, Section 14.8) contain a logical structure tree
    /// that maps semantic elements (headings, paragraphs, tables) to visual content
    /// via marked content identifiers (MCIDs). Returns `None` for untagged PDFs.
    ///
    /// Each [`StructElement`] has an `element_type` (e.g., "H1", "P", "Table"),
    /// `mcids` linking to characters, and `children` forming a tree hierarchy.
    pub fn structure_tree(&self) -> Option<&[StructElement]> {
        self.structure_tree.as_deref()
    }

    /// Returns a flattened list of all structure elements for this page.
    ///
    /// Unlike [`structure_tree()`](Self::structure_tree) which preserves the tree hierarchy,
    /// this method returns every element (including nested children) as a flat `Vec`.
    /// Returns an empty `Vec` for untagged PDFs.
    ///
    /// Each [`StructElement`] has an `element_type` (e.g., "H1", "P", "Table"),
    /// `mcids` linking to characters, and `children` forming a tree hierarchy.
    pub fn structure_elements(&self) -> Vec<&StructElement> {
        match &self.structure_tree {
            Some(tree) => collect_elements(tree),
            None => Vec::new(),
        }
    }

    /// Groups characters by their marked content identifier (MCID).
    ///
    /// Returns a map from MCID to the characters tagged with that MCID.
    /// Characters without an MCID are excluded. Each MCID group preserves
    /// content stream order.
    ///
    /// MCIDs link page content to the document's structure tree — use
    /// [`structure_tree()`](Self::structure_tree) to find the [`StructElement`]
    /// that owns each MCID.
    pub fn chars_by_mcid(&self) -> HashMap<u32, Vec<&Char>> {
        let mut groups: HashMap<u32, Vec<&Char>> = HashMap::new();
        for c in &self.chars {
            if let Some(mcid) = c.mcid {
                groups.entry(mcid).or_default().push(c);
            }
        }
        groups
    }

    /// Returns characters ordered by the structure tree (semantic reading order).
    ///
    /// Traverses the structure tree in depth-first order and collects characters
    /// matching each element's MCIDs. This gives the document's intended logical
    /// reading order rather than raw content stream order.
    ///
    /// Characters without an MCID are appended at the end. Returns all characters
    /// in content stream order if no structure tree is available.
    pub fn semantic_chars(&self) -> Vec<&Char> {
        let tree = match &self.structure_tree {
            Some(t) => t,
            None => return self.chars.iter().collect(),
        };

        let mcid_groups = self.chars_by_mcid();
        if mcid_groups.is_empty() {
            return self.chars.iter().collect();
        }

        let mut result = Vec::with_capacity(self.chars.len());
        let mut used_mcids = std::collections::HashSet::new();

        // Walk structure tree depth-first, collecting chars for each MCID
        collect_chars_by_structure_order(tree, &mcid_groups, &mut result, &mut used_mcids);

        // Append untagged chars (those without MCIDs)
        for c in &self.chars {
            if c.mcid.is_none() {
                result.push(c);
            }
        }

        result
    }

    /// Returns non-fatal warnings collected during page extraction.
    ///
    /// Warnings are purely informational and do not affect the correctness
    /// of extracted content. They indicate best-effort degradation such as
    /// missing font metrics or unresolvable resources.
    ///
    /// Warning collection is controlled by [`crate::ExtractOptions::collect_warnings`].
    /// When disabled, this returns an empty slice.
    pub fn warnings(&self) -> &[ExtractWarning] {
        &self.warnings
    }

    /// Compute edges from all geometric primitives (lines, rects, curves).
    ///
    /// Edges are line segments derived from all geometric objects on the page,
    /// suitable for table detection. Each edge tracks its source (Line, Rect side, Curve chord).
    pub fn edges(&self) -> Vec<Edge> {
        derive_edges(&self.lines, &self.rects, &self.curves)
    }

    /// Extract words from this page using the specified options.
    ///
    /// Groups characters into words based on spatial proximity using
    /// `x_tolerance` and `y_tolerance` from the options.
    pub fn extract_words(&self, options: &WordOptions) -> Vec<Word> {
        WordExtractor::extract(&self.chars, options)
    }

    /// Extract text from this page.
    ///
    /// When `options.layout` is false (default): extracts words in spatial order,
    /// joining with spaces within lines and newlines between lines.
    ///
    /// When `options.layout` is true: detects text blocks and reading order,
    /// handling multi-column layouts. Blocks are separated by double newlines.
    pub fn extract_text(&self, options: &TextOptions) -> String {
        let words = self.extract_words(&WordOptions {
            y_tolerance: options.y_tolerance,
            expand_ligatures: options.expand_ligatures,
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

    /// Extract text from the body region of this page, excluding header and footer.
    ///
    /// Uses the provided [`PageRegions`] (from [`Pdf::detect_page_regions()`]) to
    /// crop the page to the body area and extract text from it.
    pub fn extract_text_body(&self, regions: &PageRegions) -> String {
        let cropped = self.crop(regions.body);
        cropped.extract_text(&TextOptions::default())
    }

    /// Render this page's content as semantic HTML.
    ///
    /// Detects headings from font size heuristics, wraps text in `<p>` elements,
    /// converts tables to `<table>/<tr>/<td>` elements, detects bold (`<strong>`)
    /// and italic (`<em>`) from font name analysis, and detects lists (`<ul>`/`<ol>`).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let html = page.to_html(&HtmlOptions::default());
    /// println!("{html}");
    /// ```
    pub fn to_html(&self, options: &HtmlOptions) -> String {
        let tables = self.find_tables(&TableSettings::default());
        HtmlRenderer::render(&self.chars, &tables, options)
    }

    /// Detect tables on this page and return them with cell text populated.
    ///
    /// Uses [`TableFinder`] internally with the specified strategy. For the
    /// Stream strategy, words are extracted from the page's characters to
    /// generate synthetic edges from text alignment patterns.
    ///
    /// Each cell's text is populated by finding characters whose bbox center
    /// falls within the cell's bounding box.
    pub fn find_tables(&self, settings: &TableSettings) -> Vec<Table> {
        let edges = self.edges();
        let words = self.extract_words(&WordOptions::default());

        let finder = TableFinder::new_with_words(edges, words, settings.clone());
        let mut tables = finder.find_tables();

        // Populate cell text from page characters
        let cell_word_opts = WordOptions::default();
        for table in &mut tables {
            extract_text_for_cells_with_options(&mut table.cells, &self.chars, &cell_word_opts);
            for row in &mut table.rows {
                extract_text_for_cells_with_options(row, &self.chars, &cell_word_opts);
            }
            for col in &mut table.columns {
                extract_text_for_cells_with_options(col, &self.chars, &cell_word_opts);
            }
        }

        // Normalize merged cells: split wide cells into uniform grid columns,
        // text in first sub-cell only (matching Python pdfplumber behavior)
        tables = tables
            .into_iter()
            .map(|t| normalize_table_columns(&t))
            .collect();

        // Duplicate merged cell content if configured
        if settings.duplicate_merged_content {
            tables = tables
                .into_iter()
                .map(|t| duplicate_merged_content_in_table(&t))
                .collect();
        }

        // Filter by minimum accuracy if configured
        if let Some(min_acc) = settings.min_accuracy {
            tables.retain(|t| t.accuracy() >= min_acc);
        }

        tables
    }

    /// Extract tables as 2D text arrays.
    ///
    /// Returns a Vec of tables, where each table is a Vec of rows, and each row
    /// is a Vec of cell text values (`None` for empty cells).
    pub fn extract_tables(&self, settings: &TableSettings) -> Vec<Vec<Vec<Option<String>>>> {
        self.find_tables(settings)
            .into_iter()
            .map(|table| {
                table
                    .rows
                    .into_iter()
                    .map(|row| row.into_iter().map(|cell| cell.text).collect())
                    .collect()
            })
            .collect()
    }

    /// Extract the largest table as a 2D text array.
    ///
    /// Returns the table with the most cells. If multiple tables have the same
    /// number of cells, returns the one with the largest bounding box area.
    /// Returns `None` if no tables are found.
    /// Search for a text pattern on this page and return matches with bounding boxes.
    ///
    /// Concatenates all character texts and matches the pattern against the full
    /// text. Each match's bounding box is the union of its constituent character
    /// bounding boxes, allowing precise highlighting.
    ///
    /// # Arguments
    ///
    /// * `pattern` - The search pattern (regex or literal, depending on options).
    /// * `options` - Controls regex vs. literal mode and case sensitivity.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let matches = page.search("Hello", &SearchOptions { regex: false, ..Default::default() });
    /// for m in &matches {
    ///     println!("{}: ({:.0}, {:.0})", m.text, m.bbox.x0, m.bbox.top);
    /// }
    /// ```
    pub fn search(&self, pattern: &str, options: &SearchOptions) -> Vec<SearchMatch> {
        search_chars(&self.chars, pattern, options, self.page_number)
    }

    /// Return a [`CroppedPage`] with objects whose centers fall within `bbox`.
    ///
    /// Coordinates in the returned page are adjusted relative to the crop origin.
    pub fn crop(&self, bbox: BBox) -> CroppedPage {
        filter_and_build(self, bbox, FilterMode::Crop)
    }

    /// Return a [`CroppedPage`] with objects fully contained within `bbox`.
    ///
    /// Only objects whose entire bounding box is inside `bbox` are included.
    /// Coordinates are adjusted relative to the crop origin.
    pub fn within_bbox(&self, bbox: BBox) -> CroppedPage {
        filter_and_build(self, bbox, FilterMode::Within)
    }

    /// Return a [`CroppedPage`] with objects fully outside `bbox`.
    ///
    /// Only objects whose bounding box has no overlap with `bbox` are included.
    /// Coordinates are adjusted relative to the bbox origin.
    pub fn outside_bbox(&self, bbox: BBox) -> CroppedPage {
        filter_and_build(self, bbox, FilterMode::Outside)
    }

    /// Return a filtered view retaining only objects that match the predicate.
    ///
    /// The predicate receives a [`PageObject`] reference for each object on
    /// the page (characters, lines, rectangles, curves, images). Objects for
    /// which the predicate returns `true` are kept; all others are removed.
    ///
    /// The original page is not modified. The returned [`FilteredPage`] (a
    /// type alias for [`CroppedPage`]) supports the same query methods
    /// (`chars()`, `extract_text()`, `find_tables()`, etc.) and can be
    /// filtered again for composable filtering.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Keep only characters with size > 14pt, plus all non-char objects
    /// let big = page.filter(|obj| match obj {
    ///     PageObject::Char(c) => c.size > 14.0,
    ///     _ => true,
    /// });
    /// ```
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
        from_page_data(self.width, self.height, chars, lines, rects, curves, images)
    }

    /// Remove duplicate overlapping characters, returning a new page view.
    ///
    /// Two characters are considered duplicates if their positions overlap
    /// within `tolerance` and the specified `extra_attrs` match. The first
    /// occurrence is kept; subsequent duplicates are discarded.
    ///
    /// The original page is not modified.
    pub fn dedupe_chars(&self, options: &DedupeOptions) -> CroppedPage {
        let deduped = dedupe_chars(&self.chars, options);
        from_page_data(
            self.width,
            self.height,
            deduped,
            self.lines.clone(),
            self.rects.clone(),
            self.curves.clone(),
            self.images.clone(),
        )
    }

    /// Generate an SVG representation of this page.
    ///
    /// The SVG includes the page boundary rectangle and uses the same
    /// top-left origin coordinate system as pdfplumber.
    ///
    /// # Example
    ///
    /// ```
    /// # use pdfplumber::{Page, SvgOptions};
    /// let page = Page::new(0, 612.0, 792.0, vec![]);
    /// let svg = page.to_svg(&SvgOptions::default());
    /// assert!(svg.contains("<svg"));
    /// ```
    pub fn to_svg(&self, options: &pdfplumber_core::SvgOptions) -> String {
        let renderer = pdfplumber_core::SvgRenderer::new(self.width, self.height);
        renderer.to_svg(options)
    }

    /// Generate a debug SVG showing the table detection pipeline.
    ///
    /// Runs the table detection pipeline and renders intermediate results:
    /// detected edges (red), intersection points (circles), cell boundaries
    /// (dashed lines), and table regions (light blue rectangles).
    ///
    /// # Arguments
    ///
    /// * `settings` - Table detection settings (strategy, tolerances, etc.)
    /// * `options` - Controls which pipeline stages are rendered
    pub fn debug_tablefinder_svg(
        &self,
        settings: &TableSettings,
        options: &pdfplumber_core::SvgDebugOptions,
    ) -> String {
        let edges = self.edges();
        let words = self.extract_words(&WordOptions::default());
        let finder = TableFinder::new_with_words(edges, words, settings.clone());
        let debug = finder.find_tables_debug();

        let mut renderer = pdfplumber_core::SvgRenderer::new(self.width, self.height);

        if options.show_edges {
            renderer.draw_edges(&debug.edges, &pdfplumber_core::DrawStyle::edges_default());
        }
        if options.show_intersections {
            renderer.draw_intersections(
                &debug.intersections,
                &pdfplumber_core::DrawStyle::intersections_default(),
            );
        }
        if options.show_cells {
            renderer.draw_cells(&debug.cells, &pdfplumber_core::DrawStyle::cells_default());
        }
        if options.show_tables {
            renderer.draw_tables(&debug.tables, &pdfplumber_core::DrawStyle::tables_default());
        }

        renderer.to_svg(&pdfplumber_core::SvgOptions::default())
    }

    /// Extract the largest table from this page as a 2D grid of cell text.
    ///
    /// Returns `None` if no tables are found. If multiple tables exist,
    /// returns the one with the most cells (breaking ties by area).
    pub fn extract_table(&self, settings: &TableSettings) -> Option<Vec<Vec<Option<String>>>> {
        let tables = self.find_tables(settings);
        tables
            .into_iter()
            .max_by(|a, b| {
                a.cells.len().cmp(&b.cells.len()).then_with(|| {
                    let area_a = (a.bbox.x1 - a.bbox.x0) * (a.bbox.bottom - a.bbox.top);
                    let area_b = (b.bbox.x1 - b.bbox.x0) * (b.bbox.bottom - b.bbox.top);
                    area_a.partial_cmp(&area_b).unwrap()
                })
            })
            .map(|table| {
                table
                    .rows
                    .into_iter()
                    .map(|row| row.into_iter().map(|cell| cell.text).collect())
                    .collect()
            })
    }
}
