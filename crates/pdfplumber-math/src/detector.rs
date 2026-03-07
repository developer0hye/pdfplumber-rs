//! Math region detection on PDF pages.
//!
//! Identifies rectangular regions of a page that contain mathematical content
//! using Unicode range analysis, spacing anomaly detection, and vertical offset
//! heuristics. For each detected region, attempts LaTeX reconstruction.

use pdfplumber::Page;
use pdfplumber_core::{BBox, Char};

use crate::latex::reconstruct_latex;
use crate::unicode_ranges::{is_math_char, math_density};

// ─── Public types ─────────────────────────────────────────────────────────────

/// The kind of math content detected in a region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MathKind {
    /// An equation that appears inline within a line of text.
    ///
    /// Characterized by a small vertical extent (less than ~2× body line height)
    /// and surrounding text on both sides.
    Inline,
    /// A displayed (block) equation on its own line with substantial top/bottom
    /// whitespace.
    Display,
    /// A matrix, array, or multi-line equation environment.
    ///
    /// Detected by multiple math-dense rows in vertical proximity.
    Matrix,
    /// A table cell or other context where math is embedded in a larger structure.
    Embedded,
}

/// A detected mathematical region on a page.
#[derive(Debug, Clone)]
pub struct MathRegion {
    /// Page index (0-based).
    pub page: usize,
    /// Bounding box of the region on the page.
    pub bbox: BBox,
    /// The kind of math content.
    pub kind: MathKind,
    /// Best-effort LaTeX reconstruction of the content.
    ///
    /// This is heuristic — it will be correct for simple equations and approximate
    /// for complex display math. For production use on complex equations, pair with
    /// an Ollama vision model (see `ollama-escalation` feature).
    pub latex: String,
    /// The raw chars that constitute this region.
    pub chars: Vec<Char>,
    /// Confidence score (0.0–1.0) for the math detection.
    ///
    /// Higher values indicate more math Unicode characters and more
    /// anomalous spacing patterns. Threshold for actionable confidence: 0.4.
    pub confidence: f64,
}

/// Options for math region detection.
#[derive(Debug, Clone)]
pub struct MathOptions {
    /// Minimum fraction of chars in a cluster that must be math Unicode
    /// for the cluster to be classified as a math region.
    ///
    /// Default: 0.25 — a quarter of chars must be math symbols.
    /// Lower values = more recalls but more false positives.
    pub min_math_density: f64,

    /// Minimum number of math chars required for a region to be emitted.
    ///
    /// Default: 1 — even a single math symbol is a candidate.
    pub min_math_chars: usize,

    /// Horizontal gap (in points) that splits chars into different regions.
    ///
    /// Default: 20.0 — a gap wider than this separates two math expressions.
    pub h_gap_threshold: f64,

    /// Vertical gap (in points) that splits chars into different rows.
    ///
    /// Default: 4.0 — a vertical gap larger than this = different rows.
    pub v_gap_threshold: f64,

    /// Whether to expand the detected region to include adjacent non-math
    /// chars that appear to be part of the same expression.
    ///
    /// Default: true. Captures things like "for all x ∈ ℝ" as a unit.
    pub expand_to_expression: bool,
}

impl Default for MathOptions {
    fn default() -> Self {
        Self {
            min_math_density: 0.25,
            min_math_chars: 1,
            h_gap_threshold: 20.0,
            v_gap_threshold: 4.0,
            expand_to_expression: true,
        }
    }
}

/// Math region extractor.
///
/// Stateless — create once, reuse across pages.
#[derive(Debug, Clone, Default)]
pub struct MathExtractor {
    options: MathOptions,
}

impl MathExtractor {
    /// Create a new extractor with the given options.
    pub fn new(options: MathOptions) -> Self {
        Self { options }
    }

    /// Extract math regions from a page.
    ///
    /// Returns all detected [`MathRegion`]s in reading order (top-to-bottom,
    /// left-to-right within rows).
    pub fn extract_page(&self, page: &Page, page_idx: usize) -> Vec<MathRegion> {
        let chars = page.chars();
        if chars.is_empty() {
            return Vec::new();
        }

        // Step 1: Pre-filter — collect chars that are math or adjacent to math
        let candidate_chars = self.collect_candidate_chars(chars);
        if candidate_chars.is_empty() {
            return Vec::new();
        }

        // Step 2: Cluster by spatial proximity
        let clusters = self.cluster_chars(&candidate_chars);

        // Step 3: Filter clusters by math density and minimum size
        let mut regions: Vec<MathRegion> = clusters
            .into_iter()
            .filter_map(|cluster| self.cluster_to_region(cluster, page_idx))
            .collect();

        // Step 4: Sort top-to-bottom, left-to-right
        regions.sort_by(|a, b| {
            let row_diff = a.bbox.top - b.bbox.top;
            if row_diff.abs() < 5.0 {
                a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap()
            } else {
                a.bbox.top.partial_cmp(&b.bbox.top).unwrap()
            }
        });

        regions
    }

