//! Color space resolution for advanced PDF color spaces.
//!
//! Resolves ICCBased, Indexed, Separation, and DeviceN color spaces
//! from PDF resource dictionaries to concrete `Color` values.

use pdfplumber_core::painting::Color;

/// A resolved PDF color space with enough information to convert
/// component values to a `Color`.
#[derive(Debug, Clone)]
pub enum ResolvedColorSpace {
    /// DeviceGray (1 component).
    DeviceGray,
    /// DeviceRGB (3 components).
    DeviceRGB,
    /// DeviceCMYK (4 components).
    DeviceCMYK,
    /// ICCBased color space: stores the number of components and the
    /// alternate color space to use for conversion.
    ICCBased {
        /// Number of color components (1, 3, or 4).
        num_components: u32,
        /// Alternate color space for fallback conversion.
        alternate: Box<ResolvedColorSpace>,
    },
    /// Indexed color space: a palette-based color space.
    Indexed {
        /// Base color space that palette entries are specified in.
        base: Box<ResolvedColorSpace>,
        /// Maximum valid index value.
        hival: u32,
        /// Color lookup table: `(hival + 1) * num_base_components` bytes.
        lookup_table: Vec<u8>,
    },
    /// Separation color space (single-component spot color).
    /// Best-effort: uses the alternate color space.
    Separation {
        /// Alternate color space (fallback).
        alternate: Box<ResolvedColorSpace>,
    },
    /// DeviceN color space (multi-component named colors).
    /// Best-effort: uses the alternate color space.
    DeviceN {
        /// Number of color components.
        num_components: u32,
        /// Alternate color space (fallback).
        alternate: Box<ResolvedColorSpace>,
    },
}

impl ResolvedColorSpace {
    /// Number of components expected for this color space.
    pub fn num_components(&self) -> u32 {
        match self {
            ResolvedColorSpace::DeviceGray => 1,
            ResolvedColorSpace::DeviceRGB => 3,
            ResolvedColorSpace::DeviceCMYK => 4,
            ResolvedColorSpace::ICCBased { num_components, .. } => *num_components,
            ResolvedColorSpace::Indexed { .. } => 1,
            ResolvedColorSpace::Separation { .. } => 1,
            ResolvedColorSpace::DeviceN { num_components, .. } => *num_components,
        }
    }

    /// Convert color components to a `Color` value using this color space.
    pub fn resolve_color(&self, components: &[f32]) -> Color {
        match self {
            ResolvedColorSpace::DeviceGray => {
                let g = components.first().copied().unwrap_or(0.0);
                Color::Gray(g)
            }
            ResolvedColorSpace::DeviceRGB => {
                let r = components.first().copied().unwrap_or(0.0);
                let g = components.get(1).copied().unwrap_or(0.0);
                let b = components.get(2).copied().unwrap_or(0.0);
                Color::Rgb(r, g, b)
            }
            ResolvedColorSpace::DeviceCMYK => {
                let c = components.first().copied().unwrap_or(0.0);
                let m = components.get(1).copied().unwrap_or(0.0);
                let y = components.get(2).copied().unwrap_or(0.0);
                let k = components.get(3).copied().unwrap_or(0.0);
                Color::Cmyk(c, m, y, k)
            }
            ResolvedColorSpace::ICCBased { alternate, .. } => {
                // Use the alternate color space for interpretation
                alternate.resolve_color(components)
            }
            ResolvedColorSpace::Indexed {
                base,
                hival,
                lookup_table,
            } => {
                let index = components.first().copied().unwrap_or(0.0) as u32;
                let index = index.min(*hival);
                let base_n = base.num_components() as usize;
                let offset = index as usize * base_n;
                if offset + base_n <= lookup_table.len() {
                    let base_components: Vec<f32> = lookup_table[offset..offset + base_n]
                        .iter()
                        .map(|&b| b as f32 / 255.0)
                        .collect();
                    base.resolve_color(&base_components)
                } else {
                    Color::Other(components.to_vec())
                }
            }
            ResolvedColorSpace::Separation { alternate } => {
                // Best-effort: pass the tint value through to the alternate space.
                // Without evaluating the tint transform function, we use a simple
                // approximation: treat the single tint component as if it's
                // a grayscale value in the alternate space.
                let tint = components.first().copied().unwrap_or(0.0);
                match alternate.as_ref() {
                    ResolvedColorSpace::DeviceGray => Color::Gray(tint),
                    ResolvedColorSpace::DeviceRGB => Color::Rgb(tint, tint, tint),
                    ResolvedColorSpace::DeviceCMYK => Color::Cmyk(0.0, 0.0, 0.0, 1.0 - tint),
                    _ => Color::Other(components.to_vec()),
                }
            }
            ResolvedColorSpace::DeviceN { alternate, .. } => {
                // Best-effort: pass components to alternate space
                alternate.resolve_color(components)
            }
        }
    }
}

