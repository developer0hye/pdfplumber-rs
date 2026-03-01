//! Image extraction from XObject Do operator.
//!
//! Extracts Image objects from the CTM active when the `Do` operator
//! is invoked for an Image XObject. The image is placed in a 1×1 unit
//! square that is mapped to the page via the CTM.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hasher};

use crate::geometry::{BBox, Ctm, Point};

/// Metadata about an image XObject from the PDF resource dictionary.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageMetadata {
    /// Original pixel width of the image.
    pub src_width: Option<u32>,
    /// Original pixel height of the image.
    pub src_height: Option<u32>,
    /// Bits per component (e.g., 8).
    pub bits_per_component: Option<u32>,
    /// Color space name (e.g., "DeviceRGB", "DeviceGray").
    pub color_space: Option<String>,
}

/// An image extracted from a PDF page via the Do operator.
///
/// Coordinates use pdfplumber's top-left origin system.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Image {
    /// Left x coordinate.
    pub x0: f64,
    /// Top y coordinate (distance from top of page).
    pub top: f64,
    /// Right x coordinate.
    pub x1: f64,
    /// Bottom y coordinate (distance from top of page).
    pub bottom: f64,
    /// Display width in points.
    pub width: f64,
    /// Display height in points.
    pub height: f64,
    /// XObject name (e.g., "Im0").
    pub name: String,
    /// Original pixel width.
    pub src_width: Option<u32>,
    /// Original pixel height.
    pub src_height: Option<u32>,
    /// Bits per component.
    pub bits_per_component: Option<u32>,
    /// Color space name.
    pub color_space: Option<String>,
    /// Raw image stream data (populated when `extract_image_data` is enabled).
    pub data: Option<Vec<u8>>,
    /// PDF stream filter used to encode this image.
    pub filter: Option<ImageFilter>,
    /// MIME type of the image data (e.g., "image/jpeg").
    pub mime_type: Option<String>,
}

/// Extract an Image from the CTM active during a Do operator invocation.
///
/// Image XObjects are defined in a 1×1 unit square. The CTM maps this
/// unit square to the actual display area on the page. The four corners
/// of the unit square `(0,0), (1,0), (0,1), (1,1)` are transformed
/// through the CTM to compute the bounding box.
///
/// Coordinates are converted from PDF bottom-left origin to top-left origin
/// using `page_height`.
pub fn image_from_ctm(ctm: &Ctm, name: &str, page_height: f64, metadata: &ImageMetadata) -> Image {
    // Transform the 4 corners of the unit square through the CTM
    let corners = [
        ctm.transform_point(Point::new(0.0, 0.0)),
        ctm.transform_point(Point::new(1.0, 0.0)),
        ctm.transform_point(Point::new(0.0, 1.0)),
        ctm.transform_point(Point::new(1.0, 1.0)),
    ];

    // Find bounding box in PDF coordinates
    let pdf_x0 = corners.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let pdf_x1 = corners
        .iter()
        .map(|p| p.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let pdf_y0 = corners.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let pdf_y1 = corners
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, f64::max);

    // Convert to top-left origin
    let top = page_height - pdf_y1;
    let bottom = page_height - pdf_y0;

    let width = pdf_x1 - pdf_x0;
    let height = bottom - top;

    Image {
        x0: pdf_x0,
        top,
        x1: pdf_x1,
        bottom,
        width,
        height,
        name: name.to_string(),
        src_width: metadata.src_width,
        src_height: metadata.src_height,
        bits_per_component: metadata.bits_per_component,
        color_space: metadata.color_space.clone(),
        data: None,
        filter: None,
        mime_type: None,
    }
}

impl Image {
    /// Returns the bounding box in top-left origin coordinates.
    pub fn bbox(&self) -> BBox {
        BBox::new(self.x0, self.top, self.x1, self.bottom)
    }
}

/// PDF stream filter used to encode image data.
///
/// Maps to the `/Filter` entry in a PDF image XObject stream dictionary.
/// Used to identify how image data was encoded in the PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ImageFilter {
    /// JPEG compression (DCTDecode).
    DCTDecode,
    /// Flate (zlib/deflate) compression.
    FlateDecode,
    /// CCITT fax compression (Group 3 or 4).
    CCITTFaxDecode,
    /// JBIG2 compression.
    JBIG2Decode,
    /// JPEG 2000 compression (JPXDecode).
    JPXDecode,
    /// LZW compression.
    LZWDecode,
    /// Run-length encoding.
    RunLengthDecode,
    /// No filter — raw uncompressed data.
    Raw,
}

