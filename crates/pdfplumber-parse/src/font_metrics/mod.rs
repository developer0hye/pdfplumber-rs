//! Font metrics extraction from PDF font dictionaries.
//!
//! Parses /Widths, /FirstChar, /LastChar, and /FontDescriptor to provide
//! glyph widths, ascent, and descent for character bounding box calculation.

use crate::cff;
use crate::error::BackendError;
use crate::standard_fonts;
use crate::truetype;

/// Default ascent when not specified (750/1000 of text space).
const DEFAULT_ASCENT: f64 = 750.0;

/// Default descent when not specified (-250/1000 of text space).
const DEFAULT_DESCENT: f64 = -250.0;

/// Default character width when not specified (600/1000 of text space).
const DEFAULT_WIDTH: f64 = 600.0;

/// Font metrics extracted from a PDF font dictionary.
///
/// Stores glyph widths and font descriptor information (ascent, descent,
/// bounding box) needed to calculate character bounding boxes.
///
/// Width values are in glyph space units (1/1000 of text space).
#[derive(Debug, Clone)]
pub struct FontMetrics {
    /// Glyph widths indexed by (char_code - first_char).
    widths: Vec<f64>,
    /// First character code in the widths array.
    first_char: u32,
    /// Last character code in the widths array.
    last_char: u32,
    /// Default width for characters outside [first_char, last_char].
    missing_width: f64,
    /// Font ascent in glyph space units (positive, above baseline).
    ascent: f64,
    /// Font descent in glyph space units (negative, below baseline).
    descent: f64,
    /// Font bounding box [llx, lly, urx, ury] in glyph space units.
    font_bbox: Option<[f64; 4]>,
}

impl FontMetrics {
    /// Create FontMetrics from parsed PDF font dictionary values.
    pub fn new(
        widths: Vec<f64>,
        first_char: u32,
        last_char: u32,
        missing_width: f64,
        ascent: f64,
        descent: f64,
        font_bbox: Option<[f64; 4]>,
    ) -> Self {
        Self {
            widths,
            first_char,
            last_char,
            missing_width,
            ascent,
            descent,
            font_bbox,
        }
    }

    /// Create default FontMetrics for when font info is unavailable.
    pub fn default_metrics() -> Self {
        Self {
            widths: Vec::new(),
            first_char: 0,
            last_char: 0,
            missing_width: DEFAULT_WIDTH,
            ascent: DEFAULT_ASCENT,
            descent: DEFAULT_DESCENT,
            font_bbox: None,
        }
    }

    /// Get the width for a character code in glyph space (1/1000 of text space).
    pub fn get_width(&self, char_code: u32) -> f64 {
        if char_code >= self.first_char && char_code <= self.last_char {
            let index = (char_code - self.first_char) as usize;
            if index < self.widths.len() {
                return self.widths[index];
            }
        }
        self.missing_width
    }

    /// Font ascent in glyph space units (positive, above baseline).
    pub fn ascent(&self) -> f64 {
        self.ascent
    }

    /// Font descent in glyph space units (negative, below baseline).
    pub fn descent(&self) -> f64 {
        self.descent
    }

    /// Font bounding box [llx, lly, urx, ury] in glyph space units.
    pub fn font_bbox(&self) -> Option<[f64; 4]> {
        self.font_bbox
    }

    /// Missing width used for characters outside the widths range.
    pub fn missing_width(&self) -> f64 {
        self.missing_width
    }

    /// First character code in the widths array.
    pub fn first_char(&self) -> u32 {
        self.first_char
    }

    /// Last character code in the widths array.
    pub fn last_char(&self) -> u32 {
        self.last_char
    }
}