    /// Collect chars that are math chars OR are surrounded by math chars.
    ///
    /// "Surrounded" = within `h_gap_threshold` of a math char horizontally,
    /// on the same approximate baseline. This captures expressions like
    /// "x = 3" where 'x', '3' are not math chars but '=' is.
    fn collect_candidate_chars<'a>(&self, chars: &'a [Char]) -> Vec<&'a Char> {
        // Mark which chars are intrinsically math
        let is_math: Vec<bool> = chars
            .iter()
            .map(|c| c.text.chars().any(is_math_char))
            .collect();

        // If expand_to_expression: also include non-math chars that are within
        // h_gap_threshold of a math char on the same line.
        if !self.options.expand_to_expression {
            return chars
                .iter()
                .enumerate()
                .filter(|(i, _)| is_math[*i])
                .map(|(_, c)| c)
                .collect();
        }

        let n = chars.len();
        let mut include = is_math.clone();

        for i in 0..n {
            if !is_math[i] {
                continue;
            }
            let ref_char = &chars[i];
            // Look left and right within h_gap_threshold
            for j in (0..i).rev() {
                let d = ref_char.bbox.x0 - chars[j].bbox.x1;
                if d > self.options.h_gap_threshold {
                    break;
                }
                if same_baseline(ref_char, &chars[j]) {
                    include[j] = true;
                }
            }
            for j in (i + 1)..n {
                let d = chars[j].bbox.x0 - ref_char.bbox.x1;
                if d > self.options.h_gap_threshold {
                    break;
                }
                if same_baseline(ref_char, &chars[j]) {
                    include[j] = true;
                }
            }
        }

        chars
            .iter()
            .enumerate()
            .filter(|(i, _)| include[*i])
            .map(|(_, c)| c)
            .collect()
    }

    /// Cluster chars by spatial proximity.
    ///
    /// A new cluster begins when:
    /// - Horizontal gap > `h_gap_threshold`, or
    /// - Vertical gap > `v_gap_threshold` (new row)
    ///
    /// Chars must be sorted by `(bbox.top, bbox.x0)` for correct results.
    fn cluster_chars<'a>(&self, chars: &[&'a Char]) -> Vec<Vec<&'a Char>> {
        if chars.is_empty() {
            return Vec::new();
        }

        let mut sorted = chars.to_vec();
        sorted.sort_by(|a, b| {
            let row_diff = a.bbox.top - b.bbox.top;
            if row_diff.abs() < self.options.v_gap_threshold {
                a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap()
            } else {
                a.bbox.top.partial_cmp(&b.bbox.top).unwrap()
            }
        });

        let mut clusters: Vec<Vec<&Char>> = Vec::new();
        let mut current: Vec<&Char> = vec![sorted[0]];

        for curr in &sorted[1..] {
            let prev = current.last().unwrap();

            let h_gap = curr.bbox.x0 - prev.bbox.x1;
            let v_gap = (curr.bbox.top - prev.bbox.top).abs();

            if h_gap > self.options.h_gap_threshold || v_gap > self.options.v_gap_threshold {
                // New cluster — but check: maybe it's a new row in a matrix
                // (same x range, just one line lower)
                let last_cluster_bbox = cluster_bbox(&current);
                let x_overlap =
                    curr.bbox.x0 < last_cluster_bbox.x1 && curr.bbox.x1 > last_cluster_bbox.x0;

                if v_gap <= self.options.v_gap_threshold * 5.0 && x_overlap {
                    // Probably a multi-row expression — merge into current cluster
                    current.push(curr);
                } else {
                    clusters.push(current);
                    current = vec![curr];
                }
            } else {
                current.push(curr);
            }
        }
        clusters.push(current);
        clusters
    }

    /// Convert a cluster of chars into a [`MathRegion`], or `None` if below threshold.
    fn cluster_to_region(&self, cluster: Vec<&Char>, page_idx: usize) -> Option<MathRegion> {
        // Count intrinsic math chars
        let math_count = cluster
            .iter()
            .filter(|c| c.text.chars().any(is_math_char))
            .count();

        if math_count < self.options.min_math_chars {
            return None;
        }

        let text: String = cluster
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join("");
        let density = math_density(&text);

        if density < self.options.min_math_density {
            return None;
        }

        let bbox = cluster_bbox(&cluster);
        let kind = classify_region_kind(&cluster, &bbox);

        // Sort cluster left-to-right, then sub/superscripts by x
        let mut sorted_chars: Vec<Char> = cluster.iter().map(|&c| c.clone()).collect();
        sorted_chars.sort_by(|a, b| a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap());

        let latex = reconstruct_latex(&sorted_chars);
        let confidence = compute_confidence(density, math_count);

        Some(MathRegion {
            page: page_idx,
            bbox,
            kind,
            latex,
            chars: sorted_chars,
            confidence,
        })
    }
}

