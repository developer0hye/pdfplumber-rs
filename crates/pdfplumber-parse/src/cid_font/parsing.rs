//! CID font parsing helpers — internal functions for extracting metrics from lopdf.

use super::{
    CidFontMetrics, CidSystemInfo, CidToGidMap, DEFAULT_CID_ASCENT, DEFAULT_CID_DESCENT,
    DEFAULT_DW2_VY, VerticalMetric,
};
use crate::truetype;
use std::collections::HashMap;

/// Parse the /CIDToGIDMap entry from a CIDFont dictionary.
pub(super) fn parse_cid_to_gid_map(doc: &lopdf::Document, dict: &lopdf::Dictionary) -> CidToGidMap {
    match dict.get(b"CIDToGIDMap") {
        Ok(obj) => {
            let obj = resolve_object(doc, obj);
            if let Ok(name) = obj.as_name() {
                if name == b"Identity" {
                    return CidToGidMap::Identity;
                }
            }
            if let Ok(stream) = obj.as_stream() {
                let data = if stream.dict.get(b"Filter").is_ok() {
                    stream.decompressed_content().unwrap_or_default()
                } else {
                    stream.content.clone()
                };
                return CidToGidMap::from_stream(&data);
            }
            CidToGidMap::Identity
        }
        Err(_) => CidToGidMap::Identity,
    }
}

/// Parse /CIDSystemInfo from a CIDFont dictionary.
pub(super) fn parse_cid_system_info(
    doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
) -> Option<CidSystemInfo> {
    let info_obj = dict.get(b"CIDSystemInfo").ok()?;
    let info_obj = resolve_object(doc, info_obj);
    let info_dict = info_obj.as_dict().ok()?;

    let registry = info_dict
        .get(b"Registry")
        .ok()
        .and_then(|o| match o {
            lopdf::Object::String(s, _) => String::from_utf8(s.clone()).ok(),
            _ => None,
        })
        .unwrap_or_default();

    let ordering = info_dict
        .get(b"Ordering")
        .ok()
        .and_then(|o| match o {
            lopdf::Object::String(s, _) => String::from_utf8(s.clone()).ok(),
            _ => None,
        })
        .unwrap_or_default();

    let supplement = info_dict
        .get(b"Supplement")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0);

    Some(CidSystemInfo {
        registry,
        ordering,
        supplement,
    })
}

/// Parse /FontDescriptor from a CIDFont dictionary for ascent, descent, bbox.
pub(super) fn parse_cid_font_descriptor(
    doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
) -> (f64, f64, Option<[f64; 4]>) {
    let desc = match dict
        .get(b"FontDescriptor")
        .ok()
        .map(|o| resolve_object(doc, o))
        .and_then(|o| o.as_dict().ok())
    {
        Some(d) => d,
        None => return (DEFAULT_CID_ASCENT, DEFAULT_CID_DESCENT, None),
    };

    let ascent = desc
        .get(b"Ascent")
        .ok()
        .and_then(object_to_f64)
        .unwrap_or(DEFAULT_CID_ASCENT);

    // PDF spec: Descent should be negative (distance below baseline).
    // Some PDF generators (e.g., Meiryo, MSMincho) incorrectly write positive
    // values. Negate positive Descent to match expected behavior.
    let descent = desc
        .get(b"Descent")
        .ok()
        .and_then(object_to_f64)
        .map(|d| if d > 0.0 { -d } else { d })
        .unwrap_or(DEFAULT_CID_DESCENT);

    let font_bbox = desc
        .get(b"FontBBox")
        .ok()
        .and_then(|o| {
            let o = resolve_object(doc, o);
            o.as_array().ok()
        })
        .and_then(|arr| {
            if arr.len() == 4 {
                let vals: Vec<f64> = arr.iter().filter_map(object_to_f64).collect();
                if vals.len() == 4 {
                    Some([vals[0], vals[1], vals[2], vals[3]])
                } else {
                    None
                }
            } else {
                None
            }
        });

    (ascent, descent, font_bbox)
}