/// Default color space inferred from component count (fallback behavior).
pub fn default_color_space_from_components(n: usize) -> ResolvedColorSpace {
    match n {
        1 => ResolvedColorSpace::DeviceGray,
        3 => ResolvedColorSpace::DeviceRGB,
        4 => ResolvedColorSpace::DeviceCMYK,
        _ => ResolvedColorSpace::DeviceGray, // fallback
    }
}

/// Infer the alternate color space from the number of ICC profile components.
fn alternate_from_num_components(n: u32) -> ResolvedColorSpace {
    match n {
        1 => ResolvedColorSpace::DeviceGray,
        3 => ResolvedColorSpace::DeviceRGB,
        4 => ResolvedColorSpace::DeviceCMYK,
        _ => ResolvedColorSpace::DeviceRGB, // default fallback
    }
}

/// Resolve a color space name to a `ResolvedColorSpace`.
///
/// Handles both simple device color spaces (DeviceGray, DeviceRGB, DeviceCMYK)
/// and named color spaces looked up in the page Resources.
pub fn resolve_color_space_name(
    name: &str,
    doc: &lopdf::Document,
    resources: &lopdf::Dictionary,
) -> Option<ResolvedColorSpace> {
    match name {
        "DeviceGray" | "G" => Some(ResolvedColorSpace::DeviceGray),
        "DeviceRGB" | "RGB" => Some(ResolvedColorSpace::DeviceRGB),
        "DeviceCMYK" | "CMYK" => Some(ResolvedColorSpace::DeviceCMYK),
        _ => {
            // Look up in Resources /ColorSpace dictionary
            if let Ok(cs_dict) = resources.get(b"ColorSpace").and_then(|o| o.as_dict()) {
                if let Ok(cs_obj) = cs_dict.get(name.as_bytes()) {
                    return resolve_color_space_object(cs_obj, doc);
                }
            }
            None
        }
    }
}

