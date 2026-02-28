//! Header/footer detection and page region classification.
//!
//! Provides cross-page analysis to detect repeating headers and footers
//! by comparing candidate regions across pages with fuzzy matching.

use crate::geometry::BBox;

/// Configuration for page region detection.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageRegionOptions {
    /// Fraction of page height to scan for headers (from top). Default: 0.1 (10%).
    pub header_margin: f64,
    /// Fraction of page height to scan for footers (from bottom). Default: 0.1 (10%).
    pub footer_margin: f64,
    /// Minimum number of pages required for detection. Default: 3.
    pub min_pages: usize,
}

impl Default for PageRegionOptions {
    fn default() -> Self {
        Self {
            header_margin: 0.1,
            footer_margin: 0.1,
            min_pages: 3,
        }
    }
}

/// Detected regions for a single page.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageRegions {
    /// Header region, if a repeating header was detected.
    pub header: Option<BBox>,
    /// Footer region, if a repeating footer was detected.
    pub footer: Option<BBox>,
    /// Body region (the area between header and footer).
    pub body: BBox,
}

/// Mask variable elements in text for fuzzy comparison.
///
/// Replaces sequences of digits with `#` and normalizes whitespace.
/// This allows detecting repeating text even when page numbers, dates,
/// or other variable elements change across pages.
pub fn mask_variable_elements(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_digit_run = false;

    for ch in text.chars() {
        if ch.is_ascii_digit() {
            if !in_digit_run {
                result.push('#');
                in_digit_run = true;
            }
            // Skip additional digits in the run
        } else {
            in_digit_run = false;
            result.push(ch);
        }
    }

    result
}

/// Detect repeating headers and footers across multiple pages.
///
/// Takes a list of (header_text, footer_text, page_width, page_height) tuples
/// for each page, along with options. Returns a `PageRegions` for each page.
///
/// The algorithm:
/// 1. For each page, compute the header/footer candidate region BBox
/// 2. Mask variable elements (digits → `#`) in candidate text
/// 3. Count how many pages share the same masked text
/// 4. If the same masked text appears on at least `min_pages` pages,
///    mark that region as a header/footer
/// 5. Handle odd/even page variation by checking both uniform and alternating patterns
pub fn detect_page_regions(
    page_data: &[(String, String, f64, f64)],
    options: &PageRegionOptions,
) -> Vec<PageRegions> {
    let num_pages = page_data.len();

    if num_pages < options.min_pages {
        // Not enough pages for detection — return full page as body
        return page_data
            .iter()
            .map(|(_, _, width, height)| PageRegions {
                header: None,
                footer: None,
                body: BBox::new(0.0, 0.0, *width, *height),
            })
            .collect();
    }

    // Collect masked header/footer texts
    let masked_headers: Vec<String> = page_data
        .iter()
        .map(|(h, _, _, _)| mask_variable_elements(h.trim()))
        .collect();
    let masked_footers: Vec<String> = page_data
        .iter()
        .map(|(_, f, _, _)| mask_variable_elements(f.trim()))
        .collect();

    // Detect repeating headers
    let has_header = detect_repeating_text(&masked_headers, options.min_pages);

    // Detect repeating footers
    let has_footer = detect_repeating_text(&masked_footers, options.min_pages);

    // Build PageRegions for each page
    page_data
        .iter()
        .enumerate()
        .map(|(i, (_, _, width, height))| {
            let header_height = height * options.header_margin;
            let footer_height = height * options.footer_margin;

            let header = if has_header[i] && !page_data[i].0.trim().is_empty() {
                Some(BBox::new(0.0, 0.0, *width, header_height))
            } else {
                None
            };

            let footer = if has_footer[i] && !page_data[i].1.trim().is_empty() {
                Some(BBox::new(0.0, height - footer_height, *width, *height))
            } else {
                None
            };

            let body_top = if header.is_some() { header_height } else { 0.0 };
            let body_bottom = if footer.is_some() {
                height - footer_height
            } else {
                *height
            };

            PageRegions {
                header,
                footer,
                body: BBox::new(0.0, body_top, *width, body_bottom),
            }
        })
        .collect()
}

