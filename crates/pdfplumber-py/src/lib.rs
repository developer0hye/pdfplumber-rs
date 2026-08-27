//! Python bindings for pdfplumber-rs via PyO3.
//!
//! Exposes `PyPdf`, `PyPage`, `PyTable`, and `PyCroppedPage` classes to Python,
//! wrapping the Rust pdfplumber types for full API access.

/// Package version, kept in sync with Cargo.toml and pyproject.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

use ::pdfplumber::{
    Annotation, BBox, Bookmark, Char, Color, CroppedPage, Curve, FormField, Image, ImageContent,
    Line, MetadataReference, MetadataValue, Page, PageObjectKind, Pdf, PdfError, PdfErrorKind,
    RawDocumentMetadata, Rect, SearchMatch, SearchOptions, SignatureInfo, StructElement, Table,
    TableSettings, TextOptions, UnicodeNorm, ValidationIssue, Word, WordOptions,
};
use pyo3::exceptions::{
    PyAttributeError, PyException, PyIOError, PyRecursionError, PyRuntimeError, PyTypeError,
    PyValueError,
};
use pyo3::prelude::*;
use pyo3::sync::GILOnceCell;
use pyo3::types::{PyBool, PyBytes, PyDict, PyList, PyString, PyTuple};
use std::error::Error as _;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Python exception types for PdfError variants
// ---------------------------------------------------------------------------

pyo3::create_exception!(pdfplumber._native, PdfParseError, PyRuntimeError);
pyo3::create_exception!(pdfplumber._native, PdfIoError, PyIOError);
pyo3::create_exception!(pdfplumber._native, PdfFontError, PyRuntimeError);
pyo3::create_exception!(pdfplumber._native, PdfInterpreterError, PyRuntimeError);
pyo3::create_exception!(pdfplumber._native, PdfResourceLimitError, PyRuntimeError);
pyo3::create_exception!(pdfplumber._native, PdfPasswordRequired, PyRuntimeError);
pyo3::create_exception!(pdfplumber._native, PdfInvalidPassword, PyValueError);
pyo3::create_exception!(pdfplumber.utils.exceptions, PdfminerException, PyException);

fn pdfminer_parse_message(message: String) -> String {
    if message.contains("invalid file header") {
        "No /Root object! - Is this really a PDF?".to_string()
    } else {
        message
    }
}

fn map_stream_error(py: Python<'_>, error: PyErr) -> PyErr {
    if error.is_instance_of::<PyValueError>(py) {
        let message = error
            .value(py)
            .str()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|_| error.to_string());
        if message.contains("closed file") {
            return PdfminerException::new_err(message);
        }
    }
    error
}

/// Convert a PdfError to the appropriate Python exception.
fn to_py_err(e: PdfError) -> PyErr {
    let source_message = || {
        let message = e
            .source()
            .map(ToString::to_string)
            .unwrap_or_else(|| e.to_string());
        [
            "PDF parse error: ",
            "I/O error: ",
            "font error: ",
            "interpreter error: ",
        ]
        .into_iter()
        .find_map(|prefix| message.strip_prefix(prefix).map(str::to_owned))
        .unwrap_or(message)
    };

    match e.kind() {
        PdfErrorKind::Parse => PdfminerException::new_err(pdfminer_parse_message(source_message())),
        PdfErrorKind::Io => PdfIoError::new_err(source_message()),
        PdfErrorKind::Font => PdfFontError::new_err(source_message()),
        PdfErrorKind::Interpreter => PdfInterpreterError::new_err(source_message()),
        PdfErrorKind::ResourceLimit => {
            if let Some(limit) = e.resource_limit() {
                PdfResourceLimitError::new_err(format!(
                    "{} (limit: {}, actual: {})",
                    limit.name, limit.limit, limit.observed
                ))
            } else {
                PdfResourceLimitError::new_err(e.to_string())
            }
        }
        PdfErrorKind::PasswordRequired | PdfErrorKind::InvalidPassword => {
            PdfminerException::new_err(())
        }
        PdfErrorKind::Other => PyRuntimeError::new_err(source_message()),
        _ => PyRuntimeError::new_err(e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers: Rust types -> Python dicts
// ---------------------------------------------------------------------------

fn color_to_py(py: Python<'_>, color: &Color) -> PyObject {
    match color {
        Color::Gray(g) => (*g).into_pyobject(py).unwrap().into_any().unbind(),
        Color::Rgb(r, g, b) => (*r, *g, *b).into_pyobject(py).unwrap().into_any().unbind(),
        Color::Cmyk(c, m, y, k) => (*c, *m, *y, *k)
            .into_pyobject(py)
            .unwrap()
            .into_any()
            .unbind(),
        Color::Pattern(name) => name.into_pyobject(py).unwrap().into_any().unbind(),
        Color::PatternWithBase(base, name) => PyTuple::new(
            py,
            [
                color_to_py(py, base),
                name.into_pyobject(py).unwrap().into_any().unbind(),
            ],
        )
        .unwrap()
        .into_any()
        .unbind(),
        Color::Other(vals) => PyTuple::new(py, vals).unwrap().into_any().unbind(),
    }
}

fn char_color_to_py(py: Python<'_>, color: &Color) -> PyObject {
    let value = color_to_py(py, color);
    match color {
        Color::Gray(_) | Color::Pattern(_) => {
            PyTuple::new(py, [value]).unwrap().into_any().unbind()
        }
        Color::Rgb(_, _, _)
        | Color::Cmyk(_, _, _, _)
        | Color::PatternWithBase(_, _)
        | Color::Other(_) => value,
    }
}

/// Rust structs keep idiomatic field names while compatibility dictionaries
/// expose the spellings used by the pinned Python implementation.
#[derive(Clone, Copy)]
enum NativeObjectField {
    LineWidth,
    StrokeColor,
    FillColor,
    SourceDimensions,
    BitsPerComponent,
    ColorSpace,
}

impl NativeObjectField {
    const fn python_key(self) -> &'static str {
        match self {
            Self::LineWidth => "linewidth",
            Self::StrokeColor => "stroking_color",
            Self::FillColor => "non_stroking_color",
            Self::SourceDimensions => "srcsize",
            Self::BitsPerComponent => "bits",
            Self::ColorSpace => "colorspace",
        }
    }
}

fn set_compatible_bbox_geometry(
    dict: &Bound<'_, PyDict>,
    x0: f64,
    top: f64,
    x1: f64,
    bottom: f64,
    page_height: f64,
    initial_doctop: Option<f64>,
) -> PyResult<()> {
    dict.set_item("y0", page_height - bottom)?;
    dict.set_item("y1", page_height - top)?;
    dict.set_item("width", x1 - x0)?;
    dict.set_item("height", bottom - top)?;
    if let Some(initial_doctop) = initial_doctop {
        dict.set_item("doctop", initial_doctop + top)?;
    }
    Ok(())
}

fn char_to_dict(
    py: Python<'_>,
    ch: &Char,
    page_number: usize,
    page_height: f64,
) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("matrix", PyTuple::new(py, ch.ctm)?)?;
    dict.set_item("object_type", "char")?;
    dict.set_item("page_number", page_number)?;
    dict.set_item("text", &ch.text)?;
    dict.set_item("x0", ch.bbox.x0)?;
    dict.set_item("top", ch.bbox.top)?;
    dict.set_item("x1", ch.bbox.x1)?;
    dict.set_item("bottom", ch.bbox.bottom)?;
    set_compatible_bbox_geometry(
        &dict,
        ch.bbox.x0,
        ch.bbox.top,
        ch.bbox.x1,
        ch.bbox.bottom,
        page_height,
        None,
    )?;
    dict.set_item("fontname", &ch.fontname)?;
    dict.set_item("size", ch.size)?;
    dict.set_item("adv", ch.advance)?;
    dict.set_item("doctop", ch.doctop)?;
    dict.set_item("upright", ch.upright)?;
    dict.set_item("mcid", ch.mcid)?;
    dict.set_item("tag", ch.tag.as_deref())?;
    dict.set_item(
        "direction",
        match ch.direction {
            ::pdfplumber::TextDirection::Ltr => "ltr",
            ::pdfplumber::TextDirection::Rtl => "rtl",
            ::pdfplumber::TextDirection::Ttb => "ttb",
            ::pdfplumber::TextDirection::Btt => "btt",
        },
    )?;
    dict.set_item(
        "stroking_color",
        ch.stroking_color
            .as_ref()
            .map(|c| char_color_to_py(py, c))
            .unwrap_or_else(|| py.None()),
    )?;
    dict.set_item(
        "non_stroking_color",
        ch.non_stroking_color
            .as_ref()
            .map(|c| char_color_to_py(py, c))
            .unwrap_or_else(|| py.None()),
    )?;
    Ok(dict.into_any().unbind())
}

#[derive(Clone)]
struct CompatibleLayoutLine {
    bbox: BBox,
    text: String,
    char_indices: Vec<usize>,
    orientation: CompatibleLayoutOrientation,
    empty: bool,
}

#[derive(Clone)]
struct CompatibleLayoutBox {
    bbox: BBox,
    text: String,
    line_indices: Vec<usize>,
    orientation: CompatibleLayoutOrientation,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CompatibleLayoutOrientation {
    Horizontal,
    Vertical,
}

#[derive(Clone)]
enum CompatibleLayoutNode {
    Box(usize),
    Group {
        bbox: BBox,
        orientation: CompatibleLayoutOrientation,
        children: Box<[CompatibleLayoutNode; 2]>,
    },
}

struct CompatibleLayoutObjects {
    horizontal_boxes: Vec<PyObject>,
    vertical_boxes: Vec<PyObject>,
    horizontal_lines: Vec<PyObject>,
    vertical_lines: Vec<PyObject>,
    ordered_char_indices: Vec<usize>,
    family_order: Vec<&'static str>,
}

fn laparams_number(params: &Bound<'_, PyDict>, key: &str, default: f64) -> PyResult<f64> {
    params
        .get_item(key)?
        .map_or(Ok(default), |value| value.extract::<f64>())
}

fn horizontal_char_alignment(
    left: &Char,
    right: &Char,
    line_overlap: f64,
    char_margin: f64,
) -> bool {
    let vertical_overlap =
        left.bbox.bottom.min(right.bbox.bottom) - left.bbox.top.max(right.bbox.top);
    if vertical_overlap <= 0.0
        || left.bbox.height().min(right.bbox.height()) * line_overlap >= vertical_overlap
    {
        return false;
    }

    let horizontal_distance = if right.bbox.x0 <= left.bbox.x1 && left.bbox.x0 <= right.bbox.x1 {
        0.0
    } else {
        (left.bbox.x0 - right.bbox.x1)
            .abs()
            .min((left.bbox.x1 - right.bbox.x0).abs())
    };
    horizontal_distance < left.bbox.width().max(right.bbox.width()) * char_margin
}

fn compatible_layout_line(
    chars: &[Char],
    char_indices: Vec<usize>,
    orientation: CompatibleLayoutOrientation,
    word_margin: f64,
) -> CompatibleLayoutLine {
    let mut bbox = chars[char_indices[0]].bbox;
    let mut text = String::new();
    let mut previous: Option<BBox> = None;
    for &index in &char_indices {
        let ch = &chars[index];
        if let Some(previous) = previous {
            let margin = word_margin * ch.bbox.width().max(ch.bbox.height());
            let separated = match orientation {
                CompatibleLayoutOrientation::Horizontal => previous.x1 < ch.bbox.x0 - margin,
                CompatibleLayoutOrientation::Vertical => ch.bbox.top > previous.bottom + margin,
            };
            if separated {
                text.push(' ');
            }
        }
        text.push_str(&ch.text);
        previous = Some(ch.bbox);
        bbox = bbox.union(&ch.bbox);
    }
    let empty = text.trim().is_empty();
    text.push('\n');
    CompatibleLayoutLine {
        bbox,
        text,
        char_indices,
        orientation,
        empty,
    }
}

fn compatible_layout_lines(
    chars: &[Char],
    line_overlap: f64,
    char_margin: f64,
    word_margin: f64,
    detect_vertical: bool,
) -> Vec<CompatibleLayoutLine> {
    if chars.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut current: Option<(CompatibleLayoutOrientation, Vec<usize>)> = None;
    let mut previous = 0;
    for next in 1..chars.len() {
        let horizontal =
            horizontal_char_alignment(&chars[previous], &chars[next], line_overlap, char_margin);
        let vertical = detect_vertical
            && vertical_char_alignment(&chars[previous], &chars[next], line_overlap, char_margin);
        let continues = current
            .as_ref()
            .is_some_and(|(orientation, _)| match orientation {
                CompatibleLayoutOrientation::Horizontal => horizontal,
                CompatibleLayoutOrientation::Vertical => vertical,
            });
        if continues {
            current.as_mut().expect("checked as present").1.push(next);
        } else if let Some((orientation, indices)) = current.take() {
            lines.push(compatible_layout_line(
                chars,
                indices,
                orientation,
                word_margin,
            ));
        } else if vertical && !horizontal {
            current = Some((CompatibleLayoutOrientation::Vertical, vec![previous, next]));
        } else if horizontal && !vertical {
            current = Some((
                CompatibleLayoutOrientation::Horizontal,
                vec![previous, next],
            ));
        } else {
            lines.push(compatible_layout_line(
                chars,
                vec![previous],
                CompatibleLayoutOrientation::Horizontal,
                word_margin,
            ));
        }
        previous = next;
    }
    if let Some((orientation, indices)) = current {
        lines.push(compatible_layout_line(
            chars,
            indices,
            orientation,
            word_margin,
        ));
    } else {
        lines.push(compatible_layout_line(
            chars,
            vec![previous],
            CompatibleLayoutOrientation::Horizontal,
            word_margin,
        ));
    }
    lines
}

fn vertical_char_alignment(top: &Char, bottom: &Char, line_overlap: f64, char_margin: f64) -> bool {
    let horizontal_overlap = top.bbox.x1.min(bottom.bbox.x1) - top.bbox.x0.max(bottom.bbox.x0);
    if horizontal_overlap <= 0.0
        || top.bbox.width().min(bottom.bbox.width()) * line_overlap >= horizontal_overlap
    {
        return false;
    }

    let vertical_distance =
        if bottom.bbox.top <= top.bbox.bottom && top.bbox.top <= bottom.bbox.bottom {
            0.0
        } else {
            (top.bbox.top - bottom.bbox.bottom)
                .abs()
                .min((top.bbox.bottom - bottom.bbox.top).abs())
        };
    vertical_distance < top.bbox.height().max(bottom.bbox.height()) * char_margin
}

fn horizontal_line_neighbor(
    line: &CompatibleLayoutLine,
    other: &CompatibleLayoutLine,
    ratio: f64,
) -> bool {
    let distance = ratio * line.bbox.height();
    let intersects_search_area = other.bbox.x1 >= line.bbox.x0
        && other.bbox.x0 <= line.bbox.x1
        && other.bbox.bottom >= line.bbox.top - distance
        && other.bbox.top <= line.bbox.bottom + distance;
    let same_height = (other.bbox.height() - line.bbox.height()).abs() <= distance;
    let left_aligned = (other.bbox.x0 - line.bbox.x0).abs() <= distance;
    let right_aligned = (other.bbox.x1 - line.bbox.x1).abs() <= distance;
    let line_center = (line.bbox.x0 + line.bbox.x1) / 2.0;
    let other_center = (other.bbox.x0 + other.bbox.x1) / 2.0;
    intersects_search_area
        && same_height
        && (left_aligned || right_aligned || (other_center - line_center).abs() <= distance)
}

fn compatible_layout_boxes(
    lines: &[CompatibleLayoutLine],
    raw_width: f64,
    raw_height: f64,
    line_margin: f64,
) -> Vec<CompatibleLayoutBox> {
    let mut grid = std::collections::HashMap::<(i64, i64), Vec<usize>>::new();
    for (line_index, line) in lines.iter().enumerate().filter(|(_, line)| !line.empty) {
        let pdf_x0 = line.bbox.x0.max(0.0);
        let pdf_x1 = line.bbox.x1.min(raw_width);
        let pdf_y0 = (raw_height - line.bbox.bottom).max(0.0);
        let pdf_y1 = (raw_height - line.bbox.top).min(raw_height);
        if pdf_x1 <= pdf_x0 || pdf_y1 <= pdf_y0 {
            continue;
        }
        for grid_y in compatible_layout_grid_range(pdf_y0, pdf_y1) {
            for grid_x in compatible_layout_grid_range(pdf_x0, pdf_x1) {
                grid.entry((grid_x, grid_y)).or_default().push(line_index);
            }
        }
    }

    let mut line_boxes = vec![None; lines.len()];
    let mut box_members = Vec::<Vec<usize>>::new();
    for (line_index, line) in lines.iter().enumerate() {
        if line.empty {
            continue;
        }
        let mut members = vec![line_index];
        for neighbor_index in compatible_layout_neighbors(
            line_index,
            lines,
            &grid,
            raw_width,
            raw_height,
            line_margin,
        ) {
            members.push(neighbor_index);
            if let Some(box_index) = line_boxes[neighbor_index].take() {
                members.extend(&box_members[box_index]);
            }
        }
        let mut unique_members = Vec::new();
        for member in members {
            if !unique_members.contains(&member) {
                unique_members.push(member);
            }
        }
        let box_index = box_members.len();
        for &member in &unique_members {
            line_boxes[member] = Some(box_index);
        }
        box_members.push(unique_members);
    }

    let mut done = vec![false; box_members.len()];
    lines
        .iter()
        .enumerate()
        .filter_map(|(line_index, _)| {
            let box_index = line_boxes[line_index]?;
            if done[box_index] {
                return None;
            }
            done[box_index] = true;
            let mut members = box_members[box_index].clone();
            let orientation = lines[members[0]].orientation;
            members.sort_by(|left, right| match orientation {
                CompatibleLayoutOrientation::Horizontal => {
                    lines[*left].bbox.top.total_cmp(&lines[*right].bbox.top)
                }
                CompatibleLayoutOrientation::Vertical => {
                    lines[*right].bbox.x1.total_cmp(&lines[*left].bbox.x1)
                }
            });
            let mut bbox = lines[members[0]].bbox;
            let mut text = String::new();
            for &member in &members {
                bbox = bbox.union(&lines[member].bbox);
                text.push_str(&lines[member].text);
            }
            Some(CompatibleLayoutBox {
                bbox,
                text,
                line_indices: members,
                orientation,
            })
        })
        .collect()
}

fn vertical_line_neighbor(
    line: &CompatibleLayoutLine,
    other: &CompatibleLayoutLine,
    ratio: f64,
) -> bool {
    let distance = ratio * line.bbox.width();
    let intersects_search_area = other.bbox.x1 >= line.bbox.x0 - distance
        && other.bbox.x0 <= line.bbox.x1 + distance
        && other.bbox.bottom >= line.bbox.top
        && other.bbox.top <= line.bbox.bottom;
    let same_width = (other.bbox.width() - line.bbox.width()).abs() <= distance;
    let top_aligned = (other.bbox.top - line.bbox.top).abs() <= distance;
    let bottom_aligned = (other.bbox.bottom - line.bbox.bottom).abs() <= distance;
    let line_center = (line.bbox.top + line.bbox.bottom) / 2.0;
    let other_center = (other.bbox.top + other.bbox.bottom) / 2.0;
    intersects_search_area
        && same_width
        && (top_aligned || bottom_aligned || (other_center - line_center).abs() <= distance)
}

fn compatible_layout_grid_range(start: f64, end: f64) -> std::ops::Range<i64> {
    let start = (start.trunc() as i64).div_euclid(50);
    let end = ((end + 50.0).trunc() as i64).div_euclid(50);
    start..end
}

fn compatible_layout_neighbors(
    line_index: usize,
    lines: &[CompatibleLayoutLine],
    grid: &std::collections::HashMap<(i64, i64), Vec<usize>>,
    raw_width: f64,
    raw_height: f64,
    line_margin: f64,
) -> Vec<usize> {
    let line = &lines[line_index];
    let distance = match line.orientation {
        CompatibleLayoutOrientation::Horizontal => line_margin * line.bbox.height(),
        CompatibleLayoutOrientation::Vertical => line_margin * line.bbox.width(),
    };
    let query = match line.orientation {
        CompatibleLayoutOrientation::Horizontal => BBox::new(
            line.bbox.x0,
            line.bbox.top - distance,
            line.bbox.x1,
            line.bbox.bottom + distance,
        ),
        CompatibleLayoutOrientation::Vertical => BBox::new(
            line.bbox.x0 - distance,
            line.bbox.top,
            line.bbox.x1 + distance,
            line.bbox.bottom,
        ),
    };
    let pdf_x0 = query.x0.max(0.0);
    let pdf_x1 = query.x1.min(raw_width);
    let pdf_y0 = (raw_height - query.bottom).max(0.0);
    let pdf_y1 = (raw_height - query.top).min(raw_height);
    if pdf_x1 <= pdf_x0 || pdf_y1 <= pdf_y0 {
        return Vec::new();
    }

    let mut seen = vec![false; lines.len()];
    let mut neighbors = Vec::new();
    for grid_y in compatible_layout_grid_range(pdf_y0, pdf_y1) {
        for grid_x in compatible_layout_grid_range(pdf_x0, pdf_x1) {
            let Some(candidates) = grid.get(&(grid_x, grid_y)) else {
                continue;
            };
            for &candidate_index in candidates {
                if seen[candidate_index] {
                    continue;
                }
                seen[candidate_index] = true;
                let candidate = &lines[candidate_index];
                if candidate.orientation != line.orientation
                    || !layout_bbox_intersects(candidate.bbox, query)
                {
                    continue;
                }
                let is_neighbor = match line.orientation {
                    CompatibleLayoutOrientation::Horizontal => {
                        horizontal_line_neighbor(line, candidate, line_margin)
                    }
                    CompatibleLayoutOrientation::Vertical => {
                        vertical_line_neighbor(line, candidate, line_margin)
                    }
                };
                if is_neighbor {
                    neighbors.push(candidate_index);
                }
            }
        }
    }
    neighbors
}

fn layout_node_bbox(node: &CompatibleLayoutNode, boxes: &[CompatibleLayoutBox]) -> BBox {
    match node {
        CompatibleLayoutNode::Box(index) => boxes[*index].bbox,
        CompatibleLayoutNode::Group { bbox, .. } => *bbox,
    }
}

fn layout_node_orientation(
    node: &CompatibleLayoutNode,
    boxes: &[CompatibleLayoutBox],
) -> CompatibleLayoutOrientation {
    match node {
        CompatibleLayoutNode::Box(index) => boxes[*index].orientation,
        CompatibleLayoutNode::Group { orientation, .. } => *orientation,
    }
}

fn layout_bbox_distance(left: BBox, right: BBox) -> f64 {
    let union = left.union(&right);
    union.width() * union.height() - left.width() * left.height() - right.width() * right.height()
}

fn layout_bbox_intersects(left: BBox, right: BBox) -> bool {
    left.x1 > right.x0 && right.x1 > left.x0 && left.bottom > right.top && right.bottom > left.top
}

struct CompatibleLayoutPair {
    skip_intervening_check: bool,
    distance: f64,
    left_id: usize,
    right_id: usize,
    left: usize,
    right: usize,
}

impl PartialEq for CompatibleLayoutPair {
    fn eq(&self, other: &Self) -> bool {
        compatible_layout_pair_cmp(self, other) == std::cmp::Ordering::Equal
    }
}

impl Eq for CompatibleLayoutPair {}

impl PartialOrd for CompatibleLayoutPair {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CompatibleLayoutPair {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        compatible_layout_pair_cmp(other, self)
    }
}

fn compatible_layout_pair_cmp(
    left: &CompatibleLayoutPair,
    right: &CompatibleLayoutPair,
) -> std::cmp::Ordering {
    left.skip_intervening_check
        .cmp(&right.skip_intervening_check)
        .then_with(|| left.distance.total_cmp(&right.distance))
        .then_with(|| left.left_id.cmp(&right.left_id))
        .then_with(|| left.right_id.cmp(&right.right_id))
}

fn collect_layout_box_order(
    node: &CompatibleLayoutNode,
    boxes: &[CompatibleLayoutBox],
    boxes_flow: f64,
    ordered: &mut Vec<usize>,
) {
    match node {
        CompatibleLayoutNode::Box(index) => ordered.push(*index),
        CompatibleLayoutNode::Group {
            orientation,
            children,
            ..
        } => {
            let mut children = [children[0].clone(), children[1].clone()];
            children.sort_by(|left, right| {
                let left = layout_node_bbox(left, boxes);
                let right = layout_node_bbox(right, boxes);
                let (left_key, right_key) = match orientation {
                    CompatibleLayoutOrientation::Horizontal => (
                        (1.0 - boxes_flow) * left.x0
                            + (1.0 + boxes_flow) * (left.top + left.bottom),
                        (1.0 - boxes_flow) * right.x0
                            + (1.0 + boxes_flow) * (right.top + right.bottom),
                    ),
                    CompatibleLayoutOrientation::Vertical => (
                        -(1.0 + boxes_flow) * (left.x0 + left.x1) + (1.0 - boxes_flow) * left.top,
                        -(1.0 + boxes_flow) * (right.x0 + right.x1)
                            + (1.0 - boxes_flow) * right.top,
                    ),
                };
                left_key.total_cmp(&right_key)
            });
            for child in &children {
                collect_layout_box_order(child, boxes, boxes_flow, ordered);
            }
        }
    }
}

fn compatible_layout_box_order(
    boxes: &[CompatibleLayoutBox],
    boxes_flow: Option<f64>,
) -> Vec<usize> {
    if boxes_flow.is_none() {
        let mut ordered: Vec<usize> = (0..boxes.len()).collect();
        ordered.sort_by(|left, right| {
            let left_box = &boxes[*left];
            let right_box = &boxes[*right];
            let left_key = match left_box.orientation {
                CompatibleLayoutOrientation::Vertical => {
                    (0, -left_box.bbox.x1, left_box.bbox.bottom)
                }
                CompatibleLayoutOrientation::Horizontal => {
                    (1, left_box.bbox.bottom, left_box.bbox.x0)
                }
            };
            let right_key = match right_box.orientation {
                CompatibleLayoutOrientation::Vertical => {
                    (0, -right_box.bbox.x1, right_box.bbox.bottom)
                }
                CompatibleLayoutOrientation::Horizontal => {
                    (1, right_box.bbox.bottom, right_box.bbox.x0)
                }
            };
            left_key
                .0
                .cmp(&right_key.0)
                .then_with(|| left_key.1.total_cmp(&right_key.1))
                .then_with(|| left_key.2.total_cmp(&right_key.2))
        });
        return ordered;
    }
    if boxes.len() < 2 {
        return (0..boxes.len()).collect();
    }

    let mut nodes: Vec<CompatibleLayoutNode> =
        (0..boxes.len()).map(CompatibleLayoutNode::Box).collect();
    let mut node_ids: Vec<usize> = (0..boxes.len()).collect();
    let mut active = vec![true; boxes.len()];
    let mut pairs = std::collections::BinaryHeap::new();
    for left in 0..boxes.len() {
        for right in (left + 1)..boxes.len() {
            pairs.push(CompatibleLayoutPair {
                skip_intervening_check: false,
                distance: layout_bbox_distance(boxes[left].bbox, boxes[right].bbox),
                left_id: node_ids[left],
                right_id: node_ids[right],
                left,
                right,
            });
        }
    }

    while let Some(mut pair) = pairs.pop() {
        if !active[pair.left] || !active[pair.right] {
            continue;
        }
        let union = layout_node_bbox(&nodes[pair.left], boxes)
            .union(&layout_node_bbox(&nodes[pair.right], boxes));
        if !pair.skip_intervening_check
            && active.iter().enumerate().any(|(index, is_active)| {
                *is_active
                    && index != pair.left
                    && index != pair.right
                    && layout_bbox_intersects(layout_node_bbox(&nodes[index], boxes), union)
            })
        {
            pair.skip_intervening_check = true;
            pairs.push(pair);
            continue;
        }

        let orientation = if layout_node_orientation(&nodes[pair.left], boxes)
            == CompatibleLayoutOrientation::Vertical
            || layout_node_orientation(&nodes[pair.right], boxes)
                == CompatibleLayoutOrientation::Vertical
        {
            CompatibleLayoutOrientation::Vertical
        } else {
            CompatibleLayoutOrientation::Horizontal
        };
        let group = CompatibleLayoutNode::Group {
            bbox: union,
            orientation,
            children: Box::new([nodes[pair.left].clone(), nodes[pair.right].clone()]),
        };
        active[pair.left] = false;
        active[pair.right] = false;
        let group_index = nodes.len();
        let group_id = node_ids.len();
        for other in 0..nodes.len() {
            if active[other] {
                pairs.push(CompatibleLayoutPair {
                    skip_intervening_check: false,
                    distance: layout_bbox_distance(
                        layout_node_bbox(&group, boxes),
                        layout_node_bbox(&nodes[other], boxes),
                    ),
                    left_id: group_id,
                    right_id: node_ids[other],
                    left: group_index,
                    right: other,
                });
            }
        }
        nodes.push(group);
        node_ids.push(group_id);
        active.push(true);
    }

    let mut ordered = Vec::with_capacity(boxes.len());
    let boxes_flow = boxes_flow.expect("handled None above");
    for (index, is_active) in active.iter().enumerate() {
        if *is_active {
            collect_layout_box_order(&nodes[index], boxes, boxes_flow, &mut ordered);
        }
    }
    ordered
}

struct CompatibleLayoutDictContext {
    page_number: usize,
    raw_width: f64,
    raw_height: f64,
    public_height: f64,
    height_correction: f64,
    initial_doctop: f64,
}

fn compatible_layout_object_to_dict(
    py: Python<'_>,
    bbox: BBox,
    text: &str,
    object_type: &str,
    context: &CompatibleLayoutDictContext,
) -> PyResult<PyObject> {
    let x0 = bbox.x0;
    let top = bbox.top - context.height_correction;
    let x1 = bbox.x1;
    let bottom = bbox.bottom - context.height_correction;
    let y0 = context.public_height - bottom;
    let y1 = context.public_height - top;
    let dict = PyDict::new(py);
    dict.set_item("x0", x0)?;
    dict.set_item("y0", y0)?;
    dict.set_item("x1", x1)?;
    dict.set_item("y1", y1)?;
    dict.set_item("width", x1 - x0)?;
    dict.set_item("height", bottom - top)?;
    dict.set_item("object_type", object_type)?;
    dict.set_item("page_number", context.page_number)?;
    dict.set_item("text", text)?;
    dict.set_item("top", top)?;
    dict.set_item("bottom", bottom)?;
    dict.set_item("doctop", context.initial_doctop + top)?;
    Ok(dict.into_any().unbind())
}

fn compatible_layout_objects(
    py: Python<'_>,
    chars: &[Char],
    params: &Bound<'_, PyDict>,
    context: &CompatibleLayoutDictContext,
) -> PyResult<CompatibleLayoutObjects> {
    let line_overlap = laparams_number(params, "line_overlap", 0.5)?;
    let char_margin = laparams_number(params, "char_margin", 2.0)?;
    let line_margin = laparams_number(params, "line_margin", 0.5)?;
    let word_margin = laparams_number(params, "word_margin", 0.1)?;
    let detect_vertical = params
        .get_item("detect_vertical")?
        .map_or(Ok(false), |value| value.is_truthy())?;
    let boxes_flow = match params.get_item("boxes_flow")? {
        Some(value) if value.is_none() => None,
        Some(value) => Some(value.extract::<f64>()?),
        None => Some(0.5),
    };
    let lines = compatible_layout_lines(
        chars,
        line_overlap,
        char_margin,
        word_margin,
        detect_vertical,
    );
    let boxes = compatible_layout_boxes(&lines, context.raw_width, context.raw_height, line_margin);
    let box_order = compatible_layout_box_order(&boxes, boxes_flow);
    let mut horizontal_boxes = Vec::new();
    let mut vertical_boxes = Vec::new();
    let mut horizontal_lines = Vec::new();
    let mut vertical_lines = Vec::new();
    let mut ordered_char_indices = Vec::with_capacity(chars.len());
    let mut family_order = Vec::new();
    let mut push_family = |family| {
        if !family_order.contains(&family) {
            family_order.push(family);
        }
    };

    for box_index in box_order {
        let layout_box = &boxes[box_index];
        let (box_family, line_family) = match layout_box.orientation {
            CompatibleLayoutOrientation::Horizontal => ("textboxhorizontal", "textlinehorizontal"),
            CompatibleLayoutOrientation::Vertical => ("textboxvertical", "textlinevertical"),
        };
        push_family(box_family);
        let box_object = compatible_layout_object_to_dict(
            py,
            layout_box.bbox,
            &layout_box.text,
            box_family,
            context,
        )?;
        match layout_box.orientation {
            CompatibleLayoutOrientation::Horizontal => horizontal_boxes.push(box_object),
            CompatibleLayoutOrientation::Vertical => vertical_boxes.push(box_object),
        }
        for &line_index in &layout_box.line_indices {
            let line = &lines[line_index];
            push_family(line_family);
            let line_object =
                compatible_layout_object_to_dict(py, line.bbox, &line.text, line_family, context)?;
            match line.orientation {
                CompatibleLayoutOrientation::Horizontal => horizontal_lines.push(line_object),
                CompatibleLayoutOrientation::Vertical => vertical_lines.push(line_object),
            }
            push_family("char");
            ordered_char_indices.extend(&line.char_indices);
        }
    }

    for line in lines.iter().filter(|line| line.empty) {
        let line_family = match line.orientation {
            CompatibleLayoutOrientation::Horizontal => "textlinehorizontal",
            CompatibleLayoutOrientation::Vertical => "textlinevertical",
        };
        push_family(line_family);
        let line_object =
            compatible_layout_object_to_dict(py, line.bbox, &line.text, line_family, context)?;
        match line.orientation {
            CompatibleLayoutOrientation::Horizontal => horizontal_lines.push(line_object),
            CompatibleLayoutOrientation::Vertical => vertical_lines.push(line_object),
        }
        push_family("char");
        ordered_char_indices.extend(&line.char_indices);
    }
    Ok(CompatibleLayoutObjects {
        horizontal_boxes,
        vertical_boxes,
        horizontal_lines,
        vertical_lines,
        ordered_char_indices,
        family_order,
    })
}

fn word_to_dict(py: Python<'_>, word: &Word) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("text", &word.text)?;
    dict.set_item("x0", word.bbox.x0)?;
    dict.set_item("top", word.bbox.top)?;
    dict.set_item("x1", word.bbox.x1)?;
    dict.set_item("bottom", word.bbox.bottom)?;
    dict.set_item("doctop", word.doctop)?;
    dict.set_item("width", word.bbox.width())?;
    dict.set_item("height", word.bbox.height())?;
    // The current Python API exposes only the upstream default word directions:
    // upright characters use `char_dir="ltr"`, while rotated characters use
    // the flipped `char_dir_rotated="ttb"`. Keep the core's richer TRM-derived
    // direction on `Word`, but normalize the compatibility dictionary boundary.
    let direction = match word.chars.first() {
        Some(character) if character.upright => ::pdfplumber::TextDirection::Ltr,
        Some(_) => ::pdfplumber::TextDirection::Ttb,
        None => word.direction,
    };
    dict.set_item(
        "direction",
        match direction {
            ::pdfplumber::TextDirection::Ltr => "ltr",
            ::pdfplumber::TextDirection::Rtl => "rtl",
            ::pdfplumber::TextDirection::Ttb => "ttb",
            ::pdfplumber::TextDirection::Btt => "btt",
        },
    )?;
    Ok(dict.into_any().unbind())
}

fn line_to_dict(
    py: Python<'_>,
    line: &Line,
    page_number: usize,
    page_height: f64,
    initial_doctop: f64,
) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("object_type", "line")?;
    dict.set_item("page_number", page_number)?;
    dict.set_item("x0", line.x0)?;
    dict.set_item("top", line.top)?;
    dict.set_item("x1", line.x1)?;
    dict.set_item("bottom", line.bottom)?;
    set_compatible_bbox_geometry(
        &dict,
        line.x0,
        line.top,
        line.x1,
        line.bottom,
        page_height,
        Some(initial_doctop),
    )?;
    dict.set_item(NativeObjectField::LineWidth.python_key(), line.line_width)?;
    dict.set_item(
        NativeObjectField::StrokeColor.python_key(),
        color_to_py(py, &line.stroke_color),
    )?;
    dict.set_item(
        "orientation",
        match line.orientation {
            ::pdfplumber::Orientation::Horizontal => "horizontal",
            ::pdfplumber::Orientation::Vertical => "vertical",
            ::pdfplumber::Orientation::Diagonal => "diagonal",
        },
    )?;
    Ok(dict.into_any().unbind())
}

fn rect_to_dict(
    py: Python<'_>,
    rect: &Rect,
    page_number: usize,
    page_height: f64,
    initial_doctop: f64,
) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("object_type", "rect")?;
    dict.set_item("page_number", page_number)?;
    dict.set_item("x0", rect.x0)?;
    dict.set_item("top", rect.top)?;
    dict.set_item("x1", rect.x1)?;
    dict.set_item("bottom", rect.bottom)?;
    set_compatible_bbox_geometry(
        &dict,
        rect.x0,
        rect.top,
        rect.x1,
        rect.bottom,
        page_height,
        Some(initial_doctop),
    )?;
    dict.set_item(NativeObjectField::LineWidth.python_key(), rect.line_width)?;
    dict.set_item("stroke", rect.stroke)?;
    dict.set_item("fill", rect.fill)?;
    dict.set_item(
        NativeObjectField::StrokeColor.python_key(),
        color_to_py(py, &rect.stroke_color),
    )?;
    dict.set_item(
        NativeObjectField::FillColor.python_key(),
        color_to_py(py, &rect.fill_color),
    )?;
    Ok(dict.into_any().unbind())
}

