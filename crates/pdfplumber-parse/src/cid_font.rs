//! CID font support for CJK text extraction.
//!
//! Handles Type0 (composite) fonts with CIDFontType0 and CIDFontType2
//! descendant fonts. Provides CID-to-GID mapping, /W (width) array parsing,
//! and /DW (default width) handling for CID fonts.

use std::collections::HashMap;

use crate::error::BackendError;

/// Default CID font width when /DW is not specified (1000/1000 of text space = full em width).
const DEFAULT_CID_WIDTH: f64 = 1000.0;

/// Default ascent for CID fonts when not specified.
const DEFAULT_CID_ASCENT: f64 = 880.0;

/// Default descent for CID fonts when not specified.
const DEFAULT_CID_DESCENT: f64 = -120.0;

/// CID font subtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CidFontType {
    /// CIDFontType0: CID-keyed font based on Type 1 outlines.
    Type0,
    /// CIDFontType2: CID-keyed font based on TrueType outlines.
    Type2,
}

/// CID-to-GID (glyph ID) mapping strategy.
#[derive(Debug, Clone, PartialEq)]
pub enum CidToGidMap {
    /// Identity mapping: CID equals GID directly.
    Identity,
    /// Explicit mapping: byte array where GID for CID `n` is at bytes `2n` and `2n+1`
    /// (big-endian u16).
    Explicit(Vec<u16>),
}

impl CidToGidMap {
    /// Map a CID to a GID.
    pub fn map(&self, cid: u32) -> u32 {
        match self {
            CidToGidMap::Identity => cid,
            CidToGidMap::Explicit(table) => {
                if (cid as usize) < table.len() {
                    u32::from(table[cid as usize])
                } else {
                    cid
                }
            }
        }
    }

    /// Parse a CIDToGIDMap from raw stream bytes (big-endian u16 pairs).
    pub fn from_stream(data: &[u8]) -> Self {
        let mut table = Vec::with_capacity(data.len() / 2);
        for chunk in data.chunks(2) {
            if chunk.len() == 2 {
                table.push(u16::from_be_bytes([chunk[0], chunk[1]]));
            }
        }
        CidToGidMap::Explicit(table)
    }
}

/// CID system information from the /CIDSystemInfo dictionary.
#[derive(Debug, Clone, PartialEq)]
pub struct CidSystemInfo {
    /// Registry (e.g., "Adobe").
    pub registry: String,
    /// Ordering (e.g., "Japan1", "GB1", "CNS1", "Korea1").
    pub ordering: String,
    /// Supplement number.
    pub supplement: i64,
}

impl CidSystemInfo {
    /// Check if this is an Adobe CJK system.
    pub fn is_adobe_cjk(&self) -> bool {
        self.registry == "Adobe"
            && matches!(self.ordering.as_str(), "Japan1" | "GB1" | "CNS1" | "Korea1")
    }
}

/// Font metrics for a CID font, handling the /W array and /DW default width.
///
/// CID fonts use a different width specification than simple fonts:
/// - /DW: default width for all CIDs (default 1000)
/// - /W: array of width overrides in the format:
///   `[CID [w1 w2 ...] CIDstart CIDend w ...]`
#[derive(Debug, Clone)]
pub struct CidFontMetrics {
    /// Per-CID width overrides (from /W array).
    widths: HashMap<u32, f64>,
    /// Default width for CIDs not in the widths map (from /DW).
    default_width: f64,
    /// Font ascent in glyph space units.
    ascent: f64,
    /// Font descent in glyph space units.
    descent: f64,
    /// Font bounding box.
    font_bbox: Option<[f64; 4]>,
    /// CID font subtype.
    font_type: CidFontType,
    /// CID-to-GID mapping.
    cid_to_gid: CidToGidMap,
    /// CID system information.
    system_info: Option<CidSystemInfo>,
}