// ─── Region classification ────────────────────────────────────────────────────

/// Classify the kind of math region from its geometric properties.
fn classify_region_kind(chars: &[&Char], bbox: &BBox) -> MathKind {
    let height = bbox.height();
    let width = bbox.width();

    // Count distinct y-levels (rows) in the cluster
    let mut y_levels: Vec<f64> = chars
        .iter()
        .map(|c| (c.bbox.top * 2.0).round() / 2.0)
        .collect();
    y_levels.sort_by(|a, b| a.partial_cmp(b).unwrap());
    y_levels.dedup();
    let row_count = y_levels.len();

    if row_count >= 3 {
        return MathKind::Matrix;
    }

    // Heuristic: display equations tend to be wider than they are tall,
    // centered (x0 > 50pt from page edge), and on their own line.
    let is_wide = width > 100.0 && height < width * 0.3;
    // For inline: narrow height, appears within a line
    let is_narrow = height < 20.0;

    if is_wide && row_count == 1 {
        MathKind::Display
    } else if is_narrow && row_count == 1 {
        MathKind::Inline
    } else if row_count == 2 {
        // Two rows: likely fraction or super/subscript display
        MathKind::Display
    } else {
        MathKind::Inline
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Compute a confidence score from math density and raw math char count.
fn compute_confidence(density: f64, math_count: usize) -> f64 {
    // Density contributes 70%, count saturation contributes 30%
    let count_score = (math_count as f64 / 5.0).min(1.0); // saturates at 5 math chars
    (density * 0.7 + count_score * 0.3).min(1.0)
}

/// Bounding box union of a cluster.
fn cluster_bbox(chars: &[&Char]) -> BBox {
    chars
        .iter()
        .map(|c| c.bbox)
        .reduce(|a, b| a.union(&b))
        .unwrap_or_else(|| BBox::new(0.0, 0.0, 0.0, 0.0))
}

/// True if two chars are on approximately the same baseline.
///
/// Uses the bottom of the bounding box (closer to baseline than top).
fn same_baseline(a: &Char, b: &Char) -> bool {
    (a.bbox.bottom - b.bbox.bottom).abs() < 4.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::TextDirection;

    fn make_char(text: &str, x0: f64, top: f64, size: f64) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x0 + size * 0.55, top + size),
            fontname: "TestFont".to_string(),
            size,
            doctop: top,
            upright: true,
            direction: TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: None,
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: text.chars().next().unwrap_or(' ') as u32,
            mcid: None,
            tag: None,
        }
    }

    #[test]
    fn empty_page_returns_no_regions() {
        let extractor = MathExtractor::default();
        // Use a Vec<Char> directly since we don't have a real Page
        let chars: Vec<Char> = vec![];
        let clusters = extractor.cluster_chars(&chars.iter().collect::<Vec<_>>());
        assert!(clusters.is_empty());
    }

    #[test]
    fn single_integral_detected() {
        let extractor = MathExtractor::default();
        let chars = vec![
            make_char("∫", 50.0, 100.0, 14.0),
            make_char("x", 65.0, 100.0, 12.0),
            make_char("d", 75.0, 100.0, 12.0),
            make_char("x", 83.0, 100.0, 12.0),
        ];
        let clusters = extractor.cluster_chars(&chars.iter().collect::<Vec<_>>());
        assert_eq!(clusters.len(), 1);
        let region = extractor.cluster_to_region(clusters.into_iter().next().unwrap(), 0);
        assert!(region.is_some());
        let r = region.unwrap();
        assert!(r.latex.contains("\\int"), "Expected \\int in: {}", r.latex);
    }

    #[test]
    fn two_separate_expressions_become_two_clusters() {
        let extractor = MathExtractor::default();
        let chars = vec![
            make_char("∑", 50.0, 100.0, 14.0),
            make_char("i", 65.0, 100.0, 12.0),
            // big gap
            make_char("∫", 200.0, 100.0, 14.0),
            make_char("x", 215.0, 100.0, 12.0),
        ];
        let clusters = extractor.cluster_chars(&chars.iter().collect::<Vec<_>>());
        assert_eq!(
            clusters.len(),
            2,
            "Expected 2 clusters, got {}",
            clusters.len()
        );
    }

    #[test]
    fn greek_letter_becomes_latex_alpha() {
        let extractor = MathExtractor::default();
        let chars = vec![make_char("α", 50.0, 100.0, 12.0)];
        let clusters = extractor.cluster_chars(&chars.iter().collect::<Vec<_>>());
        let region = extractor.cluster_to_region(clusters.into_iter().next().unwrap(), 0);
        assert!(region.is_some());
        assert!(region.unwrap().latex.contains("alpha"));
    }

    #[test]
    fn non_math_text_below_threshold_filtered_out() {
        let extractor = MathExtractor::default();
        // All plain ASCII — no math chars
        let chars = vec![
            make_char("h", 50.0, 100.0, 12.0),
            make_char("e", 57.0, 100.0, 12.0),
            make_char("l", 64.0, 100.0, 12.0),
            make_char("l", 68.0, 100.0, 12.0),
            make_char("o", 72.0, 100.0, 12.0),
        ];
        let candidates = extractor.collect_candidate_chars(&chars);
        assert!(
            candidates.is_empty(),
            "Plain text should not be math candidates"
        );
    }

    #[test]
    fn expand_to_expression_captures_surrounding_letters() {
        let extractor = MathExtractor::default();
        // "x ∈ ℝ" — x and ℝ are not in math range, ∈ is
        let chars = vec![
            make_char("x", 50.0, 100.0, 12.0),
            make_char(" ", 57.0, 100.0, 12.0),
            make_char("∈", 60.0, 100.0, 12.0),
            make_char(" ", 72.0, 100.0, 12.0),
            make_char("ℝ", 75.0, 100.0, 12.0),
        ];
        let candidates = extractor.collect_candidate_chars(&chars);
        // With expansion, all 5 chars should be included (x and ℝ adjacent to ∈)
        assert!(
            candidates.len() >= 3,
            "Expected x, ∈, ℝ captured. Got: {}",
            candidates.len()
        );
    }

    #[test]
    fn confidence_pure_math_high() {
        // All math chars → high confidence
        let c = compute_confidence(1.0, 10);
        assert!(c >= 0.9);
    }

    #[test]
    fn confidence_mixed_text_moderate() {
        let c = compute_confidence(0.3, 2);
        assert!(c > 0.0 && c < 0.9);
    }

    #[test]
    fn inline_kind_for_narrow_single_row() {
        let chars: Vec<Char> = vec![
            make_char("α", 50.0, 100.0, 12.0),
            make_char("+", 62.0, 100.0, 12.0),
            make_char("β", 70.0, 100.0, 12.0),
        ];
        let refs: Vec<&Char> = chars.iter().collect();
        let bbox = cluster_bbox(&refs);
        let kind = classify_region_kind(&refs, &bbox);
        assert_eq!(kind, MathKind::Inline);
    }

    #[test]
    fn options_min_math_density_filters() {
        let extractor = MathExtractor::new(MathOptions {
            min_math_density: 0.9, // very strict
            ..MathOptions::default()
        });
        // Only 1 math char out of 5 → density 0.2 → below 0.9 threshold
        let chars = vec![
            make_char("∫", 50.0, 100.0, 14.0),
            make_char("a", 65.0, 100.0, 12.0),
            make_char("b", 75.0, 100.0, 12.0),
            make_char("c", 85.0, 100.0, 12.0),
            make_char("d", 95.0, 100.0, 12.0),
        ];
        let clusters = extractor.cluster_chars(&chars.iter().collect::<Vec<_>>());
        let region = extractor.cluster_to_region(clusters.into_iter().next().unwrap(), 0);
        assert!(
            region.is_none(),
            "Should be filtered by high density threshold"
        );
    }
}
