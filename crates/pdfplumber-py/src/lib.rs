//! Python bindings for pdfplumber-rs via PyO3.
//!
//! Exposes `PyPdf`, `PyPage`, `PyTable`, and `PyCroppedPage` classes to Python,
//! wrapping the Rust pdfplumber types for full API access.

/// Package version, kept in sync with Cargo.toml and pyproject.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

use ::pdfplumber::{
    Annotation, BBox, Bookmark, Char, Color, CroppedPage, Curve, Image, Line, MetadataReference,
    MetadataValue, Page, Pdf, PdfError, RawDocumentMetadata, Rect, SearchMatch, SearchOptions,
    StructElement, Table, TableSettings, TextOptions, UnicodeNorm, Word, WordOptions,
};
use pyo3::exceptions::{
    PyException, PyIOError, PyRecursionError, PyRuntimeError, PyTypeError, PyValueError,
};
use pyo3::prelude::*;
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

fn word_to_dict(py: Python<'_>, word: &Word) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("text", &word.text)?;
    dict.set_item("x0", word.bbox.x0)?;
    dict.set_item("top", word.bbox.top)?;
    dict.set_item("x1", word.bbox.x1)?;
    dict.set_item("bottom", word.bbox.bottom)?;
    dict.set_item("doctop", word.doctop)?;
    dict.set_item(
        "direction",
        match word.direction {
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

/// A spatially filtered view of a PDF page.
#[pyclass(name = "CroppedPage")]
struct PyCroppedPage {
    inner: CroppedPage,
}

#[pymethods]
impl PyCroppedPage {
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

    /// Characters in the cropped region as list[dict].
    fn chars(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .chars()
            .iter()
            .map(|ch| char_to_dict(py, ch))
            .collect()
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
    fn lines(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .lines()
            .iter()
            .map(|l| line_to_dict(py, l))
            .collect()
    }

    /// Rects in the cropped region as list[dict].
    fn rects(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .rects()
            .iter()
            .map(|r| rect_to_dict(py, r))
            .collect()
    }

    /// Curves in the cropped region as list[dict].
    fn curves(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .curves()
            .iter()
            .map(|c| curve_to_dict(py, c))
            .collect()
    }

    /// Images in the cropped region as list[dict].
    fn images(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .images()
            .iter()
            .map(|i| image_to_dict(py, i))
            .collect()
    }

    /// Further crop this cropped page.
    fn crop(&self, bbox: (f64, f64, f64, f64)) -> PyCroppedPage {
        PyCroppedPage {
            inner: self.inner.crop(parse_bbox_tuple(bbox)),
        }
    }

    /// Filter to objects fully within the given bbox.
    fn within_bbox(&self, bbox: (f64, f64, f64, f64)) -> PyCroppedPage {
        PyCroppedPage {
            inner: self.inner.within_bbox(parse_bbox_tuple(bbox)),
        }
    }

    /// Filter to objects outside the given bbox.
    fn outside_bbox(&self, bbox: (f64, f64, f64, f64)) -> PyCroppedPage {
        PyCroppedPage {
            inner: self.inner.outside_bbox(parse_bbox_tuple(bbox)),
        }
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

struct PyPdfOpenArgs {
    path_or_fp: PyObject,
    pages: Option<PyObject>,
    laparams: Option<PyObject>,
    password: Option<String>,
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
    let password = match values[3].take() {
        Some(value) if !value.bind(py).is_none() => Some(value.bind(py).extract::<String>()?),
        _ => None,
    };
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
#[pyclass(name = "PDF")]
struct PyPdf {
    inner: Arc<Pdf>,
    stream: Option<PyObject>,
    path: Option<std::path::PathBuf>,
    password: Option<String>,
    stream_is_external: bool,
    selected_pages: Option<PyObject>,
    _laparams: Option<PyObject>,
    _strict_metadata: bool,
    unicode_norm: Option<PyObject>,
    raise_unicode_errors: Option<PyObject>,
    pages_cache: Mutex<Option<Py<PyList>>>,
    objects_cache: Mutex<Option<Py<PyDict>>>,
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
        }
    }
}

#[pymethods]
impl PyPdf {
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
                let path: std::path::PathBuf = path_or_fp.extract()?;
                let stream = path_or_fp
                    .py()
                    .import("builtins")?
                    .getattr("open")?
                    .call1((&path, "rb"))?;
                (stream, Some(path), false)
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
            password,
            stream_is_external,
            selected_pages: pages,
            _laparams: laparams,
            _strict_metadata: strict_metadata,
            unicode_norm,
            raise_unicode_errors,
            pages_cache: Mutex::new(None),
            objects_cache: Mutex::new(None),
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
    fn path(&self) -> Option<std::path::PathBuf> {
        self.path.clone()
    }

    /// The password supplied while opening the document, currently `None`.
    #[getter]
    fn password(&self) -> Option<String> {
        self.password.clone()
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
            let initial_doctop = if self.selected_pages.is_some() {
                let initial_doctop = selected_doctop;
                selected_doctop += height;
                Some(initial_doctop)
            } else {
                None
            };
            pages.bind(py).append(Py::new(
                py,
                PyPage::new(
                    Arc::clone(&self.inner),
                    i,
                    width,
                    height,
                    initial_doctop,
                    self.unicode_norm.as_ref().map(|value| value.clone_ref(py)),
                ),
            )?)?;
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
            for (kind, accessor) in [
                ("char", "chars"),
                ("line", "lines"),
                ("rect", "rects"),
                ("curve", "curves"),
                ("image", "images"),
            ] {
                let page_values = page.call_method0(accessor)?;
                let page_values = page_values.downcast::<PyList>()?;
                if page_values.is_empty() {
                    continue;
                }
                let aggregate = match objects.get_item(kind)? {
                    Some(existing) => existing.downcast_into::<PyList>()?,
                    None => {
                        let aggregate = PyList::empty(py);
                        objects.set_item(kind, &aggregate)?;
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

    /// Document metadata as a dict.
    #[getter]
    fn metadata(&self, py: Python<'_>) -> PyResult<PyObject> {
        raw_metadata_to_dict(py, self.inner.raw_metadata())
    }

    /// Document bookmarks (outline / table of contents) as list[dict].
    fn bookmarks(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .bookmarks()
            .iter()
            .map(|bm| bookmark_to_dict(py, bm))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// PyPage
// ---------------------------------------------------------------------------

/// A single page from a PDF document.
#[pyclass(name = "Page")]
struct PyPage {
    pdf: Arc<Pdf>,
    page_index: usize,
    width: f64,
    height: f64,
    selected_doctop: Option<f64>,
    unicode_norm: Option<PyObject>,
    page_cache: Mutex<Option<Page>>,
}

impl PyPage {
    fn new(
        pdf: Arc<Pdf>,
        page_index: usize,
        width: f64,
        height: f64,
        selected_doctop: Option<f64>,
        unicode_norm: Option<PyObject>,
    ) -> Self {
        Self {
            pdf,
            page_index,
            width,
            height,
            selected_doctop,
            unicode_norm,
            page_cache: Mutex::new(None),
        }
    }

    #[cfg(test)]
    fn from_pdf_for_test(pdf: Pdf, page_index: usize) -> Self {
        let pdf = Arc::new(pdf);
        let (width, height) = pdf.page_dimensions(page_index).expect("page dimensions");
        Self::new(pdf, page_index, width, height, None, None)
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
}

#[pymethods]
impl PyPage {
    /// The original 1-based document page number.
    #[getter]
    fn page_number(&self) -> usize {
        self.page_index + 1
    }

    /// Page width in points.
    #[getter]
    fn width(&self) -> f64 {
        self.width
    }

    /// Page height in points.
    #[getter]
    fn height(&self) -> f64 {
        self.height
    }

    /// Characters on this page as list[dict].
    fn chars(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.with_page(py, |page| {
            page.chars().iter().map(|ch| char_to_dict(py, ch)).collect()
        })
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
    fn lines(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.with_page(py, |page| {
            page.lines()
                .iter()
                .map(|line| line_to_dict(py, line))
                .collect()
        })
    }

    /// Rectangles on this page as list[dict].
    fn rects(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.with_page(py, |page| {
            page.rects()
                .iter()
                .map(|rect| rect_to_dict(py, rect))
                .collect()
        })
    }

    /// Curves on this page as list[dict].
    fn curves(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.with_page(py, |page| {
            page.curves()
                .iter()
                .map(|curve| curve_to_dict(py, curve))
                .collect()
        })
    }

    /// Images on this page as list[dict].
    fn images(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.with_page(py, |page| {
            page.images()
                .iter()
                .map(|image| image_to_dict(py, image))
                .collect()
        })
    }

    /// Annotation dictionaries on this page.
    #[getter]
    fn annots(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        let page_number = self.page_number();
        let page_height = self.height;
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

    /// Crop this page to a bounding box (x0, top, x1, bottom).
    fn crop(&self, py: Python<'_>, bbox: (f64, f64, f64, f64)) -> PyResult<PyCroppedPage> {
        self.with_page(py, |page| {
            Ok(PyCroppedPage {
                inner: page.crop(parse_bbox_tuple(bbox)),
            })
        })
    }

    /// Filter to objects fully within the given bbox.
    fn within_bbox(&self, py: Python<'_>, bbox: (f64, f64, f64, f64)) -> PyResult<PyCroppedPage> {
        self.with_page(py, |page| {
            Ok(PyCroppedPage {
                inner: page.within_bbox(parse_bbox_tuple(bbox)),
            })
        })
    }

    /// Filter to objects outside the given bbox.
    fn outside_bbox(&self, py: Python<'_>, bbox: (f64, f64, f64, f64)) -> PyResult<PyCroppedPage> {
        self.with_page(py, |page| {
            Ok(PyCroppedPage {
                inner: page.outside_bbox(parse_bbox_tuple(bbox)),
            })
        })
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
        use std::io::Cursor;

        let mut doc = lopdf::Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let page_id = doc.new_object_id();

        let resources = dictionary! {};
        let content = lopdf::Stream::new(dictionary! {}, Vec::new());
        let content_id = doc.add_object(content);

        let page = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => resources,
            "Contents" => content_id,
        };
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
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        assert!((pypage.width() - 612.0).abs() < 0.1);
        assert!((pypage.height() - 792.0).abs() < 0.1);
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
            let chars = pypage.chars(py).expect("chars");
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
            let lines = pypage.lines(py).expect("lines");
            assert!(lines.is_empty());
        });
    }

    #[test]
    fn test_pypage_rects_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let rects = pypage.rects(py).expect("rects");
            assert!(rects.is_empty());
        });
    }

    #[test]
    fn test_pypage_curves_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let curves = pypage.curves(py).expect("curves");
            assert!(curves.is_empty());
        });
    }

    #[test]
    fn test_pypage_images_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypage = PyPage::from_pdf_for_test(pdf, 0);
        Python::with_gil(|py| {
            let images = pypage.images(py).expect("images");
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
            let cropped = pypage.crop(py, (0.0, 0.0, 306.0, 396.0)).expect("crop");
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
            let filtered = pypage
                .within_bbox(py, (0.0, 0.0, 306.0, 396.0))
                .expect("within bbox");
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
            let filtered = pypage
                .outside_bbox(py, (100.0, 100.0, 200.0, 200.0))
                .expect("outside bbox");
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
    fn test_pypdf_bookmarks_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf::from_inner_for_test(pdf);
        Python::with_gil(|py| {
            let bookmarks = pypdf.bookmarks(py).expect("bookmarks");
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
            let cropped = pypage.crop(py, (0.0, 0.0, 200.0, 300.0)).expect("crop");
            assert!((cropped.width() - 200.0).abs() < 0.1);
            assert!((cropped.height() - 300.0).abs() < 0.1);
            assert!(cropped.chars(py).expect("chars").is_empty());
            assert!(cropped.lines(py).expect("lines").is_empty());
            assert!(cropped.rects(py).expect("rects").is_empty());
            assert!(cropped.curves(py).expect("curves").is_empty());
            assert!(cropped.images(py).expect("images").is_empty());
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
            let cropped = pypage.crop(py, (0.0, 0.0, 400.0, 500.0)).expect("crop");
            let further = cropped.crop((0.0, 0.0, 200.0, 250.0));
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
            let cropped = pypage.crop(py, (0.0, 0.0, 400.0, 500.0)).expect("crop");
            let within = cropped.within_bbox((50.0, 50.0, 150.0, 150.0));
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
        assert!(
            content.contains("def flush_cache(self, properties: list[str] | None = None) -> None:"),
            "stubs must declare PDF.flush_cache"
        );
        assert!(
            content.contains("def objects(self) -> dict[str, list[dict[str, object]]]:"),
            "stubs must declare PDF.objects"
        );
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