fn curve_to_dict(
    py: Python<'_>,
    curve: &Curve,
    page_number: usize,
    page_height: f64,
    initial_doctop: f64,
) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("object_type", "curve")?;
    dict.set_item("page_number", page_number)?;
    dict.set_item("x0", curve.x0)?;
    dict.set_item("top", curve.top)?;
    dict.set_item("x1", curve.x1)?;
    dict.set_item("bottom", curve.bottom)?;
    set_compatible_bbox_geometry(
        &dict,
        curve.x0,
        curve.top,
        curve.x1,
        curve.bottom,
        page_height,
        Some(initial_doctop),
    )?;
    dict.set_item("pts", &curve.pts)?;
    dict.set_item(NativeObjectField::LineWidth.python_key(), curve.line_width)?;
    dict.set_item("stroke", curve.stroke)?;
    dict.set_item("fill", curve.fill)?;
    dict.set_item(
        NativeObjectField::StrokeColor.python_key(),
        color_to_py(py, &curve.stroke_color),
    )?;
    dict.set_item(
        NativeObjectField::FillColor.python_key(),
        color_to_py(py, &curve.fill_color),
    )?;
    Ok(dict.into_any().unbind())
}

fn native_image_to_dict(py: Python<'_>, img: &Image) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("object_type", "image")?;
    dict.set_item("x0", img.x0)?;
    dict.set_item("top", img.top)?;
    dict.set_item("x1", img.x1)?;
    dict.set_item("bottom", img.bottom)?;
    dict.set_item("width", img.width)?;
    dict.set_item("height", img.height)?;
    dict.set_item("name", &img.name)?;
    dict.set_item("src_width", img.src_width)?;
    dict.set_item("src_height", img.src_height)?;
    dict.set_item("bits_per_component", img.bits_per_component)?;
    dict.set_item("color_space", img.color_space.as_deref())?;
    Ok(dict.into_any().unbind())
}

fn page_image_to_dict(
    py: Python<'_>,
    img: &Image,
    page_number: usize,
    page_height: f64,
    initial_doctop: f64,
) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("object_type", "image")?;
    dict.set_item("page_number", page_number)?;
    dict.set_item("x0", img.x0)?;
    dict.set_item("top", img.top)?;
    dict.set_item("x1", img.x1)?;
    dict.set_item("bottom", img.bottom)?;
    dict.set_item("width", img.width)?;
    dict.set_item("height", img.height)?;
    dict.set_item("name", &img.name)?;
    dict.set_item(
        NativeObjectField::SourceDimensions.python_key(),
        (img.src_width, img.src_height),
    )?;
    dict.set_item(
        NativeObjectField::BitsPerComponent.python_key(),
        img.bits_per_component,
    )?;
    dict.set_item(
        NativeObjectField::ColorSpace.python_key(),
        img.color_space.as_deref(),
    )?;
    dict.set_item("y0", page_height - img.bottom)?;
    dict.set_item("y1", page_height - img.top)?;
    dict.set_item("doctop", initial_doctop + img.top)?;
    Ok(dict.into_any().unbind())
}

fn annotation_to_dict(
    py: Python<'_>,
    annotation: &Annotation,
    page_number: usize,
    page_height: f64,
    initial_doctop: f64,
    uri: Option<&str>,
) -> PyResult<PyObject> {
    let x0 = annotation.bbox.x0;
    let y0 = annotation.bbox.top;
    let x1 = annotation.bbox.x1;
    let y1 = annotation.bbox.bottom;
    let top = page_height - y1;
    let bottom = page_height - y0;

    let dict = PyDict::new(py);
    dict.set_item("page_number", page_number)?;
    dict.set_item("object_type", "annot")?;
    dict.set_item("x0", x0)?;
    dict.set_item("y0", y0)?;
    dict.set_item("x1", x1)?;
    dict.set_item("y1", y1)?;
    dict.set_item("doctop", initial_doctop + top)?;
    dict.set_item("top", top)?;
    dict.set_item("bottom", bottom)?;
    dict.set_item("width", x1 - x0)?;
    dict.set_item("height", bottom - top)?;
    dict.set_item("uri", uri)?;
    dict.set_item("title", annotation.author.as_deref())?;
    dict.set_item("contents", annotation.contents.as_deref())?;
    dict.set_item("data", PyDict::new(py))?;
    Ok(dict.into_any().unbind())
}