impl CidFontMetrics {
    /// Create CidFontMetrics from parsed values.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        widths: HashMap<u32, f64>,
        default_width: f64,
        ascent: f64,
        descent: f64,
        font_bbox: Option<[f64; 4]>,
        font_type: CidFontType,
        cid_to_gid: CidToGidMap,
        system_info: Option<CidSystemInfo>,
    ) -> Self {
        Self {
            widths,
            default_width,
            ascent,
            descent,
            font_bbox,
            font_type,
            cid_to_gid,
            system_info,
        }
    }

    /// Create default CidFontMetrics.
    pub fn default_metrics() -> Self {
        Self {
            widths: HashMap::new(),
            default_width: DEFAULT_CID_WIDTH,
            ascent: DEFAULT_CID_ASCENT,
            descent: DEFAULT_CID_DESCENT,
            font_bbox: None,
            font_type: CidFontType::Type2,
            cid_to_gid: CidToGidMap::Identity,
            system_info: None,
        }
    }

    /// Get the width for a CID in glyph space (1/1000 of text space).
    pub fn get_width(&self, cid: u32) -> f64 {
        self.widths.get(&cid).copied().unwrap_or(self.default_width)
    }

    /// Font ascent in glyph space units.
    pub fn ascent(&self) -> f64 {
        self.ascent
    }

    /// Font descent in glyph space units.
    pub fn descent(&self) -> f64 {
        self.descent
    }

    /// Font bounding box.
    pub fn font_bbox(&self) -> Option<[f64; 4]> {
        self.font_bbox
    }

    /// Default width for CIDs not in the width overrides.
    pub fn default_width(&self) -> f64 {
        self.default_width
    }

    /// CID font subtype.
    pub fn font_type(&self) -> CidFontType {
        self.font_type
    }

    /// CID-to-GID mapping.
    pub fn cid_to_gid(&self) -> &CidToGidMap {
        &self.cid_to_gid
    }

    /// Map a CID to a GID.
    pub fn map_cid_to_gid(&self, cid: u32) -> u32 {
        self.cid_to_gid.map(cid)
    }

    /// CID system information.
    pub fn system_info(&self) -> Option<&CidSystemInfo> {
        self.system_info.as_ref()
    }
}