/// Extract [`FontMetrics`] from a lopdf font dictionary.
///
/// Reads /Widths, /FirstChar, /LastChar from the font dictionary,
/// and /Ascent, /Descent, /FontBBox, /MissingWidth from the /FontDescriptor.
///
/// Returns default metrics if essential fields are missing.
pub fn extract_font_metrics(
    doc: &lopdf::Document,
    font_dict: &lopdf::Dictionary,
) -> Result<FontMetrics, BackendError> {
    // Parse /FirstChar and /LastChar
    let first_char = font_dict
        .get(b"FirstChar")
        .ok()
        .and_then(object_to_f64_opt)
        .map(|v| v as u32)
        .unwrap_or(0);

    let last_char = font_dict
        .get(b"LastChar")
        .ok()
        .and_then(object_to_f64_opt)
        .map(|v| v as u32)
        .unwrap_or(0);

    // Parse /Widths array
    let widths = match font_dict.get(b"Widths") {
        Ok(obj) => {
            let obj = resolve_object(doc, obj);
            match obj.as_array() {
                Ok(arr) => arr
                    .iter()
                    .map(|o| {
                        let o = resolve_object(doc, o);
                        object_to_f64_opt(o).unwrap_or(0.0)
                    })
                    .collect(),
                Err(_) => Vec::new(),
            }
        }
        Err(_) => Vec::new(),
    };

    // Parse /FontDescriptor
    let desc_info = parse_font_descriptor(doc, font_dict)?;

    // Standard font fallback: when /Widths is absent, try standard Type1 font widths
    if widths.is_empty() {
        if let Some(std_font) = lookup_standard_font(font_dict) {
            let std_widths: Vec<f64> = std_font.widths.iter().map(|&w| f64::from(w)).collect();
            let font_bbox = desc_info
                .font_bbox
                .or(Some(std_font.font_bbox.map(f64::from)));
            return Ok(FontMetrics::new(
                std_widths,
                0,
                255,
                desc_info.missing_width,
                desc_info.ascent,
                desc_info.descent,
                font_bbox,
            ));
        }
    }

    // TrueType fallback: when /Widths is absent, try parsing /FontFile2 hmtx table
    if widths.is_empty() {
        if let Some(tt_widths) = try_extract_truetype_widths(doc, font_dict) {
            let num_glyphs = tt_widths.len();
            return Ok(FontMetrics::new(
                tt_widths,
                0,
                if num_glyphs > 0 {
                    (num_glyphs - 1) as u32
                } else {
                    0
                },
                desc_info.missing_width,
                desc_info.ascent,
                desc_info.descent,
                desc_info.font_bbox,
            ));
        }
    }

    // CFF/Type1C fallback: when /Widths is absent, try parsing /FontFile3 CFF data
    if widths.is_empty() {
        if let Some(cff_widths) = try_extract_cff_widths(doc, font_dict) {
            let num_glyphs = cff_widths.len();
            return Ok(FontMetrics::new(
                cff_widths,
                0,
                if num_glyphs > 0 {
                    (num_glyphs - 1) as u32
                } else {
                    0
                },
                desc_info.missing_width,
                desc_info.ascent,
                desc_info.descent,
                desc_info.font_bbox,
            ));
        }
    }

    Ok(FontMetrics::new(
        widths,
        first_char,
        last_char,
        desc_info.missing_width,
        desc_info.ascent,
        desc_info.descent,
        desc_info.font_bbox,
    ))
}

/// Try to extract glyph widths from a TrueType /FontFile2 embedded font stream.
///
/// Accesses /FontDescriptor → /FontFile2, decompresses the stream, and parses
/// the TrueType hmtx table for per-glyph advance widths scaled to 1000 upem.
///
/// Returns `None` if /FontFile2 is absent or parsing fails.
fn try_extract_truetype_widths(
    doc: &lopdf::Document,
    font_dict: &lopdf::Dictionary,
) -> Option<Vec<f64>> {
    let desc_obj = font_dict.get(b"FontDescriptor").ok()?;
    let desc_obj = resolve_object(doc, desc_obj);
    let desc = desc_obj.as_dict().ok()?;

    let font_file_obj = desc.get(b"FontFile2").ok()?;
    let font_file_obj = resolve_object(doc, font_file_obj);
    let stream = font_file_obj.as_stream().ok()?;

    let data = if stream.dict.get(b"Filter").is_ok() {
        stream.decompressed_content().unwrap_or_default()
    } else {
        stream.content.clone()
    };

    let tt_widths = truetype::parse_truetype_widths(&data)?;

    // Build width vector indexed by glyph ID (= char code for simple fonts)
    let num_glyphs = tt_widths.num_glyphs();
    let mut widths = Vec::with_capacity(num_glyphs);
    for gid in 0..num_glyphs {
        let w = tt_widths.get_width(gid as u16).unwrap_or(0.0);
        widths.push(w);
    }

    Some(widths)
}