fn struct_element_to_dict(
    py: Python<'_>,
    element: &StructElement,
    include_page_number: bool,
) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("type", &element.element_type)?;
    if let Some(lang) = &element.lang {
        dict.set_item("lang", lang)?;
    }
    if let Some(alt_text) = &element.alt_text {
        dict.set_item("alt_text", alt_text)?;
    }
    if let Some(actual_text) = &element.actual_text {
        dict.set_item("actual_text", actual_text)?;
    }
    if include_page_number {
        if let Some(page_index) = element.page_index {
            dict.set_item("page_number", page_index + 1)?;
        }
    }
    if !element.mcids.is_empty() {
        dict.set_item("mcids", &element.mcids)?;
    }
    if !element.children.is_empty() {
        let children = element
            .children
            .iter()
            .map(|child| struct_element_to_dict(py, child, include_page_number))
            .collect::<PyResult<Vec<_>>>()?;
        dict.set_item("children", PyList::new(py, children)?)?;
    }
    Ok(dict.into_any().unbind())
}

fn search_match_to_dict(py: Python<'_>, m: &SearchMatch) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("text", &m.text)?;
    dict.set_item("x0", m.bbox.x0)?;
    dict.set_item("top", m.bbox.top)?;
    dict.set_item("x1", m.bbox.x1)?;
    dict.set_item("bottom", m.bbox.bottom)?;
    dict.set_item("page_number", m.page_number)?;
    Ok(dict.into_any().unbind())
}

fn bookmark_to_dict(py: Python<'_>, bm: &Bookmark) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("title", &bm.title)?;
    dict.set_item("level", bm.level)?;
    dict.set_item("page_number", bm.page_number)?;
    dict.set_item("dest_top", bm.dest_top)?;
    Ok(dict.into_any().unbind())
}

fn form_field_to_dict(py: Python<'_>, field: &FormField) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("name", &field.name)?;
    dict.set_item("field_type", field.field_type.to_string())?;
    dict.set_item("value", field.value.as_deref())?;
    dict.set_item("default_value", field.default_value.as_deref())?;
    dict.set_item(
        "bbox",
        (
            field.bbox.x0,
            field.bbox.top,
            field.bbox.x1,
            field.bbox.bottom,
        ),
    )?;
    dict.set_item("options", &field.options)?;
    dict.set_item("flags", field.flags)?;
    dict.set_item("page_index", field.page_index)?;
    Ok(dict.into_any().unbind())
}

fn signature_to_dict(py: Python<'_>, signature: &SignatureInfo) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("signer_name", signature.signer_name.as_deref())?;
    dict.set_item("sign_date", signature.sign_date.as_deref())?;
    dict.set_item("reason", signature.reason.as_deref())?;
    dict.set_item("location", signature.location.as_deref())?;
    dict.set_item("contact_info", signature.contact_info.as_deref())?;
    dict.set_item("is_signed", signature.is_signed)?;
    Ok(dict.into_any().unbind())
}

fn validation_issue_to_dict(py: Python<'_>, issue: &ValidationIssue) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("severity", issue.severity.to_string())?;
    dict.set_item("code", &issue.code)?;
    dict.set_item("message", &issue.message)?;
    dict.set_item("location", issue.location.as_deref())?;
    Ok(dict.into_any().unbind())
}

fn extracted_image_to_dict(
    py: Python<'_>,
    image: &Image,
    content: &ImageContent,
) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("image", native_image_to_dict(py, image)?)?;
    dict.set_item("data", PyBytes::new(py, &content.data))?;
    dict.set_item("format", content.format.extension())?;
    dict.set_item("width", content.width)?;
    dict.set_item("height", content.height)?;
    Ok(dict.into_any().unbind())
}

/// Rust-native document capabilities isolated from the compatibility surface.
#[pyclass(name = "RustPDF", module = "pdfplumber._native")]
struct PyRustPdf {
    inner: Arc<Pdf>,
}

#[pymethods]
impl PyRustPdf {
    /// Document bookmarks using Rust-native zero-based page numbers.
    fn bookmarks(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .bookmarks()
            .iter()
            .map(|bookmark| bookmark_to_dict(py, bookmark))
            .collect()
    }

    /// AcroForm fields using Rust-native field and page-index semantics.
    fn form_fields(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .form_fields()
            .map_err(to_py_err)?
            .iter()
            .map(|field| form_field_to_dict(py, field))
            .collect()
    }

    /// Digital-signature metadata. This does not verify signatures.
    fn signatures(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .signatures()
            .map_err(to_py_err)?
            .iter()
            .map(|signature| signature_to_dict(py, signature))
            .collect()
    }

    /// Validate the native PDF structure and return all detected issues.
    fn validate(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .validate()
            .map_err(to_py_err)?
            .iter()
            .map(|issue| validation_issue_to_dict(py, issue))
            .collect()
    }

    /// Extract image metadata and bytes from a zero-based native page index.
    fn extract_images(&self, py: Python<'_>, page_index: usize) -> PyResult<Vec<PyObject>> {
        self.inner
            .extract_images_with_content(page_index)
            .map_err(to_py_err)?
            .iter()
            .map(|(image, content)| extracted_image_to_dict(py, image, content))
            .collect()
    }
}

#[pyclass(name = "PDFObjRef", module = "pdfminer.pdftypes")]
struct PyMetadataReference {
    object_number: u32,
    _generation_number: u16,
}

#[pymethods]
impl PyMetadataReference {
    fn __repr__(&self) -> String {
        format!("<PDFObjRef:{}>", self.object_number)
    }

    fn resolve(&self) -> PyResult<PyObject> {
        Err(PyRecursionError::new_err(
            "maximum recursion depth exceeded",
        ))
    }
}

#[pyclass(name = "PDFStream", module = "pdfminer.pdftypes")]
struct PyMetadataStream {
    #[pyo3(get)]
    attrs: PyObject,
    #[pyo3(get)]
    rawdata: PyObject,
}

fn metadata_reference_to_object(
    py: Python<'_>,
    reference: &MetadataReference,
) -> PyResult<PyObject> {
    Ok(Py::new(
        py,
        PyMetadataReference {
            object_number: reference.object_number,
            _generation_number: reference.generation_number,
        },
    )?
    .into_any())
}

fn metadata_dictionary_to_object(
    py: Python<'_>,
    entries: &[(String, MetadataValue)],
) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    for (key, value) in entries {
        dict.set_item(key, metadata_value_to_object(py, value)?)?;
    }
    Ok(dict.into_any().unbind())
}

fn metadata_value_to_object(py: Python<'_>, value: &MetadataValue) -> PyResult<PyObject> {
    match value {
        MetadataValue::Null => Ok(py.None()),
        MetadataValue::Boolean(value) => Ok(PyBool::new(py, *value).to_owned().into_any().unbind()),
        MetadataValue::Integer(value) => Ok(value.into_pyobject(py)?.into_any().unbind()),
        MetadataValue::Real(value) => Ok(value.into_pyobject(py)?.into_any().unbind()),
        MetadataValue::String(value) => Ok(PyString::new(py, value).into_any().unbind()),
        MetadataValue::Array(values) => {
            let values = values
                .iter()
                .map(|value| metadata_value_to_object(py, value))
                .collect::<PyResult<Vec<_>>>()?;
            Ok(PyList::new(py, values)?.into_any().unbind())
        }
        MetadataValue::Dictionary(entries) => metadata_dictionary_to_object(py, entries),
        MetadataValue::Reference(reference) => metadata_reference_to_object(py, reference),
        MetadataValue::Stream { dictionary, data } => {
            let attrs = metadata_dictionary_to_object(py, dictionary)?;
            let rawdata = PyBytes::new(py, data).into_any().unbind();
            Ok(Py::new(py, PyMetadataStream { attrs, rawdata })?.into_any())
        }
    }
}

fn raw_metadata_to_dict(py: Python<'_>, metadata: &RawDocumentMetadata) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    for entry in &metadata.entries {
        dict.set_item(&entry.key, metadata_value_to_object(py, &entry.value)?)?;
    }
    Ok(dict.into_any().unbind())
}

fn log_metadata_warnings(py: Python<'_>, pdf: &Pdf) -> PyResult<()> {
    let logger = py
        .import("logging")?
        .call_method1("getLogger", ("pdfplumber.pdf",))?;
    for entry in &pdf.raw_metadata().entries {
        if let Some(error) = &entry.resolution_error {
            logger.call_method1(
                "warning",
                (format!(
                    "[WARNING] Metadata key \"{}\" could not be parsed due to exception: {}",
                    entry.key, error
                ),),
            )?;
        }
    }
    Ok(())
}

fn parse_bbox_tuple(bbox: (f64, f64, f64, f64)) -> BBox {
    BBox::new(bbox.0, bbox.1, bbox.2, bbox.3)
}

fn compatible_geometry_number(value: f64) -> f64 {
    let value = (value as f32).to_string().parse().unwrap_or(value);
    if value == 0.0 { 0.0 } else { value }
}

#[derive(Clone, Copy)]
struct CompatiblePageBox {
    bbox: BBox,
    integer_flags: [bool; 4],
}

fn normalized_page_box(
    page_box: BBox,
    integer_flags: [bool; 4],
    rotation: i32,
) -> CompatiblePageBox {
    let (x0, x0_is_integer, x1, x1_is_integer) = if page_box.x0 <= page_box.x1 {
        (page_box.x0, integer_flags[0], page_box.x1, integer_flags[2])
    } else {
        (page_box.x1, integer_flags[2], page_box.x0, integer_flags[0])
    };
    let (y0, y0_is_integer, y1, y1_is_integer) = if page_box.top <= page_box.bottom {
        (
            page_box.top,
            integer_flags[1],
            page_box.bottom,
            integer_flags[3],
        )
    } else {
        (
            page_box.bottom,
            integer_flags[3],
            page_box.top,
            integer_flags[1],
        )
    };
    let (bbox, integer_flags) = if matches!(rotation, 90 | 270) {
        (
            BBox::new(y0, x0, y1, x1),
            [y0_is_integer, x0_is_integer, y1_is_integer, x1_is_integer],
        )
    } else {
        (
            BBox::new(x0, y0, x1, y1),
            [x0_is_integer, y0_is_integer, x1_is_integer, y1_is_integer],
        )
    };
    CompatiblePageBox {
        bbox,
        integer_flags,
    }
}

fn invert_page_box(
    page_box: CompatiblePageBox,
    media_height: f64,
    media_height_is_integer: bool,
) -> CompatiblePageBox {
    CompatiblePageBox {
        bbox: BBox::new(
            page_box.bbox.x0,
            media_height - page_box.bbox.bottom,
            page_box.bbox.x1,
            media_height - page_box.bbox.top,
        ),
        integer_flags: [
            page_box.integer_flags[0],
            media_height_is_integer && page_box.integer_flags[3],
            page_box.integer_flags[2],
            media_height_is_integer && page_box.integer_flags[1],
        ],
    }
}

fn compatible_page_boxes(
    media_box: BBox,
    media_box_integer_flags: [bool; 4],
    crop_box: Option<BBox>,
    crop_box_integer_flags: Option<[bool; 4]>,
    rotation: i32,
) -> (CompatiblePageBox, CompatiblePageBox) {
    let normalized_media_box = normalized_page_box(media_box, media_box_integer_flags, rotation);
    let media_height = normalized_media_box.bbox.bottom - normalized_media_box.bbox.top;
    let media_height_is_integer =
        normalized_media_box.integer_flags[3] && normalized_media_box.integer_flags[1];
    let normalized_crop_box = normalized_page_box(
        crop_box.unwrap_or(media_box),
        crop_box_integer_flags.unwrap_or(media_box_integer_flags),
        rotation,
    );
    (
        invert_page_box(normalized_media_box, media_height, media_height_is_integer),
        invert_page_box(normalized_crop_box, media_height, media_height_is_integer),
    )
}

fn compatible_optional_page_box(
    media_box: BBox,
    media_box_integer_flags: [bool; 4],
    page_box: Option<BBox>,
    page_box_integer_flags: Option<[bool; 4]>,
    rotation: i32,
) -> Option<CompatiblePageBox> {
    let media_box = normalized_page_box(media_box, media_box_integer_flags, rotation);
    let media_height = media_box.bbox.bottom - media_box.bbox.top;
    let media_height_is_integer = media_box.integer_flags[3] && media_box.integer_flags[1];
    page_box
        .zip(page_box_integer_flags)
        .map(|(page_box, integer_flags)| {
            invert_page_box(
                normalized_page_box(page_box, integer_flags, rotation),
                media_height,
                media_height_is_integer,
            )
        })
}

fn compatible_number_to_object(py: Python<'_>, value: f64, is_integer: bool) -> PyObject {
    if is_integer {
        (value as i64)
            .into_pyobject(py)
            .unwrap()
            .into_any()
            .unbind()
    } else {
        compatible_geometry_number(value)
            .into_pyobject(py)
            .unwrap()
            .into_any()
            .unbind()
    }
}

fn compatible_bbox_to_object(py: Python<'_>, page_box: CompatiblePageBox) -> PyResult<PyObject> {
    let values = [
        compatible_number_to_object(py, page_box.bbox.x0, page_box.integer_flags[0]),
        compatible_number_to_object(py, page_box.bbox.top, page_box.integer_flags[1]),
        compatible_number_to_object(py, page_box.bbox.x1, page_box.integer_flags[2]),
        compatible_number_to_object(py, page_box.bbox.bottom, page_box.integer_flags[3]),
    ];
    Ok(PyTuple::new(py, values)?.into_any().unbind())
}

fn compatible_point2coord(
    py: Python<'_>,
    page: &Bound<'_, PyAny>,
    point: &Bound<'_, PyAny>,
) -> PyResult<(PyObject, PyObject)> {
    let operator = py.import("operator")?;
    let add = operator.getattr("add")?;
    let subtract = operator.getattr("sub")?;

    let x_origin = page.getattr("mediabox")?.get_item(0)?;
    let x = add.call1((x_origin, point.get_item(0)?))?.unbind();

    let y_origin = page.getattr("mediabox")?.get_item(1)?;
    let y_from_bottom = add.call1((y_origin, page.getattr("height")?))?;
    let y = subtract
        .call1((y_from_bottom, point.get_item(1)?))?
        .unbind();

    Ok((x, y))
}

fn compatible_page_repr(py: Python<'_>, page: &Bound<'_, PyAny>) -> PyResult<String> {
    PyString::new(py, "<Page:{}>")
        .call_method1("format", (page.getattr("page_number")?,))?
        .extract()
}

fn compatible_page_object_list(
    py: Python<'_>,
    page: &Bound<'_, PyAny>,
    kind: &str,
) -> PyResult<PyObject> {
    Ok(page
        .getattr("objects")?
        .call_method1("get", (kind, PyList::empty(py)))?
        .unbind())
}

static PAGE_CACHED_PROPERTIES: GILOnceCell<Py<PyList>> = GILOnceCell::new();

fn compatible_page_cached_properties(py: Python<'_>) -> PyResult<Py<PyList>> {
    let properties = PAGE_CACHED_PROPERTIES.get_or_try_init(py, || {
        Ok::<_, PyErr>(
            PyList::new(
                py,
                [
                    "_rect_edges",
                    "_curve_edges",
                    "_edges",
                    "_objects",
                    "_layout",
                ],
            )?
            .unbind(),
        )
    })?;
    Ok(properties.clone_ref(py))
}

fn flush_compatible_page_cache(
    page: &Bound<'_, PyAny>,
    properties: Option<&Bound<'_, PyAny>>,
    mut clear_layout: impl FnMut() -> PyResult<()>,
) -> PyResult<()> {
    let properties = match properties {
        Some(properties) => properties.clone(),
        None => page.getattr("cached_properties")?,
    };

    for property in properties.try_iter()? {
        let property = property?;
        if !property.is_instance_of::<PyString>() {
            let type_name = property.get_type().name()?.to_string_lossy().into_owned();
            return Err(PyTypeError::new_err(format!(
                "attribute name must be string, not '{type_name}'"
            )));
        }

        let property = property.extract::<String>()?;
        if page.hasattr(property.as_str())? {
            page.delattr(property.as_str())?;
        }
        if property == "_layout" {
            clear_layout()?;
        }
    }
    Ok(())
}

fn compatible_page_attribute<'py>(
    page: &Bound<'py, PyAny>,
    attribute: &str,
) -> PyResult<Bound<'py, PyAny>> {
    match page.getattr(attribute) {
        Ok(value) => Ok(value),
        Err(error) if error.is_instance_of::<PyAttributeError>(page.py()) => Err(
            PyAttributeError::new_err(format!("'Page' object has no attribute '{attribute}'")),
        ),
        Err(error) => Err(error),
    }
}

fn parse_point2coord_arg(
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<PyObject> {
    if args.len() > 1 {
        return Err(PyTypeError::new_err(format!(
            "Page.point2coord() takes 2 positional arguments but {} were given",
            args.len() + 1
        )));
    }

    let mut point = args.get_item(0).ok().map(Bound::unbind);
    if let Some(kwargs) = kwargs {
        for (key, value) in kwargs.iter() {
            let key = key.extract::<String>()?;
            if key != "pt" {
                return Err(PyTypeError::new_err(format!(
                    "Page.point2coord() got an unexpected keyword argument '{key}'"
                )));
            }
            if point.is_some() {
                return Err(PyTypeError::new_err(
                    "Page.point2coord() got multiple values for argument 'pt'",
                ));
            }
            point = Some(value.unbind());
        }
    }

    point.ok_or_else(|| {
        PyTypeError::new_err("Page.point2coord() missing 1 required positional argument: 'pt'")
    })
}

fn initial_doctop_to_object(py: Python<'_>, value: f64, is_integer: bool) -> PyObject {
    compatible_number_to_object(py, value, is_integer)
}

fn container_to_json(
    py: Python<'_>,
    data: PyObject,
    stream: Option<&Bound<'_, PyAny>>,
    include_attrs: Option<&Bound<'_, PyAny>>,
    exclude_attrs: Option<&Bound<'_, PyAny>>,
    precision: Option<&Bound<'_, PyAny>>,
    indent: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyObject> {
    let serializer_kwargs = PyDict::new(py);
    if let Some(include_attrs) = include_attrs {
        serializer_kwargs.set_item("include_attrs", include_attrs)?;
    }
    if let Some(exclude_attrs) = exclude_attrs {
        serializer_kwargs.set_item("exclude_attrs", exclude_attrs)?;
    }
    if let Some(precision) = precision {
        serializer_kwargs.set_item("precision", precision)?;
    }
    let serializer = py
        .import("pdfplumber.convert")?
        .getattr("Serializer")?
        .call((), Some(&serializer_kwargs))?;
    let serialized = serializer.call_method1("serialize", (data,))?;

    let json_kwargs = PyDict::new(py);
    if let Some(indent) = indent {
        json_kwargs.set_item("indent", indent)?;
    }
    let json = py.import("json")?;
    let result = match stream {
        Some(stream) => json.call_method("dump", (&serialized, stream), Some(&json_kwargs))?,
        None => json.call_method("dumps", (&serialized,), Some(&json_kwargs))?,
    };
    Ok(result.unbind())
}

fn container_to_csv(
    py: Python<'_>,
    pages: PyObject,
    stream: Option<&Bound<'_, PyAny>>,
    include_attrs: Option<&Bound<'_, PyAny>>,
    exclude_attrs: Option<&Bound<'_, PyAny>>,
    precision: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyObject> {
    let kwargs = PyDict::new(py);
    if let Some(stream) = stream {
        kwargs.set_item("stream", stream)?;
    }
    if let Some(precision) = precision {
        kwargs.set_item("precision", precision)?;
    }
    if let Some(include_attrs) = include_attrs {
        kwargs.set_item("include_attrs", include_attrs)?;
    }
    if let Some(exclude_attrs) = exclude_attrs {
        kwargs.set_item("exclude_attrs", exclude_attrs)?;
    }
    let result = py
        .import("pdfplumber.convert")?
        .getattr("serialize_csv")?
        .call((pages,), Some(&kwargs))?;
    Ok(result.unbind())
}

fn validate_laparams(py: Python<'_>, laparams: Option<PyObject>) -> PyResult<Option<PyObject>> {
    let Some(laparams) = laparams else {
        return Ok(None);
    };
    let raw = laparams.bind(py);
    let builtins = py.import("builtins")?;
    let mapping_type = py.import("collections.abc")?.getattr("Mapping")?;
    let is_mapping = builtins
        .getattr("isinstance")?
        .call1((raw, mapping_type))?
        .is_truthy()?;
    if !is_mapping {
        return Err(PyTypeError::new_err(
            "pdfminer.layout.LAParams() argument after ** must be a mapping",
        ));
    }
    let params_object = builtins.getattr("dict")?.call1((raw,))?;
    let params = params_object.downcast::<PyDict>()?;
    const SUPPORTED_KEYS: [&str; 7] = [
        "line_overlap",
        "char_margin",
        "line_margin",
        "word_margin",
        "boxes_flow",
        "detect_vertical",
        "all_texts",
    ];

    for (key, value) in params.iter() {
        let key: String = key.extract()?;
        if !SUPPORTED_KEYS.contains(&key.as_str()) {
            return Err(PyTypeError::new_err(format!(
                "LAParams.__init__() got an unexpected keyword argument '{key}'"
            )));
        }
        if key == "boxes_flow" && !value.is_none() {
            let boxes_flow = value.extract::<f64>().map_err(|_| {
                PyTypeError::new_err(
                    "LAParam boxes_flow should be None, or a number between -1 and +1",
                )
            })?;
            if !(-1.0..=1.0).contains(&boxes_flow) {
                return Err(PyValueError::new_err(
                    "LAParam boxes_flow should be None, or a number between -1 and +1",
                ));
            }
        }
    }

    Ok(Some(params.copy()?.into_any().unbind()))
}

fn parse_unicode_norm(py: Python<'_>, unicode_norm: Option<&PyObject>) -> PyResult<UnicodeNorm> {
    let Some(unicode_norm) = unicode_norm else {
        return Ok(UnicodeNorm::None);
    };
    let raw = unicode_norm.bind(py);
    py.import("unicodedata")?
        .getattr("normalize")?
        .call1((raw, "a"))?;
    let form = raw.extract::<String>()?;
    let norm = match form.as_str() {
        "NFC" => UnicodeNorm::Nfc,
        "NFD" => UnicodeNorm::Nfd,
        "NFKC" => UnicodeNorm::Nfkc,
        "NFKD" => UnicodeNorm::Nfkd,
        _ => return Err(PyValueError::new_err("invalid normalization form")),
    };
    Ok(norm)
}

fn table_rows_to_py(rows: &[Vec<::pdfplumber::Cell>]) -> Vec<Vec<Option<String>>> {
    rows.iter()
        .map(|row| row.iter().map(|cell| cell.text.clone()).collect())
        .collect()
}

// ---------------------------------------------------------------------------
// PyTable
// ---------------------------------------------------------------------------

/// A detected table from a PDF page.
#[pyclass(name = "Table")]
struct PyTable {
    inner: Table,
}

#[pymethods]
impl PyTable {
    /// Bounding box as (x0, top, x1, bottom).
    #[getter]
    fn bbox(&self) -> (f64, f64, f64, f64) {
        (
            self.inner.bbox.x0,
            self.inner.bbox.top,
            self.inner.bbox.x1,
            self.inner.bbox.bottom,
        )
    }

    /// Extract table content as list of rows, each row a list of cell text values.
    fn extract(&self) -> Vec<Vec<Option<String>>> {
        table_rows_to_py(&self.inner.rows)
    }

    /// Cells organized into rows as list[list[dict]].
    #[getter]
    fn rows(&self, py: Python<'_>) -> PyResult<PyObject> {
        let rows: Vec<Vec<PyObject>> = self
            .inner
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| {
                        let dict = PyDict::new(py);
                        dict.set_item("x0", cell.bbox.x0).unwrap();
                        dict.set_item("top", cell.bbox.top).unwrap();
                        dict.set_item("x1", cell.bbox.x1).unwrap();
                        dict.set_item("bottom", cell.bbox.bottom).unwrap();
                        dict.set_item("text", cell.text.as_deref()).unwrap();
                        dict.into_any().unbind()
                    })
                    .collect()
            })
            .collect();
        Ok(rows.into_pyobject(py)?.into_any().unbind())
    }

    /// Percentage of non-empty cells (0.0 to 1.0).
    #[getter]
    fn accuracy(&self) -> f64 {
        self.inner.accuracy()
    }
}

