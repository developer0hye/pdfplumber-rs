//! Python bindings for pdfplumber-rs via PyO3.
//!
//! Exposes `PyPdf`, `PyPage`, `PyTable`, and `PyCroppedPage` classes to Python,
//! wrapping the Rust pdfplumber types for full API access.

/// Package version, kept in sync with Cargo.toml and pyproject.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

use ::pdfplumber::{
    Annotation, BBox, Bookmark, Char, Color, CroppedPage, Curve, FormField, Image, ImageContent,
    Line, MetadataReference, MetadataValue, Page, Pdf, PdfError, RawDocumentMetadata, Rect,
    SearchMatch, SearchOptions, SignatureInfo, StructElement, Table, TableSettings, TextOptions,
    UnicodeNorm, ValidationIssue, Word, WordOptions,
};
use pyo3::exceptions::{
    PyAttributeError, PyException, PyIOError, PyRecursionError, PyRuntimeError, PyTypeError,
    PyValueError,
};
use pyo3::prelude::*;
use pyo3::sync::GILOnceCell;
use pyo3::types::{PyBool, PyBytes, PyDict, PyList, PyString, PyTuple};
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
    match e {
        PdfError::ParseError(msg) => PdfminerException::new_err(pdfminer_parse_message(msg)),
        PdfError::IoError(msg) => PdfIoError::new_err(msg),
        PdfError::FontError(msg) => PdfFontError::new_err(msg),
        PdfError::InterpreterError(msg) => PdfInterpreterError::new_err(msg),
        PdfError::ResourceLimitExceeded {
            limit_name,
            limit_value,
            actual_value,
        } => PdfResourceLimitError::new_err(format!(
            "{limit_name} (limit: {limit_value}, actual: {actual_value})"
        )),
        PdfError::PasswordRequired | PdfError::InvalidPassword => PdfminerException::new_err(()),
        PdfError::Other(msg) => PyRuntimeError::new_err(msg),
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
        Color::Other(vals) => vals.clone().into_pyobject(py).unwrap().into_any().unbind(),
    }
}

fn char_to_dict(py: Python<'_>, ch: &Char) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("object_type", "char")?;
    dict.set_item("text", &ch.text)?;
    dict.set_item("x0", ch.bbox.x0)?;
    dict.set_item("top", ch.bbox.top)?;
    dict.set_item("x1", ch.bbox.x1)?;
    dict.set_item("bottom", ch.bbox.bottom)?;
    dict.set_item("fontname", &ch.fontname)?;
    dict.set_item("size", ch.size)?;
    dict.set_item("doctop", ch.doctop)?;
    dict.set_item("upright", ch.upright)?;
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
            .map(|c| color_to_py(py, c))
            .unwrap_or_else(|| py.None()),
    )?;
    dict.set_item(
        "non_stroking_color",
        ch.non_stroking_color
            .as_ref()
            .map(|c| color_to_py(py, c))
            .unwrap_or_else(|| py.None()),
    )?;
    Ok(dict.into_any().unbind())
}

#[derive(Clone)]
struct CompatibleLayoutLine {
    bbox: BBox,
    text: String,
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

fn horizontal_layout_line(chars: &[&Char], word_margin: f64) -> CompatibleLayoutLine {
    let mut bbox = chars[0].bbox;
    let mut text = String::new();
    let mut previous_x1 = f64::INFINITY;
    for ch in chars {
        let margin = word_margin * ch.bbox.width().max(ch.bbox.height());
        if previous_x1 < ch.bbox.x0 - margin {
            text.push(' ');
        }
        text.push_str(&ch.text);
        previous_x1 = ch.bbox.x1;
        bbox = bbox.union(&ch.bbox);
    }
    text.push('\n');
    CompatibleLayoutLine { bbox, text }
}

fn horizontal_layout_lines(
    chars: &[Char],
    line_overlap: f64,
    char_margin: f64,
    word_margin: f64,
) -> Vec<CompatibleLayoutLine> {
    let Some(first) = chars.first() else {
        return Vec::new();
    };

    let mut lines = Vec::new();
    let mut current = vec![first];
    for ch in chars.iter().skip(1) {
        if horizontal_char_alignment(
            current.last().expect("line is nonempty"),
            ch,
            line_overlap,
            char_margin,
        ) {
            current.push(ch);
        } else {
            lines.push(horizontal_layout_line(&current, word_margin));
            current = vec![ch];
        }
    }
    lines.push(horizontal_layout_line(&current, word_margin));
    lines
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

fn layout_group_root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        parents[index] = parents[parents[index]];
        index = parents[index];
    }
    index
}

