//! Ollama vision-model fallback for scanned/image-only PDF pages.
//!
//! When native extraction returns zero characters on a page that has non-trivial
//! visual content, this module falls back to a local Ollama vision model to OCR
//! the page. Works entirely offline — no API key, no per-page cost, no data
//! residency problem.
//!
//! # Feature Flag
//!
//! This module is only compiled when the `ollama-fallback` feature is enabled:
//!
//! ```toml
//! [dependencies]
//! pdfplumber = { version = "0.2", features = ["ollama-fallback"] }
//! ```
//!
//! # Usage
//!
//! ```no_run
//! # #[cfg(feature = "ollama-fallback")]
//! # {
//! use pdfplumber::ollama::OllamaFallback;
//! use pdfplumber::Pdf;
//!
//! let fallback = OllamaFallback::builder()
//!     .base_url("http://localhost:11434")
//!     .model("llava")   // or "moondream", "llava-phi3", any vision model
//!     .build();
//!
//! let bytes = std::fs::read("scanned.pdf").unwrap();
//! let pdf = Pdf::open_with_fallback(&bytes, None, fallback).unwrap();
//! // Pages with zero native chars now attempt Ollama extraction automatically
//! let chars = pdf.page(0).unwrap().chars();
//! # }
//! ```
//!
//! # Accuracy Note
//!
//! Bounding boxes for OCR-derived chars are approximate: we estimate positions
//! using page dimensions and average glyph metrics. They are suitable for
//! chunk retrieval but not for pixel-accurate coordinate extraction.
//!
//! # Ollama Setup
//!
//! ```bash
//! ollama serve            # start server (default http://localhost:11434)
//! ollama pull llava       # download vision model (~4GB)
//! ```

use crate::PdfError;
use pdfplumber_core::{BBox, Char, TextDirection};
use reqwest::blocking::Client;
use serde_json::{Value, json};

/// OCR prompt sent to the vision model.
///
/// Instructs the model to return plain text only, preserving line breaks.
const OCR_PROMPT: &str = "Extract all text from this document page. Preserve line breaks exactly as they appear. \
     Return plain text only — no markdown, no commentary, no explanation.";

/// Configuration for the Ollama vision-model fallback.
///
/// Construct via [`OllamaFallbackBuilder`] returned by [`OllamaFallback::builder()`].
#[derive(Debug, Clone)]
pub struct OllamaFallback {
    /// Ollama server base URL (default: `http://localhost:11434`).
    pub base_url: String,
    /// Vision model to use (default: `llava`).
    pub model: String,
    /// HTTP request timeout in seconds (default: 120).
    pub timeout_secs: u64,
    /// Minimum number of rects/images needed to trigger fallback (default: 1).
    /// Pages with fewer visual elements than this are assumed empty and skipped.
    pub min_visual_elements: usize,
}

impl Default for OllamaFallback {
    fn default() -> Self {
        OllamaFallback {
            base_url: "http://localhost:11434".to_string(),
            model: "llava".to_string(),
            timeout_secs: 120,
            min_visual_elements: 1,
        }
    }
}

impl OllamaFallback {
    /// Create a new [`OllamaFallbackBuilder`] for configuring the fallback.
    pub fn builder() -> OllamaFallbackBuilder {
        OllamaFallbackBuilder::default()
    }