/// Resolve a color space from a lopdf Object (name or array).
pub fn resolve_color_space_object(
    obj: &lopdf::Object,
    doc: &lopdf::Document,
) -> Option<ResolvedColorSpace> {
    match obj {
        lopdf::Object::Name(name) => {
            let name_str = String::from_utf8_lossy(name);
            match name_str.as_ref() {
                "DeviceGray" | "G" => Some(ResolvedColorSpace::DeviceGray),
                "DeviceRGB" | "RGB" => Some(ResolvedColorSpace::DeviceRGB),
                "DeviceCMYK" | "CMYK" => Some(ResolvedColorSpace::DeviceCMYK),
                _ => None,
            }
        }
        lopdf::Object::Array(arr) => resolve_color_space_array(arr, doc),
        lopdf::Object::Reference(id) => {
            if let Ok(resolved) = doc.get_object(*id) {
                resolve_color_space_object(resolved, doc)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Resolve a color space array like [/ICCBased stream_ref] or [/Indexed base hival lookup].
fn resolve_color_space_array(
    arr: &[lopdf::Object],
    doc: &lopdf::Document,
) -> Option<ResolvedColorSpace> {
    if arr.is_empty() {
        return None;
    }

    let cs_type = match &arr[0] {
        lopdf::Object::Name(n) => String::from_utf8_lossy(n).to_string(),
        _ => return None,
    };

    match cs_type.as_str() {
        "ICCBased" => resolve_icc_based(arr, doc),
        "Indexed" | "I" => resolve_indexed(arr, doc),
        "Separation" => resolve_separation(arr, doc),
        "DeviceN" => resolve_device_n(arr, doc),
        "DeviceGray" | "G" => Some(ResolvedColorSpace::DeviceGray),
        "DeviceRGB" | "RGB" => Some(ResolvedColorSpace::DeviceRGB),
        "DeviceCMYK" | "CMYK" => Some(ResolvedColorSpace::DeviceCMYK),
        _ => None,
    }
}

/// Resolve [/ICCBased stream_ref].
fn resolve_icc_based(arr: &[lopdf::Object], doc: &lopdf::Document) -> Option<ResolvedColorSpace> {
    if arr.len() < 2 {
        return None;
    }

    // Get the ICC profile stream
    let stream_obj = match &arr[1] {
        lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };

    let stream = match stream_obj {
        lopdf::Object::Stream(s) => s,
        _ => return None,
    };

    // Get /N (number of components)
    let num_components = stream
        .dict
        .get(b"N")
        .ok()
        .and_then(|o| match o {
            lopdf::Object::Integer(n) => Some(*n as u32),
            _ => None,
        })
        .unwrap_or(3); // default to 3 (RGB)

    // Try to get /Alternate color space
    let alternate = stream
        .dict
        .get(b"Alternate")
        .ok()
        .and_then(|o| resolve_color_space_object(o, doc))
        .unwrap_or_else(|| alternate_from_num_components(num_components));

    Some(ResolvedColorSpace::ICCBased {
        num_components,
        alternate: Box::new(alternate),
    })
}

/// Resolve [/Indexed base hival lookup].
fn resolve_indexed(arr: &[lopdf::Object], doc: &lopdf::Document) -> Option<ResolvedColorSpace> {
    if arr.len() < 4 {
        return None;
    }

    // Base color space
    let base = resolve_color_space_object(&arr[1], doc)
        .or_else(|| {
            // Try resolving as reference
            if let lopdf::Object::Reference(id) = &arr[1] {
                doc.get_object(*id)
                    .ok()
                    .and_then(|o| resolve_color_space_object(o, doc))
            } else {
                None
            }
        })
        .unwrap_or(ResolvedColorSpace::DeviceRGB);

    // hival (maximum valid index)
    let hival = match &arr[2] {
        lopdf::Object::Integer(n) => *n as u32,
        _ => return None,
    };

    // Lookup table - can be a string or a stream
    let lookup_table = match &arr[3] {
        lopdf::Object::String(bytes, _) => bytes.clone(),
        lopdf::Object::Reference(id) => {
            if let Ok(obj) = doc.get_object(*id) {
                match obj {
                    lopdf::Object::Stream(s) => s
                        .decompressed_content()
                        .unwrap_or_else(|_| s.content.clone()),
                    lopdf::Object::String(bytes, _) => bytes.clone(),
                    _ => return None,
                }
            } else {
                return None;
            }
        }
        lopdf::Object::Stream(s) => s
            .decompressed_content()
            .unwrap_or_else(|_| s.content.clone()),
        _ => return None,
    };

    Some(ResolvedColorSpace::Indexed {
        base: Box::new(base),
        hival,
        lookup_table,
    })
}

/// Resolve [/Separation name alternateSpace tintTransform].
fn resolve_separation(arr: &[lopdf::Object], doc: &lopdf::Document) -> Option<ResolvedColorSpace> {
    if arr.len() < 4 {
        return None;
    }

    // alternateSpace is at index 2
    let alternate =
        resolve_color_space_object(&arr[2], doc).unwrap_or(ResolvedColorSpace::DeviceCMYK);

    Some(ResolvedColorSpace::Separation {
        alternate: Box::new(alternate),
    })
}

/// Resolve [/DeviceN names alternateSpace tintTransform].
fn resolve_device_n(arr: &[lopdf::Object], doc: &lopdf::Document) -> Option<ResolvedColorSpace> {
    if arr.len() < 4 {
        return None;
    }

    // names is an array at index 1
    let num_components = match &arr[1] {
        lopdf::Object::Array(names) => names.len() as u32,
        _ => return None,
    };

    // alternateSpace is at index 2
    let alternate =
        resolve_color_space_object(&arr[2], doc).unwrap_or(ResolvedColorSpace::DeviceCMYK);

    Some(ResolvedColorSpace::DeviceN {
        num_components,
        alternate: Box::new(alternate),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Object, Stream, dictionary};

    // --- ResolvedColorSpace::resolve_color tests ---

    #[test]
    fn resolve_device_gray() {
        let cs = ResolvedColorSpace::DeviceGray;
        assert_eq!(cs.resolve_color(&[0.5]), Color::Gray(0.5));
    }

    #[test]
    fn resolve_device_rgb() {
        let cs = ResolvedColorSpace::DeviceRGB;
        assert_eq!(
            cs.resolve_color(&[0.1, 0.2, 0.3]),
            Color::Rgb(0.1, 0.2, 0.3)
        );
    }

    #[test]
    fn resolve_device_cmyk() {
        let cs = ResolvedColorSpace::DeviceCMYK;
        assert_eq!(
            cs.resolve_color(&[0.1, 0.2, 0.3, 0.4]),
            Color::Cmyk(0.1, 0.2, 0.3, 0.4)
        );
    }

    #[test]
    fn resolve_icc_based_3_components_as_rgb() {
        let cs = ResolvedColorSpace::ICCBased {
            num_components: 3,
            alternate: Box::new(ResolvedColorSpace::DeviceRGB),
        };
        assert_eq!(cs.num_components(), 3);
        assert_eq!(
            cs.resolve_color(&[0.5, 0.6, 0.7]),
            Color::Rgb(0.5, 0.6, 0.7)
        );
    }

    #[test]
    fn resolve_icc_based_1_component_as_gray() {
        let cs = ResolvedColorSpace::ICCBased {
            num_components: 1,
            alternate: Box::new(ResolvedColorSpace::DeviceGray),
        };
        assert_eq!(cs.num_components(), 1);
        assert_eq!(cs.resolve_color(&[0.3]), Color::Gray(0.3));
    }

    #[test]
    fn resolve_icc_based_4_components_as_cmyk() {
        let cs = ResolvedColorSpace::ICCBased {
            num_components: 4,
            alternate: Box::new(ResolvedColorSpace::DeviceCMYK),
        };
        assert_eq!(cs.num_components(), 4);
        assert_eq!(
            cs.resolve_color(&[0.1, 0.2, 0.3, 0.4]),
            Color::Cmyk(0.1, 0.2, 0.3, 0.4)
        );
    }

    #[test]
    fn resolve_indexed_lookup() {
        // Indexed with DeviceRGB base, 2 colors in palette
        let cs = ResolvedColorSpace::Indexed {
            base: Box::new(ResolvedColorSpace::DeviceRGB),
            hival: 1,
            lookup_table: vec![
                255, 0, 0, // index 0: red
                0, 255, 0, // index 1: green
            ],
        };
        assert_eq!(cs.num_components(), 1);

        // Look up index 0 → red
        let color = cs.resolve_color(&[0.0]);
        assert_eq!(color, Color::Rgb(1.0, 0.0, 0.0));

        // Look up index 1 → green
        let color = cs.resolve_color(&[1.0]);
        assert_eq!(color, Color::Rgb(0.0, 1.0, 0.0));
    }

    #[test]
    fn resolve_indexed_clamps_to_hival() {
        let cs = ResolvedColorSpace::Indexed {
            base: Box::new(ResolvedColorSpace::DeviceRGB),
            hival: 1,
            lookup_table: vec![255, 0, 0, 0, 0, 255],
        };
        // Index 5 should be clamped to hival=1
        let color = cs.resolve_color(&[5.0]);
        assert_eq!(color, Color::Rgb(0.0, 0.0, 1.0));
    }

    #[test]
    fn resolve_separation_with_cmyk_alternate() {
        let cs = ResolvedColorSpace::Separation {
            alternate: Box::new(ResolvedColorSpace::DeviceCMYK),
        };
        assert_eq!(cs.num_components(), 1);
        // Tint 1.0 → full color → K=0
        let color = cs.resolve_color(&[1.0]);
        assert_eq!(color, Color::Cmyk(0.0, 0.0, 0.0, 0.0));
        // Tint 0.0 → no color → K=1
        let color = cs.resolve_color(&[0.0]);
        assert_eq!(color, Color::Cmyk(0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn resolve_separation_with_rgb_alternate() {
        let cs = ResolvedColorSpace::Separation {
            alternate: Box::new(ResolvedColorSpace::DeviceRGB),
        };
        // Tint 0.5 → gray in RGB
        let color = cs.resolve_color(&[0.5]);
        assert_eq!(color, Color::Rgb(0.5, 0.5, 0.5));
    }

    #[test]
    fn resolve_device_n_with_alternate() {
        let cs = ResolvedColorSpace::DeviceN {
            num_components: 2,
            alternate: Box::new(ResolvedColorSpace::DeviceRGB),
        };
        assert_eq!(cs.num_components(), 2);
        // Components passed through to alternate
        let color = cs.resolve_color(&[0.3, 0.7, 0.5]);
        assert_eq!(color, Color::Rgb(0.3, 0.7, 0.5));
    }

    #[test]
    fn num_components_correct() {
        assert_eq!(ResolvedColorSpace::DeviceGray.num_components(), 1);
        assert_eq!(ResolvedColorSpace::DeviceRGB.num_components(), 3);
        assert_eq!(ResolvedColorSpace::DeviceCMYK.num_components(), 4);
    }

    // --- Color space name resolution tests ---

    #[test]
    fn resolve_name_device_gray() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = dictionary! {};
        assert!(matches!(
            resolve_color_space_name("DeviceGray", &doc, &resources),
            Some(ResolvedColorSpace::DeviceGray)
        ));
    }

    #[test]
    fn resolve_name_device_rgb() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = dictionary! {};
        assert!(matches!(
            resolve_color_space_name("DeviceRGB", &doc, &resources),
            Some(ResolvedColorSpace::DeviceRGB)
        ));
    }

    #[test]
    fn resolve_name_device_cmyk() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = dictionary! {};
        assert!(matches!(
            resolve_color_space_name("DeviceCMYK", &doc, &resources),
            Some(ResolvedColorSpace::DeviceCMYK)
        ));
    }

    #[test]
    fn resolve_name_unknown_returns_none() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = dictionary! {};
        assert!(resolve_color_space_name("UnknownCS", &doc, &resources).is_none());
    }

    // --- Color space object resolution tests ---

    #[test]
    fn resolve_icc_based_from_array() {
        let mut doc = lopdf::Document::with_version("1.5");

        // Create an ICC profile stream with /N=3
        let icc_stream = Stream::new(
            dictionary! {
                "N" => Object::Integer(3),
            },
            vec![0u8; 10], // dummy ICC profile data
        );
        let icc_id = doc.add_object(icc_stream);

        let arr = vec![
            Object::Name(b"ICCBased".to_vec()),
            Object::Reference(icc_id),
        ];

        let cs = resolve_color_space_array(&arr, &doc).unwrap();
        assert_eq!(cs.num_components(), 3);
        // Should resolve color as RGB via alternate
        assert_eq!(
            cs.resolve_color(&[0.5, 0.6, 0.7]),
            Color::Rgb(0.5, 0.6, 0.7)
        );
    }

    #[test]
    fn resolve_icc_based_with_alternate() {
        let mut doc = lopdf::Document::with_version("1.5");

        let icc_stream = Stream::new(
            dictionary! {
                "N" => Object::Integer(4),
                "Alternate" => Object::Name(b"DeviceCMYK".to_vec()),
            },
            vec![0u8; 10],
        );
        let icc_id = doc.add_object(icc_stream);

        let arr = vec![
            Object::Name(b"ICCBased".to_vec()),
            Object::Reference(icc_id),
        ];

        let cs = resolve_color_space_array(&arr, &doc).unwrap();
        assert_eq!(cs.num_components(), 4);
        assert_eq!(
            cs.resolve_color(&[0.1, 0.2, 0.3, 0.4]),
            Color::Cmyk(0.1, 0.2, 0.3, 0.4)
        );
    }

    #[test]
    fn resolve_indexed_from_array() {
        let doc = lopdf::Document::with_version("1.5");

        // [/Indexed /DeviceRGB 1 <FF0000 00FF00>]
        let arr = vec![
            Object::Name(b"Indexed".to_vec()),
            Object::Name(b"DeviceRGB".to_vec()),
            Object::Integer(1),
            Object::String(vec![255, 0, 0, 0, 255, 0], lopdf::StringFormat::Hexadecimal),
        ];

        let cs = resolve_color_space_array(&arr, &doc).unwrap();
        assert_eq!(cs.resolve_color(&[0.0]), Color::Rgb(1.0, 0.0, 0.0));
        assert_eq!(cs.resolve_color(&[1.0]), Color::Rgb(0.0, 1.0, 0.0));
    }

    #[test]
    fn resolve_separation_from_array() {
        let doc = lopdf::Document::with_version("1.5");

        // [/Separation /SpotColor /DeviceCMYK <tintTransform>]
        let arr = vec![
            Object::Name(b"Separation".to_vec()),
            Object::Name(b"SpotColor".to_vec()),
            Object::Name(b"DeviceCMYK".to_vec()),
            Object::Null, // tint transform (ignored in best-effort)
        ];

        let cs = resolve_color_space_array(&arr, &doc).unwrap();
        assert_eq!(cs.num_components(), 1);
    }

    #[test]
    fn resolve_device_n_from_array() {
        let doc = lopdf::Document::with_version("1.5");

        // [/DeviceN [/Cyan /Magenta] /DeviceCMYK <tintTransform>]
        let arr = vec![
            Object::Name(b"DeviceN".to_vec()),
            Object::Array(vec![
                Object::Name(b"Cyan".to_vec()),
                Object::Name(b"Magenta".to_vec()),
            ]),
            Object::Name(b"DeviceCMYK".to_vec()),
            Object::Null, // tint transform
        ];

        let cs = resolve_color_space_array(&arr, &doc).unwrap();
        assert_eq!(cs.num_components(), 2);
    }

    #[test]
    fn resolve_named_color_space_from_resources() {
        let mut doc = lopdf::Document::with_version("1.5");

        // Create an ICC profile stream
        let icc_stream = Stream::new(
            dictionary! {
                "N" => Object::Integer(3),
            },
            vec![0u8; 10],
        );
        let icc_id = doc.add_object(icc_stream);

        // Resources with ColorSpace dictionary
        let resources = dictionary! {
            "ColorSpace" => dictionary! {
                "CS1" => Object::Array(vec![
                    Object::Name(b"ICCBased".to_vec()),
                    Object::Reference(icc_id),
                ]),
            },
        };

        let cs = resolve_color_space_name("CS1", &doc, &resources).unwrap();
        assert_eq!(cs.num_components(), 3);
    }
}
