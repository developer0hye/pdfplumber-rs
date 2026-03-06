//! Font loading and encoding resolution for the content stream interpreter.
//!
//! Handles Type1, TrueType, Type0/CID, and standard fonts. Resolves ToUnicode
//! CMaps and Differences arrays for accurate character mapping.

use super::CachedFont;
use crate::cid_font::{
    CidFontMetrics, extract_cid_font_metrics, get_descendant_font, get_type0_encoding,
    is_type0_font, parse_predefined_cmap_name, strip_subset_prefix,
};
use crate::cjk_encoding;
use crate::cmap::CMap;
use crate::font_metrics::{FontMetrics, extract_font_metrics};
use crate::handler::ContentHandler;
use crate::lopdf_backend::resolve_ref;
use pdfplumber_core::{
    ExtractOptions, ExtractWarning, ExtractWarningCode, FontEncoding, StandardEncoding,
    glyph_name_to_char,
};
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
pub(super) fn load_font_if_needed(
    doc: &lopdf::Document,
    resources: &lopdf::Dictionary,
    font_name: &str,
    cache: &mut HashMap<String, CachedFont>,
    handler: &mut dyn ContentHandler,
    options: &ExtractOptions,
    op_index: usize,
) {
    if cache.contains_key(font_name) {
        return;
    }

    // Look up /Resources/Font/<font_name>
    let font_dict = (|| -> Option<&lopdf::Dictionary> {
        let fonts_obj = resources.get(b"Font").ok()?;
        let fonts_obj = resolve_ref(doc, fonts_obj);
        let fonts_dict = fonts_obj.as_dict().ok()?;
        let font_obj = fonts_dict.get(font_name.as_bytes()).ok()?;
        let font_obj = resolve_ref(doc, font_obj);
        font_obj.as_dict().ok()
    })();

    let (
        metrics,
        cmap,
        base_name,
        cid_metrics,
        is_cid_font,
        writing_mode,
        encoding,
        cjk_enc,
        is_identity_enc,
    ) = if let Some(fd) = font_dict {
        if is_type0_font(fd) {
            // Type0 (composite/CID) font
            let (cid_met, wm) = load_cid_font(doc, fd);

            // Detect CJK encoding and Identity-H/V from predefined CMap name
            let enc_name = get_type0_encoding(fd);
            let is_identity_enc = enc_name
                .as_deref()
                .is_some_and(|n| n == "Identity-H" || n == "Identity-V");
            let cjk_enc = enc_name.and_then(|enc_name| cjk_encoding::encoding_for_cmap(&enc_name));
            let metrics = if let Some(ref cm) = cid_met {
                // Create a FontMetrics from CID font data for backward compat
                FontMetrics::new(
                    Vec::new(),
                    0,
                    0,
                    cm.default_width(),
                    cm.ascent(),
                    cm.descent(),
                    cm.font_bbox(),
                )
            } else {
                if options.collect_warnings {
                    handler.on_warning(
                        ExtractWarning::with_operator_context(
                            "CID font metrics not available, using defaults",
                            op_index,
                            font_name,
                        )
                        .set_code(ExtractWarningCode::MissingFont),
                    );
                }
                FontMetrics::default_metrics()
            };

            // Extract ToUnicode CMap if present
            let cmap = extract_tounicode_cmap(doc, fd);

            let raw_base_name_owned;
            let raw_base_name =
                if let Some(n) = fd.get(b"BaseFont").ok().and_then(|o| o.as_name().ok()) {
                    raw_base_name_owned = String::from_utf8_lossy(n).into_owned();
                    raw_base_name_owned.as_str()
                } else {
                    font_name
                };
            let base_name = strip_subset_prefix(raw_base_name).to_string();

            (
                metrics,
                cmap,
                base_name,
                cid_met,
                true,
                wm,
                None,
                cjk_enc,
                is_identity_enc,
            )
        } else {
            // Simple font
            let metrics = match extract_font_metrics(doc, fd) {
                Ok(m) => m,
                Err(_) => {
                    if options.collect_warnings {
                        handler.on_warning(
                            ExtractWarning::with_operator_context(
                                "failed to extract font metrics, using defaults",
                                op_index,
                                font_name,
                            )
                            .set_code(ExtractWarningCode::MissingFont),
                        );
                    }
                    FontMetrics::default_metrics()
                }
            };
            let cmap = extract_tounicode_cmap(doc, fd);
            let encoding = extract_font_encoding(doc, fd);
            let raw_base_name_owned;
            let raw_base_name =
                if let Some(n) = fd.get(b"BaseFont").ok().and_then(|o| o.as_name().ok()) {
                    raw_base_name_owned = String::from_utf8_lossy(n).into_owned();
                    raw_base_name_owned.as_str()
                } else {
                    font_name
                };
            let base_name = strip_subset_prefix(raw_base_name).to_string();

            // US-182-1: When a standard Type1 font has no explicit /Encoding,
            // apply StandardEncoding as the implicit base encoding per PDF spec.
            // Symbol and ZapfDingbats have their own built-in encodings.
            let encoding = encoding.or_else(|| {
                if is_standard_latin_font(&base_name) {
                    Some(FontEncoding::from_standard(StandardEncoding::Standard))
                } else {
                    None
                }
            });

            // US-182-2: When a standard font has no /Widths array, the fallback
            // widths from standard_fonts.rs are indexed by WinAnsiEncoding. If the
            // active encoding differs (e.g., StandardEncoding), remap widths so each
            // code position gets the correct glyph width for the active encoding.
            let metrics = if fd.get(b"Widths").is_err() {
                if let Some(ref enc) = encoding {
                    if let Some(remapped) =
                        crate::standard_fonts::build_remapped_widths(&base_name, |code| {
                            enc.decode(code)
                        })
                    {
                        FontMetrics::new(
                            remapped,
                            0,
                            255,
                            metrics.missing_width(),
                            metrics.ascent(),
                            metrics.descent(),
                            metrics.font_bbox(),
                        )
                    } else {
                        metrics
                    }
                } else {
                    metrics
                }
            } else {
                metrics
            };

            (
                metrics, cmap, base_name, None, false, 0, encoding, None, false,
            )
        }
    } else {
        // Font not found in page resources — use defaults
        if options.collect_warnings {
            handler.on_warning(
                ExtractWarning::with_operator_context(
                    "font not found in page resources, using defaults",
                    op_index,
                    font_name,
                )
                .set_code(ExtractWarningCode::MissingFont),
            );
        }
        (
            FontMetrics::default_metrics(),
            None,
            font_name.to_string(),
            None,
            false,
            0,
            None,
            None,
            false,
        )
    };

    cache.insert(
        font_name.to_string(),
        CachedFont {
            metrics,
            cmap,
            base_name,
            cid_metrics,
            is_cid_font,
            writing_mode,
            encoding,
            cjk_encoding: cjk_enc,
            is_identity_encoding: is_identity_enc,
        },
    );
}