impl ImageFilter {
    /// Returns the MIME type for the image data produced by this filter.
    pub fn mime_type(&self) -> &str {
        match self {
            ImageFilter::DCTDecode => "image/jpeg",
            ImageFilter::JPXDecode => "image/jp2",
            ImageFilter::JBIG2Decode => "image/x-jbig2",
            ImageFilter::CCITTFaxDecode => "image/tiff",
            ImageFilter::FlateDecode => "application/octet-stream",
            ImageFilter::LZWDecode => "application/octet-stream",
            ImageFilter::RunLengthDecode => "application/octet-stream",
            ImageFilter::Raw => "application/octet-stream",
        }
    }

    /// Returns the normalized file extension for this filter.
    ///
    /// Maps PDF stream filters to standard file extensions:
    /// DCTDecode → "jpg", FlateDecode → "png", JPXDecode → "jp2",
    /// CCITTFaxDecode → "tiff", all others → "bin".
    pub fn extension(&self) -> &str {
        match self {
            ImageFilter::DCTDecode => "jpg",
            ImageFilter::FlateDecode => "png",
            ImageFilter::JPXDecode => "jp2",
            ImageFilter::CCITTFaxDecode => "tiff",
            _ => "bin",
        }
    }

    /// Parse a PDF filter name string to an `ImageFilter`.
    pub fn from_pdf_name(name: &str) -> Self {
        match name {
            "DCTDecode" => ImageFilter::DCTDecode,
            "FlateDecode" => ImageFilter::FlateDecode,
            "CCITTFaxDecode" => ImageFilter::CCITTFaxDecode,
            "JBIG2Decode" => ImageFilter::JBIG2Decode,
            "JPXDecode" => ImageFilter::JPXDecode,
            "LZWDecode" => ImageFilter::LZWDecode,
            "RunLengthDecode" => ImageFilter::RunLengthDecode,
            _ => ImageFilter::Raw,
        }
    }
}

/// Format of extracted image data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ImageFormat {
    /// JPEG image (DCTDecode filter).
    Jpeg,
    /// PNG image.
    Png,
    /// Raw uncompressed pixel data.
    Raw,
    /// JBIG2 compressed image.
    Jbig2,
    /// CCITT fax compressed image.
    CcittFax,
}

impl ImageFormat {
    /// Returns the typical file extension for this image format.
    pub fn extension(&self) -> &str {
        match self {
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Png => "png",
            ImageFormat::Raw => "raw",
            ImageFormat::Jbig2 => "jbig2",
            ImageFormat::CcittFax => "ccitt",
        }
    }
}

/// Extracted image content (raw bytes) from a PDF image XObject.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageContent {
    /// The image data bytes.
    pub data: Vec<u8>,
    /// The format of the image data.
    pub format: ImageFormat,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

/// Options for exporting images with deterministic naming.
///
/// Controls filename pattern and deduplication behavior for image export.
/// Pattern variables: `{page}` (1-indexed page number), `{index}` (0-indexed
/// image index on page), `{ext}` (normalized extension), `{hash}` (content
/// hash prefix).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageExportOptions {
    /// Filename pattern with variable substitution.
    /// Default: `"page{page}_img{index}.{ext}"`
    pub pattern: String,
    /// When true, identical images (by content hash) share the same filename.
    /// Default: `false`
    pub deduplicate: bool,
}

impl Default for ImageExportOptions {
    fn default() -> Self {
        Self {
            pattern: "page{page}_img{index}.{ext}".to_string(),
            deduplicate: false,
        }
    }
}

/// An exported image with deterministic filename, data, and metadata.
///
/// Produced by [`export_image_set`] or `Page::export_images`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExportedImage {
    /// Deterministic filename based on the export pattern.
    pub filename: String,
    /// Raw image data bytes.
    pub data: Vec<u8>,
    /// MIME type of the image (e.g., `"image/jpeg"`).
    pub mime_type: String,
    /// 1-indexed page number.
    pub page: usize,
}