/// Parse a /W (width) array from a CID font dictionary.
///
/// The /W array has the format:
/// ```text
/// [ c [w1 w2 ...] c_first c_last w ... ]
/// ```
/// Where:
/// - `c [w1 w2 ...]` assigns widths w1, w2, ... to CIDs c, c+1, c+2, ...
/// - `c_first c_last w` assigns width w to all CIDs from c_first to c_last
pub fn parse_w_array(objects: &[lopdf::Object], doc: &lopdf::Document) -> HashMap<u32, f64> {
    let mut widths = HashMap::new();
    let mut i = 0;

    while i < objects.len() {
        let cid_start = match object_to_u32(resolve_object(doc, &objects[i])) {
            Some(v) => v,
            None => {
                i += 1;
                continue;
            }
        };
        i += 1;

        if i >= objects.len() {
            break;
        }

        let next = resolve_object(doc, &objects[i]);
        if let Ok(arr) = next.as_array() {
            // Format: CID [w1 w2 w3 ...]
            for (j, obj) in arr.iter().enumerate() {
                let obj = resolve_object(doc, obj);
                if let Some(w) = object_to_f64(obj) {
                    widths.insert(cid_start + j as u32, w);
                }
            }
            i += 1;
        } else if let Some(cid_end) = object_to_u32(next) {
            // Format: CID_start CID_end w
            i += 1;
            if i < objects.len() {
                let w_obj = resolve_object(doc, &objects[i]);
                if let Some(w) = object_to_f64(w_obj) {
                    for cid in cid_start..=cid_end {
                        widths.insert(cid, w);
                    }
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    widths
}

/// Extract CID font metrics from a CIDFont dictionary (descendant of Type0).
pub fn extract_cid_font_metrics(
    doc: &lopdf::Document,
    cid_font_dict: &lopdf::Dictionary,
) -> Result<CidFontMetrics, BackendError> {
    // Determine CIDFont subtype
    let font_type = cid_font_dict
        .get(b"Subtype")
        .ok()
        .and_then(|o| o.as_name_str().ok())
        .map(|s| match s {
            "CIDFontType0" => CidFontType::Type0,
            _ => CidFontType::Type2,
        })
        .unwrap_or(CidFontType::Type2);

    // Parse /DW (default width)
    let default_width = cid_font_dict
        .get(b"DW")
        .ok()
        .and_then(|o| object_to_f64(resolve_object(doc, o)))
        .unwrap_or(DEFAULT_CID_WIDTH);

    // Parse /W (width array)
    let widths = cid_font_dict
        .get(b"W")
        .ok()
        .map(|o| resolve_object(doc, o))
        .and_then(|o| o.as_array().ok())
        .map(|arr| parse_w_array(arr, doc))
        .unwrap_or_default();

    // Parse /CIDToGIDMap
    let cid_to_gid = parse_cid_to_gid_map(doc, cid_font_dict);

    // Parse /CIDSystemInfo
    let system_info = parse_cid_system_info(doc, cid_font_dict);

    // Parse /FontDescriptor for ascent, descent, bbox
    let (ascent, descent, font_bbox) = parse_cid_font_descriptor(doc, cid_font_dict);

    Ok(CidFontMetrics::new(
        widths,
        default_width,
        ascent,
        descent,
        font_bbox,
        font_type,
        cid_to_gid,
        system_info,
    ))
}

/// Parse the /CIDToGIDMap entry from a CIDFont dictionary.
fn parse_cid_to_gid_map(doc: &lopdf::Document, dict: &lopdf::Dictionary) -> CidToGidMap {
    match dict.get(b"CIDToGIDMap") {
        Ok(obj) => {
            let obj = resolve_object(doc, obj);
            if let Ok(name) = obj.as_name_str() {
                if name == "Identity" {
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
fn parse_cid_system_info(doc: &lopdf::Document, dict: &lopdf::Dictionary) -> Option<CidSystemInfo> {
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
fn parse_cid_font_descriptor(
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

    let descent = desc
        .get(b"Descent")
        .ok()
        .and_then(object_to_f64)
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

/// Resolve an indirect reference to the actual object.
fn resolve_object<'a>(doc: &'a lopdf::Document, obj: &'a lopdf::Object) -> &'a lopdf::Object {
    match obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).unwrap_or(obj),
        _ => obj,
    }
}

/// Convert a lopdf object to f64.
fn object_to_f64(obj: &lopdf::Object) -> Option<f64> {
    match obj {
        lopdf::Object::Integer(i) => Some(*i as f64),
        lopdf::Object::Real(f) => Some(*f as f64),
        _ => None,
    }
}

/// Convert a lopdf object to u32.
fn object_to_u32(obj: &lopdf::Object) -> Option<u32> {
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
        .and_then(|o| o.as_name_str().ok())
        .is_some_and(|s| s == "Type0")
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
    encoding.as_name_str().ok().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Document, Object, Stream, dictionary};

    // ========== CidToGidMap tests ==========

    #[test]
    fn identity_map_returns_same_cid() {
        let map = CidToGidMap::Identity;
        assert_eq!(map.map(0), 0);
        assert_eq!(map.map(100), 100);
        assert_eq!(map.map(65535), 65535);
    }

    #[test]
    fn explicit_map_looks_up_table() {
        let table = vec![10, 20, 30, 40, 50];
        let map = CidToGidMap::Explicit(table);
        assert_eq!(map.map(0), 10);
        assert_eq!(map.map(1), 20);
        assert_eq!(map.map(4), 50);
    }

    #[test]
    fn explicit_map_out_of_range_returns_cid() {
        let table = vec![10, 20, 30];
        let map = CidToGidMap::Explicit(table);
        assert_eq!(map.map(5), 5); // out of range → fallback to CID
    }

    #[test]
    fn from_stream_parses_big_endian_u16() {
        // CID 0 → GID 5, CID 1 → GID 10
        let data = vec![0x00, 0x05, 0x00, 0x0A];
        let map = CidToGidMap::from_stream(&data);
        assert_eq!(map.map(0), 5);
        assert_eq!(map.map(1), 10);
    }

    #[test]
    fn from_stream_handles_odd_length() {
        // Only one complete pair, last byte ignored
        let data = vec![0x00, 0x05, 0x00];
        let map = CidToGidMap::from_stream(&data);
        assert_eq!(map.map(0), 5);
        assert_eq!(map.map(1), 1); // out of range
    }

    #[test]
    fn from_stream_empty() {
        let map = CidToGidMap::from_stream(&[]);
        assert_eq!(map.map(0), 0); // out of range, falls back to CID
    }

    // ========== CidSystemInfo tests ==========

    #[test]
    fn cid_system_info_adobe_japan1() {
        let info = CidSystemInfo {
            registry: "Adobe".to_string(),
            ordering: "Japan1".to_string(),
            supplement: 6,
        };
        assert!(info.is_adobe_cjk());
    }

    #[test]
    fn cid_system_info_adobe_gb1() {
        let info = CidSystemInfo {
            registry: "Adobe".to_string(),
            ordering: "GB1".to_string(),
            supplement: 5,
        };
        assert!(info.is_adobe_cjk());
    }

    #[test]
    fn cid_system_info_adobe_cns1() {
        let info = CidSystemInfo {
            registry: "Adobe".to_string(),
            ordering: "CNS1".to_string(),
            supplement: 7,
        };
        assert!(info.is_adobe_cjk());
    }

    #[test]
    fn cid_system_info_adobe_korea1() {
        let info = CidSystemInfo {
            registry: "Adobe".to_string(),
            ordering: "Korea1".to_string(),
            supplement: 2,
        };
        assert!(info.is_adobe_cjk());
    }

    #[test]
    fn cid_system_info_non_adobe_not_cjk() {
        let info = CidSystemInfo {
            registry: "Custom".to_string(),
            ordering: "Japan1".to_string(),
            supplement: 0,
        };
        assert!(!info.is_adobe_cjk());
    }

    #[test]
    fn cid_system_info_adobe_non_cjk_ordering() {
        let info = CidSystemInfo {
            registry: "Adobe".to_string(),
            ordering: "Identity".to_string(),
            supplement: 0,
        };
        assert!(!info.is_adobe_cjk());
    }

    // ========== CidFontMetrics tests ==========

    #[test]
    fn cid_font_metrics_get_width_from_map() {
        let mut widths = HashMap::new();
        widths.insert(1, 500.0);
        widths.insert(2, 600.0);
        widths.insert(100, 250.0);

        let metrics = CidFontMetrics::new(
            widths,
            1000.0,
            880.0,
            -120.0,
            None,
            CidFontType::Type2,
            CidToGidMap::Identity,
            None,
        );

        assert_eq!(metrics.get_width(1), 500.0);
        assert_eq!(metrics.get_width(2), 600.0);
        assert_eq!(metrics.get_width(100), 250.0);
    }

    #[test]
    fn cid_font_metrics_get_width_returns_default() {
        let metrics = CidFontMetrics::new(
            HashMap::new(),
            1000.0,
            880.0,
            -120.0,
            None,
            CidFontType::Type2,
            CidToGidMap::Identity,
            None,
        );

        assert_eq!(metrics.get_width(0), 1000.0);
        assert_eq!(metrics.get_width(999), 1000.0);
    }

    #[test]
    fn cid_font_metrics_custom_default_width() {
        let metrics = CidFontMetrics::new(
            HashMap::new(),
            500.0,
            880.0,
            -120.0,
            None,
            CidFontType::Type0,
            CidToGidMap::Identity,
            None,
        );

        assert_eq!(metrics.get_width(0), 500.0);
        assert_eq!(metrics.default_width(), 500.0);
    }

    #[test]
    fn cid_font_metrics_accessors() {
        let info = CidSystemInfo {
            registry: "Adobe".to_string(),
            ordering: "Japan1".to_string(),
            supplement: 6,
        };
        let metrics = CidFontMetrics::new(
            HashMap::new(),
            1000.0,
            880.0,
            -120.0,
            Some([-100.0, -200.0, 1100.0, 900.0]),
            CidFontType::Type0,
            CidToGidMap::Identity,
            Some(info),
        );

        assert_eq!(metrics.ascent(), 880.0);
        assert_eq!(metrics.descent(), -120.0);
        assert_eq!(metrics.font_bbox(), Some([-100.0, -200.0, 1100.0, 900.0]));
        assert_eq!(metrics.font_type(), CidFontType::Type0);
        assert_eq!(metrics.cid_to_gid(), &CidToGidMap::Identity);
        assert!(metrics.system_info().unwrap().is_adobe_cjk());
    }

    #[test]
    fn cid_font_metrics_map_cid_to_gid() {
        let table = vec![10, 20, 30];
        let metrics = CidFontMetrics::new(
            HashMap::new(),
            1000.0,
            880.0,
            -120.0,
            None,
            CidFontType::Type2,
            CidToGidMap::Explicit(table),
            None,
        );

        assert_eq!(metrics.map_cid_to_gid(0), 10);
        assert_eq!(metrics.map_cid_to_gid(1), 20);
        assert_eq!(metrics.map_cid_to_gid(2), 30);
        assert_eq!(metrics.map_cid_to_gid(5), 5); // fallback
    }

    #[test]
    fn cid_font_metrics_default() {
        let metrics = CidFontMetrics::default_metrics();
        assert_eq!(metrics.default_width(), DEFAULT_CID_WIDTH);
        assert_eq!(metrics.ascent(), DEFAULT_CID_ASCENT);
        assert_eq!(metrics.descent(), DEFAULT_CID_DESCENT);
        assert_eq!(metrics.font_bbox(), None);
        assert_eq!(metrics.font_type(), CidFontType::Type2);
        assert_eq!(metrics.cid_to_gid(), &CidToGidMap::Identity);
        assert!(metrics.system_info().is_none());
    }

    // ========== parse_w_array tests ==========

    #[test]
    fn parse_w_array_individual_widths() {
        // [1 [500 600 700]] → CID 1=500, CID 2=600, CID 3=700
        let doc = Document::with_version("1.5");
        let objects = vec![
            Object::Integer(1),
            Object::Array(vec![
                Object::Integer(500),
                Object::Integer(600),
                Object::Integer(700),
            ]),
        ];

        let widths = parse_w_array(&objects, &doc);
        assert_eq!(widths.get(&1), Some(&500.0));
        assert_eq!(widths.get(&2), Some(&600.0));
        assert_eq!(widths.get(&3), Some(&700.0));
        assert_eq!(widths.get(&0), None);
        assert_eq!(widths.get(&4), None);
    }

    #[test]
    fn parse_w_array_range_format() {
        // [10 20 500] → CIDs 10-20 all have width 500
        let doc = Document::with_version("1.5");
        let objects = vec![
            Object::Integer(10),
            Object::Integer(20),
            Object::Integer(500),
        ];

        let widths = parse_w_array(&objects, &doc);
        for cid in 10..=20 {
            assert_eq!(widths.get(&cid), Some(&500.0), "CID {} should be 500", cid);
        }
        assert_eq!(widths.get(&9), None);
        assert_eq!(widths.get(&21), None);
    }

    #[test]
    fn parse_w_array_mixed_formats() {
        // [1 [250 300] 10 20 500]
        let doc = Document::with_version("1.5");
        let objects = vec![
            Object::Integer(1),
            Object::Array(vec![Object::Integer(250), Object::Integer(300)]),
            Object::Integer(10),
            Object::Integer(20),
            Object::Integer(500),
        ];

        let widths = parse_w_array(&objects, &doc);
        assert_eq!(widths.get(&1), Some(&250.0));
        assert_eq!(widths.get(&2), Some(&300.0));
        for cid in 10..=20 {
            assert_eq!(widths.get(&cid), Some(&500.0));
        }
    }

    #[test]
    fn parse_w_array_empty() {
        let doc = Document::with_version("1.5");
        let widths = parse_w_array(&[], &doc);
        assert!(widths.is_empty());
    }

    #[test]
    fn parse_w_array_real_values() {
        let doc = Document::with_version("1.5");
        let objects = vec![
            Object::Integer(1),
            Object::Array(vec![Object::Real(500.5), Object::Real(600.5)]),
        ];

        let widths = parse_w_array(&objects, &doc);
        assert!((widths[&1] - 500.5).abs() < 0.1);
        assert!((widths[&2] - 600.5).abs() < 0.1);
    }

    #[test]
    fn parse_w_array_single_cid_range() {
        // [5 5 700] → CID 5 = 700
        let doc = Document::with_version("1.5");
        let objects = vec![Object::Integer(5), Object::Integer(5), Object::Integer(700)];

        let widths = parse_w_array(&objects, &doc);
        assert_eq!(widths.get(&5), Some(&700.0));
        assert_eq!(widths.len(), 1);
    }

    // ========== extract_cid_font_metrics tests ==========

    #[test]
    fn extract_cid_font_metrics_basic() {
        let mut doc = Document::with_version("1.5");

        // Create a CIDFont dictionary
        let w_array = Object::Array(vec![
            Object::Integer(1),
            Object::Array(vec![Object::Integer(500), Object::Integer(600)]),
        ]);
        let w_id = doc.add_object(w_array);

        let cid_font_dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "CIDFontType2",
            "BaseFont" => "MSGothic",
            "DW" => Object::Integer(1000),
            "W" => w_id,
            "CIDToGIDMap" => "Identity",
        };

        let metrics = extract_cid_font_metrics(&doc, &cid_font_dict).unwrap();
        assert_eq!(metrics.font_type(), CidFontType::Type2);
        assert_eq!(metrics.default_width(), 1000.0);
        assert_eq!(metrics.get_width(1), 500.0);
        assert_eq!(metrics.get_width(2), 600.0);
        assert_eq!(metrics.get_width(3), 1000.0); // default
        assert_eq!(metrics.cid_to_gid(), &CidToGidMap::Identity);
    }

    #[test]
    fn extract_cid_font_metrics_type0() {
        let doc = Document::with_version("1.5");

        let cid_font_dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "CIDFontType0",
            "BaseFont" => "KozMinPro-Regular",
        };

        let metrics = extract_cid_font_metrics(&doc, &cid_font_dict).unwrap();
        assert_eq!(metrics.font_type(), CidFontType::Type0);
        assert_eq!(metrics.default_width(), DEFAULT_CID_WIDTH);
    }

    #[test]
    fn extract_cid_font_metrics_with_descriptor() {
        let mut doc = Document::with_version("1.5");

        let desc_id = doc.add_object(Object::Dictionary(dictionary! {
            "Type" => "FontDescriptor",
            "FontName" => "MSGothic",
            "Ascent" => Object::Integer(859),
            "Descent" => Object::Integer(-140),
            "FontBBox" => Object::Array(vec![
                Object::Integer(0),
                Object::Integer(-137),
                Object::Integer(1000),
                Object::Integer(859),
            ]),
        }));

        let cid_font_dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "CIDFontType2",
            "BaseFont" => "MSGothic",
            "FontDescriptor" => desc_id,
        };

        let metrics = extract_cid_font_metrics(&doc, &cid_font_dict).unwrap();
        assert_eq!(metrics.ascent(), 859.0);
        assert_eq!(metrics.descent(), -140.0);
        assert!(metrics.font_bbox().is_some());
    }

    #[test]
    fn extract_cid_font_metrics_with_system_info() {
        let doc = Document::with_version("1.5");

        let cid_font_dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "CIDFontType2",
            "BaseFont" => "MSGothic",
            "CIDSystemInfo" => Object::Dictionary(dictionary! {
                "Registry" => Object::String("Adobe".as_bytes().to_vec(), lopdf::StringFormat::Literal),
                "Ordering" => Object::String("Japan1".as_bytes().to_vec(), lopdf::StringFormat::Literal),
                "Supplement" => Object::Integer(6),
            }),
        };

        let metrics = extract_cid_font_metrics(&doc, &cid_font_dict).unwrap();
        let info = metrics.system_info().unwrap();
        assert_eq!(info.registry, "Adobe");
        assert_eq!(info.ordering, "Japan1");
        assert_eq!(info.supplement, 6);
        assert!(info.is_adobe_cjk());
    }

    #[test]
    fn extract_cid_font_metrics_explicit_gid_map() {
        let mut doc = Document::with_version("1.5");

        // CIDToGIDMap stream: CID 0→GID 5, CID 1→GID 10
        let gid_data = vec![0x00, 0x05, 0x00, 0x0A];
        let gid_stream = Stream::new(dictionary! {}, gid_data);
        let gid_stream_id = doc.add_object(Object::Stream(gid_stream));

        let cid_font_dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "CIDFontType2",
            "BaseFont" => "CustomFont",
            "CIDToGIDMap" => gid_stream_id,
        };

        let metrics = extract_cid_font_metrics(&doc, &cid_font_dict).unwrap();
        assert_eq!(metrics.map_cid_to_gid(0), 5);
        assert_eq!(metrics.map_cid_to_gid(1), 10);
    }

    // ========== Predefined CMap name parsing tests ==========

    #[test]
    fn parse_identity_h() {
        let info = parse_predefined_cmap_name("Identity-H").unwrap();
        assert_eq!(info.name, "Identity-H");
        assert_eq!(info.writing_mode, 0);
        assert!(info.is_identity);
    }

    #[test]
    fn parse_identity_v() {
        let info = parse_predefined_cmap_name("Identity-V").unwrap();
        assert_eq!(info.name, "Identity-V");
        assert_eq!(info.writing_mode, 1);
        assert!(info.is_identity);
    }

    #[test]
    fn parse_adobe_japan1() {
        let info = parse_predefined_cmap_name("Adobe-Japan1-6").unwrap();
        assert_eq!(info.registry, "Adobe");
        assert_eq!(info.ordering, "Japan1");
        assert!(!info.is_identity);
    }

    #[test]
    fn parse_adobe_gb1() {
        let info = parse_predefined_cmap_name("Adobe-GB1-5").unwrap();
        assert_eq!(info.ordering, "GB1");
    }

    #[test]
    fn parse_adobe_cns1() {
        let info = parse_predefined_cmap_name("Adobe-CNS1-7").unwrap();
        assert_eq!(info.ordering, "CNS1");
    }

    #[test]
    fn parse_adobe_korea1() {
        let info = parse_predefined_cmap_name("Adobe-Korea1-2").unwrap();
        assert_eq!(info.ordering, "Korea1");
    }

    #[test]
    fn parse_unijis_utf16_h() {
        let info = parse_predefined_cmap_name("UniJIS-UTF16-H").unwrap();
        assert_eq!(info.ordering, "Japan1");
        assert_eq!(info.writing_mode, 0);
    }

    #[test]
    fn parse_unijis_utf16_v() {
        let info = parse_predefined_cmap_name("UniJIS-UTF16-V").unwrap();
        assert_eq!(info.ordering, "Japan1");
        assert_eq!(info.writing_mode, 1);
    }

    #[test]
    fn parse_unigb_utf16_h() {
        let info = parse_predefined_cmap_name("UniGB-UTF16-H").unwrap();
        assert_eq!(info.ordering, "GB1");
    }

    #[test]
    fn parse_uniksc_utf16_h() {
        let info = parse_predefined_cmap_name("UniKS-UTF16-H").unwrap();
        assert_eq!(info.ordering, "Korea1");
    }

    #[test]
    fn parse_90ms_rksj_h() {
        let info = parse_predefined_cmap_name("90ms-RKSJ-H").unwrap();
        assert_eq!(info.ordering, "Japan1");
        assert_eq!(info.writing_mode, 0);
    }

    #[test]
    fn parse_unknown_cmap_returns_none() {
        assert!(parse_predefined_cmap_name("UnknownCMap").is_none());
    }

    #[test]
    fn parse_empty_cmap_returns_none() {
        assert!(parse_predefined_cmap_name("").is_none());
    }

    // ========== Type0 font detection tests ==========

    #[test]
    fn detect_type0_font() {
        let dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "Type0",
            "BaseFont" => "SomeFont",
        };
        assert!(is_type0_font(&dict));
    }

    #[test]
    fn detect_non_type0_font() {
        let dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        };
        assert!(!is_type0_font(&dict));
    }

    #[test]
    fn detect_truetype_font() {
        let dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "TrueType",
            "BaseFont" => "Arial",
        };
        assert!(!is_type0_font(&dict));
    }

    // ========== get_descendant_font tests ==========

    #[test]
    fn get_descendant_font_basic() {
        let mut doc = Document::with_version("1.5");

        let cid_font_dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "CIDFontType2",
            "BaseFont" => "MSGothic",
        };
        let cid_font_id = doc.add_object(Object::Dictionary(cid_font_dict));

        let type0_dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "Type0",
            "BaseFont" => "MSGothic",
            "DescendantFonts" => Object::Array(vec![Object::Reference(cid_font_id)]),
        };

        let desc = get_descendant_font(&doc, &type0_dict);
        assert!(desc.is_some());
        let desc = desc.unwrap();
        assert_eq!(
            desc.get(b"Subtype").unwrap().as_name_str().unwrap(),
            "CIDFontType2"
        );
    }

    #[test]
    fn get_descendant_font_missing() {
        let doc = Document::with_version("1.5");
        let type0_dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "Type0",
            "BaseFont" => "MSGothic",
        };

        assert!(get_descendant_font(&doc, &type0_dict).is_none());
    }

    // ========== get_type0_encoding tests ==========

    #[test]
    fn get_encoding_identity_h() {
        let dict = dictionary! {
            "Subtype" => "Type0",
            "Encoding" => "Identity-H",
        };
        assert_eq!(get_type0_encoding(&dict), Some("Identity-H".to_string()));
    }

    #[test]
    fn get_encoding_missing() {
        let dict = dictionary! {
            "Subtype" => "Type0",
        };
        assert_eq!(get_type0_encoding(&dict), None);
    }
}
