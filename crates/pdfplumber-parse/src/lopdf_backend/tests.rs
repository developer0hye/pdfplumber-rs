//! Test suite for the lopdf backend.
//!
//! Integration tests using synthetic PDF documents built with lopdf directly.

use super::*;
use crate::handler::{CharEvent, ContentHandler, ImageEvent};
use pdfplumber_core::PdfError;

fn create_test_pdf(page_count: usize) -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    let mut page_ids: Vec<Object> = Vec::new();
    for _ in 0..page_count {
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        page_ids.push(page_id.into());
    }

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids,
            "Count" => page_count as i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF where pages inherit MediaBox from the Pages parent node.
#[cfg(test)]
fn create_test_pdf_inherited_media_box() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    // Page WITHOUT its own MediaBox — should inherit from parent
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with a page that has an explicit CropBox.
#[cfg(test)]
fn create_test_pdf_with_crop_box() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "CropBox" => vec![
            Object::Real(36.0),
            Object::Real(36.0),
            Object::Real(576.0),
            Object::Real(756.0),
        ],
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with a page that has a /Rotate value.
#[cfg(test)]
fn create_test_pdf_with_rotate(rotation: i64) -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Rotate" => rotation,
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF where Rotate is inherited from the Pages parent node.
#[cfg(test)]
fn create_test_pdf_inherited_rotate(rotation: i64) -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    // Page WITHOUT Rotate — should inherit from parent
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
            "Rotate" => rotation,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with a page that references a Form XObject containing text.
