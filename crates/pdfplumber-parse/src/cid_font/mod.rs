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

/// Default vertical origin y-component (DW2[0]) per PDF spec.
const DEFAULT_DW2_VY: f64 = 880.0;

/// Default vertical advance (DW2[1]) per PDF spec.
const DEFAULT_DW2_W1: f64 = -1000.0;

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

/// Vertical glyph metrics for a single CID (from /W2 array).
#[derive(Debug, Clone, Copy)]
pub struct VerticalMetric {
    /// Vertical advance (w1y) in glyph space units.
    pub w1y: f64,
    /// Horizontal displacement of vertical origin from horizontal origin (vx).
    pub vx: f64,
    /// Vertical displacement of vertical origin from horizontal origin (vy).
    pub vy: f64,
}

/// Font metrics for a CID font, handling the /W array and /DW default width.
///
/// CID fonts use a different width specification than simple fonts:
/// - /DW: default width for all CIDs (default 1000)
/// - /W: array of width overrides in the format:
///   `[CID [w1 w2 ...] CIDstart CIDend w ...]`
/// - /DW2: default vertical metrics [vy, w1] (default [880, -1000])
/// - /W2: per-CID vertical metric overrides
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
    /// Per-CID vertical metric overrides (from /W2 array).
    vertical_widths: HashMap<u32, VerticalMetric>,
    /// Default vertical origin y-component (from DW2[0], default 880).
    default_vy: f64,
    /// Default vertical advance (from DW2[1], default -1000).
    default_w1: f64,
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
            vertical_widths: HashMap::new(),
            default_vy: DEFAULT_DW2_VY,
            default_w1: DEFAULT_DW2_W1,
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
            vertical_widths: HashMap::new(),
            default_vy: DEFAULT_DW2_VY,
            default_w1: DEFAULT_DW2_W1,
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

    /// Get the vertical advance (w1y) for a CID in glyph space.
    /// Falls back to `DW2[1]` (default -1000) if no W2 override exists.
    pub fn get_vertical_w1(&self, cid: u32) -> f64 {
        self.vertical_widths
            .get(&cid)
            .map(|vm| vm.w1y)
            .unwrap_or(self.default_w1)
    }

    /// Get the vertical metric for a CID, with fallback to defaults.
    /// Returns (w1y, vx, vy) where:
    /// - w1y: vertical advance
    /// - vx: horizontal displacement of vertical origin (default: DW/2)
    /// - vy: vertical displacement of vertical origin (default: `DW2[0]`)
    pub fn get_vertical_metric(&self, cid: u32) -> VerticalMetric {
        self.vertical_widths
            .get(&cid)
            .copied()
            .unwrap_or(VerticalMetric {
                w1y: self.default_w1,
                vx: self.default_width / 2.0,
                vy: self.default_vy,
            })
    }

    /// Set vertical metrics from parsed W2 array and DW2 values.
    pub fn set_vertical_metrics(
        &mut self,
        vertical_widths: HashMap<u32, VerticalMetric>,
        default_vy: f64,
        default_w1: f64,
    ) {
        self.vertical_widths = vertical_widths;
        self.default_vy = default_vy;
        self.default_w1 = default_w1;
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

/// Parse a /W2 (vertical width) array from a CID font dictionary.
///
/// The /W2 array format (PDF spec 9.7.4.3):
/// ```text
/// [ c [w1y v1x v1y w2y v2x v2y ...] c_first c_last w1y vx vy ... ]
/// ```
/// Where each CID gets a `VerticalMetric { w1y, vx, vy }`.
pub fn parse_w2_array(
    objects: &[lopdf::Object],
    doc: &lopdf::Document,
    default_width: f64,
) -> HashMap<u32, VerticalMetric> {
    let mut metrics = HashMap::new();
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
            // Format: CID [w1y vx vy w1y vx vy ...]
            let mut j = 0;
            let mut cid = cid_start;
            while j + 2 < arr.len() {
                let w1y = object_to_f64(resolve_object(doc, &arr[j])).unwrap_or(DEFAULT_DW2_W1);
                let vx =
                    object_to_f64(resolve_object(doc, &arr[j + 1])).unwrap_or(default_width / 2.0);
                let vy = object_to_f64(resolve_object(doc, &arr[j + 2])).unwrap_or(DEFAULT_DW2_VY);
                metrics.insert(cid, VerticalMetric { w1y, vx, vy });
                cid += 1;
                j += 3;
            }
            i += 1;
        } else if let Some(cid_end) = object_to_u32(next) {
            // Format: CID_start CID_end w1y vx vy
            i += 1;
            if i + 2 < objects.len() {
                let w1y = object_to_f64(resolve_object(doc, &objects[i])).unwrap_or(DEFAULT_DW2_W1);
                let vx = object_to_f64(resolve_object(doc, &objects[i + 1]))
                    .unwrap_or(default_width / 2.0);
                let vy =
                    object_to_f64(resolve_object(doc, &objects[i + 2])).unwrap_or(DEFAULT_DW2_VY);
                for cid in cid_start..=cid_end {
                    metrics.insert(cid, VerticalMetric { w1y, vx, vy });
                }
                i += 3;
            }
        } else {
            i += 1;
        }
    }

    metrics
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
        .and_then(|o| o.as_name().ok())
        .map(|s| match s {
            b"CIDFontType0" => CidFontType::Type0,
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

    let mut metrics = CidFontMetrics::new(
        widths,
        default_width,
        ascent,
        descent,
        font_bbox,
        font_type,
        cid_to_gid,
        system_info,
    );

    // Parse /DW2 (default vertical metrics: [vy, w1])
    let (dw2_vy, dw2_w1) = cid_font_dict
        .get(b"DW2")
        .ok()
        .map(|o| resolve_object(doc, o))
        .and_then(|o| o.as_array().ok())
        .and_then(|arr| {
            if arr.len() >= 2 {
                let vy = object_to_f64(resolve_object(doc, &arr[0]))?;
                let w1 = object_to_f64(resolve_object(doc, &arr[1]))?;
                Some((vy, w1))
            } else {
                None
            }
        })
        .unwrap_or((DEFAULT_DW2_VY, DEFAULT_DW2_W1));

    // Parse /W2 (vertical width overrides)
    let mut vertical_widths = cid_font_dict
        .get(b"W2")
        .ok()
        .map(|o| resolve_object(doc, o))
        .and_then(|o| o.as_array().ok())
        .map(|arr| parse_w2_array(arr, doc, default_width))
        .unwrap_or_default();

    // For CIDFontType2 (TrueType-based) fonts, try vmtx table as fallback
    // when W2 is not present. W2/DW2 from the PDF take precedence over vmtx.
    if vertical_widths.is_empty() && font_type == CidFontType::Type2 {
        if let Some(vmtx_metrics) = try_extract_vmtx_vertical_metrics(doc, cid_font_dict, &metrics)
        {
            vertical_widths = vmtx_metrics;
        }
    }

    metrics.set_vertical_metrics(vertical_widths, dw2_vy, dw2_w1);

    Ok(metrics)
}

mod parsing;
pub use parsing::{
    PredefinedCMapInfo, get_descendant_font, get_type0_encoding, is_subset_font, is_type0_font,
    parse_predefined_cmap_name, strip_subset_prefix,
};
use parsing::{
    object_to_f64, object_to_u32, parse_cid_font_descriptor, parse_cid_system_info,
    parse_cid_to_gid_map, resolve_object, try_extract_vmtx_vertical_metrics,
};

#[cfg(test)]
mod tests;