// ---------------------------------------------------------------------------
// PyCroppedPage
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum DerivedObjectTransform {
    Crop(BBox),
    Within(BBox),
    Outside(BBox),
}

fn bbox_overlap(left: BBox, right: BBox) -> Option<BBox> {
    let overlap = BBox::new(
        left.x0.max(right.x0),
        left.top.max(right.top),
        left.x1.min(right.x1),
        left.bottom.min(right.bottom),
    );
    let width = overlap.width();
    let height = overlap.height();
    (height >= 0.0 && width >= 0.0 && height + width > 0.0).then_some(overlap)
}

fn python_object_bbox(object: &Bound<'_, PyAny>) -> PyResult<BBox> {
    Ok(BBox::new(
        object.get_item("x0")?.extract()?,
        object.get_item("top")?.extract()?,
        object.get_item("x1")?.extract()?,
        object.get_item("bottom")?.extract()?,
    ))
}

fn transform_derived_objects<'py>(
    py: Python<'py>,
    values: &Bound<'py, PyAny>,
    transform: DerivedObjectTransform,
) -> PyResult<Bound<'py, PyList>> {
    let transformed = PyList::empty(py);
    let dict_constructor = if matches!(transform, DerivedObjectTransform::Crop(_)) {
        Some(py.import("builtins")?.getattr("dict")?)
    } else {
        None
    };

    for object in values.try_iter()? {
        let object = object?;
        let object_bbox = python_object_bbox(&object)?;
        let filter_bbox = match transform {
            DerivedObjectTransform::Crop(bbox)
            | DerivedObjectTransform::Within(bbox)
            | DerivedObjectTransform::Outside(bbox) => bbox,
        };
        let overlap = bbox_overlap(object_bbox, filter_bbox);

        match transform {
            DerivedObjectTransform::Crop(_) => {
                let Some(overlap) = overlap else {
                    continue;
                };
                let copied = dict_constructor
                    .as_ref()
                    .expect("crop transform has a dict constructor")
                    .call1((&object,))?
                    .downcast_into::<PyDict>()?;
                copied.set_item("x0", overlap.x0)?;
                copied.set_item("top", overlap.top)?;
                copied.set_item("x1", overlap.x1)?;
                copied.set_item("bottom", overlap.bottom)?;
                if copied.contains("doctop")? {
                    let doctop = object.get_item("doctop")?.extract::<f64>()?;
                    copied.set_item("doctop", doctop + overlap.top - object_bbox.top)?;
                }
                copied.set_item("width", overlap.width())?;
                copied.set_item("height", overlap.height())?;
                transformed.append(copied)?;
            }
            DerivedObjectTransform::Within(_) if overlap == Some(object_bbox) => {
                transformed.append(object)?;
            }
            DerivedObjectTransform::Outside(_) if overlap.is_none() => {
                transformed.append(object)?;
            }
            DerivedObjectTransform::Within(_) | DerivedObjectTransform::Outside(_) => {}
        }
    }

    Ok(transformed)
}

/// A spatially filtered view of a PDF page.
#[pyclass(name = "CroppedPage", dict)]
struct PyCroppedPage {
    inner: CroppedPage,
    object_transform: DerivedObjectTransform,
}

impl PyCroppedPage {
    fn from_parent(
        py: Python<'_>,
        inner: CroppedPage,
        parent_page: Py<PyAny>,
        root_page: Py<PyAny>,
        object_transform: DerivedObjectTransform,
    ) -> PyResult<Py<Self>> {
        let mediabox = parent_page.bind(py).getattr("mediabox")?.unbind();
        let page_number = parent_page.bind(py).getattr("page_number")?.unbind();
        let layout_laparams = parent_page
            .bind(py)
            .getattr("_layout_laparams")
            .ok()
            .map(Bound::unbind);
        let page = Py::new(
            py,
            Self {
                inner,
                object_transform,
            },
        )?;
        page.bind(py).setattr("parent_page", parent_page)?;
        page.bind(py).setattr("root_page", root_page)?;
        page.bind(py).setattr("mediabox", mediabox)?;
        page.bind(py).setattr("page_number", page_number)?;
        if let Some(layout_laparams) = layout_laparams {
            page.bind(py).setattr("_layout_laparams", layout_laparams)?;
        }
        Ok(page)
    }

    fn from_cropped_parent(
        py: Python<'_>,
        parent: PyRef<'_, Self>,
        inner: CroppedPage,
        object_transform: DerivedObjectTransform,
    ) -> PyResult<Py<Self>> {
        let parent: Py<Self> = parent.into();
        let root_page = parent.bind(py).getattr("root_page")?.unbind();
        Self::from_parent(py, inner, parent.into_any(), root_page, object_transform)
    }
}

#[pymethods]
impl PyCroppedPage {
    #[classattr]
    fn cached_properties(py: Python<'_>) -> PyResult<Py<PyList>> {
        compatible_page_cached_properties(py)
    }

    #[classattr]
    fn is_original() -> bool {
        false
    }

    /// Width of the cropped region.
    #[getter]
    fn width(&self) -> f64 {
        self.inner.width()
    }

    /// Height of the cropped region.
    #[getter]
    fn height(&self) -> f64 {
        self.inner.height()
    }

    fn __repr__(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<String> {
        let page: Py<Self> = slf.into();
        compatible_page_repr(py, page.bind(py).as_any())
    }

    /// Discard cached objects for this derived page.
    fn close(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<()> {
        let page: Py<Self> = slf.into();
        flush_compatible_page_cache(page.bind(py).as_any(), None, || Ok(()))
    }

    /// Discard selected cached properties for this derived page.
    #[pyo3(signature = (properties=None))]
    fn flush_cache(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        properties: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let page: Py<Self> = slf.into();
        flush_compatible_page_cache(page.bind(py).as_any(), properties, || Ok(()))
    }

    /// Objects in the cropped region grouped by their upstream type name.
    #[getter]
    fn objects(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        let page = page.bind(py);
        if page.hasattr("_objects")? {
            return Ok(page.getattr("_objects")?.unbind());
        }

        let parent_objects = page.getattr("parent_page")?.getattr("objects")?;
        let parent_objects = parent_objects.downcast::<PyDict>()?;
        let object_transform = page.borrow().object_transform;

        let objects = PyDict::new(py);
        for (kind, values) in parent_objects.iter() {
            objects.set_item(
                kind,
                transform_derived_objects(py, &values, object_transform)?,
            )?;
        }
        page.setattr("_objects", &objects)?;
        Ok(objects.into_any().unbind())
    }

    /// Horizontal text boxes created when layout analysis is requested.
    #[getter]
    fn textboxhorizontals(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "textboxhorizontal")
    }

    /// Vertical text boxes created when layout analysis is requested.
    #[getter]
    fn textboxverticals(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "textboxvertical")
    }

    /// Horizontal text lines created when layout analysis is requested.
    #[getter]
    fn textlinehorizontals(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "textlinehorizontal")
    }

    /// Vertical text lines created when layout analysis is requested.
    #[getter]
    fn textlineverticals(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "textlinevertical")
    }

    /// Convert a PDF-space point to this page view's top-origin coordinates.
    #[pyo3(signature = (*args, **kwargs), text_signature = "($self, pt)")]
    fn point2coord(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<(PyObject, PyObject)> {
        let page: Py<Self> = slf.into();
        let point = parse_point2coord_arg(args, kwargs)?;
        compatible_point2coord(py, page.bind(py).as_any(), point.bind(py))
    }

    /// Characters in the cropped region as list[dict].
    #[getter]
    fn chars(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "char")
    }

    /// Extract text from the cropped region.
    #[pyo3(signature = (layout=false))]
    fn extract_text(&self, layout: bool) -> String {
        self.inner.extract_text(&TextOptions {
            layout,
            ..TextOptions::default()
        })
    }

    /// Extract words from the cropped region.
    #[pyo3(signature = (x_tolerance=3.0, y_tolerance=3.0))]
    fn extract_words(
        &self,
        py: Python<'_>,
        x_tolerance: f64,
        y_tolerance: f64,
    ) -> PyResult<Vec<PyObject>> {
        let words = self.inner.extract_words(&WordOptions {
            x_tolerance,
            y_tolerance,
            ..WordOptions::default()
        });
        words.iter().map(|w| word_to_dict(py, w)).collect()
    }

    /// Find tables in the cropped region.
    fn find_tables(&self) -> Vec<PyTable> {
        self.inner
            .find_tables(&TableSettings::default())
            .into_iter()
            .map(|t| PyTable { inner: t })
            .collect()
    }

    /// Extract table content from the cropped region.
    fn extract_tables(&self) -> Vec<Vec<Vec<Option<String>>>> {
        let tables = self.inner.find_tables(&TableSettings::default());
        tables.iter().map(|t| table_rows_to_py(&t.rows)).collect()
    }

    /// Lines in the cropped region as list[dict].
    #[getter]
    fn lines(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "line")
    }

    /// Rects in the cropped region as list[dict].
    #[getter]
    fn rects(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "rect")
    }

    /// Curves in the cropped region as list[dict].
    #[getter]
    fn curves(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "curve")
    }

    /// Images in the cropped region as list[dict].
    #[getter]
    fn images(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "image")
    }

    /// Further crop this cropped page.
    fn crop(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        bbox: (f64, f64, f64, f64),
    ) -> PyResult<Py<Self>> {
        let bbox = parse_bbox_tuple(bbox);
        let inner = slf.inner.crop(bbox);
        Self::from_cropped_parent(py, slf, inner, DerivedObjectTransform::Crop(bbox))
    }

    /// Filter to objects fully within the given bbox.
    fn within_bbox(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        bbox: (f64, f64, f64, f64),
    ) -> PyResult<Py<Self>> {
        let bbox = parse_bbox_tuple(bbox);
        let inner = slf.inner.within_bbox(bbox);
        Self::from_cropped_parent(py, slf, inner, DerivedObjectTransform::Within(bbox))
    }

    /// Filter to objects outside the given bbox.
    fn outside_bbox(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        bbox: (f64, f64, f64, f64),
    ) -> PyResult<Py<Self>> {
        let bbox = parse_bbox_tuple(bbox);
        let inner = slf.inner.outside_bbox(bbox);
        Self::from_cropped_parent(py, slf, inner, DerivedObjectTransform::Outside(bbox))
    }
}

// ---------------------------------------------------------------------------
// PyPdf
// ---------------------------------------------------------------------------

const PDF_OPEN_PARAMETER_NAMES: [&str; 10] = [
    "path_or_fp",
    "pages",
    "laparams",
    "password",
    "strict_metadata",
    "unicode_norm",
    "repair",
    "gs_path",
    "repair_setting",
    "raise_unicode_errors",
];

const CONTAINER_TO_JSON_PARAMETER_NAMES: [&str; 6] = [
    "stream",
    "object_types",
    "include_attrs",
    "exclude_attrs",
    "precision",
    "indent",
];

const CONTAINER_TO_CSV_PARAMETER_NAMES: [&str; 5] = [
    "stream",
    "object_types",
    "precision",
    "include_attrs",
    "exclude_attrs",
];

struct PyPdfOpenArgs {
    path_or_fp: PyObject,
    pages: Option<PyObject>,
    laparams: Option<PyObject>,
    password: Option<String>,
    password_object: Option<PyObject>,
    strict_metadata: bool,
    unicode_norm: Option<PyObject>,
    repair: bool,
    gs_path: Option<PyObject>,
    repair_setting: PyObject,
    raise_unicode_errors: Option<PyObject>,
}

fn optional_py_object(py: Python<'_>, value: Option<PyObject>) -> Option<PyObject> {
    value.filter(|value| !value.bind(py).is_none())
}

struct PyContainerToJsonArgs {
    stream: Option<PyObject>,
    object_types: Option<PyObject>,
    include_attrs: Option<PyObject>,
    exclude_attrs: Option<PyObject>,
    precision: Option<PyObject>,
    indent: Option<PyObject>,
}

struct PyContainerToCsvArgs {
    stream: Option<PyObject>,
    object_types: Option<PyObject>,
    precision: Option<PyObject>,
    include_attrs: Option<PyObject>,
    exclude_attrs: Option<PyObject>,
}

fn parse_container_to_json_args(
    py: Python<'_>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<PyContainerToJsonArgs> {
    if args.len() > CONTAINER_TO_JSON_PARAMETER_NAMES.len() {
        return Err(PyTypeError::new_err(format!(
            "Container.to_json() takes from 1 to 7 positional arguments but {} were given",
            args.len() + 1
        )));
    }

    let mut values: [Option<PyObject>; CONTAINER_TO_JSON_PARAMETER_NAMES.len()] =
        std::array::from_fn(|_| None);
    for (index, value) in args.iter().enumerate() {
        values[index] = Some(value.unbind());
    }
    if let Some(kwargs) = kwargs {
        for (key, value) in kwargs.iter() {
            let key = key.extract::<String>()?;
            let Some(index) = CONTAINER_TO_JSON_PARAMETER_NAMES
                .iter()
                .position(|name| *name == key)
            else {
                return Err(PyTypeError::new_err(format!(
                    "Container.to_json() got an unexpected keyword argument '{key}'"
                )));
            };
            if values[index].is_some() {
                return Err(PyTypeError::new_err(format!(
                    "Container.to_json() got multiple values for argument '{key}'"
                )));
            }
            values[index] = Some(value.unbind());
        }
    }

    Ok(PyContainerToJsonArgs {
        stream: optional_py_object(py, values[0].take()),
        object_types: optional_py_object(py, values[1].take()),
        include_attrs: optional_py_object(py, values[2].take()),
        exclude_attrs: optional_py_object(py, values[3].take()),
        precision: optional_py_object(py, values[4].take()),
        indent: optional_py_object(py, values[5].take()),
    })
}

fn parse_container_to_csv_args(
    py: Python<'_>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<PyContainerToCsvArgs> {
    if args.len() > CONTAINER_TO_CSV_PARAMETER_NAMES.len() {
        return Err(PyTypeError::new_err(format!(
            "Container.to_csv() takes from 1 to 6 positional arguments but {} were given",
            args.len() + 1
        )));
    }

    let mut values: [Option<PyObject>; CONTAINER_TO_CSV_PARAMETER_NAMES.len()] =
        std::array::from_fn(|_| None);
    for (index, value) in args.iter().enumerate() {
        values[index] = Some(value.unbind());
    }
    if let Some(kwargs) = kwargs {
        for (key, value) in kwargs.iter() {
            let key = key.extract::<String>()?;
            let Some(index) = CONTAINER_TO_CSV_PARAMETER_NAMES
                .iter()
                .position(|name| *name == key)
            else {
                return Err(PyTypeError::new_err(format!(
                    "Container.to_csv() got an unexpected keyword argument '{key}'"
                )));
            };
            if values[index].is_some() {
                return Err(PyTypeError::new_err(format!(
                    "Container.to_csv() got multiple values for argument '{key}'"
                )));
            }
            values[index] = Some(value.unbind());
        }
    }

    Ok(PyContainerToCsvArgs {
        stream: optional_py_object(py, values[0].take()),
        object_types: optional_py_object(py, values[1].take()),
        precision: optional_py_object(py, values[2].take()),
        include_attrs: optional_py_object(py, values[3].take()),
        exclude_attrs: optional_py_object(py, values[4].take()),
    })
}

fn parse_pdf_open_args(
    py: Python<'_>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<PyPdfOpenArgs> {
    if args.len() > PDF_OPEN_PARAMETER_NAMES.len() {
        return Err(PyTypeError::new_err(format!(
            "PDF.open() takes from 1 to {} positional arguments but {} were given",
            PDF_OPEN_PARAMETER_NAMES.len(),
            args.len()
        )));
    }

    let mut values: [Option<PyObject>; PDF_OPEN_PARAMETER_NAMES.len()] =
        std::array::from_fn(|_| None);
    for (index, value) in args.iter().enumerate() {
        values[index] = Some(value.unbind());
    }
    if let Some(kwargs) = kwargs {
        for (key, value) in kwargs.iter() {
            let key = key.extract::<String>()?;
            let Some(index) = PDF_OPEN_PARAMETER_NAMES
                .iter()
                .position(|name| *name == key)
            else {
                return Err(PyTypeError::new_err(format!(
                    "PDF.open() got an unexpected keyword argument '{key}'"
                )));
            };
            if values[index].is_some() {
                return Err(PyTypeError::new_err(format!(
                    "PDF.open() got multiple values for argument '{key}'"
                )));
            }
            values[index] = Some(value.unbind());
        }
    }

    let path_or_fp = values[0].take().ok_or_else(|| {
        PyTypeError::new_err("PDF.open() missing 1 required positional argument: 'path_or_fp'")
    })?;
    let pages = optional_py_object(py, values[1].take());
    let laparams = optional_py_object(py, values[2].take());
    let password_object = optional_py_object(py, values[3].take());
    let password = password_object
        .as_ref()
        .map(|value| value.bind(py).extract::<String>())
        .transpose()?;
    let strict_metadata = match values[4].take() {
        Some(value) => value.bind(py).extract::<bool>()?,
        None => false,
    };
    let unicode_norm = optional_py_object(py, values[5].take());
    let repair = match values[6].take() {
        Some(value) => value.bind(py).extract::<bool>()?,
        None => false,
    };
    let gs_path = optional_py_object(py, values[7].take());
    let repair_setting = values[8]
        .take()
        .unwrap_or_else(|| PyString::new(py, "default").into_any().unbind());
    let raise_unicode_errors = values[9].take();

    Ok(PyPdfOpenArgs {
        path_or_fp,
        pages,
        laparams,
        password,
        password_object,
        strict_metadata,
        unicode_norm,
        repair,
        gs_path,
        repair_setting,
        raise_unicode_errors,
    })
}

/// A PDF document opened for extraction.
///
/// Use `PDF.open(path_or_fp)` or `PDF.open_bytes(data)` to open a PDF.
#[pyclass(name = "PDF", module = "pdfplumber.pdf")]
struct PyPdf {
    inner: Arc<Pdf>,
    stream: Option<PyObject>,
    path: Option<PyObject>,
    password: Option<PyObject>,
    stream_is_external: bool,
    selected_pages: Option<PyObject>,
    _laparams: Option<PyObject>,
    _strict_metadata: bool,
    unicode_norm: Option<PyObject>,
    raise_unicode_errors: Option<PyObject>,
    pages_cache: Mutex<Option<Py<PyList>>>,
    objects_cache: Mutex<Option<Py<PyDict>>>,
    metadata_cache: Mutex<Option<PyObject>>,
}