/// Detect which pages have repeating text that appears on enough pages.
///
/// Returns a boolean for each page indicating whether it participates in
/// a repeating pattern. Handles both uniform repetition (same text on all pages)
/// and odd/even alternation (different text on odd vs even pages).
fn detect_repeating_text(masked_texts: &[String], min_pages: usize) -> Vec<bool> {
    let n = masked_texts.len();
    let mut is_repeating = vec![false; n];

    // Count occurrences of each non-empty masked text
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for text in masked_texts {
        if !text.is_empty() {
            *counts.entry(text.as_str()).or_insert(0) += 1;
        }
    }

    // A text is "repeating" if it appears on >= min_pages pages
    for (i, text) in masked_texts.iter().enumerate() {
        if !text.is_empty() {
            if let Some(&count) = counts.get(text.as_str()) {
                if count >= min_pages {
                    is_repeating[i] = true;
                }
            }
        }
    }

    // Also check odd/even alternation pattern:
    // If odd pages share text A and even pages share text B,
    // and each appears on >= min_pages/2 pages, mark both as repeating
    if !is_repeating.iter().any(|&r| r) {
        let odd_texts: Vec<&str> = masked_texts
            .iter()
            .enumerate()
            .filter(|(i, t)| i % 2 == 0 && !t.is_empty()) // 0-indexed, so "page 1" is index 0
            .map(|(_, t)| t.as_str())
            .collect();
        let even_texts: Vec<&str> = masked_texts
            .iter()
            .enumerate()
            .filter(|(i, t)| i % 2 == 1 && !t.is_empty())
            .map(|(_, t)| t.as_str())
            .collect();

        let min_alt = min_pages.div_ceil(2);

        let odd_repeating = if !odd_texts.is_empty() {
            let first = odd_texts[0];
            odd_texts.iter().filter(|&&t| t == first).count() >= min_alt
        } else {
            false
        };

        let even_repeating = if !even_texts.is_empty() {
            let first = even_texts[0];
            even_texts.iter().filter(|&&t| t == first).count() >= min_alt
        } else {
            false
        };

        if odd_repeating && even_repeating {
            for (i, text) in masked_texts.iter().enumerate() {
                if !text.is_empty() {
                    is_repeating[i] = true;
                }
            }
        }
    }

    is_repeating
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- mask_variable_elements tests ---

    #[test]
    fn mask_digits_replaced_with_hash() {
        assert_eq!(mask_variable_elements("Page 1"), "Page #");
        assert_eq!(mask_variable_elements("Page 42"), "Page #");
        assert_eq!(mask_variable_elements("Page 123"), "Page #");
    }

    #[test]
    fn mask_multiple_digit_groups() {
        assert_eq!(mask_variable_elements("2024-01-15"), "#-#-#");
    }

    #[test]
    fn mask_no_digits_unchanged() {
        assert_eq!(mask_variable_elements("Chapter One"), "Chapter One");
    }

    #[test]
    fn mask_empty_string() {
        assert_eq!(mask_variable_elements(""), "");
    }

    #[test]
    fn mask_only_digits() {
        assert_eq!(mask_variable_elements("12345"), "#");
    }

    // --- PageRegionOptions tests ---

    #[test]
    fn default_options() {
        let opts = PageRegionOptions::default();
        assert_eq!(opts.header_margin, 0.1);
        assert_eq!(opts.footer_margin, 0.1);
        assert_eq!(opts.min_pages, 3);
    }

    #[test]
    fn custom_options() {
        let opts = PageRegionOptions {
            header_margin: 0.15,
            footer_margin: 0.05,
            min_pages: 5,
        };
        assert_eq!(opts.header_margin, 0.15);
        assert_eq!(opts.footer_margin, 0.05);
        assert_eq!(opts.min_pages, 5);
    }

    // --- detect_page_regions tests ---

    #[test]
    fn repeating_header_detected_across_pages() {
        let page_data: Vec<(String, String, f64, f64)> = (0..5)
            .map(|i| {
                (
                    "Company Report".to_string(), // same header on all pages
                    format!("Page {}", i + 1),    // variable footer
                    612.0,
                    792.0,
                )
            })
            .collect();

        let options = PageRegionOptions::default();
        let regions = detect_page_regions(&page_data, &options);

        assert_eq!(regions.len(), 5);
        for region in &regions {
            assert!(region.header.is_some(), "header should be detected");
            let header = region.header.unwrap();
            assert_eq!(header.top, 0.0);
            assert!((header.bottom - 79.2).abs() < 0.1); // 10% of 792
        }
    }

    #[test]
    fn page_number_in_footer_detected() {
        let page_data: Vec<(String, String, f64, f64)> = (0..5)
            .map(|i| {
                (
                    "".to_string(),            // no header
                    format!("Page {}", i + 1), // "Page 1", "Page 2", ...
                    612.0,
                    792.0,
                )
            })
            .collect();

        let options = PageRegionOptions::default();
        let regions = detect_page_regions(&page_data, &options);

        assert_eq!(regions.len(), 5);
        for region in &regions {
            assert!(region.header.is_none(), "no header should be detected");
            assert!(region.footer.is_some(), "footer should be detected");
            let footer = region.footer.unwrap();
            assert!((footer.top - (792.0 - 79.2)).abs() < 0.1); // bottom 10%
        }
    }

    #[test]
    fn no_false_positives_on_single_page() {
        let page_data = vec![("Header".to_string(), "Footer".to_string(), 612.0, 792.0)];

        let options = PageRegionOptions::default();
        let regions = detect_page_regions(&page_data, &options);

        assert_eq!(regions.len(), 1);
        assert!(regions[0].header.is_none());
        assert!(regions[0].footer.is_none());
        // Body should be the full page
        assert_eq!(regions[0].body, BBox::new(0.0, 0.0, 612.0, 792.0));
    }

    #[test]
    fn no_false_positives_on_two_pages() {
        let page_data = vec![
            ("Header".to_string(), "Footer".to_string(), 612.0, 792.0),
            ("Header".to_string(), "Footer".to_string(), 612.0, 792.0),
        ];

        let options = PageRegionOptions::default();
        let regions = detect_page_regions(&page_data, &options);

        assert_eq!(regions.len(), 2);
        // min_pages is 3, so 2 pages shouldn't trigger detection
        assert!(regions[0].header.is_none());
        assert!(regions[0].footer.is_none());
    }

    #[test]
    fn both_header_and_footer_detected() {
        let page_data: Vec<(String, String, f64, f64)> = (0..4)
            .map(|i| {
                (
                    "Annual Report 2024".to_string(),
                    format!("- {} -", i + 1),
                    612.0,
                    792.0,
                )
            })
            .collect();

        let options = PageRegionOptions {
            min_pages: 3,
            ..PageRegionOptions::default()
        };
        let regions = detect_page_regions(&page_data, &options);

        assert_eq!(regions.len(), 4);
        for region in &regions {
            assert!(region.header.is_some());
            assert!(region.footer.is_some());
            // Body should be between header and footer
            let header = region.header.unwrap();
            let footer = region.footer.unwrap();
            assert!((region.body.top - header.bottom).abs() < 0.1);
            assert!((region.body.bottom - footer.top).abs() < 0.1);
        }
    }

    #[test]
    fn empty_header_text_not_detected() {
        let page_data: Vec<(String, String, f64, f64)> = (0..5)
            .map(|_| {
                (
                    "".to_string(), // empty header
                    "".to_string(), // empty footer
                    612.0,
                    792.0,
                )
            })
            .collect();

        let options = PageRegionOptions::default();
        let regions = detect_page_regions(&page_data, &options);

        for region in &regions {
            assert!(region.header.is_none());
            assert!(region.footer.is_none());
            assert_eq!(region.body, BBox::new(0.0, 0.0, 612.0, 792.0));
        }
    }

    #[test]
    fn different_text_on_each_page_not_detected() {
        // Use truly unique text (no digits that would be masked to the same value)
        let headers = ["Alpha", "Bravo", "Charlie", "Delta", "Echo"];
        let footers = ["Foxtrot", "Golf", "Hotel", "India", "Juliet"];
        let page_data: Vec<(String, String, f64, f64)> = (0..5)
            .map(|i| (headers[i].to_string(), footers[i].to_string(), 612.0, 792.0))
            .collect();

        let options = PageRegionOptions::default();
        let regions = detect_page_regions(&page_data, &options);

        // Each header/footer is unique, so no detection
        for region in &regions {
            assert!(region.header.is_none());
            assert!(region.footer.is_none());
        }
    }

    #[test]
    fn odd_even_page_headers_detected() {
        let page_data: Vec<(String, String, f64, f64)> = (0..6)
            .map(|i| {
                let header = if i % 2 == 0 {
                    "Left Header".to_string()
                } else {
                    "Right Header".to_string()
                };
                (header, "".to_string(), 612.0, 792.0)
            })
            .collect();

        let options = PageRegionOptions::default();
        let regions = detect_page_regions(&page_data, &options);

        for region in &regions {
            assert!(
                region.header.is_some(),
                "odd/even headers should be detected"
            );
        }
    }

    #[test]
    fn body_region_excludes_header_and_footer() {
        let page_data: Vec<(String, String, f64, f64)> = (0..4)
            .map(|i| {
                (
                    "Header Text".to_string(),
                    format!("Page {}", i + 1),
                    612.0,
                    792.0,
                )
            })
            .collect();

        let options = PageRegionOptions {
            header_margin: 0.1,
            footer_margin: 0.15,
            min_pages: 3,
        };
        let regions = detect_page_regions(&page_data, &options);

        for region in &regions {
            let body = &region.body;
            // Body top = 10% of 792 = 79.2
            assert!((body.top - 79.2).abs() < 0.1);
            // Body bottom = 792 - 15% of 792 = 792 - 118.8 = 673.2
            assert!((body.bottom - 673.2).abs() < 0.1);
        }
    }

    #[test]
    fn custom_min_pages_threshold() {
        // 3 pages with same header, but min_pages=4 — should not detect
        let page_data: Vec<(String, String, f64, f64)> = (0..3)
            .map(|_| ("Same Header".to_string(), "".to_string(), 612.0, 792.0))
            .collect();

        let options = PageRegionOptions {
            min_pages: 4,
            ..PageRegionOptions::default()
        };
        let regions = detect_page_regions(&page_data, &options);

        for region in &regions {
            assert!(region.header.is_none());
        }
    }
}