    /// Create with default settings (localhost:11434, model=llava).
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        OllamaFallback {
            base_url: base_url.into(),
            model: model.into(),
            ..Default::default()
        }
    }

    /// OCR a single page image (PNG bytes) via Ollama.
    ///
    /// Returns the extracted text, or an error if the request fails.
    ///
    /// # Arguments
    ///
    /// * `png_bytes` — Raw PNG image data of the page.
    /// * `page_width` — Page width in points (used for bbox estimation).
    /// * `page_height` — Page height in points (used for bbox estimation).
    pub fn ocr_page(
        &self,
        png_bytes: &[u8],
        page_width: f64,
        page_height: f64,
    ) -> Result<Vec<Char>, OllamaError> {
        let image_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png_bytes);

        let payload = json!({
            "model": self.model,
            "prompt": OCR_PROMPT,
            "images": [image_b64],
            "stream": false,
        });

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(self.timeout_secs))
            .build()
            .map_err(|e| OllamaError::HttpError(e.to_string()))?;

        let url = format!("{}/api/generate", self.base_url);
        let response = client
            .post(&url)
            .json(&payload)
            .send()
            .map_err(|e| OllamaError::HttpError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(OllamaError::HttpError(format!(
                "Ollama returned HTTP {}",
                response.status()
            )));
        }

        let body: Value = response
            .json()
            .map_err(|e| OllamaError::ParseError(e.to_string()))?;

        let text = body["response"]
            .as_str()
            .ok_or_else(|| {
                OllamaError::ParseError("missing 'response' field in Ollama reply".to_string())
            })?
            .to_string();

        Ok(text_to_chars(&text, page_width, page_height))
    }

    /// True if the Ollama server is reachable and has the configured model loaded.
    ///
    /// Makes a lightweight `GET /api/tags` call to verify availability.
    /// Use this to gate fallback usage at startup.
    pub fn is_available(&self) -> bool {
        let client = match Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };

        let url = format!("{}/api/tags", self.base_url);
        match client.get(&url).send() {
            Ok(r) if r.status().is_success() => {
                // Check if our model is in the list
                if let Ok(body) = r.json::<Value>() {
                    if let Some(models) = body["models"].as_array() {
                        return models.iter().any(|m| {
                            m["name"]
                                .as_str()
                                .map(|n| n.starts_with(&self.model))
                                .unwrap_or(false)
                        });
                    }
                }
                // If we can't parse models, server is at least alive
                true
            }
            _ => false,
        }
    }
}

/// Builder for [`OllamaFallback`].
#[derive(Debug, Default)]
pub struct OllamaFallbackBuilder {
    base_url: Option<String>,
    model: Option<String>,
    timeout_secs: Option<u64>,
    min_visual_elements: Option<usize>,
}

impl OllamaFallbackBuilder {
    /// Set the Ollama server base URL.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the vision model name.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the HTTP request timeout in seconds.
    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Set the minimum visual element count to trigger fallback.
    pub fn min_visual_elements(mut self, n: usize) -> Self {
        self.min_visual_elements = Some(n);
        self
    }

    /// Build the [`OllamaFallback`].
    pub fn build(self) -> OllamaFallback {
        let default = OllamaFallback::default();
        OllamaFallback {
            base_url: self.base_url.unwrap_or(default.base_url),
            model: self.model.unwrap_or(default.model),
            timeout_secs: self.timeout_secs.unwrap_or(default.timeout_secs),
            min_visual_elements: self
                .min_visual_elements
                .unwrap_or(default.min_visual_elements),
        }
    }
}

/// Error type for Ollama fallback failures.
#[derive(Debug)]
pub enum OllamaError {
    /// HTTP request failed.
    HttpError(String),
    /// Response could not be parsed.
    ParseError(String),
    /// Model not available.
    ModelNotAvailable(String),
}

impl std::fmt::Display for OllamaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OllamaError::HttpError(e) => write!(f, "Ollama HTTP error: {e}"),
            OllamaError::ParseError(e) => write!(f, "Ollama parse error: {e}"),
            OllamaError::ModelNotAvailable(m) => write!(f, "Ollama model not available: {m}"),
        }
    }
}

impl std::error::Error for OllamaError {}

impl From<OllamaError> for PdfError {
    fn from(e: OllamaError) -> Self {
        PdfError::Other(e.to_string())
    }
}