/// Try to extract vertical metrics from a TrueType vmtx table embedded in FontFile2.
///
/// For CIDFontType2 fonts, when /W2 is absent, falls back to the vmtx table
/// for per-glyph vertical advance heights. Maps CIDs to GIDs using the font's
/// CIDToGIDMap, then converts vmtx advance heights to VerticalMetric structs.
///
/// Returns `None` if FontFile2 is absent or vmtx table is not present.
pub(super) fn try_extract_vmtx_vertical_metrics(
    doc: &lopdf::Document,
    cid_font_dict: &lopdf::Dictionary,
    metrics: &CidFontMetrics,
) -> Option<HashMap<u32, VerticalMetric>> {
    let desc_obj = cid_font_dict.get(b"FontDescriptor").ok()?;
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

    let vmetrics = truetype::parse_truetype_vertical_metrics(&data)?;

    // Build vertical metric map: CID → VerticalMetric
    // For each glyph with a vertical advance, create an entry using the CIDToGIDMap
    let mut vertical_widths = HashMap::new();
    let num_glyphs = vmetrics.num_glyphs();

    for gid in 0..num_glyphs {
        if let Some(advance_height) = vmetrics.get_height(gid as u16) {
            // Find CIDs that map to this GID
            // For Identity mapping: CID == GID
            // For explicit mappings, we build the reverse lookup
            let cid = gid as u32; // Default: assume identity
            let mapped_gid = metrics.map_cid_to_gid(cid);
            if mapped_gid == gid as u32 {
                let hw = metrics.get_width(cid);
                vertical_widths.insert(
                    cid,
                    VerticalMetric {
                        w1y: -advance_height, // negative = downward advance
                        vx: hw / 2.0,         // horizontal origin at half-width
                        vy: DEFAULT_DW2_VY,   // default vertical origin
                    },
                );
            }
        }
    }

    if vertical_widths.is_empty() {
        None
    } else {
        Some(vertical_widths)
    }
}

/// Resolve an indirect reference to the actual object.
pub(super) fn resolve_object<'a>(
    doc: &'a lopdf::Document,
    obj: &'a lopdf::Object,
) -> &'a lopdf::Object {
    match obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).unwrap_or(obj),
        _ => obj,
    }
}

/// Convert a lopdf object to f64.
pub(super) fn object_to_f64(obj: &lopdf::Object) -> Option<f64> {
    match obj {
        lopdf::Object::Integer(i) => Some(*i as f64),
        lopdf::Object::Real(f) => Some(*f as f64),
        _ => None,
    }
}

/// Convert a lopdf object to u32.
pub(super) fn object_to_u32(obj: &lopdf::Object) -> Option<u32> {
    match obj {
        lopdf::Object::Integer(i) => Some(*i as u32),
        lopdf::Object::Real(f) => Some(*f as u32),
        _ => None,
    }
}

/// Information about a predefined CMap encoding.
#[derive(Debug, Clone, PartialEq)]
pub struct PredefinedCMapInfo {
    /// The full CMap name (e.g., "Adobe-Japan1-6").
    pub name: String,
    /// Registry (e.g., "Adobe").
    pub registry: String,
    /// Ordering (e.g., "Japan1").
    pub ordering: String,
    /// Writing mode: 0 = horizontal, 1 = vertical.
    pub writing_mode: u8,
    /// Whether this is an Identity CMap.
    pub is_identity: bool,
}

