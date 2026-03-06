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
///
/// `#[non_exhaustive]` — the PDF spec and extensions define additional filters
/// (e.g., `Crypt`, `ASCII85Decode`); new variants will be added in minor releases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
///
/// `#[non_exhaustive]` — new formats (e.g., AVIF, WebP via future PDF extensions)
/// may be added in minor releases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
mod tests;