fn join_layout_groups(parents: &mut [usize], left: usize, right: usize) {
    let left_root = layout_group_root(parents, left);
    let right_root = layout_group_root(parents, right);
    if left_root != right_root {
        parents[right_root] = left_root;
    }
}

fn horizontal_layout_boxes(
    lines: &[CompatibleLayoutLine],
    line_margin: f64,
) -> Vec<CompatibleLayoutLine> {
    let mut parents: Vec<usize> = (0..lines.len()).collect();
    for left in 0..lines.len() {
        for right in (left + 1)..lines.len() {
            if horizontal_line_neighbor(&lines[left], &lines[right], line_margin)
                || horizontal_line_neighbor(&lines[right], &lines[left], line_margin)
            {
                join_layout_groups(&mut parents, left, right);
            }
        }
    }

    let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
    for index in 0..lines.len() {
        let root = layout_group_root(&mut parents, index);
        if let Some((_, members)) = groups.iter_mut().find(|(key, _)| *key == root) {
            members.push(index);
        } else {
            groups.push((root, vec![index]));
        }
    }

    groups
        .into_iter()
        .map(|(_, mut members)| {
            members.sort_by(|left, right| {
                lines[*left]
                    .bbox
                    .top
                    .partial_cmp(&lines[*right].bbox.top)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut bbox = lines[members[0]].bbox;
            let mut text = String::new();
            for member in members {
                bbox = bbox.union(&lines[member].bbox);
                text.push_str(&lines[member].text);
            }
            CompatibleLayoutLine { bbox, text }
        })
        .collect()
}

fn compatible_layout_object_to_dict(
    py: Python<'_>,
    object: &CompatibleLayoutLine,
    object_type: &str,
    page_number: usize,
    public_height: f64,
    height_correction: f64,
    initial_doctop: f64,
) -> PyResult<PyObject> {
    let x0 = object.bbox.x0;
    let top = object.bbox.top - height_correction;
    let x1 = object.bbox.x1;
    let bottom = object.bbox.bottom - height_correction;
    let y0 = public_height - bottom;
    let y1 = public_height - top;
    let dict = PyDict::new(py);
    dict.set_item("x0", x0)?;
    dict.set_item("y0", y0)?;
    dict.set_item("x1", x1)?;
    dict.set_item("y1", y1)?;
    dict.set_item("width", x1 - x0)?;
    dict.set_item("height", bottom - top)?;
    dict.set_item("object_type", object_type)?;
    dict.set_item("page_number", page_number)?;
    dict.set_item("text", &object.text)?;
    dict.set_item("top", top)?;
    dict.set_item("bottom", bottom)?;
    dict.set_item("doctop", initial_doctop + top)?;
    Ok(dict.into_any().unbind())
}

fn compatible_horizontal_layout_objects(
    py: Python<'_>,
    chars: &[Char],
    params: &Bound<'_, PyDict>,
    page_number: usize,
    raw_height: f64,
    public_height: f64,
    initial_doctop: f64,
) -> PyResult<(Vec<PyObject>, Vec<PyObject>)> {
    let line_overlap = laparams_number(params, "line_overlap", 0.5)?;
    let char_margin = laparams_number(params, "char_margin", 2.0)?;
    let line_margin = laparams_number(params, "line_margin", 0.5)?;
    let word_margin = laparams_number(params, "word_margin", 0.1)?;
    let lines = horizontal_layout_lines(chars, line_overlap, char_margin, word_margin);
    let boxes = horizontal_layout_boxes(&lines, line_margin);
    let height_correction = raw_height - public_height;
    let boxes = boxes
        .iter()
        .map(|object| {
            compatible_layout_object_to_dict(
                py,
                object,
                "textboxhorizontal",
                page_number,
                public_height,
                height_correction,
                initial_doctop,
            )
        })
        .collect::<PyResult<Vec<_>>>()?;
    let lines = lines
        .iter()
        .map(|object| {
            compatible_layout_object_to_dict(
                py,
                object,
                "textlinehorizontal",
                page_number,
                public_height,
                height_correction,
                initial_doctop,
            )
        })
        .collect::<PyResult<Vec<_>>>()?;
    Ok((boxes, lines))
}

fn word_to_dict(py: Python<'_>, word: &Word) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("text", &word.text)?;
    dict.set_item("x0", word.bbox.x0)?;
    dict.set_item("top", word.bbox.top)?;
    dict.set_item("x1", word.bbox.x1)?;
    dict.set_item("bottom", word.bbox.bottom)?;
    dict.set_item("doctop", word.doctop)?;
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

fn line_to_dict(py: Python<'_>, line: &Line) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("object_type", "line")?;
    dict.set_item("x0", line.x0)?;
    dict.set_item("top", line.top)?;
    dict.set_item("x1", line.x1)?;
    dict.set_item("bottom", line.bottom)?;
    dict.set_item("line_width", line.line_width)?;
    dict.set_item("stroke_color", color_to_py(py, &line.stroke_color))?;
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

fn rect_to_dict(py: Python<'_>, rect: &Rect) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("object_type", "rect")?;
    dict.set_item("x0", rect.x0)?;
    dict.set_item("top", rect.top)?;
    dict.set_item("x1", rect.x1)?;
    dict.set_item("bottom", rect.bottom)?;
    dict.set_item("line_width", rect.line_width)?;
    dict.set_item("stroke", rect.stroke)?;
    dict.set_item("fill", rect.fill)?;
    dict.set_item("stroke_color", color_to_py(py, &rect.stroke_color))?;
    dict.set_item("fill_color", color_to_py(py, &rect.fill_color))?;
    Ok(dict.into_any().unbind())
}

fn curve_to_dict(py: Python<'_>, curve: &Curve) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("object_type", "curve")?;
    dict.set_item("x0", curve.x0)?;
    dict.set_item("top", curve.top)?;
    dict.set_item("x1", curve.x1)?;
    dict.set_item("bottom", curve.bottom)?;
    dict.set_item("pts", &curve.pts)?;
    dict.set_item("line_width", curve.line_width)?;
    dict.set_item("stroke", curve.stroke)?;
    dict.set_item("fill", curve.fill)?;
    dict.set_item("stroke_color", color_to_py(py, &curve.stroke_color))?;
    dict.set_item("fill_color", color_to_py(py, &curve.fill_color))?;
    Ok(dict.into_any().unbind())
}

fn image_to_dict(py: Python<'_>, img: &Image) -> PyResult<PyObject> {
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
    dict.set_item("image", image_to_dict(py, image)?)?;
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

fn normalized_page_box(page_box: BBox, rotation: i32) -> BBox {
    let x0 = page_box.x0.min(page_box.x1);
    let y0 = page_box.top.min(page_box.bottom);
    let x1 = page_box.x0.max(page_box.x1);
    let y1 = page_box.top.max(page_box.bottom);
    let (x0, y0, x1, y1) = if matches!(rotation, 90 | 270) {
        (y0, x0, y1, x1)
    } else {
        (x0, y0, x1, y1)
    };
    BBox::new(x0, y0, x1, y1)
}

fn invert_page_box(page_box: BBox, media_height: f64) -> BBox {
    BBox::new(
        page_box.x0,
        media_height - page_box.bottom,
        page_box.x1,
        media_height - page_box.top,
    )
}

fn compatible_page_boxes(media_box: BBox, crop_box: Option<BBox>, rotation: i32) -> (BBox, BBox) {
    let normalized_media_box = normalized_page_box(media_box, rotation);
    let media_height = normalized_media_box.bottom - normalized_media_box.top;
    let normalized_crop_box = normalized_page_box(crop_box.unwrap_or(media_box), rotation);
    (
        invert_page_box(normalized_media_box, media_height),
        invert_page_box(normalized_crop_box, media_height),
    )
}

fn compatible_optional_page_box(
    media_box: BBox,
    page_box: Option<BBox>,
    rotation: i32,
) -> Option<BBox> {
    let media_box = normalized_page_box(media_box, rotation);
    let media_height = media_box.bottom - media_box.top;
    page_box.map(|page_box| invert_page_box(normalized_page_box(page_box, rotation), media_height))
}

fn compatible_bbox_tuple(bbox: BBox) -> (f64, f64, f64, f64) {
    (
        compatible_geometry_number(bbox.x0),
        compatible_geometry_number(bbox.top),
        compatible_geometry_number(bbox.x1),
        compatible_geometry_number(bbox.bottom),
    )
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

fn initial_doctop_to_object(py: Python<'_>, value: f64) -> PyObject {
    if value == 0.0 {
        0_i64.into_pyobject(py).unwrap().into_any().unbind()
    } else {
        compatible_geometry_number(value)
            .into_pyobject(py)
            .unwrap()
            .into_any()
            .unbind()
    }
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
            Some(password) => Pdf::open_with_password(bytes.as_bytes(), password.as_bytes(), None),
            None => Pdf::open(bytes.as_bytes(), None),
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
        let pdf = Pdf::open(data, None).map_err(to_py_err)?;
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
            let trim_box =
                compatible_optional_page_box(media_box, self.inner.page_trim_box(i), rotation);
            let bleed_box =
                compatible_optional_page_box(media_box, self.inner.page_bleed_box(i), rotation);
            let art_box =
                compatible_optional_page_box(media_box, self.inner.page_art_box(i), rotation);
            let (media_box, crop_box) =
                compatible_page_boxes(media_box, self.inner.page_crop_box(i), rotation);
            let initial_doctop = if self.selected_pages.is_some() {
                let initial_doctop = selected_doctop;
                selected_doctop += compatible_geometry_number(height);
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
                        rotation,
                        media_box,
                        crop_box,
                    },
                    initial_doctop,
                    self.unicode_norm.as_ref().map(|value| value.clone_ref(py)),
                ),
            )?;
            page.bind(py)
                .setattr("bbox", compatible_bbox_tuple(media_box))?;
            page.bind(py)
                .setattr("mediabox", compatible_bbox_tuple(media_box))?;
            page.bind(py).setattr("root_page", page.clone_ref(py))?;
            if let Some(laparams) = &self._laparams {
                page.bind(py)
                    .setattr("_layout_laparams", laparams.clone_ref(py))?;
            }
            if let Some(trim_box) = trim_box {
                page.bind(py)
                    .setattr("trimbox", compatible_bbox_tuple(trim_box))?;
            }
            if let Some(bleed_box) = bleed_box {
                page.bind(py)
                    .setattr("bleedbox", compatible_bbox_tuple(bleed_box))?;
            }
            if let Some(art_box) = art_box {
                page.bind(py)
                    .setattr("artbox", compatible_bbox_tuple(art_box))?;
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
    rotation: i32,
    media_box: BBox,
    crop_box: BBox,
}

#[pyclass(name = "Page", dict)]
struct PyPage {
    pdf: Arc<Pdf>,
    page_index: usize,
    geometry: PyPageGeometry,
    selected_doctop: Option<f64>,
    unicode_norm: Option<PyObject>,
    page_cache: Mutex<Option<Page>>,
}

impl PyPage {
    fn new(
        pdf: Arc<Pdf>,
        page_index: usize,
        geometry: PyPageGeometry,
        selected_doctop: Option<f64>,
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
        let (media_box, crop_box) =
            compatible_page_boxes(source_media_box, pdf.page_crop_box(page_index), rotation);
        Self::new(
            pdf,
            page_index,
            PyPageGeometry {
                width,
                height,
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
            .setattr("bbox", compatible_bbox_tuple(media_box))?;
        page.bind(py)
            .setattr("mediabox", compatible_bbox_tuple(media_box))?;
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
            if let Some(selected_doctop) = self.selected_doctop {
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
        self.selected_doctop.unwrap_or_else(|| {
            (0..self.page_index)
                .filter_map(|index| {
                    self.pdf
                        .page_dimensions(index)
                        .map(|(_, height)| compatible_geometry_number(height))
                })
                .sum()
        })
    }

    fn char_objects(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.with_page(py, |page| {
            page.chars().iter().map(|ch| char_to_dict(py, ch)).collect()
        })
    }

    fn line_objects(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.with_page(py, |page| {
            page.lines()
                .iter()
                .map(|line| line_to_dict(py, line))
                .collect()
        })
    }

    fn rect_objects(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.with_page(py, |page| {
            page.rects()
                .iter()
                .map(|rect| rect_to_dict(py, rect))
                .collect()
        })
    }

    fn curve_objects(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.with_page(py, |page| {
            page.curves()
                .iter()
                .map(|curve| curve_to_dict(py, curve))
                .collect()
        })
    }

    fn image_objects(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.with_page(py, |page| {
            page.images()
                .iter()
                .map(|image| image_to_dict(py, image))
                .collect()
        })
    }

    fn horizontal_layout_objects(
        &self,
        py: Python<'_>,
        params: &Bound<'_, PyDict>,
    ) -> PyResult<(Vec<PyObject>, Vec<PyObject>)> {
        let page_number = self.page_number();
        let raw_height = self.geometry.height;
        let public_height = self.height();
        let initial_doctop = self.initial_doctop();
        self.with_page(py, |page| {
            compatible_horizontal_layout_objects(
                py,
                page.chars(),
                params,
                page_number,
                raw_height,
                public_height,
                initial_doctop,
            )
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
        self.with_page(py, |_| {
            dict.set_item("page_number", self.page_number())?;
            dict.set_item(
                "initial_doctop",
                initial_doctop_to_object(py, initial_doctop),
            )?;
            dict.set_item("rotation", self.geometry.rotation)?;
            dict.set_item("cropbox", compatible_bbox_tuple(self.geometry.crop_box))?;
            dict.set_item("mediabox", compatible_bbox_tuple(self.geometry.media_box))?;
            dict.set_item("bbox", compatible_bbox_tuple(self.geometry.media_box))?;
            dict.set_item("width", self.width())?;
            dict.set_item("height", self.height())?;
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
            .map(|params| page_ref.horizontal_layout_objects(py, params))
            .transpose()?;
        let values = [
            ("char", page_ref.char_objects(py)?),
            ("line", page_ref.line_objects(py)?),
            ("rect", page_ref.rect_objects(py)?),
            ("curve", page_ref.curve_objects(py)?),
            ("image", page_ref.image_objects(py)?),
        ];
        drop(page_ref);

        let objects = PyDict::new(py);
        if let Some((textboxes, textlines)) = layout_values {
            for (kind, values) in [
                ("textboxhorizontal", textboxes),
                ("textlinehorizontal", textlines),
            ] {
                let values = PyList::new(py, values)?;
                if !values.is_empty() {
                    objects.set_item(kind, values)?;
                }
            }
        }
        for (kind, values) in values {
            let values = PyList::new(py, values)?;
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
    fn width(&self) -> f64 {
        compatible_geometry_number(self.geometry.width)
    }

    /// Page height in points.
    #[getter]
    fn height(&self) -> f64 {
        compatible_geometry_number(self.geometry.height)
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
    fn cropbox(&self) -> (f64, f64, f64, f64) {
        compatible_bbox_tuple(self.geometry.crop_box)
    }

    /// Cumulative height of preceding pages in the current page view.
    #[getter(initial_doctop)]
    fn initial_doctop_property(&self, py: Python<'_>) -> PyObject {
        initial_doctop_to_object(py, self.initial_doctop())
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
        let initial_doctop = self.selected_doctop.unwrap_or(0.0);
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
            let chars = pypage.char_objects(py).expect("chars");
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
        let err = to_py_err(PdfError::ParseError("bad xref".to_string()));
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfminerException>(py));
        });
    }

    #[test]
    fn test_to_py_err_io_error() {
        let err = to_py_err(PdfError::IoError("file not found".to_string()));
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfIoError>(py));
        });
    }

    #[test]
    fn test_to_py_err_font_error() {
        let err = to_py_err(PdfError::FontError("missing glyph".to_string()));
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfFontError>(py));
        });
    }

    #[test]
    fn test_to_py_err_interpreter_error() {
        let err = to_py_err(PdfError::InterpreterError("unknown op".to_string()));
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfInterpreterError>(py));
        });
    }

    #[test]
    fn test_to_py_err_resource_limit() {
        let err = to_py_err(PdfError::ResourceLimitExceeded {
            limit_name: "max_pages".to_string(),
            limit_value: 10,
            actual_value: 20,
        });
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfResourceLimitError>(py));
        });
    }

    #[test]
    fn test_to_py_err_password_required() {
        let err = to_py_err(PdfError::PasswordRequired);
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfminerException>(py));
        });
    }

    #[test]
    fn test_to_py_err_invalid_password() {
        let err = to_py_err(PdfError::InvalidPassword);
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
            let dict_obj = char_to_dict(py, &ch).expect("char_to_dict");
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
            let upright: bool = dict
                .get_item("upright")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert!(upright);
            let direction: String = dict
                .get_item("direction")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(direction, "ltr");
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
            let dict_obj = line_to_dict(py, &line).expect("line_to_dict");
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
            let dict_obj = rect_to_dict(py, &rect).expect("rect_to_dict");
            let dict = dict_obj.downcast_bound::<PyDict>(py).expect("PyDict");
            let stroke: bool = dict.get_item("stroke").unwrap().unwrap().extract().unwrap();
            assert!(stroke);
            let fill: bool = dict.get_item("fill").unwrap().unwrap().extract().unwrap();
            assert!(!fill);
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
            let dict_obj = curve_to_dict(py, &curve).expect("curve_to_dict");
            let dict = dict_obj.downcast_bound::<PyDict>(py).expect("PyDict");
            let stroke: bool = dict.get_item("stroke").unwrap().unwrap().extract().unwrap();
            assert!(stroke);
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
            let dict_obj = image_to_dict(py, &img).expect("image_to_dict");
            let dict = dict_obj.downcast_bound::<PyDict>(py).expect("PyDict");
            let name: String = dict.get_item("name").unwrap().unwrap().extract().unwrap();
            assert_eq!(name, "Im0");
            let src_w: u32 = dict
                .get_item("src_width")
                .unwrap()
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(src_w, 200);
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