/// Convert raw OCR text into approximate [`Char`] objects with estimated bboxes.
///
/// Algorithm:
/// 1. Split text into lines.
/// 2. For each line, compute a baseline y from line index and estimated line height.
/// 3. For each character in the line, compute x from char index and estimated char width.
/// 4. Produce one [`Char`] per non-whitespace glyph, space chars get a narrow bbox.
///
/// Bbox precision is intentionally approximate — we're in fallback mode.
/// Char widths use a rough average of 0.55 × font_size, line heights 1.3 × font_size.
pub fn text_to_chars(text: &str, page_width: f64, page_height: f64) -> Vec<Char> {
    if text.is_empty() {
        return Vec::new();
    }

    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    // Estimate font size from page dimensions (assumes body text at ~1% of page height)
    let font_size = (page_height * 0.012).clamp(8.0, 18.0);
    let line_height = font_size * 1.3;
    let char_width_avg = font_size * 0.55;

    // Left margin estimate
    let left_margin = page_width * 0.05;

    // Top margin estimate (start of text area)
    let top_margin = page_height * 0.05;

    let mut chars = Vec::new();
    let mut doctop = 0.0_f64;

    for (line_idx, line) in lines.iter().enumerate() {
        let line_top = top_margin + line_idx as f64 * line_height;
        let line_bottom = line_top + font_size;
        doctop = doctop.max(line_top);

        let mut x = left_margin;
        for ch in line.chars() {
            let w = if ch == ' ' || ch == '\t' {
                char_width_avg * 0.4
            } else {
                // Use a slightly variable width based on char category
                // (wide chars like 'M', 'W' vs narrow like 'i', 'l')
                match ch {
                    'M' | 'W' | 'm' | 'w' => char_width_avg * 1.2,
                    'i' | 'l' | 'I' | 'j' | '1' | '!' | '|' => char_width_avg * 0.4,
                    _ => char_width_avg,
                }
            };

            let bbox = BBox::new(x, line_top, x + w, line_bottom);

            chars.push(Char {
                text: ch.to_string(),
                bbox,
                fontname: "OllamaOCR".to_string(),
                size: font_size,
                doctop: line_top,
                upright: true,
                direction: TextDirection::Ltr,
                stroking_color: None,
                non_stroking_color: None,
                ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                char_code: ch as u32,
                mcid: None,
                tag: None,
            });

            x += w;

            // If we've gone past the page width, wrap (shouldn't happen with
            // reasonable OCR output, but handles runaway long lines)
            if x > page_width * 0.95 {
                x = left_margin;
            }
        }
    }

    chars
}

