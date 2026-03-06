//! Private helper functions and types for PDF document processing.
//!
//! Contains the `CollectingHandler` event sink and free functions called from
//! `impl Pdf`. Nothing here is part of the public API.

use pdfplumber_core::{
    BBox, Color, DashPattern, ExtractWarning, FillRule, Line, Orientation, PaintedPath, Path,
    StructElement, TextDirection,
};
use pdfplumber_parse::{
    CharEvent, ContentHandler, ImageEvent, PageGeometry, PaintOp, PathEvent,
};

/// Internal handler that collects content stream events during page interpretation.
pub(super) struct CollectingHandler {
    pub(super) chars: Vec<CharEvent>,
    pub(super) paths: Vec<PathEvent>,
    pub(super) images: Vec<ImageEvent>,
    pub(super) warnings: Vec<ExtractWarning>,
    page_index: usize,
    collect_warnings: bool,
}

impl CollectingHandler {
    pub(super) fn new(page_index: usize, collect_warnings: bool) -> Self {
        Self {
            chars: Vec::new(),
            paths: Vec::new(),
            images: Vec::new(),
            warnings: Vec::new(),
            page_index,
            collect_warnings,
        }
    }
}

impl ContentHandler for CollectingHandler {
    fn on_char(&mut self, event: CharEvent) {
        self.chars.push(event);
    }

    fn on_path_painted(&mut self, event: PathEvent) {
        self.paths.push(event);
    }

    fn on_image(&mut self, event: ImageEvent) {
        self.images.push(event);
    }

    fn on_warning(&mut self, mut warning: ExtractWarning) {
        if self.collect_warnings {
            if warning.page.is_none() {
                warning.page = Some(self.page_index);
            }
            self.warnings.push(warning);
        }
    }
}

/// Convert a [`PathEvent`] from the interpreter into a [`PaintedPath`] for shape extraction.
pub(super) fn path_event_to_painted_path(event: &PathEvent) -> PaintedPath {
    let (stroke, fill) = match event.paint_op {
        PaintOp::Stroke => (true, false),
        PaintOp::Fill => (false, true),
        PaintOp::FillAndStroke => (true, true),
    };

    PaintedPath {
        path: Path {
            segments: event.segments.clone(),
        },
        stroke,
        fill,
        fill_rule: event.fill_rule.unwrap_or_default(),
        line_width: event.line_width,
        stroke_color: event.stroking_color.clone().unwrap_or(Color::black()),
        fill_color: event.non_stroking_color.clone().unwrap_or(Color::black()),
        dash_pattern: event
            .dash_pattern
            .clone()
            .unwrap_or_else(DashPattern::solid),
        stroke_alpha: 1.0,
        fill_alpha: 1.0,
    }
}

/// Recursively walks the structure tree and includes elements whose `page_index`
/// matches the target page. Elements without a page_index are included if any of
/// their children belong to the page.
pub(super) fn filter_struct_elements_for_page(
    elements: &[StructElement],
    page_index: usize,
) -> Vec<StructElement> {
    elements
        .iter()
        .filter_map(|elem| filter_struct_element(elem, page_index))
        .collect()
}

/// Filter a single structure element and its children for a specific page.
pub(super) fn filter_struct_element(
    elem: &StructElement,
    page_index: usize,
) -> Option<StructElement> {
    let filtered_children = filter_struct_elements_for_page(&elem.children, page_index);
    let belongs_to_page = elem.page_index == Some(page_index);
    let has_page_children = !filtered_children.is_empty();

    if belongs_to_page || has_page_children {
        Some(StructElement {
            element_type: elem.element_type.clone(),
            mcids: if belongs_to_page {
                elem.mcids.clone()
            } else {
                Vec::new()
            },
            alt_text: elem.alt_text.clone(),
            actual_text: elem.actual_text.clone(),
            lang: elem.lang.clone(),
            bbox: elem.bbox,
            children: filtered_children,
            page_index: elem.page_index,
        })
    } else {
        None
    }
}

/// Rotate a text direction by the page rotation angle (clockwise).
pub(super) fn rotate_direction(dir: TextDirection, rotation: i32) -> TextDirection {
    match rotation {
        90 => match dir {
            TextDirection::Ltr => TextDirection::Ttb,
            TextDirection::Rtl => TextDirection::Btt,
            TextDirection::Ttb => TextDirection::Rtl,
            TextDirection::Btt => TextDirection::Ltr,
        },
        180 => match dir {
            TextDirection::Ltr => TextDirection::Rtl,
            TextDirection::Rtl => TextDirection::Ltr,
            TextDirection::Ttb => TextDirection::Btt,
            TextDirection::Btt => TextDirection::Ttb,
        },
        270 => match dir {
            TextDirection::Ltr => TextDirection::Btt,
            TextDirection::Rtl => TextDirection::Ttb,
            TextDirection::Ttb => TextDirection::Ltr,
            TextDirection::Btt => TextDirection::Rtl,
        },
        _ => dir,
    }
}

/// Undo a simple y-flip and re-apply through [`PageGeometry`] to account for rotation.
pub(super) fn rotate_bbox(
    x0: f64,
    top: f64,
    x1: f64,
    bottom: f64,
    page_height: f64,
    geometry: &PageGeometry,
) -> BBox {
    let native_min_y = page_height - bottom;
    let native_max_y = page_height - top;
    geometry.normalize_bbox(x0, native_min_y, x1, native_max_y)
}

/// Re-classify line orientation after rotation.
pub(super) fn classify_orientation(line: &Line) -> Orientation {
    let dx = (line.x1 - line.x0).abs();
    let dy = (line.bottom - line.top).abs();
    if dy < 1e-6 {
        Orientation::Horizontal
    } else if dx < 1e-6 {
        Orientation::Vertical
    } else {
        Orientation::Diagonal
    }
}