/// Parse a predefined CMap name and extract its information.
///
/// Recognizes standard Adobe CJK CMap names:
/// - `Identity-H` / `Identity-V`
/// - `Adobe-Japan1-*` (with `-H` or `-V` suffix for writing mode)
/// - `Adobe-GB1-*`
/// - `Adobe-CNS1-*`
/// - `Adobe-Korea1-*`
/// - Standard encoding names like `UniJIS-UTF16-H`, `UniGB-UTF16-H`, etc.
pub fn parse_predefined_cmap_name(name: &str) -> Option<PredefinedCMapInfo> {
    // Identity CMaps
    if name == "Identity-H" {
        return Some(PredefinedCMapInfo {
            name: name.to_string(),
            registry: "Adobe".to_string(),
            ordering: "Identity".to_string(),
            writing_mode: 0,
            is_identity: true,
        });
    }
    if name == "Identity-V" {
        return Some(PredefinedCMapInfo {
            name: name.to_string(),
            registry: "Adobe".to_string(),
            ordering: "Identity".to_string(),
            writing_mode: 1,
            is_identity: true,
        });
    }

    // Adobe CJK CMap names (e.g., "Adobe-Japan1-6")
    if let Some(rest) = name.strip_prefix("Adobe-") {
        let (ordering, supplement) = if let Some(r) = rest.strip_prefix("Japan1-") {
            ("Japan1".to_string(), r)
        } else if let Some(r) = rest.strip_prefix("GB1-") {
            ("GB1".to_string(), r)
        } else if let Some(r) = rest.strip_prefix("CNS1-") {
            ("CNS1".to_string(), r)
        } else if let Some(r) = rest.strip_prefix("Korea1-") {
            ("Korea1".to_string(), r)
        } else {
            return None;
        };

        // Supplement should be a number
        if supplement.parse::<i32>().is_ok() {
            return Some(PredefinedCMapInfo {
                name: name.to_string(),
                registry: "Adobe".to_string(),
                ordering,
                writing_mode: 0,
                is_identity: false,
            });
        }
    }

    // Japanese: Raw JIS X 0208 CMaps (H = horizontal, V = vertical)
    if name == "H" {
        return Some(PredefinedCMapInfo {
            name: name.to_string(),
            registry: "Adobe".to_string(),
            ordering: "Japan1".to_string(),
            writing_mode: 0,
            is_identity: false,
        });
    }
    if name == "V" {
        return Some(PredefinedCMapInfo {
            name: name.to_string(),
            registry: "Adobe".to_string(),
            ordering: "Japan1".to_string(),
            writing_mode: 1,
            is_identity: false,
        });
    }

    // Standard CJK encoding CMaps with -H/-V suffix
    let (base, writing_mode) = if let Some(b) = name.strip_suffix("-H") {
        (b, 0u8)
    } else if let Some(b) = name.strip_suffix("-V") {
        (b, 1u8)
    } else {
        return None;
    };

    // Recognize known CMap base names by their ordering
    let ordering = if base.contains("JIS")
        || base.contains("Japan")
        || base.contains("EUC-JP")
        || base == "EUC"
        || base == "78-RKSJ"
        || base == "83pv-RKSJ"
        || base == "90pv-RKSJ"
        || base == "90ms-RKSJ"
        || base == "Hankaku"
        || base == "Hiragana"
        || base == "Katakana"
        || base == "Roman"
        || base == "WP-Symbol"
        || base == "Add-RKSJ"
        || base == "Ext-RKSJ"
    {
        "Japan1"
    } else if base.contains("GB")
        || base.contains("GBK")
        || base.contains("GBpc")
        || base.contains("GBT")
        || base == "UniCNS-UCS2"
    {
        // Note: UniCNS is actually CNS1, but GB-prefixed are GB1
        if base.starts_with("UniCNS") {
            "CNS1"
        } else {
            "GB1"
        }
    } else if base.contains("CNS") || base.contains("ETen") || base.contains("HKscs") {
        "CNS1"
    } else if base.contains("KSC") || base.contains("KSCms") || base.contains("UniKS") {
        "Korea1"
    } else {
        return None;
    };

    Some(PredefinedCMapInfo {
        name: name.to_string(),
        registry: "Adobe".to_string(),
        ordering: ordering.to_string(),
        writing_mode,
        is_identity: false,
    })
}

/// Detect whether a font dictionary represents a Type0 (composite/CID) font.
pub fn is_type0_font(font_dict: &lopdf::Dictionary) -> bool {
    font_dict
        .get(b"Subtype")
        .ok()
        .and_then(|o| o.as_name().ok())
        .is_some_and(|s| s == b"Type0")
}

/// Extract the descendant CIDFont dictionary from a Type0 font.
pub fn get_descendant_font<'a>(
    doc: &'a lopdf::Document,
    type0_dict: &'a lopdf::Dictionary,
) -> Option<&'a lopdf::Dictionary> {
    let descendants = type0_dict.get(b"DescendantFonts").ok()?;
    let descendants = resolve_object(doc, descendants);
    let arr = descendants.as_array().ok()?;
    let first = arr.first()?;
    let first = resolve_object(doc, first);
    first.as_dict().ok()
}

/// Get the encoding name from a Type0 font dictionary.
pub fn get_type0_encoding(font_dict: &lopdf::Dictionary) -> Option<String> {
    let encoding = font_dict.get(b"Encoding").ok()?;
    encoding
        .as_name()
        .ok()
        .map(|s| String::from_utf8_lossy(s).into_owned())
}

/// Check if a font name has a subset prefix.
///
/// PDF subset fonts have a 6-uppercase-letter prefix followed by '+' and the
/// real font name, e.g. `ABCDEF+ArialMT`. Returns `true` if the name matches
/// this pattern.
pub fn is_subset_font(font_name: &str) -> bool {
    if font_name.len() < 8 {
        return false;
    }
    let bytes = font_name.as_bytes();
    // First 6 chars must be uppercase ASCII letters
    for &b in &bytes[..6] {
        if !b.is_ascii_uppercase() {
            return false;
        }
    }
    // 7th char must be '+'
    bytes[6] == b'+'
}

/// Strip the subset prefix from a font name.
///
/// If the font name has the pattern `ABCDEF+RealName`, returns `RealName`.
/// Otherwise returns the original name unchanged.
pub fn strip_subset_prefix(font_name: &str) -> &str {
    if is_subset_font(font_name) {
        &font_name[7..]
    } else {
        font_name
    }
}
