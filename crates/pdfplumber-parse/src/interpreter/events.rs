//! Content event emission for the interpreter.
//!
//! Converts raw character and path data into typed events dispatched to
//! [`ContentHandler`]. Also handles graphics state extension (`gs` operator).

use super::{CachedFont, MarkedContentEntry};
use crate::cjk_encoding;
use crate::handler::{CharEvent, ContentHandler, PaintOp, PathEvent};
use crate::interpreter_state::InterpreterState;
use crate::lopdf_backend::{object_to_f64, resolve_ref};
use crate::text_state::TextState;
use pdfplumber_core::FillRule;

pub(super) fn emit_char_events(
    raw_chars: Vec<crate::text_renderer::RawChar>,
    tstate: &TextState,
    gstate: &InterpreterState,
    handler: &mut dyn ContentHandler,
    cached: Option<&CachedFont>,
    marked_content_stack: &[MarkedContentEntry],
) {
    let ctm = gstate.ctm_array();
    let font_name = cached.map_or_else(|| tstate.font_name.clone(), |c| c.base_name.clone());

    for rc in raw_chars {
        // Unicode resolution chain: CMap → FontEncoding → CJK encoding → char::from_u32
        let unicode = cached
            .and_then(|c| {
                // 1. Try ToUnicode CMap (highest priority)
                c.cmap
                    .as_ref()
                    .and_then(|cm| cm.lookup(rc.char_code).map(|s| s.to_string()))
            })
            .or_else(|| {
                // 2. Try font encoding (for simple fonts)
                cached.and_then(|c| {
                    c.encoding.as_ref().and_then(|enc| {
                        if rc.char_code <= 255 {
                            enc.decode(rc.char_code as u8).map(|ch| ch.to_string())
                        } else {
                            None
                        }
                    })
                })
            })
            .or_else(|| {
                // 3. Try CJK encoding (for CID fonts with predefined CMaps like GBK-EUC-H, EUC-H, H)
                cached.and_then(|c| {
                    c.cjk_encoding.map(|enc| {
                        let bytes = if rc.char_code > 0xFF {
                            vec![(rc.char_code >> 8) as u8, (rc.char_code & 0xFF) as u8]
                        } else {
                            vec![rc.char_code as u8]
                        };
                        cjk_encoding::decode_to_unicode(&bytes, enc)
                    })
                })
            })
            .or_else(|| {
                // 4. Try Adobe CID→Unicode mapping for CID fonts with known ordering
                cached.and_then(|c| {
                    c.cid_metrics.as_ref().and_then(|cm| {
                        cm.system_info().and_then(|si| match si.ordering.as_str() {
                            "Japan1" => {
                                crate::adobe_japan1_ucs2::lookup_japan1_unicode(rc.char_code)
                                    .map(|ch| ch.to_string())
                            }
                            "GB1" => crate::adobe_gb1_ucs2::lookup_gb1_unicode(rc.char_code)
                                .map(|ch| ch.to_string()),
                            "CNS1" => crate::adobe_cns1_ucs2::lookup_cns1_unicode(rc.char_code)
                                .map(|ch| ch.to_string()),
                            "Korea1" => {
                                crate::adobe_korea1_ucs2::lookup_korea1_unicode(rc.char_code)
                                    .map(|ch| ch.to_string())
                            }
                            _ => None,
                        })
                    })
                })
            })
            .or_else(|| {
                // 5. Fallback: determine whether to treat char_code as Unicode
                // or output (cid:N) for unmapped CID font characters.
                if cached.is_some_and(|c| c.is_cid_font) {
                    // For CID fonts, check if Identity fallback applies:
                    // - ToUnicode CMap is explicitly Identity (full-range), OR
                    // - Encoding is Identity-H/V AND CIDSystemInfo ordering is
                    //   "Identity" (not a CJK collection), meaning CID = Unicode.
                    let identity_fallback = cached.is_some_and(|c| {
                        c.cmap.as_ref().is_some_and(|cm| cm.is_identity())
                            || (c.is_identity_encoding
                                && c.cid_metrics
                                    .as_ref()
                                    .and_then(|cm| cm.system_info())
                                    .is_none_or(|si| si.ordering == "Identity"))
                    });
                    if identity_fallback {
                        char::from_u32(rc.char_code).map(|ch| ch.to_string())
                    } else {
                        Some(format!("(cid:{})", rc.char_code))
                    }
                } else {
                    // Simple fonts: interpret char_code as Unicode code point.
                    char::from_u32(rc.char_code).map(|ch| ch.to_string())
                }
            });

        // Use CID font metrics for displacement if available.
        // For vertical writing mode, use the full em square (1000 glyph units)
        // as the bbox width, matching pdfminer's vertical text behavior.
        let displacement = if cached.is_some_and(|c| c.writing_mode == 1) {
            1000.0
        } else {
            match cached {
                Some(cf) if cf.is_cid_font => cf
                    .cid_metrics
                    .as_ref()
                    .map_or(600.0, |cm| cm.get_width(rc.char_code)),
                Some(cf) => cf.metrics.get_width(rc.char_code),
                None => 600.0,
            }
        };

        // Ascent/descent for bounding box calculation.
        // Use FontDescriptor /Descent for vertical anchoring, then derive ascent
        // as (1000 + descent) to keep char height = font_size. This matches
        // Python pdfminer/pdfplumber-py behavior where bbox height always equals
        // the font size. For CID fonts, prefer CID font descriptor descent over
        // the parent Type0 font metrics.
        // When both Ascent=0 AND Descent=0 (signals "unknown"), use 1000/0 so
        // bbox spans baseline to baseline+fontsize.
        let (ascent, descent) = match cached {
            Some(cf) if cf.is_cid_font && cf.writing_mode == 1 => {
                // Vertical writing mode: use em-square aligned bbox (0, 1000)
                // to match pdfminer's vertical text behavior where bbox bottom
                // aligns with the baseline.
                (1000.0, 0.0)
            }
            Some(cf) if cf.is_cid_font => {
                let desc = cf
                    .cid_metrics
                    .as_ref()
                    .map_or(cf.metrics.descent(), |cm| cm.descent());
                if desc == 0.0
                    && cf
                        .cid_metrics
                        .as_ref()
                        .map_or(cf.metrics.ascent(), |cm| cm.ascent())
                        == 0.0
                {
                    (1000.0, 0.0)
                } else {
                    (1000.0 + desc, desc)
                }
            }
            Some(cf) if cf.metrics.ascent() == 0.0 && cf.metrics.descent() == 0.0 => (1000.0, 0.0),
            Some(cf) => {
                let desc = cf.metrics.descent();
                (1000.0 + desc, desc)
            }
            _ => (750.0, -250.0),
        };

        // Vertical origin displacement for vertical writing mode.
        // For WMode=1 fonts, the text position is the vertical origin,
        // displaced from the horizontal origin by (vx, vy) in glyph space.
        let vertical_origin = if cached.is_some_and(|c| c.writing_mode == 1) {
            cached
                .and_then(|c| c.cid_metrics.as_ref())
                .map(|cm| {
                    let vm = cm.get_vertical_metric(rc.char_code);
                    (vm.vx, vm.vy)
                })
                .unwrap_or((0.0, 0.0))
        } else {
            (0.0, 0.0)
        };

        handler.on_char(CharEvent {
            char_code: rc.char_code,
            unicode,
            font_name: font_name.clone(),
            font_size: tstate.font_size,
            text_matrix: rc.text_matrix,
            ctm,
            displacement,
            char_spacing: tstate.char_spacing,
            word_spacing: tstate.word_spacing,
            h_scaling: tstate.h_scaling_normalized(),
            rise: tstate.rise,
            ascent,
            descent,
            vertical_origin,
            mcid: marked_content_stack.iter().rev().find_map(|mc| mc.mcid),
            tag: marked_content_stack.last().map(|mc| mc.tag.clone()),
        });
    }
}