/// Try to extract glyph widths from a CFF /FontFile3 embedded font stream.
///
/// Accesses /FontDescriptor → /FontFile3, decompresses the stream, and parses
/// the CFF data for per-glyph advance widths.
///
/// Returns `None` if /FontFile3 is absent, not Type1C, or parsing fails.
fn try_extract_cff_widths(
    doc: &lopdf::Document,
    font_dict: &lopdf::Dictionary,
) -> Option<Vec<f64>> {
    let desc_obj = font_dict.get(b"FontDescriptor").ok()?;
    let desc_obj = resolve_object(doc, desc_obj);
    let desc = desc_obj.as_dict().ok()?;

    let font_file_obj = desc.get(b"FontFile3").ok()?;
    let font_file_obj = resolve_object(doc, font_file_obj);
    let stream = font_file_obj.as_stream().ok()?;

    // Verify subtype is Type1C (CFF)
    let subtype = stream
        .dict
        .get(b"Subtype")
        .ok()
        .and_then(|o| o.as_name().ok())
        .unwrap_or(b"");
    if subtype != b"Type1C" && subtype != b"CIDFontType0C" {
        return None;
    }

    let data = if stream.dict.get(b"Filter").is_ok() {
        stream.decompressed_content().unwrap_or_default()
    } else {
        stream.content.clone()
    };

    let cff_widths = cff::parse_cff_widths(&data)?;

    let num_glyphs = cff_widths.num_glyphs();
    let mut widths = Vec::with_capacity(num_glyphs);
    for gid in 0..num_glyphs {
        let w = cff_widths.get_width(gid as u16).unwrap_or(0.0);
        widths.push(w);
    }

    Some(widths)
}

/// Look up standard font data from a font dictionary's /BaseFont entry.
///
/// Handles subset-prefixed names (e.g., "ABCDEF+Helvetica").
fn lookup_standard_font(
    font_dict: &lopdf::Dictionary,
) -> Option<&'static standard_fonts::StandardFontData> {
    let base_font = font_dict
        .get(b"BaseFont")
        .ok()
        .and_then(|obj| obj.as_name().ok())
        .map(|name| std::str::from_utf8(name).unwrap_or(""))?;
    let stripped = crate::cid_font::strip_subset_prefix(base_font);
    standard_fonts::lookup(stripped)
}

/// Parsed font descriptor values.
struct FontDescriptorInfo {
    ascent: f64,
    descent: f64,
    font_bbox: Option<[f64; 4]>,
    missing_width: f64,
}

/// Parse /FontDescriptor dictionary for ascent, descent, bbox, and missing width.
fn parse_font_descriptor(
    doc: &lopdf::Document,
    font_dict: &lopdf::Dictionary,
) -> Result<FontDescriptorInfo, BackendError> {
    let descriptor_dict = font_dict
        .get(b"FontDescriptor")
        .ok()
        .map(|obj| resolve_object(doc, obj))
        .and_then(|obj| obj.as_dict().ok());

    let Some(desc) = descriptor_dict else {
        return Ok(FontDescriptorInfo {
            ascent: DEFAULT_ASCENT,
            descent: DEFAULT_DESCENT,
            font_bbox: None,
            missing_width: DEFAULT_WIDTH,
        });
    };

    let ascent = desc
        .get(b"Ascent")
        .ok()
        .and_then(object_to_f64_opt)
        .unwrap_or(DEFAULT_ASCENT);

    // PDF spec §9.8: Descent "shall be a negative number". Some malformed PDFs
    // (e.g., annotations.pdf BAAAAA+Arial-BoldMT) store a positive value.
    // Normalize to negative to match pdfminer/pdfplumber-py behavior.
    let raw_descent = desc
        .get(b"Descent")
        .ok()
        .and_then(object_to_f64_opt)
        .unwrap_or(DEFAULT_DESCENT);
    let descent = if raw_descent > 0.0 {
        -raw_descent
    } else {
        raw_descent
    };

    let missing_width = desc
        .get(b"MissingWidth")
        .ok()
        .and_then(object_to_f64_opt)
        .unwrap_or(DEFAULT_WIDTH);

    let font_bbox = desc
        .get(b"FontBBox")
        .ok()
        .and_then(|o| {
            let o = resolve_object(doc, o);
            o.as_array().ok()
        })
        .and_then(|arr| {
            if arr.len() == 4 {
                let vals: Vec<f64> = arr.iter().filter_map(object_to_f64_opt).collect();
                if vals.len() == 4 {
                    Some([vals[0], vals[1], vals[2], vals[3]])
                } else {
                    None
                }
            } else {
                None
            }
        });

    Ok(FontDescriptorInfo {
        ascent,
        descent,
        font_bbox,
        missing_width,
    })
}

/// Resolve an indirect reference to the actual object.
fn resolve_object<'a>(doc: &'a lopdf::Document, obj: &'a lopdf::Object) -> &'a lopdf::Object {
    match obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).unwrap_or(obj),
        _ => obj,
    }
}

/// Convert a lopdf object to f64, returning None if not a number.
fn object_to_f64_opt(obj: &lopdf::Object) -> Option<f64> {
    match obj {
        lopdf::Object::Integer(i) => Some(*i as f64),
        lopdf::Object::Real(f) => Some(*f as f64),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