impl PyPdf {
    fn clear_pages_cache(&self) -> PyResult<()> {
        self.pages_cache
            .lock()
            .map_err(|_| PyRuntimeError::new_err("page cache lock poisoned"))?
            .take();
        Ok(())
    }

    fn clear_objects_cache(&self) -> PyResult<()> {
        self.objects_cache
            .lock()
            .map_err(|_| PyRuntimeError::new_err("object cache lock poisoned"))?
            .take();
        Ok(())
    }

    fn clear_document_caches(&self) -> PyResult<()> {
        self.clear_objects_cache()?;
        self.clear_pages_cache()
    }

    fn metadata_object(&self, py: Python<'_>) -> PyResult<PyObject> {
        let mut cache = self
            .metadata_cache
            .lock()
            .map_err(|_| PyRuntimeError::new_err("metadata cache lock poisoned"))?;
        if let Some(metadata) = cache.as_ref() {
            return Ok(metadata.clone_ref(py));
        }

        let metadata = raw_metadata_to_dict(py, self.inner.raw_metadata())?;
        *cache = Some(metadata.clone_ref(py));
        Ok(metadata)
    }
}

#[cfg(test)]
impl PyPdf {
    fn from_inner_for_test(inner: Pdf) -> Self {
        Self {
            inner: Arc::new(inner),
            stream: None,
            path: None,
            password: None,
            stream_is_external: false,
            selected_pages: None,
            _laparams: None,
            _strict_metadata: false,
            unicode_norm: None,
            raise_unicode_errors: None,
            pages_cache: Mutex::new(None),
            objects_cache: Mutex::new(None),
            metadata_cache: Mutex::new(None),
        }
    }
}

#[pymethods]
impl PyPdf {
    #[classattr]
    fn cached_properties() -> Vec<&'static str> {
        vec![
            "_rect_edges",
            "_curve_edges",
            "_edges",
            "_objects",
            "_pages",
        ]
    }

    /// Open a PDF from a filesystem path or seekable binary stream.
    #[staticmethod]
    #[pyo3(
        signature = (*args, **kwargs),
        text_signature = "(path_or_fp, pages=None, laparams=None, password=None, strict_metadata=False, unicode_norm=None, repair=False, gs_path=None, repair_setting='default', raise_unicode_errors=True)"
    )]
    fn open(
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let PyPdfOpenArgs {
            path_or_fp,
            pages,
            laparams,
            password,
            password_object,
            strict_metadata,
            unicode_norm,
            repair,
            gs_path,
            repair_setting,
            raise_unicode_errors,
        } = parse_pdf_open_args(py, args, kwargs)?;
        let path_or_fp = path_or_fp.bind(py);
        let laparams = validate_laparams(py, laparams)?;
        let (stream, path, stream_is_external) = if repair {
            let gs_path = gs_path.unwrap_or_else(|| py.None());
            let repaired = path_or_fp
                .py()
                .import("pdfplumber.repair")?
                .getattr("_repair")?
                .call1((path_or_fp, password.as_deref(), gs_path, repair_setting))?;
            (repaired, None, false)
        } else {
            if path_or_fp.is_instance_of::<PyString>() || path_or_fp.hasattr("__fspath__")? {
                let path = path_or_fp
                    .py()
                    .import("pathlib")?
                    .getattr("Path")?
                    .call1((path_or_fp,))?;
                let stream = path_or_fp
                    .py()
                    .import("builtins")?
                    .getattr("open")?
                    .call1((&path, "rb"))?;
                (stream, Some(path.unbind()), false)
            } else {
                (path_or_fp.clone(), None, true)
            }
        };
        stream
            .call_method1("seek", (0,))
            .map_err(|error| map_stream_error(path_or_fp.py(), error))?;
        let data = stream
            .call_method0("read")
            .map_err(|error| map_stream_error(path_or_fp.py(), error))?;
        let bytes = data.downcast::<PyBytes>()?;
        let pdf_result = match password.as_deref() {
            Some(password) => {
                Pdf::open_bytes_with_password(bytes.as_bytes(), password.as_bytes(), None)
            }
            None => Pdf::open_bytes(bytes.as_bytes(), None),
        };
        let pdf = match pdf_result {
            Ok(pdf) => pdf,
            Err(error) => {
                if !stream_is_external {
                    stream.call_method0("close")?;
                }
                return Err(to_py_err(error));
            }
        };
        if strict_metadata && pdf.validate_metadata().is_err() {
            if !stream_is_external {
                stream.call_method0("close")?;
            }
            return Err(PyRecursionError::new_err(
                "maximum recursion depth exceeded",
            ));
        }
        if !strict_metadata {
            log_metadata_warnings(py, &pdf)?;
        }
        Ok(PyPdf {
            inner: Arc::new(pdf),
            stream: Some(stream.unbind()),
            path,
            password: password_object,
            stream_is_external,
            selected_pages: pages,
            _laparams: laparams,
            _strict_metadata: strict_metadata,
            unicode_norm,
            raise_unicode_errors,
            pages_cache: Mutex::new(None),
            objects_cache: Mutex::new(None),
            metadata_cache: Mutex::new(None),
        })
    }

    /// Open a PDF from bytes in memory.
    #[staticmethod]
    fn open_bytes(py: Python<'_>, data: &[u8]) -> PyResult<Self> {
        let pdf = Pdf::open_bytes(data, None).map_err(to_py_err)?;
        log_metadata_warnings(py, &pdf)?;
        let stream = py
            .import("io")?
            .getattr("BytesIO")?
            .call1((PyBytes::new(py, data),))?;
        Ok(PyPdf {
            inner: Arc::new(pdf),
            stream: Some(stream.unbind()),
            path: None,
            password: None,
            stream_is_external: false,
            selected_pages: None,
            _laparams: None,
            _strict_metadata: false,
            unicode_norm: None,
            raise_unicode_errors: None,
            pages_cache: Mutex::new(None),
            objects_cache: Mutex::new(None),
            metadata_cache: Mutex::new(None),
        })
    }

    /// The owned or caller-provided binary stream used to open the document.
    #[getter]
    fn stream(&self, py: Python<'_>) -> PyObject {
        self.stream
            .as_ref()
            .map(|stream| stream.clone_ref(py))
            .unwrap_or_else(|| py.None())
    }

    /// The filesystem path for internally opened documents, otherwise `None`.
    #[getter]
    fn path(&self, py: Python<'_>) -> PyObject {
        self.path
            .as_ref()
            .map(|path| path.clone_ref(py))
            .unwrap_or_else(|| py.None())
    }

    /// The password supplied while opening the document, otherwise `None`.
    #[getter]
    fn password(&self, py: Python<'_>) -> PyObject {
        self.password
            .as_ref()
            .map(|password| password.clone_ref(py))
            .unwrap_or_else(|| py.None())
    }

    /// The exact page-number collection supplied while opening the document.
    #[getter]
    fn pages_to_parse(&self, py: Python<'_>) -> PyObject {
        self.selected_pages
            .as_ref()
            .map(|pages| pages.clone_ref(py))
            .unwrap_or_else(|| py.None())
    }

    /// Whether the input stream remains owned by the caller.
    #[getter]
    fn stream_is_external(&self) -> bool {
        self.stream_is_external
    }

    /// Explicit namespace for Rust-only document capabilities.
    #[getter]
    fn rust(&self) -> PyRustPdf {
        PyRustPdf {
            inner: Arc::clone(&self.inner),
        }
    }

    /// The Unicode normalization form applied during extraction, if any.
    #[getter]
    fn unicode_norm(&self, py: Python<'_>) -> PyObject {
        self.unicode_norm
            .as_ref()
            .map(|value| value.clone_ref(py))
            .unwrap_or_else(|| py.None())
    }

    /// Whether malformed annotation text should raise a Unicode decoding error.
    #[getter]
    fn raise_unicode_errors(&self, py: Python<'_>) -> PyObject {
        self.raise_unicode_errors
            .as_ref()
            .map(|value| value.clone_ref(py))
            .unwrap_or_else(|| PyBool::new(py, true).to_owned().into_any().unbind())
    }

    /// Release internally owned resources without closing caller-owned streams.
    fn close(&self, py: Python<'_>) -> PyResult<()> {
        self.clear_document_caches()?;
        self.pages(py)?;

        if !self.stream_is_external {
            if let Some(stream) = &self.stream {
                stream.bind(py).call_method0("close")?;
            }
        }
        Ok(())
    }

    /// Discard selected cached document properties without closing resources.
    #[pyo3(signature = (properties=None))]
    fn flush_cache(&self, properties: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let Some(properties) = properties else {
            return self.clear_document_caches();
        };

        for property in properties.try_iter()? {
            let property = property?;
            if !property.is_instance_of::<PyString>() {
                let type_name = property.get_type().name()?.to_string_lossy().into_owned();
                return Err(PyTypeError::new_err(format!(
                    "attribute name must be string, not '{type_name}'"
                )));
            }
            match property.extract::<String>()?.as_str() {
                "_objects" => self.clear_objects_cache()?,
                "_pages" => self.clear_pages_cache()?,
                _ => {}
            }
        }
        Ok(())
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        py: Python<'_>,
        _exc_type: &Bound<'_, PyAny>,
        _exc_value: &Bound<'_, PyAny>,
        _traceback: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.close(py)
    }

    /// The list of pages in the PDF.
    #[getter]
    fn pages(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        if let Some(pages) = self
            .pages_cache
            .lock()
            .map_err(|_| PyRuntimeError::new_err("page cache lock poisoned"))?
            .as_ref()
        {
            return Ok(pages.clone_ref(py));
        }

        let pages = PyList::empty(py).unbind();
        {
            let mut cache = self
                .pages_cache
                .lock()
                .map_err(|_| PyRuntimeError::new_err("page cache lock poisoned"))?;
            if let Some(existing) = cache.as_ref() {
                return Ok(existing.clone_ref(py));
            }
            *cache = Some(pages.clone_ref(py));
        }

        let mut selected_doctop = 0.0;
        let mut selected_doctop_is_integer = true;
        for i in 0..self.inner.page_count() {
            let page_number = (i + 1) as isize;
            if let Some(selected) = &self.selected_pages {
                if !selected.bind(py).contains(page_number)? {
                    continue;
                }
            }
            let (width, height) = self.inner.page_dimensions(i).ok_or_else(|| {
                PyRuntimeError::new_err(format!("missing geometry for page {}", i + 1))
            })?;
            let rotation = self.inner.page_rotation(i).ok_or_else(|| {
                PyRuntimeError::new_err(format!("missing rotation for page {}", i + 1))
            })?;
            let media_box = self.inner.page_media_box(i).ok_or_else(|| {
                PyRuntimeError::new_err(format!("missing MediaBox for page {}", i + 1))
            })?;
            let media_box_integer_flags =
                self.inner.page_media_box_integer_flags(i).ok_or_else(|| {
                    PyRuntimeError::new_err(format!(
                        "missing MediaBox number types for page {}",
                        i + 1
                    ))
                })?;
            let trim_box = compatible_optional_page_box(
                media_box,
                media_box_integer_flags,
                self.inner.page_trim_box(i),
                self.inner.page_trim_box_integer_flags(i),
                rotation,
            );
            let bleed_box = compatible_optional_page_box(
                media_box,
                media_box_integer_flags,
                self.inner.page_bleed_box(i),
                self.inner.page_bleed_box_integer_flags(i),
                rotation,
            );
            let art_box = compatible_optional_page_box(
                media_box,
                media_box_integer_flags,
                self.inner.page_art_box(i),
                self.inner.page_art_box_integer_flags(i),
                rotation,
            );
            let (media_box, crop_box) = compatible_page_boxes(
                media_box,
                media_box_integer_flags,
                self.inner.page_crop_box(i),
                self.inner.page_crop_box_integer_flags(i),
                rotation,
            );
            let width_is_integer = media_box.integer_flags[2] && media_box.integer_flags[0];
            let height_is_integer = media_box.integer_flags[3] && media_box.integer_flags[1];
            let initial_doctop = if self.selected_pages.is_some() {
                let initial_doctop = (selected_doctop, selected_doctop_is_integer);
                selected_doctop += compatible_geometry_number(height);
                selected_doctop_is_integer &= height_is_integer;
                Some(initial_doctop)
            } else {
                None
            };
            let page = Py::new(
                py,
                PyPage::new(
                    Arc::clone(&self.inner),
                    i,
                    PyPageGeometry {
                        width,
                        height,
                        width_is_integer,
                        height_is_integer,
                        rotation,
                        media_box,
                        crop_box,
                    },
                    initial_doctop,
                    self.unicode_norm.as_ref().map(|value| value.clone_ref(py)),
                ),
            )?;
            page.bind(py)
                .setattr("bbox", compatible_bbox_to_object(py, media_box)?)?;
            page.bind(py)
                .setattr("mediabox", compatible_bbox_to_object(py, media_box)?)?;
            page.bind(py).setattr("root_page", page.clone_ref(py))?;
            if let Some(laparams) = &self._laparams {
                page.bind(py)
                    .setattr("_layout_laparams", laparams.clone_ref(py))?;
            }
            if let Some(trim_box) = trim_box {
                page.bind(py)
                    .setattr("trimbox", compatible_bbox_to_object(py, trim_box)?)?;
            }
            if let Some(bleed_box) = bleed_box {
                page.bind(py)
                    .setattr("bleedbox", compatible_bbox_to_object(py, bleed_box)?)?;
            }
            if let Some(art_box) = art_box {
                page.bind(py)
                    .setattr("artbox", compatible_bbox_to_object(py, art_box)?)?;
            }
            pages.bind(py).append(page)?;
        }
        Ok(pages)
    }

    /// Objects from all selected pages grouped by their upstream type name.
    #[getter]
    fn objects(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        if let Some(objects) = self
            .objects_cache
            .lock()
            .map_err(|_| PyRuntimeError::new_err("object cache lock poisoned"))?
            .as_ref()
        {
            return Ok(objects.clone_ref(py));
        }

        let objects = PyDict::new(py);
        let pages = self.pages(py)?;
        for page in pages.bind(py).iter() {
            let page_objects = page.getattr("objects")?;
            for (kind, page_values) in page_objects.downcast::<PyDict>()?.iter() {
                let page_values = page_values.downcast::<PyList>()?;
                if page_values.is_empty() {
                    continue;
                }
                let aggregate = match objects.get_item(&kind)? {
                    Some(existing) => existing.downcast_into::<PyList>()?,
                    None => {
                        let aggregate = PyList::empty(py);
                        objects.set_item(&kind, &aggregate)?;
                        aggregate
                    }
                };
                for value in page_values.iter() {
                    aggregate.append(value)?;
                }
            }
        }

        let objects = objects.unbind();
        let mut cache = self
            .objects_cache
            .lock()
            .map_err(|_| PyRuntimeError::new_err("object cache lock poisoned"))?;
        if let Some(existing) = cache.as_ref() {
            return Ok(existing.clone_ref(py));
        }
        *cache = Some(objects.clone_ref(py));
        Ok(objects)
    }

    /// Horizontal text boxes created when layout analysis is requested.
    #[getter]
    fn textboxhorizontals(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let document: Py<Self> = slf.into();
        compatible_page_object_list(py, document.bind(py).as_any(), "textboxhorizontal")
    }

    /// Vertical text boxes created when layout analysis is requested.
    #[getter]
    fn textboxverticals(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let document: Py<Self> = slf.into();
        compatible_page_object_list(py, document.bind(py).as_any(), "textboxvertical")
    }

    /// Horizontal text lines created when layout analysis is requested.
    #[getter]
    fn textlinehorizontals(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let document: Py<Self> = slf.into();
        compatible_page_object_list(py, document.bind(py).as_any(), "textlinehorizontal")
    }

    /// Vertical text lines created when layout analysis is requested.
    #[getter]
    fn textlineverticals(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let document: Py<Self> = slf.into();
        compatible_page_object_list(py, document.bind(py).as_any(), "textlinevertical")
    }

    /// Annotation dictionaries from all selected pages in document order.
    #[getter]
    fn annots(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        let mut annots = Vec::new();
        for page in self.pages(py)?.bind(py).iter() {
            let page_annots = page.getattr("annots")?;
            for annotation in page_annots.downcast::<PyList>()?.iter() {
                annots.push(annotation.unbind());
            }
        }
        Ok(annots)
    }

    /// URI annotation dictionaries from all selected pages in document order.
    #[getter]
    fn hyperlinks(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        let mut hyperlinks = Vec::new();
        for page in self.pages(py)?.bind(py).iter() {
            let page_hyperlinks = page.getattr("hyperlinks")?;
            for hyperlink in page_hyperlinks.downcast::<PyList>()?.iter() {
                hyperlinks.push(hyperlink.unbind());
            }
        }
        Ok(hyperlinks)
    }

    /// Compact document structure-tree dictionaries in depth-first order.
    #[getter]
    fn structure_tree(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .structure_tree()
            .iter()
            .map(|element| struct_element_to_dict(py, element, true))
            .collect()
    }

    /// Document metadata and selected pages in upstream dictionary form.
    #[pyo3(signature = (object_types=None))]
    fn to_dict(
        &self,
        py: Python<'_>,
        object_types: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("metadata", self.metadata_object(py)?)?;
        let serialized_pages = PyList::empty(py);
        for page in self.pages(py)?.bind(py).iter() {
            let serialized = match object_types {
                Some(object_types) => page.call_method1("to_dict", (object_types,))?,
                None => page.call_method1("to_dict", (py.None(),))?,
            };
            serialized_pages.append(serialized)?;
        }
        dict.set_item("pages", serialized_pages)?;
        Ok(dict.into_any().unbind())
    }

    /// Serialize document metadata and selected pages as upstream JSON.
    #[pyo3(
        signature = (*args, **kwargs),
        text_signature = "(stream=None, object_types=None, include_attrs=None, exclude_attrs=None, precision=None, indent=None)"
    )]
    fn to_json(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let options = parse_container_to_json_args(py, args, kwargs)?;
        let data = self.to_dict(
            py,
            options.object_types.as_ref().map(|value| value.bind(py)),
        )?;
        container_to_json(
            py,
            data,
            options.stream.as_ref().map(|value| value.bind(py)),
            options.include_attrs.as_ref().map(|value| value.bind(py)),
            options.exclude_attrs.as_ref().map(|value| value.bind(py)),
            options.precision.as_ref().map(|value| value.bind(py)),
            options.indent.as_ref().map(|value| value.bind(py)),
        )
    }

    /// Serialize selected page objects as upstream CSV.
    #[pyo3(
        signature = (*args, **kwargs),
        text_signature = "(stream=None, object_types=None, precision=None, include_attrs=None, exclude_attrs=None)"
    )]
    fn to_csv(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let options = parse_container_to_csv_args(py, args, kwargs)?;
        let page_dicts = PyList::empty(py);
        for page in self.pages(py)?.bind(py).iter() {
            let data = match options.object_types.as_ref() {
                Some(object_types) => page.call_method1("to_dict", (object_types.bind(py),))?,
                None => page.call_method1("to_dict", (py.None(),))?,
            };
            page_dicts.append(data)?;
        }
        container_to_csv(
            py,
            page_dicts.into_any().unbind(),
            options.stream.as_ref().map(|value| value.bind(py)),
            options.include_attrs.as_ref().map(|value| value.bind(py)),
            options.exclude_attrs.as_ref().map(|value| value.bind(py)),
            options.precision.as_ref().map(|value| value.bind(py)),
        )
    }

    /// Document metadata as a dict.
    #[getter]
    fn metadata(&self, py: Python<'_>) -> PyResult<PyObject> {
        self.metadata_object(py)
    }
}

// ---------------------------------------------------------------------------
// PyPage
// ---------------------------------------------------------------------------

/// A single page from a PDF document.
#[derive(Clone, Copy)]
struct PyPageGeometry {
    width: f64,
    height: f64,
    width_is_integer: bool,
    height_is_integer: bool,
    rotation: i32,
    media_box: CompatiblePageBox,
    crop_box: CompatiblePageBox,
}

#[pyclass(name = "Page", dict)]
struct PyPage {
    pdf: Arc<Pdf>,
    page_index: usize,
    geometry: PyPageGeometry,
    selected_doctop: Option<(f64, bool)>,
    unicode_norm: Option<PyObject>,
    page_cache: Mutex<Option<Page>>,
}

impl PyPage {
    fn new(
        pdf: Arc<Pdf>,
        page_index: usize,
        geometry: PyPageGeometry,
        selected_doctop: Option<(f64, bool)>,
        unicode_norm: Option<PyObject>,
    ) -> Self {
        Self {
            pdf,
            page_index,
            geometry,
            selected_doctop,
            unicode_norm,
            page_cache: Mutex::new(None),
        }
    }