// --- Path painting ---

/// Emit a PathEvent from a PaintedPath produced by the PathBuilder.
pub(super) fn emit_path_event(
    handler: &mut dyn ContentHandler,
    gstate: &InterpreterState,
    painted: &pdfplumber_core::PaintedPath,
    paint_op: PaintOp,
    fill_rule: Option<FillRule>,
) {
    if painted.path.segments.is_empty() {
        return;
    }
    handler.on_path_painted(PathEvent {
        segments: painted.path.segments.clone(),
        paint_op,
        line_width: painted.line_width,
        stroking_color: Some(painted.stroke_color.clone()),
        non_stroking_color: Some(painted.fill_color.clone()),
        ctm: gstate.ctm_array(),
        dash_pattern: if painted.dash_pattern.is_solid() {
            None
        } else {
            Some(painted.dash_pattern.clone())
        },
        fill_rule,
    });
}

// --- gs operator: Extended Graphics State ---

/// Look up the named ExtGState from page resources and apply it to the current
/// graphics state. Unknown keys are silently ignored. If the name is not found
/// in resources, this is a no-op (graceful degradation).
pub(super) fn apply_ext_gstate(
    doc: &lopdf::Document,
    resources: &lopdf::Dictionary,
    gstate: &mut InterpreterState,
    name: &str,
) {
    let ext_dict = (|| -> Option<&lopdf::Dictionary> {
        let egs_obj = resources.get(b"ExtGState").ok()?;
        let egs_obj = resolve_ref(doc, egs_obj);
        let egs_dict = egs_obj.as_dict().ok()?;
        let entry = egs_dict.get(name.as_bytes()).ok()?;
        let entry = resolve_ref(doc, entry);
        entry.as_dict().ok()
    })();

    let Some(ext_dict) = ext_dict else {
        return;
    };

    let mut ext = pdfplumber_core::ExtGState::default();

    // /LW — Line width
    if let Ok(obj) = ext_dict.get(b"LW") {
        let obj = resolve_ref(doc, obj);
        if let Ok(v) = object_to_f64(obj) {
            ext.line_width = Some(v);
        }
    }

    // /D — Dash pattern: [dash_array phase]
    if let Ok(obj) = ext_dict.get(b"D") {
        let obj = resolve_ref(doc, obj);
        if let Ok(arr) = obj.as_array() {
            if arr.len() >= 2 {
                if let Ok(dash_arr) = arr[0].as_array() {
                    let dash_array: Vec<f64> = dash_arr
                        .iter()
                        .filter_map(|o| match o {
                            lopdf::Object::Integer(i) => Some(*i as f64),
                            lopdf::Object::Real(f) => Some(*f as f64),
                            _ => None,
                        })
                        .collect();
                    let phase = match &arr[1] {
                        lopdf::Object::Integer(i) => *i as f64,
                        lopdf::Object::Real(f) => *f as f64,
                        _ => 0.0,
                    };
                    ext.dash_pattern = Some(pdfplumber_core::DashPattern::new(dash_array, phase));
                }
            }
        }
    }

    // /CA — Stroking alpha
    if let Ok(obj) = ext_dict.get(b"CA") {
        let obj = resolve_ref(doc, obj);
        if let Ok(v) = object_to_f64(obj) {
            ext.stroke_alpha = Some(v);
        }
    }

    // /ca — Non-stroking alpha
    if let Ok(obj) = ext_dict.get(b"ca") {
        let obj = resolve_ref(doc, obj);
        if let Ok(v) = object_to_f64(obj) {
            ext.fill_alpha = Some(v);
        }
    }

    // /Font — Font array [fontRef size]
    if let Ok(obj) = ext_dict.get(b"Font") {
        let obj = resolve_ref(doc, obj);
        if let Ok(arr) = obj.as_array() {
            if arr.len() >= 2 {
                let font_name = match &arr[0] {
                    lopdf::Object::Name(n) => Some(String::from_utf8_lossy(n).to_string()),
                    _ => None,
                };
                let font_size = match &arr[1] {
                    lopdf::Object::Integer(i) => Some(*i as f64),
                    lopdf::Object::Real(f) => Some(*f as f64),
                    _ => None,
                };
                if let (Some(name), Some(size)) = (font_name, font_size) {
                    ext.font = Some((name, size));
                }
            }
        }
    }

    gstate.graphics_state_mut().apply_ext_gstate(&ext);
}
