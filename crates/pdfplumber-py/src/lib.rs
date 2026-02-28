//! Python bindings for pdfplumber-rs via PyO3.
//!
//! Exposes `PyPdf` and `PyPage` classes to Python, wrapping the Rust
//! `pdfplumber::Pdf` and `pdfplumber::Page` types.

use ::pdfplumber::{Page, Pdf, PdfError};
use pyo3::exceptions::PyIOError;
use pyo3::prelude::*;

/// Convert a PdfError to a Python exception.
fn to_py_err(e: PdfError) -> PyErr {
    PyIOError::new_err(format!("{e}"))
}

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
}

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
}

/// The Python module definition.
#[pymodule]
fn pdfplumber(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPdf>()?;
    m.add_class::<PyPage>()?;
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
        // Letter size: 612 x 792 points
        assert!((pypage.width() - 612.0).abs() < 0.1);
        assert!((pypage.height() - 792.0).abs() < 0.1);
    }

    #[test]
    fn test_open_invalid_bytes_returns_error() {
        let result = Pdf::open(b"not a pdf", None);
        assert!(result.is_err());
    }
}
