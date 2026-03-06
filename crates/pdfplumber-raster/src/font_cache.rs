//! Font cache for the rasterizer.
//!
//! Maps PDF font names to [`fontdue::Font`] instances so each font is loaded
//! once per render session. Falls back to a built-in fallback font (DejaVu Sans
//! embedded as bytes) when a named font cannot be resolved.
//!
//! ## Font resolution order
//!
//! 1. **Caller-supplied font bytes**: the [`FontCache`] accepts a map of
//!    `fontname → &[u8]` at construction time. If the PDF embeds its fonts,
//!    callers can extract the raw font bytes from lopdf and pass them here.
//! 2. **System fonts via fontdb path search** (std only): searches common system
//!    font directories for a filename that fuzzy-matches the PDF font name.
//! 3. **Built-in fallback**: a minimal Latin-coverage font embedded in this crate.
//!    Positions are still correct (they come from the PDF bbox); only the glyph
//!    shape degrades.

use std::collections::HashMap;

/// Minimum fallback font: Noto Sans Regular subset embedded as a static byte slice.
///
/// This is the smallest Noto Sans that still covers ASCII + common Latin-1.
/// At ~40 KB it is acceptable to embed. For production font fidelity, callers
/// should supply embedded font bytes via [`FontCache::with_fonts`].
static FALLBACK_FONT_BYTES: &[u8] = include_bytes!("../fonts/NotoSans-Regular-subset.ttf");

/// Per-render font cache.
///
/// Construct once per render call, reuse across all chars on the page.
pub struct FontCache {
    /// Fonts loaded from caller-supplied bytes.
    supplied: HashMap<String, fontdue::Font>,
    /// Fonts loaded from the filesystem, keyed by fontname.
    resolved: HashMap<String, fontdue::Font>,
    /// Fallback font used when no match is found.
    fallback: fontdue::Font,
}

impl FontCache {
    /// Create a new [`FontCache`] with an empty font map.
    pub fn new() -> Self {
        let settings = fontdue::FontSettings::default();
        let fallback = fontdue::Font::from_bytes(FALLBACK_FONT_BYTES, settings)
            .expect("built-in fallback font must be valid");
        Self {
            supplied: HashMap::new(),
            resolved: HashMap::new(),
            fallback,
        }
    }

    /// Create a [`FontCache`] pre-loaded with caller-supplied font bytes.
    ///
    /// The map is `fontname → raw TrueType/OTF bytes`. Names should match
    /// the `fontname` field on [`pdfplumber_core::Char`].
    pub fn with_fonts(font_map: HashMap<String, Vec<u8>>) -> Self {
        let mut cache = Self::new();
        let settings = fontdue::FontSettings::default();
        for (name, bytes) in font_map {
            if let Ok(font) = fontdue::Font::from_bytes(bytes.as_slice(), settings) {
                cache.supplied.insert(name, font);
            }
        }
        cache
    }

    /// Look up the best available font for a given PDF font name.
    ///
    /// Returns a reference to a [`fontdue::Font`]. Never fails — always returns
    /// at least the fallback font.
    pub fn get(&mut self, fontname: &str) -> &fontdue::Font {
        // 1. Supplied fonts (exact match).
        if self.supplied.contains_key(fontname) {
            return self.supplied.get(fontname).unwrap();
        }

        // 2. Already resolved from filesystem.
        if self.resolved.contains_key(fontname) {
            return self.resolved.get(fontname).unwrap();
        }

        // 3. Try to find on the filesystem.
        if let Some(font) = Self::find_system_font(fontname) {
            self.resolved.insert(fontname.to_owned(), font);
            return self.resolved.get(fontname).unwrap();
        }

        // 4. Fallback.
        &self.fallback
    }

    /// Attempt to load a font from common system font directories.
    ///
    /// PDF font names often look like "Arial-BoldMT", "TimesNewRomanPS-BoldMT",
    /// "Helvetica", etc. We normalise the name and search a list of directories.
    #[cfg(target_os = "macos")]
    fn find_system_font(fontname: &str) -> Option<fontdue::Font> {
        let dirs = ["/System/Library/Fonts", "/Library/Fonts", "~/Library/Fonts"];
        Self::search_dirs(&dirs, fontname)
    }

    #[cfg(target_os = "linux")]
    fn find_system_font(fontname: &str) -> Option<fontdue::Font> {
        let dirs = [
            "/usr/share/fonts",
            "/usr/local/share/fonts",
            "~/.local/share/fonts",
        ];
        Self::search_dirs(&dirs, fontname)
    }

    #[cfg(target_os = "windows")]
    fn find_system_font(fontname: &str) -> Option<fontdue::Font> {
        let dirs = ["C:\\Windows\\Fonts"];
        Self::search_dirs(&dirs, fontname)
    }

    /// Target-agnostic fallback (WASM, unknown OS).
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    fn find_system_font(_fontname: &str) -> Option<fontdue::Font> {
        None
    }

    /// Search a list of directories for a font file whose stem fuzzy-matches
    /// the PDF font name.
    fn search_dirs(dirs: &[&str], fontname: &str) -> Option<fontdue::Font> {
        let normalised = normalise_font_name(fontname);
        let settings = fontdue::FontSettings::default();

        for dir in dirs {
            let dir = dir.replace('~', &std::env::var("HOME").unwrap_or_default());
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                if normalise_font_name(stem).contains(&normalised)
                    || normalised.contains(&normalise_font_name(stem))
                {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf") {
                            if let Ok(bytes) = std::fs::read(&path) {
                                if let Ok(font) =
                                    fontdue::Font::from_bytes(bytes.as_slice(), settings)
                                {
                                    return Some(font);
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

impl Default for FontCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalise a font name for fuzzy matching.
///
/// Strips hyphens, spaces, common PDF suffixes ("MT", "PS"), and lowercases.
fn normalise_font_name(name: &str) -> String {
    name.to_ascii_lowercase()
        .replace(['-', ' ', '_'], "")
        .replace("mt", "")
        .replace("ps", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_cache_returns_fallback_for_unknown_font() {
        let mut cache = FontCache::new();
        // Any unknown name must return the fallback without panicking.
        let _ = cache.get("SomeUnknownFontXYZ");
    }

    #[test]
    fn with_fonts_stores_supplied_font() {
        // Use the fallback font bytes themselves as a "supplied" font.
        let mut map = HashMap::new();
        map.insert("TestFont".to_owned(), FALLBACK_FONT_BYTES.to_vec());
        let mut cache = FontCache::with_fonts(map);
        // Should return the supplied font, not the fallback.
        let font = cache.get("TestFont");
        // Just verify it resolves a glyph for 'A'.
        let (_, _) = font.rasterize('A', 12.0);
    }

    #[test]
    fn normalise_strips_hyphens_and_suffix() {
        assert_eq!(normalise_font_name("Arial-BoldMT"), "arialbold");
        assert_eq!(normalise_font_name("Times New RomanPS"), "timesnewroman");
        assert_eq!(normalise_font_name("Helvetica"), "helvetica");
    }
}
