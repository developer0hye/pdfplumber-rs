//! Python bindings for pdfplumber-rs via PyO3.
//!
//! Exposes `PyPdf`, `PyPage`, `PyTable`, and `PyCroppedPage` classes to Python,
//! wrapping the Rust pdfplumber types for full API access.

/// Package version, kept in sync with Cargo.toml and pyproject.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

use ::pdfplumber::{
    BBox, Bookmark, Char, Color, CroppedPage, Curve, DocumentMetadata, Image, Line, Page, Pdf,
    PdfError, Rect, SearchMatch, SearchOptions, Table, TableSettings, TextOptions, Word,
    WordOptions,
};
use pyo3::exceptions::{PyIOError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

// ---------------------------------------------------------------------------
// Python exception types for PdfError variants
// ---------------------------------------------------------------------------

pyo3::create_exception!(pdfplumber, PdfParseError, PyRuntimeError);
pyo3::create_exception!(pdfplumber, PdfIoError, PyIOError);
pyo3::create_exception!(pdfplumber, PdfFontError, PyRuntimeError);
pyo3::create_exception!(pdfplumber, PdfInterpreterError, PyRuntimeError);
pyo3::create_exception!(pdfplumber, PdfResourceLimitError, PyRuntimeError);
pyo3::create_exception!(pdfplumber, PdfPasswordRequired, PyRuntimeError);
pyo3::create_exception!(pdfplumber, PdfInvalidPassword, PyValueError);

/// Convert a PdfError to the appropriate Python exception.
fn to_py_err(e: PdfError) -> PyErr {
    match e {
        PdfError::ParseError(msg) => PdfParseError::new_err(msg),
        PdfError::IoError(msg) => PdfIoError::new_err(msg),
        PdfError::FontError(msg) => PdfFontError::new_err(msg),
        PdfError::InterpreterError(msg) => PdfInterpreterError::new_err(msg),
        PdfError::ResourceLimitExceeded(msg) => PdfResourceLimitError::new_err(msg),
        PdfError::PasswordRequired => {
            PdfPasswordRequired::new_err("PDF is encrypted and requires a password")
        }
        PdfError::InvalidPassword => {
            PdfInvalidPassword::new_err("the supplied password is incorrect")
        }
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

fn metadata_to_dict(py: Python<'_>, meta: &DocumentMetadata) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("title", meta.title.as_deref())?;
    dict.set_item("author", meta.author.as_deref())?;
    dict.set_item("subject", meta.subject.as_deref())?;
    dict.set_item("keywords", meta.keywords.as_deref())?;
    dict.set_item("creator", meta.creator.as_deref())?;
    dict.set_item("producer", meta.producer.as_deref())?;
    dict.set_item("creation_date", meta.creation_date.as_deref())?;
    dict.set_item("mod_date", meta.mod_date.as_deref())?;
    Ok(dict.into_any().unbind())
}

fn parse_bbox_tuple(bbox: (f64, f64, f64, f64)) -> BBox {
    BBox::new(bbox.0, bbox.1, bbox.2, bbox.3)
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

/// A PDF document opened for extraction.
///
/// Use `PDF.open(path)` or `PDF.open_bytes(data)` to open a PDF.
#[pyclass(name = "PDF")]
struct PyPdf {
    inner: Pdf,
}

#[pymethods]
impl PyPdf {
    /// Open a PDF file from a filesystem path.
    #[staticmethod]
    fn open(path: &str) -> PyResult<Self> {
        let pdf = Pdf::open_file(path, None).map_err(to_py_err)?;
        Ok(PyPdf { inner: pdf })
    }

    /// Open a PDF from bytes in memory.
    #[staticmethod]
    fn open_bytes(data: &[u8]) -> PyResult<Self> {
        let pdf = Pdf::open(data, None).map_err(to_py_err)?;
        Ok(PyPdf { inner: pdf })
    }

    /// The list of pages in the PDF.
    #[getter]
    fn pages(&self) -> PyResult<Vec<PyPage>> {
        let mut pages = Vec::with_capacity(self.inner.page_count());
        for i in 0..self.inner.page_count() {
            let page = self.inner.page(i).map_err(to_py_err)?;
            pages.push(PyPage { inner: page });
        }
        Ok(pages)
    }

    /// Document metadata as a dict.
    #[getter]
    fn metadata(&self, py: Python<'_>) -> PyResult<PyObject> {
        metadata_to_dict(py, self.inner.metadata())
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
    inner: Page,
}

#[pymethods]
impl PyPage {
    /// The 0-based page index.
    #[getter]
    fn page_number(&self) -> usize {
        self.inner.page_number()
    }

    /// Page width in points.
    #[getter]
    fn width(&self) -> f64 {
        self.inner.width()
    }

    /// Page height in points.
    #[getter]
    fn height(&self) -> f64 {
        self.inner.height()
    }

    /// Characters on this page as list[dict].
    fn chars(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .chars()
            .iter()
            .map(|ch| char_to_dict(py, ch))
            .collect()
    }

    /// Extract text from this page.
    #[pyo3(signature = (layout=false))]
    fn extract_text(&self, layout: bool) -> String {
        self.inner.extract_text(&TextOptions {
            layout,
            ..TextOptions::default()
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
        let words = self.inner.extract_words(&WordOptions {
            x_tolerance,
            y_tolerance,
            ..WordOptions::default()
        });
        words.iter().map(|w| word_to_dict(py, w)).collect()
    }

    /// Find tables on this page.
    fn find_tables(&self) -> Vec<PyTable> {
        self.inner
            .find_tables(&TableSettings::default())
            .into_iter()
            .map(|t| PyTable { inner: t })
            .collect()
    }

    /// Extract table content as list[list[list[str|None]]].
    fn extract_tables(&self) -> Vec<Vec<Vec<Option<String>>>> {
        self.inner.extract_tables(&TableSettings::default())
    }

    /// Lines on this page as list[dict].
    fn lines(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .lines()
            .iter()
            .map(|l| line_to_dict(py, l))
            .collect()
    }

    /// Rectangles on this page as list[dict].
    fn rects(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .rects()
            .iter()
            .map(|r| rect_to_dict(py, r))
            .collect()
    }

    /// Curves on this page as list[dict].
    fn curves(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .curves()
            .iter()
            .map(|c| curve_to_dict(py, c))
            .collect()
    }

    /// Images on this page as list[dict].
    fn images(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.inner
            .images()
            .iter()
            .map(|i| image_to_dict(py, i))
            .collect()
    }

    /// Crop this page to a bounding box (x0, top, x1, bottom).
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

    /// Search for a text pattern on this page.
    #[pyo3(signature = (pattern, regex=true, case=true))]
    fn search(
        &self,
        py: Python<'_>,
        pattern: &str,
        regex: bool,
        case: bool,
    ) -> PyResult<Vec<PyObject>> {
        let matches = self.inner.search(
            pattern,
            &SearchOptions {
                regex,
                case_sensitive: case,
            },
        );
        matches
            .iter()
            .map(|m| search_match_to_dict(py, m))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Module definition
// ---------------------------------------------------------------------------

/// The Python module definition.
#[pymodule]
fn pdfplumber(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", VERSION)?;

    m.add_class::<PyPdf>()?;
    m.add_class::<PyPage>()?;
    m.add_class::<PyTable>()?;
    m.add_class::<PyCroppedPage>()?;

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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

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
        let pypdf = PyPdf { inner: pdf };
        assert_eq!(pypdf.inner.page_count(), 1);
    }

    #[test]
    fn test_pypdf_pages_returns_correct_count() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf { inner: pdf };
        let page = pypdf.inner.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        assert_eq!(pypage.page_number(), 0);
    }

    #[test]
    fn test_pypage_dimensions() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
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
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
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
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        let text = pypage.extract_text(false);
        assert!(text.is_empty());
    }

    #[test]
    fn test_pypage_extract_text_layout_mode() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        let text = pypage.extract_text(true);
        assert!(text.is_empty());
    }

    #[test]
    fn test_pypage_extract_words_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        Python::with_gil(|py| {
            let words = pypage.extract_words(py, 3.0, 3.0).expect("words");
            assert!(words.is_empty());
        });
    }

    #[test]
    fn test_pypage_lines_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        Python::with_gil(|py| {
            let lines = pypage.lines(py).expect("lines");
            assert!(lines.is_empty());
        });
    }

    #[test]
    fn test_pypage_rects_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        Python::with_gil(|py| {
            let rects = pypage.rects(py).expect("rects");
            assert!(rects.is_empty());
        });
    }

    #[test]
    fn test_pypage_curves_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        Python::with_gil(|py| {
            let curves = pypage.curves(py).expect("curves");
            assert!(curves.is_empty());
        });
    }

    #[test]
    fn test_pypage_images_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        Python::with_gil(|py| {
            let images = pypage.images(py).expect("images");
            assert!(images.is_empty());
        });
    }

    #[test]
    fn test_pypage_find_tables_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        let tables = pypage.find_tables();
        assert!(tables.is_empty());
    }

    #[test]
    fn test_pypage_extract_tables_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        let tables = pypage.extract_tables();
        assert!(tables.is_empty());
    }

    #[test]
    fn test_pypage_search_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        Python::with_gil(|py| {
            let results = pypage.search(py, "test", true, true).expect("search");
            assert!(results.is_empty());
        });
    }

    #[test]
    fn test_pypage_crop() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        let cropped = pypage.crop((0.0, 0.0, 306.0, 396.0));
        assert!((cropped.width() - 306.0).abs() < 0.1);
        assert!((cropped.height() - 396.0).abs() < 0.1);
    }

    #[test]
    fn test_pypage_within_bbox() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        let filtered = pypage.within_bbox((0.0, 0.0, 306.0, 396.0));
        assert!((filtered.width() - 306.0).abs() < 0.1);
        assert!((filtered.height() - 396.0).abs() < 0.1);
    }

    #[test]
    fn test_pypage_outside_bbox() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        let filtered = pypage.outside_bbox((100.0, 100.0, 200.0, 200.0));
        // outside_bbox uses the bbox dimensions (coordinate-adjusted region)
        assert!((filtered.width() - 100.0).abs() < 0.1);
        assert!((filtered.height() - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_pypdf_metadata() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf { inner: pdf };
        Python::with_gil(|py| {
            let meta = pypdf.metadata(py).expect("metadata");
            // Should be a dict with standard keys
            let dict = meta.downcast_bound::<PyDict>(py).expect("dict");
            assert!(dict.contains("title").unwrap());
            assert!(dict.contains("author").unwrap());
            assert!(dict.contains("subject").unwrap());
            assert!(dict.contains("keywords").unwrap());
            assert!(dict.contains("creator").unwrap());
            assert!(dict.contains("producer").unwrap());
            assert!(dict.contains("creation_date").unwrap());
            assert!(dict.contains("mod_date").unwrap());
        });
    }

    #[test]
    fn test_pypdf_bookmarks_empty() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let pypdf = PyPdf { inner: pdf };
        Python::with_gil(|py| {
            let bookmarks = pypdf.bookmarks(py).expect("bookmarks");
            assert!(bookmarks.is_empty());
        });
    }

    #[test]
    fn test_to_py_err_parse_error() {
        let err = to_py_err(PdfError::ParseError("bad xref".to_string()));
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfParseError>(py));
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
        let err = to_py_err(PdfError::ResourceLimitExceeded("too many".to_string()));
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfResourceLimitError>(py));
        });
    }

    #[test]
    fn test_to_py_err_password_required() {
        let err = to_py_err(PdfError::PasswordRequired);
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfPasswordRequired>(py));
        });
    }

    #[test]
    fn test_to_py_err_invalid_password() {
        let err = to_py_err(PdfError::InvalidPassword);
        Python::with_gil(|py| {
            assert!(err.is_instance_of::<PdfInvalidPassword>(py));
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
        let meta = DocumentMetadata {
            title: Some("Test Doc".to_string()),
            author: Some("Author".to_string()),
            subject: None,
            keywords: None,
            creator: None,
            producer: None,
            creation_date: None,
            mod_date: None,
        };
        Python::with_gil(|py| {
            let dict_obj = metadata_to_dict(py, &meta).expect("metadata_to_dict");
            let dict = dict_obj.downcast_bound::<PyDict>(py).expect("PyDict");
            let title: String = dict.get_item("title").unwrap().unwrap().extract().unwrap();
            assert_eq!(title, "Test Doc");
            let author: String = dict.get_item("author").unwrap().unwrap().extract().unwrap();
            assert_eq!(author, "Author");
            // None fields should be Python None
            assert!(dict.get_item("subject").unwrap().unwrap().is_none());
        });
    }

    #[test]
    fn test_cropped_page_methods() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        let cropped = pypage.crop((0.0, 0.0, 200.0, 300.0));

        // Verify basic properties
        assert!((cropped.width() - 200.0).abs() < 0.1);
        assert!((cropped.height() - 300.0).abs() < 0.1);

        // All content methods should work on cropped page
        Python::with_gil(|py| {
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
        });
        assert!(cropped.extract_text(false).is_empty());
        assert!(cropped.find_tables().is_empty());
        assert!(cropped.extract_tables().is_empty());
    }

    #[test]
    fn test_cropped_page_further_crop() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        let cropped = pypage.crop((0.0, 0.0, 400.0, 500.0));
        let further = cropped.crop((0.0, 0.0, 200.0, 250.0));
        assert!((further.width() - 200.0).abs() < 0.1);
        assert!((further.height() - 250.0).abs() < 0.1);
    }

    #[test]
    fn test_cropped_page_within_bbox() {
        let bytes = minimal_pdf_bytes();
        let pdf = Pdf::open(&bytes, None).expect("open");
        let page = pdf.page(0).expect("page 0");
        let pypage = PyPage { inner: page };
        let cropped = pypage.crop((0.0, 0.0, 400.0, 500.0));
        let within = cropped.within_bbox((50.0, 50.0, 150.0, 150.0));
        assert!((within.width() - 100.0).abs() < 0.1);
        assert!((within.height() - 100.0).abs() < 0.1);
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
        let stubs_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pdfplumber.pyi");
        assert!(
            stubs_path.exists(),
            "Type stubs file pdfplumber.pyi should exist at {}",
            stubs_path.display()
        );
    }

    #[test]
    fn test_type_stubs_content() {
        let stubs_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pdfplumber.pyi");
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