/// Extract ToUnicode CMap from a font dictionary.
fn extract_tounicode_cmap(doc: &lopdf::Document, fd: &lopdf::Dictionary) -> Option<CMap> {
    let tounicode_obj = fd.get(b"ToUnicode").ok()?;
    let tounicode_obj = resolve_ref(doc, tounicode_obj);
    let stream = tounicode_obj.as_stream().ok()?;
    let data = super::xobjects::decode_stream(stream).ok()?;
    CMap::parse(&data).ok()
}

/// Extract font encoding from a simple font dictionary's /Encoding entry.
fn extract_font_encoding(doc: &lopdf::Document, fd: &lopdf::Dictionary) -> Option<FontEncoding> {
    let encoding_obj = fd.get(b"Encoding").ok()?;
    let encoding_obj = resolve_ref(doc, encoding_obj);

    // Case 1: /Encoding is a name (e.g., /WinAnsiEncoding)
    if let Ok(name) = encoding_obj.as_name() {
        let std_enc = match name {
            b"WinAnsiEncoding" => Some(StandardEncoding::WinAnsi),
            b"MacRomanEncoding" => Some(StandardEncoding::MacRoman),
            b"MacExpertEncoding" => Some(StandardEncoding::MacExpert),
            b"StandardEncoding" => Some(StandardEncoding::Standard),
            _ => None,
        };
        return std_enc.map(FontEncoding::from_standard);
    }

    // Case 2: /Encoding is a dictionary with /BaseEncoding and/or /Differences
    if let Ok(enc_dict) = encoding_obj.as_dict() {
        let base = enc_dict
            .get(b"BaseEncoding")
            .ok()
            .and_then(|o| o.as_name().ok())
            .and_then(|name| match name {
                b"WinAnsiEncoding" => Some(StandardEncoding::WinAnsi),
                b"MacRomanEncoding" => Some(StandardEncoding::MacRoman),
                b"MacExpertEncoding" => Some(StandardEncoding::MacExpert),
                b"StandardEncoding" => Some(StandardEncoding::Standard),
                _ => None,
            })
            .unwrap_or(StandardEncoding::Standard);

        let mut enc = FontEncoding::from_standard(base);

        // Apply /Differences array
        if let Ok(diff_obj) = enc_dict.get(b"Differences") {
            let diff_obj = resolve_ref(doc, diff_obj);
            if let Ok(diff_arr) = diff_obj.as_array() {
                let differences = parse_differences_array(diff_arr);
                enc.apply_differences(&differences);
            }
        }

        return Some(enc);
    }

    None
}