    #[cfg(test)]
    fn from_pdf_for_test(pdf: Pdf, page_index: usize) -> Self {
        let pdf = Arc::new(pdf);
        let (width, height) = pdf.page_dimensions(page_index).expect("page dimensions");
        let rotation = pdf.page_rotation(page_index).expect("page rotation");
        let source_media_box = pdf.page_media_box(page_index).expect("page MediaBox");
        let media_box_integer_flags = pdf
            .page_media_box_integer_flags(page_index)
            .expect("MediaBox number types");
        let (media_box, crop_box) = compatible_page_boxes(
            source_media_box,
            media_box_integer_flags,
            pdf.page_crop_box(page_index),
            pdf.page_crop_box_integer_flags(page_index),
            rotation,
        );
        let width_is_integer = media_box.integer_flags[2] && media_box.integer_flags[0];
        let height_is_integer = media_box.integer_flags[3] && media_box.integer_flags[1];
        Self::new(
            pdf,
            page_index,
            PyPageGeometry {
                width,
                height,
                width_is_integer,
                height_is_integer,
                rotation,
                media_box,
                crop_box,
            },
            None,
            None,
        )
    }

    #[cfg(test)]
    fn into_py_for_test(self, py: Python<'_>) -> PyResult<Py<Self>> {
        let media_box = self.geometry.media_box;
        let page = Py::new(py, self)?;
        page.bind(py)
            .setattr("bbox", compatible_bbox_to_object(py, media_box)?)?;
        page.bind(py)
            .setattr("mediabox", compatible_bbox_to_object(py, media_box)?)?;
        page.bind(py).setattr("root_page", page.clone_ref(py))?;
        Ok(page)
    }

    fn with_page<T>(
        &self,
        py: Python<'_>,
        operation: impl FnOnce(&Page) -> PyResult<T>,
    ) -> PyResult<T> {
        let unicode_norm = parse_unicode_norm(py, self.unicode_norm.as_ref())?;
        let mut cache = self
            .page_cache
            .lock()
            .map_err(|_| PyRuntimeError::new_err("page cache lock poisoned"))?;
        if cache.is_none() {
            let mut page = self.pdf.page(self.page_index).map_err(to_py_err)?;
            page.apply_unicode_norm(&unicode_norm);
            if let Some((selected_doctop, _)) = self.selected_doctop {
                page.rebase_doctop(selected_doctop);
            }
            *cache = Some(page);
        }
        let page = cache
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("page cache was not initialized"))?;
        operation(page)
    }

    fn clear_page_cache(&self) -> PyResult<()> {
        self.page_cache
            .lock()
            .map_err(|_| PyRuntimeError::new_err("page cache lock poisoned"))?
            .take();
        Ok(())
    }

    fn initial_doctop(&self) -> f64 {
        self.selected_doctop
            .map(|(value, _)| value)
            .unwrap_or_else(|| {
                (0..self.page_index)
                    .filter_map(|index| {
                        self.pdf
                            .page_dimensions(index)
                            .map(|(_, height)| compatible_geometry_number(height))
                    })
                    .sum()
            })
    }

    fn page_dimension_integer_flags(&self, index: usize) -> Option<(bool, bool)> {
        let media_box = self.pdf.page_media_box(index)?;
        let media_box_integer_flags = self.pdf.page_media_box_integer_flags(index)?;
        let rotation = self.pdf.page_rotation(index)?;
        let (media_box, _) =
            compatible_page_boxes(media_box, media_box_integer_flags, None, None, rotation);
        Some((
            media_box.integer_flags[2] && media_box.integer_flags[0],
            media_box.integer_flags[3] && media_box.integer_flags[1],
        ))
    }

    fn initial_doctop_is_integer(&self) -> bool {
        self.selected_doctop
            .map(|(_, is_integer)| is_integer)
            .unwrap_or_else(|| {
                (0..self.page_index).all(|index| {
                    self.page_dimension_integer_flags(index)
                        .is_some_and(|(_, height_is_integer)| height_is_integer)
                })
            })
    }

    fn width_value(&self) -> f64 {
        compatible_geometry_number(self.geometry.width)
    }

    fn height_value(&self) -> f64 {
        compatible_geometry_number(self.geometry.height)
    }

    fn char_objects_in_order(
        &self,
        py: Python<'_>,
        order: Option<&[usize]>,
    ) -> PyResult<Vec<PyObject>> {
        let page_number = self.page_number();
        let page_height = self.height_value();
        self.with_page(py, |page| {
            if let Some(order) = order {
                order
                    .iter()
                    .map(|&index| char_to_dict(py, &page.chars()[index], page_number, page_height))
                    .collect()
            } else {
                page.chars()
                    .iter()
                    .map(|ch| char_to_dict(py, ch, page_number, page_height))
                    .collect()
            }
        })
    }

    fn base_object_order(&self, py: Python<'_>) -> PyResult<Vec<PageObjectKind>> {
        self.with_page(py, |page| Ok(page.object_order().to_vec()))
    }

    fn line_objects(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        let page_number = self.page_number();
        let page_height = self.height_value();
        let initial_doctop = self.initial_doctop();
        self.with_page(py, |page| {
            page.lines()
                .iter()
                .map(|line| line_to_dict(py, line, page_number, page_height, initial_doctop))
                .collect()
        })
    }

    fn rect_objects(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        let page_number = self.page_number();
        let page_height = self.height_value();
        let initial_doctop = self.initial_doctop();
        self.with_page(py, |page| {
            page.rects()
                .iter()
                .map(|rect| rect_to_dict(py, rect, page_number, page_height, initial_doctop))
                .collect()
        })
    }

    fn curve_objects(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        let page_number = self.page_number();
        let page_height = self.height_value();
        let initial_doctop = self.initial_doctop();
        self.with_page(py, |page| {
            page.curves()
                .iter()
                .map(|curve| curve_to_dict(py, curve, page_number, page_height, initial_doctop))
                .collect()
        })
    }

    fn image_objects(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        let page_number = self.page_number();
        let page_height = self.height_value();
        let initial_doctop = self.initial_doctop();
        self.with_page(py, |page| {
            page.images()
                .iter()
                .map(|image| {
                    page_image_to_dict(py, image, page_number, page_height, initial_doctop)
                })
                .collect()
        })
    }

    fn layout_objects(
        &self,
        py: Python<'_>,
        params: &Bound<'_, PyDict>,
    ) -> PyResult<CompatibleLayoutObjects> {
        let page_number = self.page_number();
        let raw_width = self.geometry.width;
        let raw_height = self.geometry.height;
        let public_height = self.height_value();
        let initial_doctop = self.initial_doctop();
        let context = CompatibleLayoutDictContext {
            page_number,
            raw_width,
            raw_height,
            public_height,
            height_correction: raw_height - public_height,
            initial_doctop,
        };
        self.with_page(py, |page| {
            compatible_layout_objects(py, page.chars(), params, &context)
        })
    }

    fn to_dict_impl(
        &self,
        py: Python<'_>,
        page: &Bound<'_, PyAny>,
        object_types: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        let initial_doctop = self.initial_doctop();
        let initial_doctop_is_integer = self.initial_doctop_is_integer();
        self.with_page(py, |_| {
            dict.set_item("page_number", self.page_number())?;
            dict.set_item(
                "initial_doctop",
                initial_doctop_to_object(py, initial_doctop, initial_doctop_is_integer),
            )?;
            dict.set_item("rotation", self.geometry.rotation)?;
            dict.set_item(
                "cropbox",
                compatible_bbox_to_object(py, self.geometry.crop_box)?,
            )?;
            dict.set_item(
                "mediabox",
                compatible_bbox_to_object(py, self.geometry.media_box)?,
            )?;
            dict.set_item(
                "bbox",
                compatible_bbox_to_object(py, self.geometry.media_box)?,
            )?;
            dict.set_item(
                "width",
                compatible_number_to_object(py, self.width_value(), self.geometry.width_is_integer),
            )?;
            dict.set_item(
                "height",
                compatible_number_to_object(
                    py,
                    self.height_value(),
                    self.geometry.height_is_integer,
                ),
            )?;
            Ok(())
        })?;

        if let Some(object_types) = object_types {
            let add = py.import("operator")?.getattr("add")?;
            for object_type in object_types.try_iter()? {
                let object_type = object_type?;
                let attribute = add.call1((&object_type, "s"))?;
                let attribute = attribute.extract::<String>()?;
                dict.set_item(&attribute, compatible_page_attribute(page, &attribute)?)?;
            }
        } else {
            let objects = page.getattr("objects")?;
            let keys = objects.call_method0("keys")?;
            let add = py.import("operator")?.getattr("add")?;
            for object_type in keys.try_iter()? {
                let attribute = add.call1((object_type?, "s"))?;
                let attribute = attribute.extract::<String>()?;
                dict.set_item(&attribute, compatible_page_attribute(page, &attribute)?)?;
            }
            dict.set_item("annots", page.getattr("annots")?)?;
        }

        Ok(dict.into_any().unbind())
    }
}

#[pymethods]
impl PyPage {
    #[classattr]
    fn cached_properties(py: Python<'_>) -> PyResult<Py<PyList>> {
        compatible_page_cached_properties(py)
    }

    #[classattr]
    fn is_original() -> bool {
        true
    }

    /// The original 1-based document page number.
    #[getter]
    fn page_number(&self) -> usize {
        self.page_index + 1
    }

    fn __repr__(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<String> {
        let page: Py<Self> = slf.into();
        compatible_page_repr(py, page.bind(py).as_any())
    }

    /// Discard cached parsed content and objects for this page.
    fn close(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<()> {
        let page: Py<Self> = slf.into();
        let page = page.bind(py);
        flush_compatible_page_cache(page.as_any(), None, || page.borrow().clear_page_cache())
    }

    /// Discard selected cached properties for this page.
    #[pyo3(signature = (properties=None))]
    fn flush_cache(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        properties: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let page: Py<Self> = slf.into();
        let page = page.bind(py);
        flush_compatible_page_cache(page.as_any(), properties, || {
            page.borrow().clear_page_cache()
        })
    }

    /// Objects on this page grouped by their upstream type name.
    #[getter]
    fn objects(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        let page = page.bind(py);
        if page.hasattr("_objects")? {
            return Ok(page.getattr("_objects")?.unbind());
        }

        let layout_params = page
            .getattr("_layout_laparams")
            .ok()
            .map(|params| params.downcast_into::<PyDict>())
            .transpose()?;
        let page_ref = page.borrow();
        let layout_values = layout_params
            .as_ref()
            .map(|params| page_ref.layout_objects(py, params))
            .transpose()?;
        let char_values = page_ref.char_objects_in_order(
            py,
            layout_values
                .as_ref()
                .map(|layout| layout.ordered_char_indices.as_slice()),
        )?;
        let base_order = page_ref.base_object_order(py)?;
        let line_values = page_ref.line_objects(py)?;
        let rect_values = page_ref.rect_objects(py)?;
        let curve_values = page_ref.curve_objects(py)?;
        let image_values = page_ref.image_objects(py)?;
        drop(page_ref);

        let objects = PyDict::new(py);
        let (layout_order, horizontal_boxes, vertical_boxes, horizontal_lines, vertical_lines) =
            if let Some(layout) = layout_values {
                (
                    layout.family_order,
                    PyList::new(py, layout.horizontal_boxes)?,
                    PyList::new(py, layout.vertical_boxes)?,
                    PyList::new(py, layout.horizontal_lines)?,
                    PyList::new(py, layout.vertical_lines)?,
                )
            } else {
                (
                    Vec::new(),
                    PyList::empty(py),
                    PyList::empty(py),
                    PyList::empty(py),
                    PyList::empty(py),
                )
            };
        let chars = PyList::new(py, char_values)?;
        let lines = PyList::new(py, line_values)?;
        let rects = PyList::new(py, rect_values)?;
        let curves = PyList::new(py, curve_values)?;
        let images = PyList::new(py, image_values)?;
        let families = [
            ("textboxhorizontal", &horizontal_boxes),
            ("textboxvertical", &vertical_boxes),
            ("textlinehorizontal", &horizontal_lines),
            ("textlinevertical", &vertical_lines),
            ("char", &chars),
            ("line", &lines),
            ("rect", &rects),
            ("curve", &curves),
            ("image", &images),
        ];
        let mut family_order = layout_order;
        family_order.extend(base_order.into_iter().map(|kind| match kind {
            PageObjectKind::Char => "char",
            PageObjectKind::Line => "line",
            PageObjectKind::Rect => "rect",
            PageObjectKind::Curve => "curve",
            PageObjectKind::Image => "image",
        }));
        family_order.extend(["char", "line", "rect", "curve", "image"]);
        for kind in family_order {
            if objects.contains(kind)? {
                continue;
            }
            let values = families
                .iter()
                .find_map(|(family, values)| (*family == kind).then_some(*values))
                .expect("every ordered family has a value list");
            if !values.is_empty() {
                objects.set_item(kind, values)?;
            }
        }
        page.setattr("_objects", &objects)?;
        Ok(objects.into_any().unbind())
    }

    /// Horizontal text boxes created when layout analysis is requested.
    #[getter]
    fn textboxhorizontals(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "textboxhorizontal")
    }

    /// Vertical text boxes created when layout analysis is requested.
    #[getter]
    fn textboxverticals(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "textboxvertical")
    }

    /// Horizontal text lines created when layout analysis is requested.
    #[getter]
    fn textlinehorizontals(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "textlinehorizontal")
    }

    /// Vertical text lines created when layout analysis is requested.
    #[getter]
    fn textlineverticals(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "textlinevertical")
    }

    /// Page width in points.
    #[getter]
    fn width(&self, py: Python<'_>) -> PyObject {
        compatible_number_to_object(py, self.width_value(), self.geometry.width_is_integer)
    }

    /// Page height in points.
    #[getter]
    fn height(&self, py: Python<'_>) -> PyObject {
        compatible_number_to_object(py, self.height_value(), self.geometry.height_is_integer)
    }

    /// Convert a PDF-space point to this page view's top-origin coordinates.
    #[pyo3(signature = (*args, **kwargs), text_signature = "($self, pt)")]
    fn point2coord(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<(PyObject, PyObject)> {
        let page: Py<Self> = slf.into();
        let point = parse_point2coord_arg(args, kwargs)?;
        compatible_point2coord(py, page.bind(py).as_any(), point.bind(py))
    }

    /// Page rotation normalized to the range 0 through 359 degrees.
    #[getter]
    fn rotation(&self) -> i32 {
        self.geometry.rotation
    }

    /// CropBox in the page's rotation-aware, top-origin coordinate space.
    #[getter]
    fn cropbox(&self, py: Python<'_>) -> PyResult<PyObject> {
        compatible_bbox_to_object(py, self.geometry.crop_box)
    }

    /// Cumulative height of preceding pages in the current page view.
    #[getter(initial_doctop)]
    fn initial_doctop_property(&self, py: Python<'_>) -> PyObject {
        initial_doctop_to_object(py, self.initial_doctop(), self.initial_doctop_is_integer())
    }

    /// Characters on this page as list[dict].
    #[getter]
    fn chars(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "char")
    }

    /// Extract text from this page.
    #[pyo3(signature = (layout=false))]
    fn extract_text(&self, py: Python<'_>, layout: bool) -> PyResult<String> {
        self.with_page(py, |page| {
            Ok(page.extract_text(&TextOptions {
                layout,
                ..TextOptions::default()
            }))
        })
    }

    /// Extract words from this page.
    #[pyo3(signature = (x_tolerance=3.0, y_tolerance=3.0))]
    fn extract_words(
        &self,
        py: Python<'_>,
        x_tolerance: f64,
        y_tolerance: f64,
    ) -> PyResult<Vec<PyObject>> {
        self.with_page(py, |page| {
            let words = page.extract_words(&WordOptions {
                x_tolerance,
                y_tolerance,
                ..WordOptions::default()
            });
            words.iter().map(|w| word_to_dict(py, w)).collect()
        })
    }

    /// Find tables on this page.
    fn find_tables(&self, py: Python<'_>) -> PyResult<Vec<PyTable>> {
        self.with_page(py, |page| {
            Ok(page
                .find_tables(&TableSettings::default())
                .into_iter()
                .map(|t| PyTable { inner: t })
                .collect())
        })
    }

    /// Extract table content as list[list[list[str|None]]].
    fn extract_tables(&self, py: Python<'_>) -> PyResult<Vec<Vec<Vec<Option<String>>>>> {
        self.with_page(
            py,
            |page| Ok(page.extract_tables(&TableSettings::default())),
        )
    }

    /// Lines on this page as list[dict].
    #[getter]
    fn lines(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "line")
    }

    /// Rectangles on this page as list[dict].
    #[getter]
    fn rects(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "rect")
    }

    /// Curves on this page as list[dict].
    #[getter]
    fn curves(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "curve")
    }

    /// Images on this page as list[dict].
    #[getter]
    fn images(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        compatible_page_object_list(py, page.bind(py).as_any(), "image")
    }

    /// Annotation dictionaries on this page.
    #[getter]
    fn annots(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        let page_number = self.page_number();
        let page_height = self.geometry.height;
        let initial_doctop = self.selected_doctop.map(|(value, _)| value).unwrap_or(0.0);
        self.with_page(py, |page| {
            page.annots()
                .iter()
                .map(|annotation| {
                    let uri = page
                        .uri_hyperlinks()
                        .iter()
                        .find(|hyperlink| hyperlink.bbox == annotation.bbox)
                        .map(|hyperlink| hyperlink.uri.as_str());
                    annotation_to_dict(
                        py,
                        annotation,
                        page_number,
                        page_height,
                        initial_doctop,
                        uri,
                    )
                })
                .collect()
        })
    }

    /// Annotation dictionaries whose URI is not null.
    #[getter]
    fn hyperlinks(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        let mut hyperlinks = Vec::new();
        for annotation in self.annots(py)? {
            let uri = annotation
                .bind(py)
                .downcast::<PyDict>()?
                .get_item("uri")?
                .ok_or_else(|| PyRuntimeError::new_err("annotation is missing uri"))?;
            if !uri.is_none() {
                hyperlinks.push(annotation);
            }
        }
        Ok(hyperlinks)
    }

    /// Compact structure-tree dictionaries for this page.
    #[getter]
    fn structure_tree(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.with_page(py, |page| {
            page.structure_tree()
                .unwrap_or(&[])
                .iter()
                .map(|element| struct_element_to_dict(py, element, false))
                .collect()
        })
    }

    /// Page geometry and requested object lists in upstream dictionary form.
    #[pyo3(signature = (object_types=None))]
    fn to_dict(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        object_types: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        let page: Py<Self> = slf.into();
        page.borrow(py)
            .to_dict_impl(py, page.bind(py).as_any(), object_types)
    }

    /// Serialize page geometry and requested objects as upstream JSON.
    #[pyo3(
        signature = (*args, **kwargs),
        text_signature = "(stream=None, object_types=None, include_attrs=None, exclude_attrs=None, precision=None, indent=None)"
    )]
    fn to_json(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let options = parse_container_to_json_args(py, args, kwargs)?;
        let page: Py<Self> = slf.into();
        let data = page.borrow(py).to_dict_impl(
            py,
            page.bind(py).as_any(),
            options.object_types.as_ref().map(|value| value.bind(py)),
        )?;
        container_to_json(
            py,
            data,
            options.stream.as_ref().map(|value| value.bind(py)),
            options.include_attrs.as_ref().map(|value| value.bind(py)),
            options.exclude_attrs.as_ref().map(|value| value.bind(py)),
            options.precision.as_ref().map(|value| value.bind(py)),
            options.indent.as_ref().map(|value| value.bind(py)),
        )
    }

    /// Serialize page objects as upstream CSV.
    #[pyo3(
        signature = (*args, **kwargs),
        text_signature = "(stream=None, object_types=None, precision=None, include_attrs=None, exclude_attrs=None)"
    )]
    fn to_csv(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let options = parse_container_to_csv_args(py, args, kwargs)?;
        let page: Py<Self> = slf.into();
        let data = page.borrow(py).to_dict_impl(
            py,
            page.bind(py).as_any(),
            options.object_types.as_ref().map(|value| value.bind(py)),
        )?;
        let page_dicts = PyList::empty(py);
        page_dicts.append(data)?;
        container_to_csv(
            py,
            page_dicts.into_any().unbind(),
            options.stream.as_ref().map(|value| value.bind(py)),
            options.include_attrs.as_ref().map(|value| value.bind(py)),
            options.exclude_attrs.as_ref().map(|value| value.bind(py)),
            options.precision.as_ref().map(|value| value.bind(py)),
        )
    }

    /// Crop this page to a bounding box (x0, top, x1, bottom).
    fn crop(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        bbox: (f64, f64, f64, f64),
    ) -> PyResult<Py<PyCroppedPage>> {
        let bbox = parse_bbox_tuple(bbox);
        let inner = slf.with_page(py, |page| Ok(page.crop(bbox)))?;
        let original: Py<Self> = slf.into();
        let root_page = original.clone_ref(py).into_any();
        PyCroppedPage::from_parent(
            py,
            inner,
            original.into_any(),
            root_page,
            DerivedObjectTransform::Crop(bbox),
        )
    }

    /// Filter to objects fully within the given bbox.
    fn within_bbox(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        bbox: (f64, f64, f64, f64),
    ) -> PyResult<Py<PyCroppedPage>> {
        let bbox = parse_bbox_tuple(bbox);
        let inner = slf.with_page(py, |page| Ok(page.within_bbox(bbox)))?;
        let original: Py<Self> = slf.into();
        let root_page = original.clone_ref(py).into_any();
        PyCroppedPage::from_parent(
            py,
            inner,
            original.into_any(),
            root_page,
            DerivedObjectTransform::Within(bbox),
        )
    }

    /// Filter to objects outside the given bbox.
    fn outside_bbox(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        bbox: (f64, f64, f64, f64),
    ) -> PyResult<Py<PyCroppedPage>> {
        let bbox = parse_bbox_tuple(bbox);
        let inner = slf.with_page(py, |page| Ok(page.outside_bbox(bbox)))?;
        let original: Py<Self> = slf.into();
        let root_page = original.clone_ref(py).into_any();
        PyCroppedPage::from_parent(
            py,
            inner,
            original.into_any(),
            root_page,
            DerivedObjectTransform::Outside(bbox),
        )
    }

    /// Search for a text pattern on this page.
    #[pyo3(signature = (pattern, regex=true, case=true))]
    fn search(
        &self,
        py: Python<'_>,
        pattern: &str,
        regex: bool,
        case: bool,
    ) -> PyResult<Vec<PyObject>> {
        self.with_page(py, |page| {
            let matches = page.search(
                pattern,
                &SearchOptions {
                    regex,
                    case_sensitive: case,
                },
            );
            matches
                .iter()
                .map(|item| search_match_to_dict(py, item))
                .collect()
        })
    }
}

