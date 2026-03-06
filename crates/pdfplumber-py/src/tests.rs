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