/// Check if a font name is one of the standard 14 Latin fonts (all except
/// Symbol and ZapfDingbats, which have their own built-in encodings).
fn is_standard_latin_font(base_name: &str) -> bool {
    matches!(
        base_name,
        "Courier"
            | "Courier-Bold"
            | "Courier-Oblique"
            | "Courier-BoldOblique"
            | "Helvetica"
            | "Helvetica-Bold"
            | "Helvetica-Oblique"
            | "Helvetica-BoldOblique"
            | "Times-Roman"
            | "Times-Bold"
            | "Times-Italic"
            | "Times-BoldItalic"
    )
}

/// Parse a PDF /Differences array into (code, char) pairs.
///
/// Format: `[code1 /name1 /name2 ... codeN /nameN ...]`
/// Each integer starts a run; subsequent names are assigned consecutive codes.
fn parse_differences_array(arr: &[lopdf::Object]) -> Vec<(u8, char)> {
    let mut result = Vec::new();
    let mut current_code: Option<u8> = None;

    for obj in arr {
        match obj {
            lopdf::Object::Integer(i) => {
                current_code = Some(*i as u8);
            }
            lopdf::Object::Name(name_bytes) => {
                if let Some(code) = current_code {
                    let name = String::from_utf8_lossy(name_bytes);
                    if let Some(ch) = glyph_name_to_char(&name) {
                        result.push((code, ch));
                    }
                    current_code = Some(code.wrapping_add(1));
                }
            }
            _ => {}
        }
    }

    result
}

/// Load CID font information from a Type0 font dictionary.
fn load_cid_font(
    doc: &lopdf::Document,
    type0_dict: &lopdf::Dictionary,
) -> (Option<CidFontMetrics>, u8) {
    // Determine writing mode from encoding name
    let writing_mode = get_type0_encoding(type0_dict)
        .and_then(|enc| parse_predefined_cmap_name(&enc))
        .map(|info| info.writing_mode)
        .unwrap_or(0);

    // Get descendant CIDFont dictionary
    let cid_metrics = get_descendant_font(doc, type0_dict)
        .and_then(|desc| extract_cid_font_metrics(doc, desc).ok());

    (cid_metrics, writing_mode)
}

// --- Text rendering ---

/// Build a width lookup function for a cached font.
/// For CID fonts, uses CidFontMetrics; for simple fonts, uses FontMetrics.
pub(super) fn get_width_fn(cached: Option<&CachedFont>) -> Box<dyn Fn(u32) -> f64 + '_> {
    match cached {
        Some(cf) if cf.is_cid_font => {
            if let Some(ref cid_met) = cf.cid_metrics {
                Box::new(move |code: u32| cid_met.get_width(code))
            } else {
                Box::new(move |code: u32| cf.metrics.get_width(code))
            }
        }
        Some(cf) => Box::new(move |code: u32| cf.metrics.get_width(code)),
        None => {
            let default_metrics = FontMetrics::default_metrics();
            Box::new(move |code: u32| default_metrics.get_width(code))
        }
    }
}

/// Build a vertical advance lookup function for a cached CID font.
#[allow(dead_code)]
pub(super) fn get_vertical_advance_fn(cached: Option<&CachedFont>) -> Box<dyn Fn(u32) -> f64 + '_> {
    match cached {
        Some(cf) if cf.is_cid_font => {
            if let Some(ref cid_met) = cf.cid_metrics {
                Box::new(move |code: u32| cid_met.get_vertical_w1(code))
            } else {
                Box::new(|_: u32| -1000.0)
            }
        }
        _ => Box::new(|_: u32| -1000.0),
    }
}