// ---------------------------------------------------------------------------
// Module definition
// ---------------------------------------------------------------------------

/// The Python module definition.
#[pymodule(name = "_native")]
fn pdfplumber(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", VERSION)?;

    m.add_class::<PyPdf>()?;
    m.add_class::<PyRustPdf>()?;
    m.add_class::<PyPage>()?;
    m.add_class::<PyTable>()?;
    m.add_class::<PyCroppedPage>()?;
    m.add_class::<PyMetadataReference>()?;
    m.add_class::<PyMetadataStream>()?;

    // Register exception types
    m.add("PdfParseError", m.py().get_type::<PdfParseError>())?;
    m.add("PdfIoError", m.py().get_type::<PdfIoError>())?;
    m.add("PdfFontError", m.py().get_type::<PdfFontError>())?;
    m.add(
        "PdfInterpreterError",
        m.py().get_type::<PdfInterpreterError>(),
    )?;
    m.add(
        "PdfResourceLimitError",
        m.py().get_type::<PdfResourceLimitError>(),
    )?;
    m.add(
        "PdfPasswordRequired",
        m.py().get_type::<PdfPasswordRequired>(),
    )?;
    m.add(
        "PdfInvalidPassword",
        m.py().get_type::<PdfInvalidPassword>(),
    )?;
    m.add("PdfminerException", m.py().get_type::<PdfminerException>())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    #[test]
    fn native_extension_uses_private_submodule_identity() {
        Python::with_gil(|py| {
            let module = pyo3::wrap_pymodule!(super::pdfplumber)(py);
            let name = module.bind(py).name().unwrap();
            assert_eq!(name.to_str().unwrap(), "_native");
        });
    }

    /// Helper: create a minimal valid PDF in memory using lopdf.
    fn minimal_pdf_bytes() -> Vec<u8> {
        minimal_pdf_bytes_with_optional_boxes(None, None, None)
    }

    fn minimal_pdf_bytes_with_optional_boxes(
        trim_box: Option<[i64; 4]>,
        bleed_box: Option<[i64; 4]>,
        art_box: Option<[i64; 4]>,
    ) -> Vec<u8> {
        use std::io::Cursor;

        let mut doc = lopdf::Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let page_id = doc.new_object_id();

        let resources = dictionary! {};
        let content = lopdf::Stream::new(dictionary! {}, Vec::new());
        let content_id = doc.add_object(content);

        let mut page = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => resources,
            "Contents" => content_id,
        };
        if let Some([x0, y0, x1, y1]) = trim_box {
            page.set("TrimBox", vec![x0.into(), y0.into(), x1.into(), y1.into()]);
        }
        if let Some([x0, y0, x1, y1]) = bleed_box {
            page.set("BleedBox", vec![x0.into(), y0.into(), x1.into(), y1.into()]);
        }
        if let Some([x0, y0, x1, y1]) = art_box {
            page.set("ArtBox", vec![x0.into(), y0.into(), x1.into(), y1.into()]);
        }
        doc.objects.insert(page_id, lopdf::Object::Dictionary(page));

        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects
            .insert(pages_id, lopdf::Object::Dictionary(pages));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Cursor::new(Vec::new());
        doc.save_to(&mut buf).expect("save PDF");
        buf.into_inner()
    }

    // -----------------------------------------------------------------------
    // US-073 tests (preserved from original)
    // -----------------------------------------------------------------------

    #[test]
    fn test_open_bytes_creates_pypdf() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf::from_inner_for_test(pdf);
        assert_eq!(pypdf.inner.page_count(), 1);
    }

    #[test]
    fn test_pypdf_pages_returns_correct_count() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf::from_inner_for_test(pdf);
        Python::with_gil(|py| {
            let pages = pypdf.pages(py).expect("pages");
            assert_eq!(pages.bind(py).len(), 1);
        });
    }

    #[test]
    fn test_pypage_dimensions() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf::from_inner_for_test(pdf);
        Python::with_gil(|py| {
            let pages = pypdf.pages(py).expect("pages");
            let page = pages.bind(py).get_item(0).expect("page");
            assert!((page.getattr("width").unwrap().extract::<f64>().unwrap() - 612.0).abs() < 0.1);
            assert!(
                (page.getattr("height").unwrap().extract::<f64>().unwrap() - 792.0).abs() < 0.1
            );
            assert_eq!(
                page.getattr("rotation").unwrap().extract::<i32>().unwrap(),
                0
            );
            assert_eq!(
                page.getattr("mediabox")
                    .unwrap()
                    .extract::<(f64, f64, f64, f64)>()
                    .unwrap(),
                (0.0, 0.0, 612.0, 792.0)
            );
            assert_eq!(
                page.getattr("cropbox")
                    .unwrap()
                    .extract::<(f64, f64, f64, f64)>()
                    .unwrap(),
                (0.0, 0.0, 612.0, 792.0)
            );
        });
    }

    #[test]
    fn test_pypage_close_clears_native_page_cache() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let page = page.into_py_for_test(py).expect("Python page");
            page.bind(py).getattr("chars").expect("materialize objects");
            {
                let page_ref = page.bind(py).borrow();
                let cache = page_ref.page_cache.lock().expect("page cache");
                assert!(cache.is_some());
            }
            assert!(
                page.bind(py)
                    .getattr("__dict__")
                    .expect("page dictionary")
                    .downcast::<PyDict>()
                    .expect("dictionary")
                    .contains("_objects")
                    .expect("object cache presence")
            );

            page.bind(py).call_method0("close").expect("close page");

            {
                let page_ref = page.bind(py).borrow();
                let cache = page_ref.page_cache.lock().expect("page cache");
                assert!(cache.is_none());
            }
            assert!(
                !page
                    .bind(py)
                    .getattr("__dict__")
                    .expect("page dictionary")
                    .downcast::<PyDict>()
                    .expect("dictionary")
                    .contains("_objects")
                    .expect("object cache presence")
            );
        });
    }

    #[test]
    fn test_pypage_flush_cache_selectively_clears_native_page_cache() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let page = page.into_py_for_test(py).expect("Python page");
            page.bind(py).getattr("chars").expect("materialize objects");

            let only_objects = PyList::new(py, ["_objects"]).expect("property list");
            page.bind(py)
                .call_method1("flush_cache", (only_objects,))
                .expect("flush objects");
            {
                let page_ref = page.bind(py).borrow();
                let cache = page_ref.page_cache.lock().expect("page cache");
                assert!(cache.is_some());
            }
            assert!(
                !page
                    .bind(py)
                    .getattr("__dict__")
                    .expect("page dictionary")
                    .downcast::<PyDict>()
                    .expect("dictionary")
                    .contains("_objects")
                    .expect("object cache presence")
            );

            page.bind(py)
                .getattr("chars")
                .expect("rematerialize objects");
            let only_layout = PyList::new(py, ["_layout"]).expect("property list");
            page.bind(py)
                .call_method1("flush_cache", (only_layout,))
                .expect("flush layout");
            {
                let page_ref = page.bind(py).borrow();
                let cache = page_ref.page_cache.lock().expect("page cache");
                assert!(cache.is_none());
            }
            assert!(
                page.bind(py)
                    .getattr("__dict__")
                    .expect("page dictionary")
                    .downcast::<PyDict>()
                    .expect("dictionary")
                    .contains("_objects")
                    .expect("object cache presence")
            );

            page.bind(py)
                .call_method0("flush_cache")
                .expect("flush default caches");
            assert!(
                !page
                    .bind(py)
                    .getattr("__dict__")
                    .expect("page dictionary")
                    .downcast::<PyDict>()
                    .expect("dictionary")
                    .contains("_objects")
                    .expect("object cache presence")
            );
        });
    }

    #[test]
    fn test_pypage_trimbox_is_explicit_and_optional() {
        let bytes = minimal_pdf_bytes_with_optional_boxes(Some([40, 50, 560, 740]), None, None);
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf::from_inner_for_test(pdf);
        Python::with_gil(|py| {
            let pages = pypdf.pages(py).expect("pages");
            let page = pages.bind(py).get_item(0).expect("page");
            assert_eq!(
                page.getattr("trimbox")
                    .unwrap()
                    .extract::<(f64, f64, f64, f64)>()
                    .unwrap(),
                (40.0, 52.0, 560.0, 742.0)
            );
            assert!(
                page.getattr("__dict__")
                    .unwrap()
                    .downcast::<PyDict>()
                    .unwrap()
                    .contains("trimbox")
                    .unwrap()
            );
        });

        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf::from_inner_for_test(pdf);
        Python::with_gil(|py| {
            let pages = pypdf.pages(py).expect("pages");
            let page = pages.bind(py).get_item(0).expect("page");
            let error = page.getattr("trimbox").unwrap_err();
            assert!(error.is_instance_of::<PyAttributeError>(py));
            assert!(
                !page
                    .getattr("__dict__")
                    .unwrap()
                    .downcast::<PyDict>()
                    .unwrap()
                    .contains("trimbox")
                    .unwrap()
            );
        });
    }

    #[test]
    fn test_pypage_bleedbox_is_explicit_and_optional() {
        let bytes = minimal_pdf_bytes_with_optional_boxes(None, Some([45, 55, 555, 735]), None);
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf::from_inner_for_test(pdf);
        Python::with_gil(|py| {
            let pages = pypdf.pages(py).expect("pages");
            let page = pages.bind(py).get_item(0).expect("page");
            assert_eq!(
                page.getattr("bleedbox")
                    .unwrap()
                    .extract::<(f64, f64, f64, f64)>()
                    .unwrap(),
                (45.0, 57.0, 555.0, 737.0)
            );
            assert!(
                page.getattr("__dict__")
                    .unwrap()
                    .downcast::<PyDict>()
                    .unwrap()
                    .contains("bleedbox")
                    .unwrap()
            );
        });

        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf::from_inner_for_test(pdf);
        Python::with_gil(|py| {
            let pages = pypdf.pages(py).expect("pages");
            let page = pages.bind(py).get_item(0).expect("page");
            let error = page.getattr("bleedbox").unwrap_err();
            assert!(error.is_instance_of::<PyAttributeError>(py));
            assert!(
                !page
                    .getattr("__dict__")
                    .unwrap()
                    .downcast::<PyDict>()
                    .unwrap()
                    .contains("bleedbox")
                    .unwrap()
            );
        });
    }

    #[test]
    fn test_pypage_artbox_is_explicit_and_optional() {
        let bytes = minimal_pdf_bytes_with_optional_boxes(None, None, Some([50, 60, 550, 730]));
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf::from_inner_for_test(pdf);
        Python::with_gil(|py| {
            let pages = pypdf.pages(py).expect("pages");
            let page = pages.bind(py).get_item(0).expect("page");
            assert_eq!(
                page.getattr("artbox")
                    .unwrap()
                    .extract::<(f64, f64, f64, f64)>()
                    .unwrap(),
                (50.0, 62.0, 550.0, 732.0)
            );
            assert!(
                page.getattr("__dict__")
                    .unwrap()
                    .downcast::<PyDict>()
                    .unwrap()
                    .contains("artbox")
                    .unwrap()
            );
        });

        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf::from_inner_for_test(pdf);
        Python::with_gil(|py| {
            let pages = pypdf.pages(py).expect("pages");
            let page = pages.bind(py).get_item(0).expect("page");
            let error = page.getattr("artbox").unwrap_err();
            assert!(error.is_instance_of::<PyAttributeError>(py));
            assert!(
                !page
                    .getattr("__dict__")
                    .unwrap()
                    .downcast::<PyDict>()
                    .unwrap()
                    .contains("artbox")
                    .unwrap()
            );
        });
    }

    #[test]
    fn test_open_invalid_bytes_returns_error() {
        let result = Pdf::open(b"not a pdf", None);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // US-074 tests: Full API exposure
    // -----------------------------------------------------------------------

    #[test]
    fn test_pypage_chars_returns_list() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        // Empty page should return empty list
        Python::with_gil(|py| {
            let chars = pypage.char_objects_in_order(py, None).expect("chars");
            assert!(chars.is_empty());
        });
    }

    #[test]
    fn test_pypage_extract_text_empty_page() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let text = pypage.extract_text(py, false).expect("extract text");
            assert!(text.is_empty());
        });
    }

    #[test]
    fn test_pypage_extract_text_layout_mode() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let text = pypage.extract_text(py, true).expect("extract text");
            assert!(text.is_empty());
        });
    }

    #[test]
    fn test_pypage_extract_words_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let words = pypage.extract_words(py, 3.0, 3.0).expect("words");
            assert!(words.is_empty());
        });
    }

    #[test]
    fn test_pypage_lines_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let lines = pypage.line_objects(py).expect("lines");
            assert!(lines.is_empty());
        });
    }

    #[test]
    fn test_pypage_rects_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let rects = pypage.rect_objects(py).expect("rects");
            assert!(rects.is_empty());
        });
    }

    #[test]
    fn test_pypage_curves_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let curves = pypage.curve_objects(py).expect("curves");
            assert!(curves.is_empty());
        });
    }

    #[test]
    fn test_pypage_images_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let images = pypage.image_objects(py).expect("images");
            assert!(images.is_empty());
        });
    }

    #[test]
    fn test_pypage_find_tables_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let tables = pypage.find_tables(py).expect("find tables");
            assert!(tables.is_empty());
        });
    }

    #[test]
    fn test_pypage_extract_tables_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let tables = pypage.extract_tables(py).expect("extract tables");
            assert!(tables.is_empty());
        });
    }

    #[test]
    fn test_pypage_search_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let results = pypage.search(py, "test", true, true).expect("search");
            assert!(results.is_empty());
        });
    }

    #[test]
    fn test_pypage_crop() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let pypage = pypage.into_py_for_test(py).expect("bind page");
            let cropped =
                PyPage::crop(pypage.bind(py).borrow(), py, (0.0, 0.0, 306.0, 396.0)).expect("crop");
            let cropped = cropped.bind(py).borrow();
            assert!((cropped.width() - 306.0).abs() < 0.1);
            assert!((cropped.height() - 396.0).abs() < 0.1);
        });
    }

    #[test]
    fn test_pypage_within_bbox() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let pypage = pypage.into_py_for_test(py).expect("bind page");
            let filtered =
                PyPage::within_bbox(pypage.bind(py).borrow(), py, (0.0, 0.0, 306.0, 396.0))
                    .expect("within bbox");
            let filtered = filtered.bind(py).borrow();
            assert!((filtered.width() - 306.0).abs() < 0.1);
            assert!((filtered.height() - 396.0).abs() < 0.1);
        });
    }

    #[test]
    fn test_pypage_outside_bbox() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let pypage = pypage.into_py_for_test(py).expect("bind page");
            let filtered =
                PyPage::outside_bbox(pypage.bind(py).borrow(), py, (100.0, 100.0, 200.0, 200.0))
                    .expect("outside bbox");
            let filtered = filtered.bind(py).borrow();
            // outside_bbox uses the bbox dimensions (coordinate-adjusted region)
            assert!((filtered.width() - 100.0).abs() < 0.1);
            assert!((filtered.height() - 100.0).abs() < 0.1);
        });
    }

    #[test]
    fn test_pypdf_metadata() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf::from_inner_for_test(pdf);
        Python::with_gil(|py| {
            let meta = pypdf.metadata(py).expect("metadata");
            let dict = meta.downcast_bound::<PyDict>(py).expect("dict");
            assert!(dict.is_empty());
        });
    }

    #[test]
    fn test_pypdf_rust_bookmarks_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf::from_inner_for_test(pdf);
        Python::with_gil(|py| {
            let bookmarks = pypdf.rust().bookmarks(py).expect("bookmarks");
            assert!(bookmarks.is_empty());
        });
    }

    #[test]
    fn test_to_py_err_parse_error() {
        let err = to_py_err(PdfError::parse("bad xref"));
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfminerException>(py));
        });
    }

    #[test]
    fn test_to_py_err_io_error() {
        let err =
            to_py_err(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found").into());
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfIoError>(py));
        });
    }

    #[test]
    fn test_to_py_err_font_error() {
        let err = to_py_err(PdfError::font("missing glyph"));
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfFontError>(py));
        });
    }

    #[test]
    fn test_to_py_err_interpreter_error() {
        let err = to_py_err(PdfError::interpreter("unknown op"));
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfInterpreterError>(py));
        });
    }

    #[test]
    fn test_to_py_err_resource_limit() {
        let err = to_py_err(PdfError::limit_exceeded("max_pages", 10, 20));
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfResourceLimitError>(py));
        });
    }

    #[test]
    fn test_to_py_err_resource_limit_without_details() {
        let err = to_py_err(PdfError::new(PdfErrorKind::ResourceLimit));
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfResourceLimitError>(py));
        });
    }

    #[test]
    fn test_to_py_err_password_required() {
        let err = to_py_err(PdfError::password_required());
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfminerException>(py));
        });
    }

    #[test]
    fn test_to_py_err_invalid_password() {
        let err = to_py_err(PdfError::invalid_password());
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfminerException>(py));
        });
    }

    #[test]
    fn test_char_to_dict_conversion() {
        let ch = Char {
            text: "A".to_string(),
            bbox: BBox::new(10.0, 20.0, 20.0, 32.0),
            fontname: "Helvetica".to_string(),
            size: 12.0,
            advance: 10.0,
            doctop: 20.0,
            upright: true,
            direction: ::pdfplumber::TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: Some(Color::Gray(0.0)),
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: 65,
            mcid: None,
            tag: None,
        };
        Python::with_gil(|py| {
            let dict_obj = char_to_dict(py, &ch, 7, 400.0).expect("char_to_dict");
            let dict = dict_obj.downcast_bound::<PyDict>(py).expect("PyDict");
            let text: String = dict.get_item("text").unwrap().unwrap().extract().unwrap();
            assert_eq!(text, "A");
            let x0: f64 = dict.get_item("x0").unwrap().unwrap().extract().unwrap();
            assert!((x0 - 10.0).abs() < 0.01);
            let fontname: String = dict
                .get_item("fontname")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(fontname, "Helvetica");
            let size: f64 = dict.get_item("size").unwrap().unwrap().extract().unwrap();
            assert!((size - 12.0).abs() < 0.01);
            let advance: f64 = dict.get_item("adv").unwrap().unwrap().extract().unwrap();
            assert_eq!(advance, 10.0);
            let upright: bool = dict
                .get_item("upright")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert!(upright);
            let matrix: (f64, f64, f64, f64, f64, f64) =
                dict.get_item("matrix").unwrap().unwrap().extract().unwrap();
            assert_eq!(matrix, (1.0, 0.0, 0.0, 1.0, 0.0, 0.0));
            assert!(dict.get_item("mcid").unwrap().unwrap().is_none());
            assert!(dict.get_item("tag").unwrap().unwrap().is_none());
            let direction: String = dict
                .get_item("direction")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(direction, "ltr");
            let page_number: usize = dict
                .get_item("page_number")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(page_number, 7);
            let non_stroking_color: (f32,) = dict
                .get_item("non_stroking_color")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(non_stroking_color, (0.0,));
        });
    }

    #[test]
    fn test_word_to_dict_conversion() {
        let word = Word {
            text: "Hello".to_string(),
            bbox: BBox::new(10.0, 20.0, 60.0, 32.0),
            doctop: 20.0,
            direction: ::pdfplumber::TextDirection::Ltr,
            chars: vec![],
        };
        Python::with_gil(|py| {
            let dict_obj = word_to_dict(py, &word).expect("word_to_dict");
            let dict = dict_obj.downcast_bound::<PyDict>(py).expect("PyDict");
            let text: String = dict.get_item("text").unwrap().unwrap().extract().unwrap();
            assert_eq!(text, "Hello");
            let x0: f64 = dict.get_item("x0").unwrap().unwrap().extract().unwrap();
            assert!((x0 - 10.0).abs() < 0.01);
        });
    }

    #[test]
    fn test_line_to_dict_conversion() {
        let line = Line {
            x0: 10.0,
            top: 20.0,
            x1: 100.0,
            bottom: 20.0,
            line_width: 1.5,
            stroke_color: Color::Rgb(1.0, 0.0, 0.0),
            orientation: ::pdfplumber::Orientation::Horizontal,
        };
        Python::with_gil(|py| {
            let dict_obj = line_to_dict(py, &line, 7, 400.0, 100.0).expect("line_to_dict");
            let dict = dict_obj.downcast_bound::<PyDict>(py).expect("PyDict");
            let x0: f64 = dict.get_item("x0").unwrap().unwrap().extract().unwrap();
            assert!((x0 - 10.0).abs() < 0.01);
            let orientation: String = dict
                .get_item("orientation")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(orientation, "horizontal");
            let page_number: usize = dict
                .get_item("page_number")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(page_number, 7);
        });
    }

    #[test]
    fn test_rect_to_dict_conversion() {
        let rect = Rect {
            x0: 50.0,
            top: 100.0,
            x1: 200.0,
            bottom: 300.0,
            line_width: 2.0,
            stroke: true,
            fill: false,
            stroke_color: Color::Gray(0.0),
            fill_color: Color::Gray(1.0),
        };
        Python::with_gil(|py| {
            let dict_obj = rect_to_dict(py, &rect, 7, 400.0, 100.0).expect("rect_to_dict");
            let dict = dict_obj.downcast_bound::<PyDict>(py).expect("PyDict");
            let stroke: bool = dict.get_item("stroke").unwrap().unwrap().extract().unwrap();
            assert!(stroke);
            let fill: bool = dict.get_item("fill").unwrap().unwrap().extract().unwrap();
            assert!(!fill);
            let page_number: usize = dict
                .get_item("page_number")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(page_number, 7);
        });
    }

    #[test]
    fn test_curve_to_dict_conversion() {
        let curve = Curve {
            x0: 0.0,
            top: 50.0,
            x1: 100.0,
            bottom: 100.0,
            pts: vec![(0.0, 100.0), (30.0, 50.0), (70.0, 50.0), (100.0, 100.0)],
            line_width: 1.0,
            stroke: true,
            fill: false,
            stroke_color: Color::black(),
            fill_color: Color::black(),
        };
        Python::with_gil(|py| {
            let dict_obj = curve_to_dict(py, &curve, 7, 400.0, 100.0).expect("curve_to_dict");
            let dict = dict_obj.downcast_bound::<PyDict>(py).expect("PyDict");
            let stroke: bool = dict.get_item("stroke").unwrap().unwrap().extract().unwrap();
            assert!(stroke);
            let page_number: usize = dict
                .get_item("page_number")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(page_number, 7);
        });
    }

    #[test]
    fn test_image_to_dict_conversion() {
        let img = Image {
            x0: 0.0,
            top: 0.0,
            x1: 100.0,
            bottom: 100.0,
            width: 100.0,
            height: 100.0,
            name: "Im0".to_string(),
            src_width: Some(200),
            src_height: Some(200),
            bits_per_component: Some(8),
            color_space: Some("DeviceRGB".to_string()),
            data: None,
            filter: None,
            mime_type: None,
        };
        Python::with_gil(|py| {
            let dict_obj =
                page_image_to_dict(py, &img, 7, 400.0, 100.0).expect("page_image_to_dict");
            let dict = dict_obj.downcast_bound::<PyDict>(py).expect("PyDict");
            let name: String = dict.get_item("name").unwrap().unwrap().extract().unwrap();
            assert_eq!(name, "Im0");
            let srcsize: (u32, u32) = dict
                .get_item("srcsize")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(srcsize, (200, 200));
            let page_number: usize = dict
                .get_item("page_number")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(page_number, 7);
            let y0: f64 = dict.get_item("y0").unwrap().unwrap().extract().unwrap();
            assert_eq!(y0, 300.0);
            let y1: f64 = dict.get_item("y1").unwrap().unwrap().extract().unwrap();
            assert_eq!(y1, 400.0);
            let doctop: f64 = dict.get_item("doctop").unwrap().unwrap().extract().unwrap();
            assert_eq!(doctop, 100.0);

            let raw_obj = native_image_to_dict(py, &img).expect("native_image_to_dict");
            let raw = raw_obj.downcast_bound::<PyDict>(py).expect("PyDict");
            for key in ["page_number", "y0", "y1", "doctop"] {
                assert!(raw.get_item(key).unwrap().is_none());
            }
        });
    }

    #[test]
    fn test_python_object_key_adapter_preserves_native_struct_names() {
        let line = Line {
            x0: 10.0,
            top: 20.0,
            x1: 100.0,
            bottom: 20.0,
            line_width: 1.5,
            stroke_color: Color::Rgb(1.0, 0.0, 0.0),
            orientation: ::pdfplumber::Orientation::Horizontal,
        };
        let rect = Rect {
            x0: 50.0,
            top: 100.0,
            x1: 200.0,
            bottom: 300.0,
            line_width: 2.0,
            stroke: true,
            fill: false,
            stroke_color: Color::Gray(0.0),
            fill_color: Color::Gray(1.0),
        };
        let curve = Curve {
            x0: 0.0,
            top: 50.0,
            x1: 100.0,
            bottom: 100.0,
            pts: vec![(0.0, 100.0), (100.0, 100.0)],
            line_width: 1.0,
            stroke: true,
            fill: false,
            stroke_color: Color::black(),
            fill_color: Color::Gray(1.0),
        };
        let image = Image {
            x0: 0.0,
            top: 0.0,
            x1: 100.0,
            bottom: 100.0,
            width: 100.0,
            height: 100.0,
            name: "Im0".to_string(),
            src_width: Some(200),
            src_height: Some(150),
            bits_per_component: Some(8),
            color_space: Some("DeviceRGB".to_string()),
            data: None,
            filter: None,
            mime_type: None,
        };

        Python::with_gil(|py| {
            let line_object = line_to_dict(py, &line, 7, 400.0, 100.0).expect("compatible line");
            let line_dict = line_object.downcast_bound::<PyDict>(py).expect("line dict");
            assert_eq!(
                line_dict
                    .get_item("linewidth")
                    .unwrap()
                    .unwrap()
                    .extract::<f64>()
                    .unwrap(),
                1.5
            );
            assert!(line_dict.get_item("stroking_color").unwrap().is_some());
            for native_key in ["line_width", "stroke_color"] {
                assert!(line_dict.get_item(native_key).unwrap().is_none());
            }

            let rect_object = rect_to_dict(py, &rect, 7, 400.0, 100.0).expect("compatible rect");
            let rect_dict = rect_object.downcast_bound::<PyDict>(py).expect("rect dict");
            for python_key in ["linewidth", "stroking_color", "non_stroking_color"] {
                assert!(rect_dict.get_item(python_key).unwrap().is_some());
            }
            for native_key in ["line_width", "stroke_color", "fill_color"] {
                assert!(rect_dict.get_item(native_key).unwrap().is_none());
            }

            let curve_object =
                curve_to_dict(py, &curve, 7, 400.0, 100.0).expect("compatible curve");
            let curve_dict = curve_object
                .downcast_bound::<PyDict>(py)
                .expect("curve dict");
            for python_key in ["linewidth", "stroking_color", "non_stroking_color"] {
                assert!(curve_dict.get_item(python_key).unwrap().is_some());
            }
            for native_key in ["line_width", "stroke_color", "fill_color"] {
                assert!(curve_dict.get_item(native_key).unwrap().is_none());
            }

            let page_image_object =
                page_image_to_dict(py, &image, 7, 400.0, 100.0).expect("compatible page image");
            let page_image = page_image_object
                .downcast_bound::<PyDict>(py)
                .expect("page image dict");
            assert_eq!(
                page_image
                    .get_item("srcsize")
                    .unwrap()
                    .unwrap()
                    .extract::<(u32, u32)>()
                    .unwrap(),
                (200, 150)
            );
            for python_key in ["bits", "colorspace"] {
                assert!(page_image.get_item(python_key).unwrap().is_some());
            }
            for native_key in [
                "src_width",
                "src_height",
                "bits_per_component",
                "color_space",
            ] {
                assert!(page_image.get_item(native_key).unwrap().is_none());
            }

            let raw_image_object = native_image_to_dict(py, &image).expect("native raw image");
            let raw_image = raw_image_object
                .downcast_bound::<PyDict>(py)
                .expect("raw image dict");
            for native_key in [
                "src_width",
                "src_height",
                "bits_per_component",
                "color_space",
            ] {
                assert!(raw_image.get_item(native_key).unwrap().is_some());
            }
            for python_key in ["srcsize", "bits", "colorspace"] {
                assert!(raw_image.get_item(python_key).unwrap().is_none());
            }
        });
    }

    #[test]
    fn test_search_match_to_dict_conversion() {
        let m = SearchMatch {
            text: "Hello".to_string(),
            bbox: BBox::new(10.0, 20.0, 60.0, 32.0),
            page_number: 0,
            char_indices: vec![0, 1, 2, 3, 4],
        };
        Python::with_gil(|py| {
            let dict_obj = search_match_to_dict(py, &m).expect("search_match_to_dict");
            let dict = dict_obj.downcast_bound::<PyDict>(py).expect("PyDict");
            let text: String = dict.get_item("text").unwrap().unwrap().extract().unwrap();
            assert_eq!(text, "Hello");
            let page: usize = dict
                .get_item("page_number")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(page, 0);
        });
    }

    #[test]
    fn test_bookmark_to_dict_conversion() {
        let bm = Bookmark {
            title: "Chapter 1".to_string(),
            level: 0,
            page_number: Some(0),
            dest_top: Some(792.0),
        };
        Python::with_gil(|py| {
            let dict_obj = bookmark_to_dict(py, &bm).expect("bookmark_to_dict");
            let dict = dict_obj.downcast_bound::<PyDict>(py).expect("PyDict");
            let title: String = dict.get_item("title").unwrap().unwrap().extract().unwrap();
            assert_eq!(title, "Chapter 1");
            let level: usize = dict.get_item("level").unwrap().unwrap().extract().unwrap();
            assert_eq!(level, 0);
        });
    }

    #[test]
    fn test_metadata_to_dict_conversion() {
        let metadata = RawDocumentMetadata {
            entries: vec![
                ::pdfplumber::MetadataEntry {
                    key: "Title".to_string(),
                    value: MetadataValue::String("Test Doc".to_string()),
                    resolution_error: None,
                },
                ::pdfplumber::MetadataEntry {
                    key: "Count".to_string(),
                    value: MetadataValue::Integer(2),
                    resolution_error: None,
                },
            ],
        };
        Python::with_gil(|py| {
            let dict_obj = raw_metadata_to_dict(py, &metadata).expect("raw_metadata_to_dict");
            let dict = dict_obj.downcast_bound::<PyDict>(py).expect("PyDict");
            let title: String = dict.get_item("Title").unwrap().unwrap().extract().unwrap();
            assert_eq!(title, "Test Doc");
            let count: i64 = dict.get_item("Count").unwrap().unwrap().extract().unwrap();
            assert_eq!(count, 2);
        });
    }

    #[test]
    fn test_cropped_page_methods() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let pypage = pypage.into_py_for_test(py).expect("bind page");
            let cropped =
                PyPage::crop(pypage.bind(py).borrow(), py, (0.0, 0.0, 200.0, 300.0)).expect("crop");
            let cropped = cropped.bind(py).borrow();
            assert!((cropped.width() - 200.0).abs() < 0.1);
            assert!((cropped.height() - 300.0).abs() < 0.1);
            assert!(cropped.inner.chars().is_empty());
            assert!(cropped.inner.lines().is_empty());
            assert!(cropped.inner.rects().is_empty());
            assert!(cropped.inner.curves().is_empty());
            assert!(cropped.inner.images().is_empty());
            assert!(
                cropped
                    .extract_words(py, 3.0, 3.0)
                    .expect("words")
                    .is_empty()
            );
            assert!(cropped.extract_text(false).is_empty());
            assert!(cropped.find_tables().is_empty());
            assert!(cropped.extract_tables().is_empty());
        });
    }

    #[test]
    fn test_cropped_page_further_crop() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let pypage = pypage.into_py_for_test(py).expect("bind page");
            let cropped =
                PyPage::crop(pypage.bind(py).borrow(), py, (0.0, 0.0, 400.0, 500.0)).expect("crop");
            let further =
                PyCroppedPage::crop(cropped.bind(py).borrow(), py, (0.0, 0.0, 200.0, 250.0))
                    .expect("further crop");
            let further = further.bind(py).borrow();
            assert!((further.width() - 200.0).abs() < 0.1);
            assert!((further.height() - 250.0).abs() < 0.1);
        });
    }

    #[test]
    fn test_cropped_page_within_bbox() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let pypage = pypage.into_py_for_test(py).expect("bind page");
            let cropped =
                PyPage::crop(pypage.bind(py).borrow(), py, (0.0, 0.0, 400.0, 500.0)).expect("crop");
            let within = PyCroppedPage::within_bbox(
                cropped.bind(py).borrow(),
                py,
                (50.0, 50.0, 150.0, 150.0),
            )
            .expect("within bbox");
            let within = within.bind(py).borrow();
            assert!((within.width() - 100.0).abs() < 0.1);
            assert!((within.height() - 100.0).abs() < 0.1);
        });
    }

    #[test]
    fn test_pytable_bbox() {
        let table = Table {
            bbox: BBox::new(10.0, 20.0, 300.0, 400.0),
            cells: vec![],
            rows: vec![],
            columns: vec![],
        };
        let pytable = PyTable { inner: table };
        let bbox = pytable.bbox();
        assert!((bbox.0 - 10.0).abs() < 0.01);
        assert!((bbox.1 - 20.0).abs() < 0.01);
        assert!((bbox.2 - 300.0).abs() < 0.01);
        assert!((bbox.3 - 400.0).abs() < 0.01);
    }

    #[test]
    fn test_pytable_accuracy() {
        let table = Table {
            bbox: BBox::new(0.0, 0.0, 100.0, 100.0),
            cells: vec![
                ::pdfplumber::Cell {
                    bbox: BBox::new(0.0, 0.0, 50.0, 50.0),
                    text: Some("data".to_string()),
                },
                ::pdfplumber::Cell {
                    bbox: BBox::new(50.0, 0.0, 100.0, 50.0),
                    text: None,
                },
            ],
            rows: vec![],
            columns: vec![],
        };
        let pytable = PyTable { inner: table };
        assert!((pytable.accuracy() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_bbox_tuple() {
        let bbox = parse_bbox_tuple((10.0, 20.0, 30.0, 40.0));
        assert!((bbox.x0 - 10.0).abs() < 0.01);
        assert!((bbox.top - 20.0).abs() < 0.01);
        assert!((bbox.x1 - 30.0).abs() < 0.01);
        assert!((bbox.bottom - 40.0).abs() < 0.01);
    }

    // -----------------------------------------------------------------------
    // US-075 tests: PyPI packaging
    // -----------------------------------------------------------------------

    #[test]
    fn test_version_constant_matches_cargo_toml() {
        // VERSION should be a valid semver string from Cargo.toml
        assert!(!VERSION.is_empty(), "VERSION must not be empty");
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "VERSION should be semver (major.minor.patch)"
        );
        for part in &parts {
            part.parse::<u32>()
                .unwrap_or_else(|_| panic!("VERSION part '{part}' is not a valid number"));
        }
    }

    #[test]
    fn test_version_matches_workspace() {
        // The pdfplumber-py version should match the main pdfplumber crate version
        assert_eq!(
            VERSION,
            env!("CARGO_PKG_VERSION"),
            "VERSION constant must match CARGO_PKG_VERSION"
        );
    }

    #[test]
    fn test_version_is_registered_in_module_init() {
        // Verify the module init function registers __version__.
        // We cannot import the compiled extension in a pure Rust unit test,
        // but we can verify the VERSION constant is the value that will be used.
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
        // The module init (fn pdfplumber) adds: m.add("__version__", VERSION)
        // This is verified by the constant being non-empty and valid semver.
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_type_stubs_exist() {
        // The .pyi file should exist alongside the crate
        let stubs_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("python/pdfplumber/_native.pyi");
        assert!(
            stubs_path.exists(),
            "Type stubs file pdfplumber/_native.pyi should exist at {}",
            stubs_path.display()
        );
    }

    #[test]
    fn test_type_stubs_content() {
        let stubs_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("python/pdfplumber/_native.pyi");
        let content = std::fs::read_to_string(&stubs_path).expect("read .pyi file");
        // Must declare the main classes
        assert!(
            content.contains("class PDF:"),
            "stubs must declare PDF class"
        );
        assert!(
            content.contains("class Page:"),
            "stubs must declare Page class"
        );
        assert!(
            content.contains("class Table:"),
            "stubs must declare Table class"
        );
        assert!(
            content.contains("class CroppedPage:"),
            "stubs must declare CroppedPage class"
        );
        // Must declare exception types
        assert!(
            content.contains("class PdfParseError"),
            "stubs must declare PdfParseError"
        );
        // Must have __version__
        assert!(
            content.contains("__version__"),
            "stubs must declare __version__"
        );
        assert_eq!(
            content
                .matches("def flush_cache(self, properties: list[str] | None = None) -> None:")
                .count(),
            3,
            "stubs must declare PDF, Page, and CroppedPage.flush_cache"
        );
        assert_eq!(
            content.matches("    def close(self) -> None:").count(),
            2,
            "stubs must declare Page and CroppedPage.close"
        );
        assert_eq!(
            content
                .matches("def objects(self) -> dict[str, list[dict[str, object]]]:")
                .count(),
            3,
            "stubs must declare PDF, Page, and CroppedPage.objects"
        );
        for declaration in [
            "    def textboxhorizontals(self) -> list[dict[str, object]]:",
            "    def textboxverticals(self) -> list[dict[str, object]]:",
            "    def textlinehorizontals(self) -> list[dict[str, object]]:",
            "    def textlineverticals(self) -> list[dict[str, object]]:",
        ] {
            assert_eq!(
                content.matches(declaration).count(),
                3,
                "stubs must declare document, page, and cropped-page layout properties"
            );
        }
        for declaration in [
            "    @property\n    def chars(self) -> list[CharDict]:",
            "    @property\n    def lines(self) -> list[LineDict]:",
            "    @property\n    def rects(self) -> list[RectDict]:",
            "    @property\n    def curves(self) -> list[CurveDict]:",
            "    @property\n    def images(self) -> list[ImageDict]:",
        ] {
            assert_eq!(
                content.matches(declaration).count(),
                2,
                "stubs must declare Page and CroppedPage object-list properties"
            );
        }
        assert!(
            content.contains("def annots(self) -> list[AnnotDict]:"),
            "stubs must declare document and page annotations"
        );
        assert!(
            content.contains("def hyperlinks(self) -> list[AnnotDict]:"),
            "stubs must declare document and page hyperlinks"
        );
        assert!(
            content.contains("def structure_tree(self) -> list[StructElementDict]:"),
            "stubs must declare document and page structure trees"
        );
        assert!(
            content.contains("def initial_doctop(self) -> int | float:"),
            "stubs must declare Page.initial_doctop"
        );
        assert_eq!(
            content.matches("is_original: ClassVar[bool]").count(),
            2,
            "stubs must declare Page and CroppedPage.is_original"
        );
        assert_eq!(
            content.matches("root_page: Page").count(),
            2,
            "stubs must declare Page and CroppedPage.root_page"
        );
        assert_eq!(
            content
                .matches("mediabox: tuple[float, float, float, float]")
                .count(),
            2,
            "stubs must declare writable Page and CroppedPage.mediabox"
        );
        assert!(
            content.contains("parent_page: Page | CroppedPage"),
            "stubs must declare CroppedPage.parent_page"
        );
        assert_eq!(
            content.matches("def point2coord(").count(),
            2,
            "stubs must declare Page and CroppedPage.point2coord"
        );
        assert_eq!(
            content
                .matches(
                    "def to_dict(self, object_types: Iterable[str] | None = None) -> dict[str, object]:"
                )
                .count(),
            2,
            "stubs must declare document and page dictionary serialization"
        );
        assert_eq!(
            content.matches("def to_json(").count(),
            2,
            "stubs must declare document and page JSON serialization"
        );
        assert_eq!(
            content.matches("def to_csv(").count(),
            2,
            "stubs must declare document and page CSV serialization"
        );
    }

    #[test]
    fn test_pyproject_toml_has_required_metadata() {
        let pyproject_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pyproject.toml");
        let content = std::fs::read_to_string(&pyproject_path).expect("read pyproject.toml");
        assert!(
            content.contains("name = \"pdfplumber-rs\""),
            "pyproject.toml must have name = 'pdfplumber-rs'"
        );
        assert!(
            content.contains("description"),
            "pyproject.toml must have description"
        );
        assert!(
            content.contains("license"),
            "pyproject.toml must have license"
        );
        assert!(
            content.contains("requires-python"),
            "pyproject.toml must have requires-python"
        );
        assert!(
            content.contains("classifiers"),
            "pyproject.toml must have classifiers"
        );
    }

    #[test]
    fn test_readme_exists_for_pypi() {
        let readme_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
        assert!(
            readme_path.exists(),
            "README.md should exist for PyPI at {}",
            readme_path.display()
        );
        let content = std::fs::read_to_string(&readme_path).expect("read README.md");
        assert!(
            content.contains("install"),
            "README must contain installation instructions"
        );
        assert!(
            content.contains("pdfplumber"),
            "README must reference pdfplumber"
        );
    }
}