/// Detect whether a page is likely a scanned/image-only page.
///
/// Returns `true` if the page has no text chars AND has at least one image
/// or a significant number of path/rect elements.
///
/// This is the signal used by [`Pdf::open_with_fallback`] to decide whether
/// to invoke the Ollama OCR path.
pub fn is_scanned_page(
    char_count: usize,
    image_count: usize,
    rect_count: usize,
    min_visual_elements: usize,
) -> bool {
    char_count == 0 && (image_count + rect_count) >= min_visual_elements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_to_chars_empty_text() {
        let chars = text_to_chars("", 612.0, 792.0);
        assert!(chars.is_empty());
    }

    #[test]
    fn text_to_chars_single_word() {
        let chars = text_to_chars("Hello", 612.0, 792.0);
        assert_eq!(chars.len(), 5);
        // All chars should be on the first line (same top)
        let top = chars[0].bbox.top;
        for ch in &chars {
            assert!((ch.bbox.top - top).abs() < 1e-6);
        }
        // x-positions should be increasing
        for i in 1..chars.len() {
            assert!(
                chars[i].bbox.x0 > chars[i - 1].bbox.x0,
                "x should increase: {} > {}",
                chars[i].bbox.x0,
                chars[i - 1].bbox.x0
            );
        }
    }

    #[test]
    fn text_to_chars_two_lines() {
        let chars = text_to_chars("Line one\nLine two", 612.0, 792.0);
        // Should have chars for both lines
        assert!(chars.len() >= 14); // "Line one" = 8 + space, "Line two" = 8
        // Second line chars should have a higher top value than first line
        let line1_top = chars[0].bbox.top;
        let line2_chars: Vec<_> = chars
            .iter()
            .filter(|c| c.bbox.top > line1_top + 1.0)
            .collect();
        assert!(
            !line2_chars.is_empty(),
            "should have chars on second line with higher top"
        );
    }

    #[test]
    fn text_to_chars_all_have_valid_bboxes() {
        let chars = text_to_chars("The quick brown fox jumps over the lazy dog.", 612.0, 792.0);
        for ch in &chars {
            assert!(ch.bbox.x0 < ch.bbox.x1, "x0 < x1");
            assert!(ch.bbox.top < ch.bbox.bottom, "top < bottom");
            assert!(ch.bbox.x0 >= 0.0, "x0 >= 0");
            assert!(ch.bbox.top >= 0.0, "top >= 0");
        }
    }

    #[test]
    fn text_to_chars_space_chars_are_included() {
        // Spaces should produce chars (important for word reconstruction)
        let chars = text_to_chars("Hello World", 612.0, 792.0);
        assert_eq!(chars.len(), 11); // H-e-l-l-o-' '-W-o-r-l-d
        let space = &chars[5];
        assert_eq!(space.text, " ");
    }

    #[test]
    fn text_to_chars_respects_page_bounds() {
        // Very long line should not produce chars past page_width
        let long_line: String = "a".repeat(200);
        let chars = text_to_chars(&long_line, 612.0, 792.0);
        for ch in &chars {
            assert!(
                ch.bbox.x1 <= 612.0 * 1.05,
                "char should not exceed page width"
            );
        }
    }

    #[test]
    fn text_to_chars_direction_is_ltr() {
        let chars = text_to_chars("abc", 612.0, 792.0);
        for ch in &chars {
            assert_eq!(ch.direction, TextDirection::Ltr);
            assert!(ch.upright);
        }
    }

    #[test]
    fn text_to_chars_fontname_is_ollama_ocr() {
        let chars = text_to_chars("abc", 612.0, 792.0);
        for ch in &chars {
            assert_eq!(ch.fontname, "OllamaOCR");
        }
    }

    #[test]
    fn is_scanned_page_true_when_no_chars_and_has_images() {
        assert!(is_scanned_page(0, 1, 0, 1));
        assert!(is_scanned_page(0, 0, 5, 1));
        assert!(is_scanned_page(0, 2, 3, 1));
    }

    #[test]
    fn is_scanned_page_false_when_has_chars() {
        assert!(!is_scanned_page(10, 1, 5, 1));
        assert!(!is_scanned_page(1, 0, 0, 1));
    }

    #[test]
    fn is_scanned_page_false_when_no_visual_elements() {
        assert!(!is_scanned_page(0, 0, 0, 1));
    }

    #[test]
    fn is_scanned_page_respects_min_visual_elements() {
        assert!(!is_scanned_page(0, 1, 0, 2)); // only 1 element, need 2
        assert!(is_scanned_page(0, 1, 1, 2)); // 2 elements, need 2
    }

    #[test]
    fn ollama_fallback_builder_defaults() {
        let fb = OllamaFallback::builder().build();
        assert_eq!(fb.base_url, "http://localhost:11434");
        assert_eq!(fb.model, "llava");
        assert_eq!(fb.timeout_secs, 120);
        assert_eq!(fb.min_visual_elements, 1);
    }

    #[test]
    fn ollama_fallback_builder_overrides() {
        let fb = OllamaFallback::builder()
            .base_url("http://192.168.1.10:11434")
            .model("moondream")
            .timeout_secs(60)
            .min_visual_elements(3)
            .build();
        assert_eq!(fb.base_url, "http://192.168.1.10:11434");
        assert_eq!(fb.model, "moondream");
        assert_eq!(fb.timeout_secs, 60);
        assert_eq!(fb.min_visual_elements, 3);
    }

    #[test]
    fn ollama_fallback_new() {
        let fb = OllamaFallback::new("http://localhost:11434", "llava-phi3");
        assert_eq!(fb.model, "llava-phi3");
    }

    #[test]
    fn ollama_error_display() {
        let e = OllamaError::HttpError("timeout".to_string());
        assert!(e.to_string().contains("timeout"));
        let e2 = OllamaError::ModelNotAvailable("llava".to_string());
        assert!(e2.to_string().contains("llava"));
    }
}