///
/// Page content: `q /FM1 Do Q`
/// Form XObject FM1 content: `BT /F1 12 Tf 72 700 Td (Hello) Tj ET`
#[cfg(test)]
fn create_test_pdf_with_form_xobject() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    // Minimal Type1 font dictionary
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // Form XObject stream: contains text
    let form_content = b"BT /F1 12 Tf 72 700 Td (Hello) Tj ET";
    let form_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => Object::Dictionary(dictionary! {
                "Font" => Object::Dictionary(dictionary! {
                    "F1" => font_id,
                }),
            }),
        },
        form_content.to_vec(),
    );
    let form_id = doc.add_object(Object::Stream(form_stream));

    // Page content: invoke the form XObject
    let page_content = b"q /FM1 Do Q";
    let page_stream = Stream::new(lopdf::Dictionary::new(), page_content.to_vec());
    let content_id = doc.add_object(Object::Stream(page_stream));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
        "Resources" => Object::Dictionary(dictionary! {
            "Font" => Object::Dictionary(dictionary! {
                "F1" => font_id,
            }),
            "XObject" => Object::Dictionary(dictionary! {
                "FM1" => form_id,
            }),
        }),
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with nested Form XObjects (2 levels).
///
/// Page content: `q /FM1 Do Q`
/// FM1 content: `q /FM2 Do Q` (references FM2)
/// FM2 content: `BT /F1 10 Tf (Deep) Tj ET` (actual text)
#[cfg(test)]
fn create_test_pdf_with_nested_form_xobjects() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // Inner Form XObject (FM2): contains actual text
    let fm2_content = b"BT /F1 10 Tf (Deep) Tj ET";
    let fm2_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => Object::Dictionary(dictionary! {
                "Font" => Object::Dictionary(dictionary! {
                    "F1" => font_id,
                }),
            }),
        },
        fm2_content.to_vec(),
    );
    let fm2_id = doc.add_object(Object::Stream(fm2_stream));

    // Outer Form XObject (FM1): references FM2
    let fm1_content = b"q /FM2 Do Q";
    let fm1_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => Object::Dictionary(dictionary! {
                "XObject" => Object::Dictionary(dictionary! {
                    "FM2" => fm2_id,
                }),
                "Font" => Object::Dictionary(dictionary! {
                    "F1" => font_id,
                }),
            }),
        },
        fm1_content.to_vec(),
    );
    let fm1_id = doc.add_object(Object::Stream(fm1_stream));

    // Page content: invoke FM1
    let page_content = b"q /FM1 Do Q";
    let page_stream = Stream::new(lopdf::Dictionary::new(), page_content.to_vec());
    let content_id = doc.add_object(Object::Stream(page_stream));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
        "Resources" => Object::Dictionary(dictionary! {
            "XObject" => Object::Dictionary(dictionary! {
                "FM1" => fm1_id,
            }),
            "Font" => Object::Dictionary(dictionary! {
                "F1" => font_id,
            }),
        }),
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with a Form XObject that has a /Matrix transform.
///
/// The Form XObject has /Matrix [2 0 0 2 10 20] (scale 2x + translate).
#[cfg(test)]
fn create_test_pdf_form_xobject_with_matrix() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let form_content = b"BT /F1 12 Tf (A) Tj ET";
    let form_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Matrix" => vec![
                Object::Real(2.0), Object::Real(0.0),
                Object::Real(0.0), Object::Real(2.0),
                Object::Real(10.0), Object::Real(20.0),
            ],
            "Resources" => Object::Dictionary(dictionary! {
                "Font" => Object::Dictionary(dictionary! {
                    "F1" => font_id,
                }),
            }),
        },
        form_content.to_vec(),
    );
    let form_id = doc.add_object(Object::Stream(form_stream));

    let page_content = b"q /FM1 Do Q";
    let page_stream = Stream::new(lopdf::Dictionary::new(), page_content.to_vec());
    let content_id = doc.add_object(Object::Stream(page_stream));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
        "Resources" => Object::Dictionary(dictionary! {
            "XObject" => Object::Dictionary(dictionary! {
                "FM1" => form_id,
            }),
            "Font" => Object::Dictionary(dictionary! {
                "F1" => font_id,
            }),
        }),
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with an Image XObject (not Form).
#[cfg(test)]
fn create_test_pdf_with_image_xobject() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    // 2x2 RGB image (12 bytes of pixel data)
    let image_data = vec![255u8, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0];
    let image_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 2i64,
            "Height" => 2i64,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8i64,
        },
        image_data,
    );
    let image_id = doc.add_object(Object::Stream(image_stream));

    // Page content: scale then place image
    let page_content = b"q 200 0 0 150 100 300 cm /Im0 Do Q";
    let page_stream = Stream::new(lopdf::Dictionary::new(), page_content.to_vec());
    let content_id = doc.add_object(Object::Stream(page_stream));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
        "Resources" => Object::Dictionary(dictionary! {
            "XObject" => Object::Dictionary(dictionary! {
                "Im0" => image_id,
            }),
        }),
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with a JPEG (DCTDecode) image XObject.
#[cfg(test)]
fn create_test_pdf_with_jpeg_image() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    // Minimal JPEG data (SOI + APP0 + EOI markers)
    // A real JPEG starts with FF D8 and ends with FF D9
    let jpeg_data = vec![
        0xFF, 0xD8, 0xFF, 0xE0, // SOI + APP0 marker
        0x00, 0x10, // Length of APP0
        0x4A, 0x46, 0x49, 0x46, 0x00, // "JFIF\0"
        0x01, 0x01, // Version
        0x00, // Units
        0x00, 0x01, 0x00, 0x01, // X/Y density
        0x00, 0x00, // No thumbnail
        0xFF, 0xD9, // EOI marker
    ];

    let image_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 2i64,
            "Height" => 2i64,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8i64,
            "Filter" => "DCTDecode",
        },
        jpeg_data,
    );
    let image_id = doc.add_object(Object::Stream(image_stream));

    let page_content = b"q 200 0 0 150 100 300 cm /Im0 Do Q";
    let page_stream = Stream::new(lopdf::Dictionary::new(), page_content.to_vec());
    let content_id = doc.add_object(Object::Stream(page_stream));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
        "Resources" => Object::Dictionary(dictionary! {
            "XObject" => Object::Dictionary(dictionary! {
                "Im0" => image_id,
            }),
        }),
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with a page that has direct text content (no XObjects).
#[cfg(test)]
fn create_test_pdf_with_text_content() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let page_content = b"BT /F1 12 Tf 72 700 Td (Hi) Tj ET";
    let page_stream = Stream::new(lopdf::Dictionary::new(), page_content.to_vec());
    let content_id = doc.add_object(Object::Stream(page_stream));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
        "Resources" => Object::Dictionary(dictionary! {
            "Font" => Object::Dictionary(dictionary! {
                "F1" => font_id,
            }),
        }),
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a test PDF with an /Info metadata dictionary.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn create_test_pdf_with_metadata(
    title: Option<&str>,
    author: Option<&str>,
    subject: Option<&str>,
    keywords: Option<&str>,
    creator: Option<&str>,
    producer: Option<&str>,
    creation_date: Option<&str>,
    mod_date: Option<&str>,
) -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    // Build /Info dictionary
    let mut info_dict = lopdf::Dictionary::new();
    if let Some(v) = title {
        info_dict.set("Title", Object::string_literal(v));
    }
    if let Some(v) = author {
        info_dict.set("Author", Object::string_literal(v));
    }
    if let Some(v) = subject {
        info_dict.set("Subject", Object::string_literal(v));
    }
    if let Some(v) = keywords {
        info_dict.set("Keywords", Object::string_literal(v));
    }
    if let Some(v) = creator {
        info_dict.set("Creator", Object::string_literal(v));
    }
    if let Some(v) = producer {
        info_dict.set("Producer", Object::string_literal(v));
    }
    if let Some(v) = creation_date {
        info_dict.set("CreationDate", Object::string_literal(v));
    }
    if let Some(v) = mod_date {
        info_dict.set("ModDate", Object::string_literal(v));
    }

    let info_id = doc.add_object(Object::Dictionary(info_dict));
    doc.trailer.set("Info", Object::Reference(info_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{CharEvent, ContentHandler, ImageEvent};
    use pdfplumber_core::PdfError;

    // --- CollectingHandler for interpret_page tests ---

    struct CollectingHandler {
        chars: Vec<CharEvent>,
        images: Vec<ImageEvent>,
    }

    impl CollectingHandler {
        fn new() -> Self {
            Self {
                chars: Vec::new(),
                images: Vec::new(),
            }
        }
    }

    impl ContentHandler for CollectingHandler {
        fn on_char(&mut self, event: CharEvent) {
            self.chars.push(event);
        }
        fn on_image(&mut self, event: ImageEvent) {
            self.images.push(event);
        }
    }

    // --- open() tests ---

    #[test]
    fn open_valid_single_page_pdf() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 1);
    }

    #[test]
    fn open_valid_multi_page_pdf() {
        let pdf_bytes = create_test_pdf(5);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 5);
    }

    #[test]
    fn open_invalid_bytes_returns_error() {
        let result = LopdfBackend::open(b"not a pdf");
        assert!(result.is_err());
    }

    #[test]
    fn open_empty_bytes_returns_error() {
        let result = LopdfBackend::open(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn open_error_converts_to_pdf_error() {
        let err = LopdfBackend::open(b"garbage").unwrap_err();
        let pdf_err: PdfError = err.into();
        assert!(matches!(pdf_err, PdfError::ParseError(_)));
    }

    // --- page_count() tests ---

    #[test]
    fn page_count_zero_pages() {
        let pdf_bytes = create_test_pdf(0);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 0);
    }

    #[test]
    fn page_count_three_pages() {
        let pdf_bytes = create_test_pdf(3);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 3);
    }

    // --- get_page() tests ---

    #[test]
    fn get_page_first_page() {
        let pdf_bytes = create_test_pdf(3);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        assert_eq!(page.index, 0);
    }

    #[test]
    fn get_page_last_page() {
        let pdf_bytes = create_test_pdf(3);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 2).unwrap();
        assert_eq!(page.index, 2);
    }

    #[test]
    fn get_page_out_of_bounds() {
        let pdf_bytes = create_test_pdf(2);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let result = LopdfBackend::get_page(&doc, 2);
        assert!(result.is_err());
    }

    #[test]
    fn get_page_out_of_bounds_error_converts_to_pdf_error() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let err = LopdfBackend::get_page(&doc, 5).unwrap_err();
        let pdf_err: PdfError = err.into();
        assert!(matches!(pdf_err, PdfError::ParseError(_)));
        assert!(pdf_err.to_string().contains("out of range"));
    }

    #[test]
    fn get_page_on_empty_document() {
        let pdf_bytes = create_test_pdf(0);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let result = LopdfBackend::get_page(&doc, 0);
        assert!(result.is_err());
    }

    // --- Page object IDs are distinct ---

    #[test]
    fn pages_have_distinct_object_ids() {
        let pdf_bytes = create_test_pdf(3);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page0 = LopdfBackend::get_page(&doc, 0).unwrap();
        let page1 = LopdfBackend::get_page(&doc, 1).unwrap();
        let page2 = LopdfBackend::get_page(&doc, 2).unwrap();
        assert_ne!(page0.object_id, page1.object_id);
        assert_ne!(page1.object_id, page2.object_id);
        assert_ne!(page0.object_id, page2.object_id);
    }

    // --- Integration: open + page_count + get_page round-trip ---

    #[test]
    fn round_trip_open_count_access() {
        let pdf_bytes = create_test_pdf(4);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let count = LopdfBackend::page_count(&doc);
        assert_eq!(count, 4);

        for i in 0..count {
            let page = LopdfBackend::get_page(&doc, i).unwrap();
            assert_eq!(page.index, i);
        }

        // One past the end should fail
        assert!(LopdfBackend::get_page(&doc, count).is_err());
    }

    // --- page_media_box() tests ---

    #[test]
    fn media_box_explicit_us_letter() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let media_box = LopdfBackend::page_media_box(&doc, &page).unwrap();
        assert_eq!(media_box, BBox::new(0.0, 0.0, 612.0, 792.0));
    }

    #[test]
    fn media_box_inherited_from_parent() {
        let pdf_bytes = create_test_pdf_inherited_media_box();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let media_box = LopdfBackend::page_media_box(&doc, &page).unwrap();
        // Inherited A4 size from parent Pages node
        assert_eq!(media_box, BBox::new(0.0, 0.0, 595.0, 842.0));
    }

    #[test]
    fn media_box_width_height() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let media_box = LopdfBackend::page_media_box(&doc, &page).unwrap();
        assert_eq!(media_box.width(), 612.0);
        assert_eq!(media_box.height(), 792.0);
    }

    // --- page_crop_box() tests ---

    #[test]
    fn crop_box_present() {
        let pdf_bytes = create_test_pdf_with_crop_box();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let crop_box = LopdfBackend::page_crop_box(&doc, &page).unwrap();
        assert_eq!(crop_box, Some(BBox::new(36.0, 36.0, 576.0, 756.0)));
    }

    #[test]
    fn crop_box_absent() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let crop_box = LopdfBackend::page_crop_box(&doc, &page).unwrap();
        assert_eq!(crop_box, None);
    }

    // --- page_rotate() tests ---

    #[test]
    fn rotate_default_zero() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let rotation = LopdfBackend::page_rotate(&doc, &page).unwrap();
        assert_eq!(rotation, 0);
    }

    #[test]
    fn rotate_90() {
        let pdf_bytes = create_test_pdf_with_rotate(90);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let rotation = LopdfBackend::page_rotate(&doc, &page).unwrap();
        assert_eq!(rotation, 90);
    }

    #[test]
    fn rotate_180() {
        let pdf_bytes = create_test_pdf_with_rotate(180);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let rotation = LopdfBackend::page_rotate(&doc, &page).unwrap();
        assert_eq!(rotation, 180);
    }

    #[test]
    fn rotate_270() {
        let pdf_bytes = create_test_pdf_with_rotate(270);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let rotation = LopdfBackend::page_rotate(&doc, &page).unwrap();
        assert_eq!(rotation, 270);
    }

    #[test]
    fn rotate_inherited_from_parent() {
        let pdf_bytes = create_test_pdf_inherited_rotate(90);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let rotation = LopdfBackend::page_rotate(&doc, &page).unwrap();
        assert_eq!(rotation, 90);
    }

    // --- Integration: all page properties together ---

    #[test]
    fn page_properties_round_trip() {
        let pdf_bytes = create_test_pdf_with_crop_box();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();

        let media_box = LopdfBackend::page_media_box(&doc, &page).unwrap();
        let crop_box = LopdfBackend::page_crop_box(&doc, &page).unwrap();
        let rotation = LopdfBackend::page_rotate(&doc, &page).unwrap();

        assert_eq!(media_box, BBox::new(0.0, 0.0, 612.0, 792.0));
        assert!(crop_box.is_some());
        assert_eq!(rotation, 0);
    }

    // --- interpret_page: basic text extraction ---

    #[test]
    fn interpret_page_simple_text() {
        let pdf_bytes = create_test_pdf_with_text_content();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        LopdfBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();

        // "Hi" = 2 characters
        assert_eq!(handler.chars.len(), 2);
        assert_eq!(handler.chars[0].char_code, b'H' as u32);
        assert_eq!(handler.chars[1].char_code, b'i' as u32);
        assert_eq!(handler.chars[0].font_size, 12.0);
        assert_eq!(handler.chars[0].font_name, "Helvetica");
    }

    #[test]
    fn interpret_page_no_content() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        // Page with no /Contents should not fail
        LopdfBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();
        assert_eq!(handler.chars.len(), 0);
    }

    // --- interpret_page: Form XObject tests (US-016) ---

    #[test]
    fn interpret_page_form_xobject_text() {
        let pdf_bytes = create_test_pdf_with_form_xobject();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        LopdfBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();

        // Form XObject contains "Hello" = 5 chars
        assert_eq!(handler.chars.len(), 5);
        assert_eq!(handler.chars[0].char_code, b'H' as u32);
        assert_eq!(handler.chars[1].char_code, b'e' as u32);
        assert_eq!(handler.chars[2].char_code, b'l' as u32);
        assert_eq!(handler.chars[3].char_code, b'l' as u32);
        assert_eq!(handler.chars[4].char_code, b'o' as u32);
        assert_eq!(handler.chars[0].font_name, "Helvetica");
        assert_eq!(handler.chars[0].font_size, 12.0);
    }

    #[test]
    fn interpret_page_nested_form_xobjects() {
        let pdf_bytes = create_test_pdf_with_nested_form_xobjects();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        LopdfBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();

        // Nested form XObject FM1→FM2 contains "Deep" = 4 chars
        assert_eq!(handler.chars.len(), 4);
        assert_eq!(handler.chars[0].char_code, b'D' as u32);
        assert_eq!(handler.chars[1].char_code, b'e' as u32);
        assert_eq!(handler.chars[2].char_code, b'e' as u32);
        assert_eq!(handler.chars[3].char_code, b'p' as u32);
    }

    #[test]
    fn interpret_page_form_xobject_matrix_applied() {
        let pdf_bytes = create_test_pdf_form_xobject_with_matrix();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        LopdfBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();

        // Form XObject has /Matrix [2 0 0 2 10 20], character "A"
        assert_eq!(handler.chars.len(), 1);
        assert_eq!(handler.chars[0].char_code, b'A' as u32);
        // CTM should include the form's matrix transform
        let ctm = handler.chars[0].ctm;
        // Form matrix [2 0 0 2 10 20] applied on top of identity
        assert!((ctm[0] - 2.0).abs() < 0.01);
        assert!((ctm[3] - 2.0).abs() < 0.01);
        assert!((ctm[4] - 10.0).abs() < 0.01);
        assert!((ctm[5] - 20.0).abs() < 0.01);
    }

    #[test]
    fn interpret_page_form_xobject_state_restored() {
        // After processing a Form XObject, the graphics state should be restored.
        // The Form XObject is wrapped in q/Q on the page, and the interpreter
        // also saves/restores state around the Form XObject.
        let pdf_bytes = create_test_pdf_with_form_xobject();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        // This should complete without errors (state properly saved/restored)
        let result = LopdfBackend::interpret_page(&doc, &page, &mut handler, &options);
        assert!(result.is_ok());
    }

    #[test]
    fn interpret_page_image_xobject() {
        let pdf_bytes = create_test_pdf_with_image_xobject();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        LopdfBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();

        // Should have 1 image event, no chars
        assert_eq!(handler.chars.len(), 0);
        assert_eq!(handler.images.len(), 1);
        assert_eq!(handler.images[0].name, "Im0");
        assert_eq!(handler.images[0].width, 2);
        assert_eq!(handler.images[0].height, 2);
        assert_eq!(handler.images[0].colorspace.as_deref(), Some("DeviceRGB"));
        assert_eq!(handler.images[0].bits_per_component, Some(8));
        // CTM should be [200 0 0 150 100 300] from the cm operator
        let ctm = handler.images[0].ctm;
        assert!((ctm[0] - 200.0).abs() < 0.01);
        assert!((ctm[3] - 150.0).abs() < 0.01);
        assert!((ctm[4] - 100.0).abs() < 0.01);
        assert!((ctm[5] - 300.0).abs() < 0.01);
    }

    #[test]
    fn interpret_page_recursion_limit() {
        // Use the nested form XObject PDF but with max_recursion_depth = 0
        let pdf_bytes = create_test_pdf_with_form_xobject();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let mut options = ExtractOptions::default();
        options.max_recursion_depth = 0; // Page level = 0, Form XObject = 1 > limit
        let mut handler = CollectingHandler::new();

        let result = LopdfBackend::interpret_page(&doc, &page, &mut handler, &options);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("recursion depth"));
    }

    // --- document_metadata() tests ---

    #[test]
    fn metadata_full_info_dictionary() {
        let pdf_bytes = create_test_pdf_with_metadata(
            Some("Test Document"),
            Some("John Doe"),
            Some("Testing metadata"),
            Some("test, pdf, rust"),
            Some("LibreOffice"),
            Some("pdfplumber-rs"),
            Some("D:20240101120000+00'00'"),
            Some("D:20240615153000+00'00'"),
        );
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let meta = LopdfBackend::document_metadata(&doc).unwrap();

        assert_eq!(meta.title.as_deref(), Some("Test Document"));
        assert_eq!(meta.author.as_deref(), Some("John Doe"));
        assert_eq!(meta.subject.as_deref(), Some("Testing metadata"));
        assert_eq!(meta.keywords.as_deref(), Some("test, pdf, rust"));
        assert_eq!(meta.creator.as_deref(), Some("LibreOffice"));
        assert_eq!(meta.producer.as_deref(), Some("pdfplumber-rs"));
        assert_eq!(
            meta.creation_date.as_deref(),
            Some("D:20240101120000+00'00'")
        );
        assert_eq!(meta.mod_date.as_deref(), Some("D:20240615153000+00'00'"));
        assert!(!meta.is_empty());
    }

    #[test]
    fn metadata_partial_info_dictionary() {
        let pdf_bytes = create_test_pdf_with_metadata(
            Some("Only Title"),
            None,
            None,
            None,
            None,
            Some("A Producer"),
            None,
            None,
        );
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let meta = LopdfBackend::document_metadata(&doc).unwrap();

        assert_eq!(meta.title.as_deref(), Some("Only Title"));
        assert_eq!(meta.author, None);
        assert_eq!(meta.subject, None);
        assert_eq!(meta.keywords, None);
        assert_eq!(meta.creator, None);
        assert_eq!(meta.producer.as_deref(), Some("A Producer"));
        assert_eq!(meta.creation_date, None);
        assert_eq!(meta.mod_date, None);
        assert!(!meta.is_empty());
    }

    #[test]
    fn metadata_no_info_dictionary() {
        // create_test_pdf doesn't add an /Info dictionary
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let meta = LopdfBackend::document_metadata(&doc).unwrap();

        assert!(meta.is_empty());
        assert_eq!(meta.title, None);
        assert_eq!(meta.author, None);
    }

    // --- extract_image_content() tests ---

    #[test]
    fn extract_image_content_raw_data() {
        let pdf_bytes = create_test_pdf_with_image_xobject();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();

        let content = LopdfBackend::extract_image_content(&doc, &page, "Im0").unwrap();

        assert_eq!(content.format, pdfplumber_core::ImageFormat::Raw);
        assert_eq!(content.width, 2);
        assert_eq!(content.height, 2);
        // 2x2 RGB image = 12 bytes
        assert_eq!(content.data.len(), 12);
        assert_eq!(
            content.data,
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]
        );
    }

    #[test]
    fn extract_image_content_not_found() {
        let pdf_bytes = create_test_pdf_with_image_xobject();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();

        let result = LopdfBackend::extract_image_content(&doc, &page, "NonExistent");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not found"));
    }

    #[test]
    fn extract_image_content_jpeg() {
        // Create a PDF with a JPEG (DCTDecode) image
        let pdf_bytes = create_test_pdf_with_jpeg_image();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();

        let content = LopdfBackend::extract_image_content(&doc, &page, "Im0").unwrap();

        assert_eq!(content.format, pdfplumber_core::ImageFormat::Jpeg);
        assert_eq!(content.width, 2);
        assert_eq!(content.height, 2);
        // JPEG data should be returned as-is
        assert!(content.data.starts_with(&[0xFF, 0xD8]));
    }

    #[test]
    fn extract_image_content_no_xobject_resources() {
        // A page without XObject resources
        let pdf_bytes = create_test_pdf_with_text_content();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();

        let result = LopdfBackend::extract_image_content(&doc, &page, "Im0");
        assert!(result.is_err());
    }

    // --- Encrypted PDF test helpers ---

    /// PDF standard padding bytes used in encryption key derivation.
    const PAD_BYTES: [u8; 32] = [
        0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01,
        0x08, 0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53,
        0x69, 0x7A,
    ];

    /// Simple RC4 implementation for test encryption.
    fn rc4_transform(key: &[u8], data: &[u8]) -> Vec<u8> {
        // RC4 KSA
        let mut s: Vec<u8> = (0..=255).collect();
        let mut j: usize = 0;
        for i in 0..256 {
            j = (j + s[i] as usize + key[i % key.len()] as usize) & 0xFF;
            s.swap(i, j);
        }
        // RC4 PRGA
        let mut out = Vec::with_capacity(data.len());
        let mut i: usize = 0;
        j = 0;
        for &byte in data {
            i = (i + 1) & 0xFF;
            j = (j + s[i] as usize) & 0xFF;
            s.swap(i, j);
            let k = s[(s[i] as usize + s[j] as usize) & 0xFF];
            out.push(byte ^ k);
        }
        out
    }

    /// Create an encrypted PDF with the given user password (RC4, 40-bit, V=1, R=2).
    fn create_encrypted_test_pdf(user_password: &[u8]) -> Vec<u8> {
        use lopdf::{Document, Object, ObjectId, Stream, StringFormat, dictionary};

        let file_id = b"testfileid123456"; // 16 bytes
        let permissions: i32 = -4; // all permissions

        // Pad password to 32 bytes
        let mut padded_pw = Vec::with_capacity(32);
        let pw_len = user_password.len().min(32);
        padded_pw.extend_from_slice(&user_password[..pw_len]);
        padded_pw.extend_from_slice(&PAD_BYTES[..32 - pw_len]);

        // Algorithm 3.3: Compute /O value (owner password hash)
        // Using same password for owner and user (simplification for tests)
        let o_key_digest = md5::compute(&padded_pw);
        let o_key = &o_key_digest[..5]; // 40-bit key = 5 bytes
        let o_value = rc4_transform(o_key, &padded_pw);

        // Algorithm 3.2: Compute encryption key
        let mut key_input = Vec::with_capacity(128);
        key_input.extend_from_slice(&padded_pw);
        key_input.extend_from_slice(&o_value);
        key_input.extend_from_slice(&(permissions as u32).to_le_bytes());
        key_input.extend_from_slice(file_id);
        let key_digest = md5::compute(&key_input);
        let enc_key = key_digest[..5].to_vec(); // 40-bit key

        // Algorithm 3.4: Compute /U value (R=2)
        let u_value = rc4_transform(&enc_key, &PAD_BYTES);

        // Build the PDF document
        let mut doc = Document::with_version("1.5");
        let pages_id: ObjectId = doc.new_object_id();

        // Create page with text content (will be encrypted)
        let content_bytes = b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET";
        let stream = Stream::new(dictionary! {}, content_bytes.to_vec());
        let content_id = doc.add_object(Object::Stream(stream));

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => Object::Reference(content_id),
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => Object::Reference(font_id),
                },
            },
        });

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => 1_i64,
            }),
        );

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        // Now encrypt all string/stream objects
        for (&obj_id, obj) in doc.objects.iter_mut() {
            // Compute per-object key: MD5(enc_key + obj_num_le + gen_num_le)[:key_len+5]
            let mut obj_key_input = Vec::with_capacity(10);
            obj_key_input.extend_from_slice(&enc_key);
            obj_key_input.extend_from_slice(&obj_id.0.to_le_bytes()[..3]);
            obj_key_input.extend_from_slice(&obj_id.1.to_le_bytes()[..2]);
            let obj_key_digest = md5::compute(&obj_key_input);
            let obj_key_len = (enc_key.len() + 5).min(16);
            let obj_key = &obj_key_digest[..obj_key_len];

            match obj {
                Object::Stream(stream) => {
                    let encrypted = rc4_transform(obj_key, &stream.content);
                    stream.set_content(encrypted);
                }
                Object::String(content, _) => {
                    let encrypted = rc4_transform(obj_key, content);
                    *content = encrypted;
                }
                _ => {}
            }
        }

        // Add /Encrypt dictionary
        let encrypt_id = doc.add_object(dictionary! {
            "Filter" => "Standard",
            "V" => 1_i64,
            "R" => 2_i64,
            "Length" => 40_i64,
            "O" => Object::String(o_value, StringFormat::Literal),
            "U" => Object::String(u_value, StringFormat::Literal),
            "P" => permissions as i64,
        });
        doc.trailer.set("Encrypt", Object::Reference(encrypt_id));

        // Add /ID array
        doc.trailer.set(
            "ID",
            Object::Array(vec![
                Object::String(file_id.to_vec(), StringFormat::Literal),
                Object::String(file_id.to_vec(), StringFormat::Literal),
            ]),
        );

        let mut buf = Vec::new();
        doc.save_to(&mut buf)
            .expect("failed to save encrypted test PDF");
        buf
    }

    // --- Encrypted PDF tests ---

    #[test]
    fn open_encrypted_pdf_without_password_returns_password_required() {
        let pdf_bytes = create_encrypted_test_pdf(b"secret123");
        let result = LopdfBackend::open(&pdf_bytes);
        assert!(result.is_err());
        let err: pdfplumber_core::PdfError = result.unwrap_err().into();
        assert_eq!(err, pdfplumber_core::PdfError::PasswordRequired);
    }

    #[test]
    fn open_encrypted_pdf_with_correct_password() {
        // Use the real pr-138-example.pdf which is encrypted with an empty user password
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("pdfplumber/tests/fixtures/pdfs/pr-138-example.pdf");
        if !fixture_path.exists() {
            eprintln!("skipping: fixture not found at {}", fixture_path.display());
            return;
        }
        let pdf_bytes = std::fs::read(&fixture_path).unwrap();
        let result = LopdfBackend::open_with_password(&pdf_bytes, b"");
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 2);
    }

    #[test]
    fn open_encrypted_pdf_with_wrong_password_returns_invalid_password() {
        let pdf_bytes = create_encrypted_test_pdf(b"secret123");
        let result = LopdfBackend::open_with_password(&pdf_bytes, b"wrongpassword");
        assert!(result.is_err());
        let err: pdfplumber_core::PdfError = result.unwrap_err().into();
        assert_eq!(err, pdfplumber_core::PdfError::InvalidPassword);
    }

    #[test]
    fn open_unencrypted_pdf_with_password_succeeds() {
        // Password is ignored for unencrypted PDFs
        let pdf_bytes = create_test_pdf(1);
        let result = LopdfBackend::open_with_password(&pdf_bytes, b"anypassword");
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 1);
    }

    #[test]
    fn open_encrypted_pdf_with_empty_password() {
        // Encrypted with empty password — should be openable with empty password
        let pdf_bytes = create_encrypted_test_pdf(b"");
        let result = LopdfBackend::open_with_password(&pdf_bytes, b"");
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 1);
    }

    #[test]
    fn open_auto_decrypts_empty_password_pdf() {
        // PDFs encrypted with an empty user password should be auto-decrypted
        // by open() without requiring the caller to provide a password.
        // This matches Python pdfplumber behavior (via pdfminer).
        let pdf_bytes = create_encrypted_test_pdf(b"");
        let result = LopdfBackend::open(&pdf_bytes);
        assert!(
            result.is_ok(),
            "open() should auto-decrypt empty-password PDFs"
        );
        let doc = result.unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 1);
    }

    #[test]
    fn open_still_rejects_non_empty_password_pdf() {
        // PDFs encrypted with a real password should still fail with PasswordRequired
        let pdf_bytes = create_encrypted_test_pdf(b"secret123");
        let result = LopdfBackend::open(&pdf_bytes);
        assert!(result.is_err());
        let err: pdfplumber_core::PdfError = result.unwrap_err().into();
        assert_eq!(err, pdfplumber_core::PdfError::PasswordRequired);
    }

    // --- Form field extraction tests ---

    /// Create a PDF with form fields for testing AcroForm extraction.
    fn create_test_pdf_with_form_fields() -> Vec<u8> {
        use lopdf::{Document, Object, ObjectId, dictionary};

        let mut doc = Document::with_version("1.7");
        let pages_id: ObjectId = doc.new_object_id();

        // Create a page
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => Object::Integer(1),
            }),
        );

        // Text field
        let text_field_id = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Widget",
            "T" => Object::string_literal("name"),
            "FT" => "Tx",
            "V" => Object::string_literal("John Doe"),
            "DV" => Object::string_literal(""),
            "Rect" => vec![50.into(), 700.into(), 200.into(), 720.into()],
            "Ff" => Object::Integer(0),
            "P" => Object::Reference(page_id),
        });

        // Checkbox field (Button)
        let checkbox_field_id = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Widget",
            "T" => Object::string_literal("agree"),
            "FT" => "Btn",
            "V" => "Yes",
            "DV" => "Off",
            "Rect" => vec![50.into(), 650.into(), 70.into(), 670.into()],
            "Ff" => Object::Integer(0),
            "P" => Object::Reference(page_id),
        });

        // Radio button field (Button with flags)
        let radio_field_id = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Widget",
            "T" => Object::string_literal("gender"),
            "FT" => "Btn",
            "V" => "Male",
            "Rect" => vec![50.into(), 600.into(), 70.into(), 620.into()],
            "Ff" => Object::Integer(49152), // Radio flag (bit 15) + NoToggleToOff (bit 14)
            "P" => Object::Reference(page_id),
        });

        // Dropdown field (Choice)
        let dropdown_field_id = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Widget",
            "T" => Object::string_literal("country"),
            "FT" => "Ch",
            "V" => Object::string_literal("US"),
            "Rect" => vec![50.into(), 550.into(), 200.into(), 570.into()],
            "Opt" => vec![
                Object::string_literal("US"),
                Object::string_literal("UK"),
                Object::string_literal("FR"),
            ],
            "Ff" => Object::Integer(0),
            "P" => Object::Reference(page_id),
        });

        // Field with no value
        let empty_field_id = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Widget",
            "T" => Object::string_literal("email"),
            "FT" => "Tx",
            "Rect" => vec![50.into(), 500.into(), 200.into(), 520.into()],
            "Ff" => Object::Integer(0),
            "P" => Object::Reference(page_id),
        });

        // AcroForm dictionary
        let acroform_id = doc.add_object(dictionary! {
            "Fields" => vec![
                Object::Reference(text_field_id),
                Object::Reference(checkbox_field_id),
                Object::Reference(radio_field_id),
                Object::Reference(dropdown_field_id),
                Object::Reference(empty_field_id),
            ],
        });

        // Catalog
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "AcroForm" => Object::Reference(acroform_id),
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("failed to save test PDF");
        buf
    }

    #[test]
    fn form_fields_text_field() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();

        let text_field = fields.iter().find(|f| f.name == "name").unwrap();
        assert_eq!(text_field.field_type, FieldType::Text);
        assert_eq!(text_field.value.as_deref(), Some("John Doe"));
        assert_eq!(text_field.default_value.as_deref(), Some(""));
    }

    #[test]
    fn form_fields_checkbox() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();

        let checkbox = fields.iter().find(|f| f.name == "agree").unwrap();
        assert_eq!(checkbox.field_type, FieldType::Button);
        assert_eq!(checkbox.value.as_deref(), Some("Yes"));
        assert_eq!(checkbox.default_value.as_deref(), Some("Off"));
    }

    #[test]
    fn form_fields_radio_button() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();

        let radio = fields.iter().find(|f| f.name == "gender").unwrap();
        assert_eq!(radio.field_type, FieldType::Button);
        assert_eq!(radio.value.as_deref(), Some("Male"));
        assert_eq!(radio.flags, 49152); // Radio flags
    }

    #[test]
    fn form_fields_dropdown_with_options() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();

        let dropdown = fields.iter().find(|f| f.name == "country").unwrap();
        assert_eq!(dropdown.field_type, FieldType::Choice);
        assert_eq!(dropdown.value.as_deref(), Some("US"));
        assert_eq!(dropdown.options, vec!["US", "UK", "FR"]);
    }

    #[test]
    fn form_fields_no_value() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();

        let empty = fields.iter().find(|f| f.name == "email").unwrap();
        assert_eq!(empty.field_type, FieldType::Text);
        assert!(empty.value.is_none());
        assert!(empty.default_value.is_none());
    }

    #[test]
    fn form_fields_count() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();
        assert_eq!(fields.len(), 5);
    }

    #[test]
    fn form_fields_no_acroform_returns_empty() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();
        assert!(fields.is_empty());
    }

    #[test]
    fn form_fields_have_bbox() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();

        let text_field = fields.iter().find(|f| f.name == "name").unwrap();
        assert!((text_field.bbox.x0 - 50.0).abs() < 0.1);
        assert!((text_field.bbox.x1 - 200.0).abs() < 0.1);
    }

    #[test]
    fn form_fields_have_page_index() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();

        // All fields reference page 0
        for field in &fields {
            assert_eq!(field.page_index, Some(0));
        }
    }

    // --- Structure tree tests (US-081) ---

    /// Create a test PDF with a structure tree (tagged PDF).
    ///
    /// Structure: Document -> H1 (MCID 0) -> P (MCID 1)
    fn create_test_pdf_with_structure_tree() -> Vec<u8> {
        use lopdf::{Document, Object, ObjectId, Stream, dictionary};

        let mut doc = Document::with_version("1.7");
        let pages_id: ObjectId = doc.new_object_id();

        // Content stream with marked content
        let content = b"BT /F1 24 Tf /H1 <</MCID 0>> BDC 72 700 Td (Chapter 1) Tj EMC /P <</MCID 1>> BDC /F1 12 Tf 72 670 Td (This is paragraph text.) Tj EMC ET";
        let stream = Stream::new(dictionary! {}, content.to_vec());
        let content_id = doc.add_object(Object::Stream(stream));

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => Object::Reference(content_id),
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => Object::Reference(font_id),
                },
            },
        });

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => Object::Integer(1),
            }),
        );

        // Structure tree elements
        // H1 element with MCID 0
        let h1_elem_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "H1",
            "K" => Object::Integer(0),
            "Pg" => Object::Reference(page_id),
        });

        // P element with MCID 1
        let p_elem_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "P",
            "K" => Object::Integer(1),
            "Pg" => Object::Reference(page_id),
            "Lang" => Object::string_literal("en-US"),
        });

        // Document root element
        let doc_elem_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "Document",
            "K" => vec![
                Object::Reference(h1_elem_id),
                Object::Reference(p_elem_id),
            ],
        });

        // StructTreeRoot
        let struct_tree_id = doc.add_object(dictionary! {
            "Type" => "StructTreeRoot",
            "K" => Object::Reference(doc_elem_id),
        });

        // Mark document as tagged
        let mark_info_id = doc.add_object(dictionary! {
            "Marked" => Object::Boolean(true),
        });

        // Catalog
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "StructTreeRoot" => Object::Reference(struct_tree_id),
            "MarkInfo" => Object::Reference(mark_info_id),
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf)
            .expect("failed to save tagged test PDF");
        buf
    }

    /// Create a test PDF with a structure tree containing a table.
    fn create_test_pdf_with_table_structure() -> Vec<u8> {
        use lopdf::{Document, Object, ObjectId, Stream, dictionary};

        let mut doc = Document::with_version("1.7");
        let pages_id: ObjectId = doc.new_object_id();

        let content = b"BT /F1 12 Tf 72 700 Td (Cell 1) Tj ET";
        let stream = Stream::new(dictionary! {}, content.to_vec());
        let content_id = doc.add_object(Object::Stream(stream));

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => Object::Reference(content_id),
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => Object::Reference(font_id),
                },
            },
        });

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => Object::Integer(1),
            }),
        );

        // Table structure: Table -> TR -> TD (MCID 0), TD (MCID 1)
        let td1_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "TD",
            "K" => Object::Integer(0),
            "Pg" => Object::Reference(page_id),
        });

        let td2_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "TD",
            "K" => Object::Integer(1),
            "Pg" => Object::Reference(page_id),
        });

        let tr_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "TR",
            "K" => vec![Object::Reference(td1_id), Object::Reference(td2_id)],
        });

        let table_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "Table",
            "K" => Object::Reference(tr_id),
            "Pg" => Object::Reference(page_id),
        });

        let struct_tree_id = doc.add_object(dictionary! {
            "Type" => "StructTreeRoot",
            "K" => Object::Reference(table_id),
        });

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "StructTreeRoot" => Object::Reference(struct_tree_id),
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("failed to save test PDF");
        buf
    }

    #[test]
    fn structure_tree_tagged_pdf_has_elements() {
        let pdf_bytes = create_test_pdf_with_structure_tree();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        assert!(!elements.is_empty());
    }

    #[test]
    fn structure_tree_document_root_element() {
        let pdf_bytes = create_test_pdf_with_structure_tree();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        // Root should be "Document" element
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].element_type, "Document");
        assert_eq!(elements[0].children.len(), 2);
    }

    #[test]
    fn structure_tree_heading_element() {
        let pdf_bytes = create_test_pdf_with_structure_tree();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        let doc_elem = &elements[0];
        let h1 = &doc_elem.children[0];
        assert_eq!(h1.element_type, "H1");
        assert_eq!(h1.mcids, vec![0]);
        assert_eq!(h1.page_index, Some(0));
    }

    #[test]
    fn structure_tree_paragraph_element() {
        let pdf_bytes = create_test_pdf_with_structure_tree();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        let doc_elem = &elements[0];
        let p = &doc_elem.children[1];
        assert_eq!(p.element_type, "P");
        assert_eq!(p.mcids, vec![1]);
        assert_eq!(p.page_index, Some(0));
        assert_eq!(p.lang.as_deref(), Some("en-US"));
    }

    #[test]
    fn structure_tree_untagged_pdf_returns_empty() {
        // Use the basic test PDF helper (no structure tree)
        let pdf_bytes = create_test_pdf_with_text_content();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        assert!(elements.is_empty());
    }

    #[test]
    fn structure_tree_table_nested_structure() {
        let pdf_bytes = create_test_pdf_with_table_structure();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        // Root is Table element
        assert_eq!(elements.len(), 1);
        let table = &elements[0];
        assert_eq!(table.element_type, "Table");

        // Table -> TR
        assert_eq!(table.children.len(), 1);
        let tr = &table.children[0];
        assert_eq!(tr.element_type, "TR");

        // TR -> TD, TD
        assert_eq!(tr.children.len(), 2);
        assert_eq!(tr.children[0].element_type, "TD");
        assert_eq!(tr.children[0].mcids, vec![0]);
        assert_eq!(tr.children[1].element_type, "TD");
        assert_eq!(tr.children[1].mcids, vec![1]);
    }

    #[test]
    fn structure_tree_mcr_dictionary_handling() {
        // Test with MCR (marked content reference) dictionaries instead of integer MCIDs
        use lopdf::{Document, Object, ObjectId, Stream, dictionary};

        let mut doc = Document::with_version("1.7");
        let pages_id: ObjectId = doc.new_object_id();

        let content = b"BT /F1 12 Tf 72 700 Td (text) Tj ET";
        let stream = Stream::new(dictionary! {}, content.to_vec());
        let content_id = doc.add_object(Object::Stream(stream));

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => Object::Reference(content_id),
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => Object::Reference(font_id),
                },
            },
        });

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => Object::Integer(1),
            }),
        );

        // Structure element with MCR dictionary in /K
        let p_elem_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "P",
            "K" => dictionary! {
                "Type" => "MCR",
                "MCID" => Object::Integer(5),
                "Pg" => Object::Reference(page_id),
            },
            "Pg" => Object::Reference(page_id),
        });

        let struct_tree_id = doc.add_object(dictionary! {
            "Type" => "StructTreeRoot",
            "K" => Object::Reference(p_elem_id),
        });

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "StructTreeRoot" => Object::Reference(struct_tree_id),
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("failed to save test PDF");

        let doc = LopdfBackend::open(&buf).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        assert_eq!(elements.len(), 1);
        let p = &elements[0];
        assert_eq!(p.element_type, "P");
        assert_eq!(p.mcids, vec![5]); // MCID from MCR dictionary
    }

    #[test]
    fn structure_tree_alt_text() {
        use lopdf::{Document, Object, ObjectId, Stream, dictionary};

        let mut doc = Document::with_version("1.7");
        let pages_id: ObjectId = doc.new_object_id();

        let content = b"BT /F1 12 Tf 72 700 Td (image) Tj ET";
        let stream = Stream::new(dictionary! {}, content.to_vec());
        let content_id = doc.add_object(Object::Stream(stream));

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => Object::Reference(content_id),
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => Object::Reference(font_id),
                },
            },
        });

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => Object::Integer(1),
            }),
        );

        // Figure element with /Alt and /ActualText
        let fig_elem_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "Figure",
            "K" => Object::Integer(0),
            "Pg" => Object::Reference(page_id),
            "Alt" => Object::string_literal("A photo of a sunset"),
            "ActualText" => Object::string_literal("Sunset photo"),
        });

        let struct_tree_id = doc.add_object(dictionary! {
            "Type" => "StructTreeRoot",
            "K" => Object::Reference(fig_elem_id),
        });

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "StructTreeRoot" => Object::Reference(struct_tree_id),
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("failed to save test PDF");

        let doc = LopdfBackend::open(&buf).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        assert_eq!(elements.len(), 1);
        let fig = &elements[0];
        assert_eq!(fig.element_type, "Figure");
        assert_eq!(fig.alt_text.as_deref(), Some("A photo of a sunset"));
        assert_eq!(fig.actual_text.as_deref(), Some("Sunset photo"));
    }

    // --- indirect reference box tests (Issue #163) ---

    /// Create a PDF where the page's MediaBox is an indirect reference.
    /// This reproduces the structure seen in annotations.pdf where
    /// `/MediaBox 174 0 R` points to a separate array object.
    fn create_test_pdf_indirect_media_box() -> Vec<u8> {
        use lopdf::{Document, Object, ObjectId, dictionary};

        let mut doc = Document::with_version("1.5");
        let pages_id: ObjectId = doc.new_object_id();

        // Create the MediaBox array as a separate indirect object
        let media_box_id = doc.add_object(Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(595),
            Object::Integer(842),
        ]));

        // Page references MediaBox indirectly via `174 0 R` style
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Reference(media_box_id),
        });

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::from(page_id)],
                "Count" => 1i64,
            }),
        );

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("failed to save test PDF");
        buf
    }

    #[test]
    fn media_box_indirect_reference() {
        let pdf_bytes = create_test_pdf_indirect_media_box();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let media_box = LopdfBackend::page_media_box(&doc, &page).unwrap();
        assert_eq!(media_box, BBox::new(0.0, 0.0, 595.0, 842.0));
    }

    // --- US-165-1: Handle broken startxref / 0-page PDF ---

    /// Create a minimal PDF with a traditional xref table and a deliberately
    /// wrong startxref offset. This mimics issue-297-example.pdf.
    fn create_pdf_with_broken_startxref() -> Vec<u8> {
        // Hand-craft a minimal valid PDF with traditional xref, then corrupt startxref.
        // Object layout:
        //   1 0 obj: /Pages (empty, Count 0)
        //   2 0 obj: /Catalog -> /Pages 1 0 R
        let body = b"%PDF-1.5\n\
            1 0 obj\n<< /Type /Pages /Kids [] /Count 0 >>\nendobj\n\
            2 0 obj\n<< /Type /Catalog /Pages 1 0 R >>\nendobj\n";
        let xref_offset = body.len();
        let xref_and_trailer = format!(
            "xref\n\
             0 3\n\
             0000000000 65535 f \n\
             0000000009 00000 n \n\
             0000000062 00000 n \n\
             trailer\n<< /Size 3 /Root 2 0 R >>\n\
             startxref\n{}\n%%EOF\n",
            xref_offset + 5 // deliberately wrong: off by +5
        );
        let mut pdf = body.to_vec();
        pdf.extend_from_slice(xref_and_trailer.as_bytes());
        pdf
    }

    #[test]
    fn open_pdf_with_broken_startxref_recovers() {
        let broken_bytes = create_pdf_with_broken_startxref();

        // Normal lopdf parsing should fail due to wrong startxref
        assert!(
            lopdf::Document::load_mem(&broken_bytes).is_err(),
            "lopdf should fail on broken startxref"
        );

        // Our LopdfBackend::open should recover via startxref repair
        let result = LopdfBackend::open(&broken_bytes);
        assert!(
            result.is_ok(),
            "LopdfBackend::open should recover from broken startxref, got: {:?}",
            result.err()
        );

        // Should have 0 pages (empty /Kids)
        let doc = result.unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 0);
    }

    #[test]
    fn open_issue_297_example_pdf_succeeds() {
        let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("pdfplumber/tests/fixtures/pdfs/issue-297-example.pdf");
        if !pdf_path.exists() {
            eprintln!(
                "Skipping: issue-297-example.pdf not found at {:?}",
                pdf_path
            );
            return;
        }
        let bytes = std::fs::read(&pdf_path).unwrap();
        let result = LopdfBackend::open(&bytes);
        assert!(
            result.is_ok(),
            "issue-297-example.pdf should open without error, got: {:?}",
            result.err()
        );
    }

    #[test]
    fn try_strip_preamble_no_preamble() {
        let valid_pdf = create_test_pdf_with_text_content();
        // No preamble — should return None
        assert!(try_strip_preamble(&valid_pdf).is_none());
    }

    #[test]
    fn try_strip_preamble_with_preamble() {
        let valid_pdf = create_test_pdf_with_text_content();
        let preamble = b"GPL Ghostscript 10.02.0\nCopyright notice\n";
        let mut pdf_with_preamble = Vec::with_capacity(preamble.len() + valid_pdf.len());
        pdf_with_preamble.extend_from_slice(preamble);
        pdf_with_preamble.extend_from_slice(&valid_pdf);

        let result = try_strip_preamble(&pdf_with_preamble);
        assert!(result.is_some(), "should detect and strip preamble");

        let stripped = result.unwrap();
        // Stripped bytes should start with %PDF-
        assert!(
            stripped.starts_with(b"%PDF-"),
            "stripped bytes should start with %PDF-"
        );
        // Preamble bytes should be removed
        assert_eq!(
            stripped.len(),
            valid_pdf.len(),
            "stripped bytes should equal original PDF length"
        );
    }

    #[test]
    fn try_strip_preamble_removes_page_markers() {
        // Simulate a Ghostscript PDF with "Page N\n" markers before endstream
        let mut pdf = b"%PDF-1.7\n1 0 obj\n<</Type/Catalog>>\nendobj\n".to_vec();
        // A fake stream with a Page marker injected before endstream
        pdf.extend_from_slice(b"2 0 obj\n<</Length 5>>\nstream\nhello");
        pdf.extend_from_slice(b"Page 1\n");
        pdf.extend_from_slice(b"endstream\nendobj\n");
        pdf.extend_from_slice(b"xref\n0 3\n");

        let result = try_strip_preamble(&pdf);
        assert!(result.is_some(), "should detect page markers");
        let cleaned = result.unwrap();
        // The "Page 1\n" marker should be removed
        assert!(
            !cleaned.windows(7).any(|w| w == b"Page 1\n"),
            "page marker should be removed"
        );
        // endstream should still be present
        assert!(
            cleaned.windows(9).any(|w| w == b"endstream"),
            "endstream should still be present"
        );
    }

    #[test]
    fn open_issue_848_pdf_succeeds() {
        let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("pdfplumber/tests/fixtures/pdfs/issue-848.pdf");
        if !pdf_path.exists() {
            eprintln!("Skipping: issue-848.pdf not found at {:?}", pdf_path);
            return;
        }
        let bytes = std::fs::read(&pdf_path).unwrap();
        let result = LopdfBackend::open(&bytes);
        assert!(
            result.is_ok(),
            "issue-848.pdf should open without error, got: {:?}",
            result.err()
        );
        let doc = result.unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 8);
    }

    #[test]
    fn issue_297_example_pdf_has_zero_pages() {
        let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("pdfplumber/tests/fixtures/pdfs/issue-297-example.pdf");
        if !pdf_path.exists() {
            eprintln!("Skipping: issue-297-example.pdf not found");
            return;
        }
        let bytes = std::fs::read(&pdf_path).unwrap();
        let doc = LopdfBackend::open(&bytes).unwrap();
        assert_eq!(
            LopdfBackend::page_count(&doc),
            0,
            "issue-297-example.pdf should have 0 pages (matching Python pdfplumber)"
        );
    }
}