/// Compute a deterministic content hash prefix for image data.
///
/// Returns a 16-character hex string derived from the data bytes.
/// Uses SipHash (via `DefaultHasher::new()`) which is deterministic
/// for the same input within the same Rust version.
pub fn content_hash_prefix(data: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    hasher.write(data);
    format!("{:016x}", hasher.finish())
}

/// Apply a filename pattern by substituting variables.
///
/// Variables: `{page}` → 1-indexed page, `{index}` → 0-indexed image index,
/// `{ext}` → normalized extension, `{hash}` → content hash prefix.
pub fn apply_export_pattern(
    pattern: &str,
    page: usize,
    index: usize,
    ext: &str,
    hash: &str,
) -> String {
    pattern
        .replace("{page}", &page.to_string())
        .replace("{index}", &index.to_string())
        .replace("{ext}", ext)
        .replace("{hash}", hash)
}

/// Export a set of images from a page with deterministic filenames.
///
/// Takes the images from a page (with optional data populated),
/// the 1-indexed page number, and export options. Images without
/// data (`data: None`) are skipped.
///
/// When `options.deduplicate` is true, images with identical content
/// (by hash) share the same filename.
pub fn export_image_set(
    images: &[Image],
    page_number: usize,
    options: &ImageExportOptions,
) -> Vec<ExportedImage> {
    let mut results = Vec::new();
    let mut seen_hashes: HashMap<String, String> = HashMap::new();

    for (index, image) in images.iter().enumerate() {
        let data = match &image.data {
            Some(d) => d.clone(),
            None => continue,
        };

        let ext = image
            .filter
            .as_ref()
            .map(|f| f.extension())
            .unwrap_or("bin");

        let hash = content_hash_prefix(&data);

        let filename = if options.deduplicate {
            if let Some(existing) = seen_hashes.get(&hash) {
                existing.clone()
            } else {
                let name = apply_export_pattern(&options.pattern, page_number, index, ext, &hash);
                seen_hashes.insert(hash.clone(), name.clone());
                name
            }
        } else {
            apply_export_pattern(&options.pattern, page_number, index, ext, &hash)
        };

        let mime_type = image
            .filter
            .as_ref()
            .map(|f| f.mime_type().to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());

        results.push(ExportedImage {
            filename,
            data,
            mime_type,
            page: page_number,
        });
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx(a: f64, b: f64) {
        assert!(
            (a - b).abs() < 1e-6,
            "expected {b}, got {a}, diff={}",
            (a - b).abs()
        );
    }

    const PAGE_HEIGHT: f64 = 792.0;

    // --- Image struct ---

    #[test]
    fn test_image_construction_and_field_access() {
        let img = Image {
            x0: 72.0,
            top: 100.0,
            x1: 272.0,
            bottom: 250.0,
            width: 200.0,
            height: 150.0,
            name: "Im0".to_string(),
            src_width: Some(1920),
            src_height: Some(1080),
            bits_per_component: Some(8),
            color_space: Some("DeviceRGB".to_string()),
            data: None,
            filter: None,
            mime_type: None,
        };
        assert_eq!(img.x0, 72.0);
        assert_eq!(img.top, 100.0);
        assert_eq!(img.x1, 272.0);
        assert_eq!(img.bottom, 250.0);
        assert_eq!(img.width, 200.0);
        assert_eq!(img.height, 150.0);
        assert_eq!(img.name, "Im0");
        assert_eq!(img.src_width, Some(1920));
        assert_eq!(img.src_height, Some(1080));
        assert_eq!(img.bits_per_component, Some(8));
        assert_eq!(img.color_space, Some("DeviceRGB".to_string()));
        assert_eq!(img.data, None);
        assert_eq!(img.filter, None);
        assert_eq!(img.mime_type, None);

        let bbox = img.bbox();
        assert_approx(bbox.x0, 72.0);
        assert_approx(bbox.top, 100.0);
        assert_approx(bbox.x1, 272.0);
        assert_approx(bbox.bottom, 250.0);
    }

    #[test]
    fn test_image_bbox() {
        let img = Image {
            x0: 100.0,
            top: 200.0,
            x1: 300.0,
            bottom: 400.0,
            width: 200.0,
            height: 200.0,
            name: "Im0".to_string(),
            src_width: Some(640),
            src_height: Some(480),
            bits_per_component: Some(8),
            color_space: Some("DeviceRGB".to_string()),
            data: None,
            filter: None,
            mime_type: None,
        };
        let bbox = img.bbox();
        assert_approx(bbox.x0, 100.0);
        assert_approx(bbox.top, 200.0);
        assert_approx(bbox.x1, 300.0);
        assert_approx(bbox.bottom, 400.0);
    }

    // --- image_from_ctm ---

    #[test]
    fn test_image_from_ctm_simple_placement() {
        // CTM places a 200x150 image at (100, 500) in PDF coords
        // a=200 (width), d=150 (height), e=100 (x), f=500 (y)
        let ctm = Ctm::new(200.0, 0.0, 0.0, 150.0, 100.0, 500.0);
        let meta = ImageMetadata {
            src_width: Some(640),
            src_height: Some(480),
            bits_per_component: Some(8),
            color_space: Some("DeviceRGB".to_string()),
        };

        let img = image_from_ctm(&ctm, "Im0", PAGE_HEIGHT, &meta);

        assert_approx(img.x0, 100.0);
        assert_approx(img.x1, 300.0);
        // y-flip: top = 792 - 650 = 142, bottom = 792 - 500 = 292
        assert_approx(img.top, 142.0);
        assert_approx(img.bottom, 292.0);
        assert_approx(img.width, 200.0);
        assert_approx(img.height, 150.0);
        assert_eq!(img.name, "Im0");
        assert_eq!(img.src_width, Some(640));
        assert_eq!(img.src_height, Some(480));
        assert_eq!(img.bits_per_component, Some(8));
        assert_eq!(img.color_space, Some("DeviceRGB".to_string()));
    }

    #[test]
    fn test_image_from_ctm_identity() {
        // Identity CTM: image is 1×1 at origin
        let ctm = Ctm::identity();
        let meta = ImageMetadata::default();

        let img = image_from_ctm(&ctm, "Im1", PAGE_HEIGHT, &meta);

        assert_approx(img.x0, 0.0);
        assert_approx(img.x1, 1.0);
        // y-flip: top = 792 - 1 = 791, bottom = 792 - 0 = 792
        assert_approx(img.top, 791.0);
        assert_approx(img.bottom, 792.0);
        assert_approx(img.width, 1.0);
        assert_approx(img.height, 1.0);
    }

    #[test]
    fn test_image_from_ctm_translation_only() {
        // 1×1 image translated to (300, 400)
        let ctm = Ctm::new(1.0, 0.0, 0.0, 1.0, 300.0, 400.0);
        let meta = ImageMetadata::default();

        let img = image_from_ctm(&ctm, "Im2", PAGE_HEIGHT, &meta);

        assert_approx(img.x0, 300.0);
        assert_approx(img.x1, 301.0);
        // y-flip: top = 792 - 401 = 391, bottom = 792 - 400 = 392
        assert_approx(img.top, 391.0);
        assert_approx(img.bottom, 392.0);
    }

    #[test]
    fn test_image_from_ctm_scale_and_translate() {
        // 400×300 image at (50, 200)
        let ctm = Ctm::new(400.0, 0.0, 0.0, 300.0, 50.0, 200.0);
        let meta = ImageMetadata::default();

        let img = image_from_ctm(&ctm, "Im3", PAGE_HEIGHT, &meta);

        assert_approx(img.x0, 50.0);
        assert_approx(img.x1, 450.0);
        // y-flip: top = 792 - 500 = 292, bottom = 792 - 200 = 592
        assert_approx(img.top, 292.0);
        assert_approx(img.bottom, 592.0);
        assert_approx(img.width, 400.0);
        assert_approx(img.height, 300.0);
    }

    #[test]
    fn test_image_from_ctm_no_metadata() {
        let ctm = Ctm::new(100.0, 0.0, 0.0, 100.0, 200.0, 300.0);
        let meta = ImageMetadata::default();

        let img = image_from_ctm(&ctm, "ImX", PAGE_HEIGHT, &meta);

        assert_eq!(img.name, "ImX");
        assert_eq!(img.src_width, None);
        assert_eq!(img.src_height, None);
        assert_eq!(img.bits_per_component, None);
        assert_eq!(img.color_space, None);
    }

    #[test]
    fn test_image_from_ctm_different_page_height() {
        // Letter-size page (11 inches = 792pt) vs A4 (842pt)
        let ctm = Ctm::new(100.0, 0.0, 0.0, 100.0, 0.0, 0.0);
        let meta = ImageMetadata::default();

        let img_letter = image_from_ctm(&ctm, "Im0", 792.0, &meta);
        let img_a4 = image_from_ctm(&ctm, "Im0", 842.0, &meta);

        // Same width
        assert_approx(img_letter.width, img_a4.width);
        // Different top due to different page height
        assert_approx(img_letter.top, 692.0); // 792 - 100
        assert_approx(img_a4.top, 742.0); // 842 - 100
    }

    #[test]
    fn test_image_metadata_default() {
        let meta = ImageMetadata::default();
        assert_eq!(meta.src_width, None);
        assert_eq!(meta.src_height, None);
        assert_eq!(meta.bits_per_component, None);
        assert_eq!(meta.color_space, None);
    }

    // --- ImageFormat ---

    #[test]
    fn test_image_format_extension() {
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Raw.extension(), "raw");
        assert_eq!(ImageFormat::Jbig2.extension(), "jbig2");
        assert_eq!(ImageFormat::CcittFax.extension(), "ccitt");
    }

    #[test]
    fn test_image_format_clone_eq() {
        let fmt = ImageFormat::Jpeg;
        let fmt2 = fmt;
        assert_eq!(fmt, fmt2);
    }

    // --- ImageContent ---

    #[test]
    fn test_image_content_construction() {
        let content = ImageContent {
            data: vec![0xFF, 0xD8, 0xFF, 0xE0],
            format: ImageFormat::Jpeg,
            width: 640,
            height: 480,
        };
        assert_eq!(content.data.len(), 4);
        assert_eq!(content.format, ImageFormat::Jpeg);
        assert_eq!(content.width, 640);
        assert_eq!(content.height, 480);
    }

    #[test]
    fn test_image_content_raw_format() {
        // 2x2 RGB image = 12 bytes
        let data = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0];
        let content = ImageContent {
            data: data.clone(),
            format: ImageFormat::Raw,
            width: 2,
            height: 2,
        };
        assert_eq!(content.data, data);
        assert_eq!(content.format, ImageFormat::Raw);
        assert_eq!(content.width, 2);
        assert_eq!(content.height, 2);
    }

    #[test]
    fn test_image_content_clone_eq() {
        let content = ImageContent {
            data: vec![1, 2, 3],
            format: ImageFormat::Png,
            width: 10,
            height: 10,
        };
        let content2 = content.clone();
        assert_eq!(content, content2);
    }

    // --- ImageFilter tests ---

    #[test]
    fn test_image_filter_variants() {
        // Verify all 8 variants exist and are distinct
        let filters = [
            ImageFilter::DCTDecode,
            ImageFilter::FlateDecode,
            ImageFilter::CCITTFaxDecode,
            ImageFilter::JBIG2Decode,
            ImageFilter::JPXDecode,
            ImageFilter::LZWDecode,
            ImageFilter::RunLengthDecode,
            ImageFilter::Raw,
        ];
        for (i, a) in filters.iter().enumerate() {
            for (j, b) in filters.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn test_image_filter_mime_type() {
        assert_eq!(ImageFilter::DCTDecode.mime_type(), "image/jpeg");
        assert_eq!(ImageFilter::JPXDecode.mime_type(), "image/jp2");
        assert_eq!(ImageFilter::JBIG2Decode.mime_type(), "image/x-jbig2");
        assert_eq!(ImageFilter::CCITTFaxDecode.mime_type(), "image/tiff");
        assert_eq!(
            ImageFilter::FlateDecode.mime_type(),
            "application/octet-stream"
        );
        assert_eq!(
            ImageFilter::LZWDecode.mime_type(),
            "application/octet-stream"
        );
        assert_eq!(
            ImageFilter::RunLengthDecode.mime_type(),
            "application/octet-stream"
        );
        assert_eq!(ImageFilter::Raw.mime_type(), "application/octet-stream");
    }

    #[test]
    fn test_image_filter_from_pdf_name() {
        assert_eq!(
            ImageFilter::from_pdf_name("DCTDecode"),
            ImageFilter::DCTDecode
        );
        assert_eq!(
            ImageFilter::from_pdf_name("FlateDecode"),
            ImageFilter::FlateDecode
        );
        assert_eq!(
            ImageFilter::from_pdf_name("CCITTFaxDecode"),
            ImageFilter::CCITTFaxDecode
        );
        assert_eq!(
            ImageFilter::from_pdf_name("JBIG2Decode"),
            ImageFilter::JBIG2Decode
        );
        assert_eq!(
            ImageFilter::from_pdf_name("JPXDecode"),
            ImageFilter::JPXDecode
        );
        assert_eq!(
            ImageFilter::from_pdf_name("LZWDecode"),
            ImageFilter::LZWDecode
        );
        assert_eq!(
            ImageFilter::from_pdf_name("RunLengthDecode"),
            ImageFilter::RunLengthDecode
        );
        assert_eq!(
            ImageFilter::from_pdf_name("UnknownFilter"),
            ImageFilter::Raw
        );
    }

    #[test]
    fn test_image_filter_clone_copy() {
        let f = ImageFilter::DCTDecode;
        let f2 = f; // Copy
        let f3 = f.clone();
        assert_eq!(f, f2);
        assert_eq!(f, f3);
    }

    // --- Image with data fields ---

    #[test]
    fn test_image_with_data_populated() {
        let jpeg_data = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        let img = Image {
            x0: 72.0,
            top: 100.0,
            x1: 272.0,
            bottom: 250.0,
            width: 200.0,
            height: 150.0,
            name: "Im0".to_string(),
            src_width: Some(640),
            src_height: Some(480),
            bits_per_component: Some(8),
            color_space: Some("DeviceRGB".to_string()),
            data: Some(jpeg_data.clone()),
            filter: Some(ImageFilter::DCTDecode),
            mime_type: Some("image/jpeg".to_string()),
        };
        assert_eq!(img.data, Some(jpeg_data));
        assert_eq!(img.filter, Some(ImageFilter::DCTDecode));
        assert_eq!(img.mime_type, Some("image/jpeg".to_string()));
    }

    #[test]
    fn test_image_data_none_by_default() {
        // image_from_ctm should produce None for data/filter/mime_type
        let ctm = Ctm::new(100.0, 0.0, 0.0, 100.0, 50.0, 50.0);
        let meta = ImageMetadata::default();
        let img = image_from_ctm(&ctm, "Im0", PAGE_HEIGHT, &meta);
        assert_eq!(img.data, None);
        assert_eq!(img.filter, None);
        assert_eq!(img.mime_type, None);
    }

    // --- ImageFilter::extension ---

    #[test]
    fn test_image_filter_extension_normalization() {
        assert_eq!(ImageFilter::DCTDecode.extension(), "jpg");
        assert_eq!(ImageFilter::FlateDecode.extension(), "png");
        assert_eq!(ImageFilter::JPXDecode.extension(), "jp2");
        assert_eq!(ImageFilter::CCITTFaxDecode.extension(), "tiff");
        assert_eq!(ImageFilter::Raw.extension(), "bin");
        assert_eq!(ImageFilter::LZWDecode.extension(), "bin");
        assert_eq!(ImageFilter::RunLengthDecode.extension(), "bin");
        assert_eq!(ImageFilter::JBIG2Decode.extension(), "bin");
    }

    // --- ImageExportOptions ---

    #[test]
    fn test_image_export_options_default() {
        let opts = ImageExportOptions::default();
        assert_eq!(opts.pattern, "page{page}_img{index}.{ext}");
        assert!(!opts.deduplicate);
    }

    #[test]
    fn test_image_export_options_custom() {
        let opts = ImageExportOptions {
            pattern: "img_{hash}.{ext}".to_string(),
            deduplicate: true,
        };
        assert_eq!(opts.pattern, "img_{hash}.{ext}");
        assert!(opts.deduplicate);
    }

    #[test]
    fn test_image_export_options_clone() {
        let opts = ImageExportOptions::default();
        let opts2 = opts.clone();
        assert_eq!(opts, opts2);
    }

    // --- ExportedImage ---

    #[test]
    fn test_exported_image_construction() {
        let exported = ExportedImage {
            filename: "page1_img0.jpg".to_string(),
            data: vec![0xFF, 0xD8],
            mime_type: "image/jpeg".to_string(),
            page: 1,
        };
        assert_eq!(exported.filename, "page1_img0.jpg");
        assert_eq!(exported.data, vec![0xFF, 0xD8]);
        assert_eq!(exported.mime_type, "image/jpeg");
        assert_eq!(exported.page, 1);
    }

    // --- content_hash_prefix ---

    #[test]
    fn test_content_hash_prefix_deterministic() {
        let data = vec![1, 2, 3, 4, 5];
        let hash1 = content_hash_prefix(&data);
        let hash2 = content_hash_prefix(&data);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 16); // 16 hex chars
    }

    #[test]
    fn test_content_hash_prefix_different_data() {
        let hash1 = content_hash_prefix(&[1, 2, 3]);
        let hash2 = content_hash_prefix(&[4, 5, 6]);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_content_hash_prefix_empty_data() {
        let hash = content_hash_prefix(&[]);
        assert_eq!(hash.len(), 16);
    }

    // --- apply_export_pattern ---

    #[test]
    fn test_apply_export_pattern_default() {
        let result = apply_export_pattern("page{page}_img{index}.{ext}", 1, 0, "jpg", "abc123");
        assert_eq!(result, "page1_img0.jpg");
    }

    #[test]
    fn test_apply_export_pattern_with_hash() {
        let result = apply_export_pattern("img_{hash}.{ext}", 2, 3, "png", "deadbeef01234567");
        assert_eq!(result, "img_deadbeef01234567.png");
    }

    #[test]
    fn test_apply_export_pattern_all_variables() {
        let result = apply_export_pattern(
            "{page}_{index}_{hash}_{ext}",
            5,
            2,
            "jp2",
            "abcdef0123456789",
        );
        assert_eq!(result, "5_2_abcdef0123456789_jp2");
    }

    #[test]
    fn test_apply_export_pattern_no_variables() {
        let result = apply_export_pattern("static_name.png", 1, 0, "jpg", "hash");
        assert_eq!(result, "static_name.png");
    }

    // --- export_image_set ---

    fn make_test_image(name: &str, data: Vec<u8>, filter: ImageFilter) -> Image {
        Image {
            x0: 0.0,
            top: 0.0,
            x1: 100.0,
            bottom: 100.0,
            width: 100.0,
            height: 100.0,
            name: name.to_string(),
            src_width: Some(640),
            src_height: Some(480),
            bits_per_component: Some(8),
            color_space: Some("DeviceRGB".to_string()),
            data: Some(data),
            filter: Some(filter),
            mime_type: Some(filter.mime_type().to_string()),
        }
    }

    #[test]
    fn test_export_image_set_default_pattern() {
        let images = vec![
            make_test_image("Im0", vec![0xFF, 0xD8, 0xFF], ImageFilter::DCTDecode),
            make_test_image("Im1", vec![0x89, 0x50, 0x4E], ImageFilter::FlateDecode),
        ];

        let exported = export_image_set(&images, 1, &ImageExportOptions::default());
        assert_eq!(exported.len(), 2);
        assert_eq!(exported[0].filename, "page1_img0.jpg");
        assert_eq!(exported[0].mime_type, "image/jpeg");
        assert_eq!(exported[0].page, 1);
        assert_eq!(exported[0].data, vec![0xFF, 0xD8, 0xFF]);
        assert_eq!(exported[1].filename, "page1_img1.png");
        assert_eq!(exported[1].mime_type, "application/octet-stream");
        assert_eq!(exported[1].page, 1);
    }

    #[test]
    fn test_export_image_set_deduplication() {
        let shared_data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // same content
        let images = vec![
            make_test_image("Im0", shared_data.clone(), ImageFilter::DCTDecode),
            make_test_image("Im1", shared_data.clone(), ImageFilter::DCTDecode),
            make_test_image("Im2", vec![0x89, 0x50], ImageFilter::FlateDecode),
        ];

        let opts = ImageExportOptions {
            deduplicate: true,
            ..Default::default()
        };
        let exported = export_image_set(&images, 1, &opts);
        assert_eq!(exported.len(), 3);
        // First two images have identical data, so deduplicated filename
        assert_eq!(exported[0].filename, exported[1].filename);
        // Third image is different
        assert_ne!(exported[0].filename, exported[2].filename);
    }

    #[test]
    fn test_export_image_set_no_deduplication() {
        let shared_data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        let images = vec![
            make_test_image("Im0", shared_data.clone(), ImageFilter::DCTDecode),
            make_test_image("Im1", shared_data.clone(), ImageFilter::DCTDecode),
        ];

        let opts = ImageExportOptions {
            deduplicate: false,
            ..Default::default()
        };
        let exported = export_image_set(&images, 1, &opts);
        assert_eq!(exported.len(), 2);
        // Without deduplication, filenames differ by index
        assert_eq!(exported[0].filename, "page1_img0.jpg");
        assert_eq!(exported[1].filename, "page1_img1.jpg");
    }

    #[test]
    fn test_export_image_set_custom_pattern_with_hash() {
        let data = vec![0xFF, 0xD8, 0xFF];
        let images = vec![make_test_image("Im0", data.clone(), ImageFilter::DCTDecode)];
        let hash = content_hash_prefix(&data);

        let opts = ImageExportOptions {
            pattern: "img_{hash}.{ext}".to_string(),
            deduplicate: false,
        };
        let exported = export_image_set(&images, 1, &opts);
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].filename, format!("img_{hash}.jpg"));
    }

    #[test]
    fn test_export_image_set_skips_images_without_data() {
        let mut img_no_data = make_test_image("Im0", vec![1, 2, 3], ImageFilter::DCTDecode);
        img_no_data.data = None; // Remove data

        let img_with_data = make_test_image("Im1", vec![4, 5, 6], ImageFilter::FlateDecode);
        let images = vec![img_no_data, img_with_data];

        let exported = export_image_set(&images, 1, &ImageExportOptions::default());
        assert_eq!(exported.len(), 1);
        // Index is 1 because the skipped image was at index 0
        assert_eq!(exported[0].filename, "page1_img1.png");
    }

    #[test]
    fn test_export_image_set_empty_images() {
        let exported = export_image_set(&[], 1, &ImageExportOptions::default());
        assert!(exported.is_empty());
    }

    #[test]
    fn test_export_image_set_no_filter_defaults_to_bin() {
        let mut img = make_test_image("Im0", vec![1, 2, 3], ImageFilter::Raw);
        img.filter = None;
        img.mime_type = None;

        let exported = export_image_set(&[img], 1, &ImageExportOptions::default());
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].filename, "page1_img0.bin");
        assert_eq!(exported[0].mime_type, "application/octet-stream");
    }

    #[test]
    fn test_export_image_set_deterministic() {
        let images = vec![
            make_test_image("Im0", vec![0xFF, 0xD8], ImageFilter::DCTDecode),
            make_test_image("Im1", vec![0x89, 0x50], ImageFilter::FlateDecode),
        ];
        let opts = ImageExportOptions::default();

        let exported1 = export_image_set(&images, 1, &opts);
        let exported2 = export_image_set(&images, 1, &opts);
        assert_eq!(exported1, exported2);
    }

    #[test]
    fn test_export_image_set_multi_page() {
        let images = vec![make_test_image(
            "Im0",
            vec![0xFF, 0xD8],
            ImageFilter::DCTDecode,
        )];

        let exported_p1 = export_image_set(&images, 1, &ImageExportOptions::default());
        let exported_p3 = export_image_set(&images, 3, &ImageExportOptions::default());
        assert_eq!(exported_p1[0].filename, "page1_img0.jpg");
        assert_eq!(exported_p1[0].page, 1);
        assert_eq!(exported_p3[0].filename, "page3_img0.jpg");
        assert_eq!(exported_p3[0].page, 3);
    }

    #[test]
    fn test_export_image_set_all_filter_extensions() {
        let filters_expected = [
            (ImageFilter::DCTDecode, "jpg"),
            (ImageFilter::FlateDecode, "png"),
            (ImageFilter::JPXDecode, "jp2"),
            (ImageFilter::CCITTFaxDecode, "tiff"),
            (ImageFilter::Raw, "bin"),
            (ImageFilter::LZWDecode, "bin"),
            (ImageFilter::RunLengthDecode, "bin"),
            (ImageFilter::JBIG2Decode, "bin"),
        ];

        for (i, (filter, expected_ext)) in filters_expected.iter().enumerate() {
            let images = vec![make_test_image(&format!("Im{i}"), vec![i as u8], *filter)];
            let exported = export_image_set(&images, 1, &ImageExportOptions::default());
            assert_eq!(
                exported[0].filename,
                format!("page1_img0.{expected_ext}"),
                "Filter {:?} should produce extension {expected_ext}",
                filter,
            );
        }
    }
}
