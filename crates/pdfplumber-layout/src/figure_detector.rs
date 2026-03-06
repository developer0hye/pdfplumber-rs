//! Figure detection: identifies page regions that have visual content (paths,
//! rects, images) but no text chars. These regions are likely figures, charts,
//! photographs, or decorative elements.

use pdfplumber::Page;
use pdfplumber_core::BBox;

/// A detected figure region on a page.
#[derive(Debug, Clone)]
pub struct Figure {
    /// Page index (0-based).
    pub page: usize,
    /// Bounding box of the figure region on the page.
    pub bbox: BBox,
    /// The kind of figure detected.
    pub kind: FigureKind,
}

/// Broad classification of figure type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FigureKind {
    /// A rasterised image embedded in the PDF.
    Image,
    /// A vector graphics region (paths, curves, rects) with no text.
    VectorGraphic,
    /// Mixed image + vector content.
    Mixed,
}

/// Minimum area (in points²) for a region to be considered a figure.
/// Regions smaller than this (e.g., decorative rules, borders) are ignored.
const MIN_FIGURE_AREA_PT2: f64 = 1000.0; // ~1 square inch at 72dpi / 4

/// Detect figures on a single page.
///
/// Strategy:
/// 1. Collect all image bboxes from `page.images()`.
/// 2. Collect all path/rect bboxes that have area ≥ `MIN_FIGURE_AREA_PT2`.
/// 3. Merge overlapping bboxes using a simple union-find pass.
/// 4. For each merged region: if it overlaps any text char bbox, skip it
///    (it's a background element under text, not a figure).
/// 5. Return remaining regions as [`Figure`]s.
pub fn detect_figures(page: &Page, page_idx: usize) -> Vec<Figure> {
    let page_width = page.width();
    let page_height = page.height();

    // Collect candidate visual bboxes
    let mut image_bboxes: Vec<BBox> = page
        .images()
        .iter()
        .map(|img| BBox::new(img.x0, img.top, img.x1, img.bottom))
        .filter(|b| bbox_area(b) >= MIN_FIGURE_AREA_PT2)
        .collect();

    let mut path_bboxes: Vec<BBox> = page
        .rects()
        .iter()
        .map(|r| BBox::new(r.x0, r.top, r.x1, r.bottom))
        .filter(|b| bbox_area(b) >= MIN_FIGURE_AREA_PT2)
        .collect();

    // Also check curves / painted paths for large vector regions
    let curve_bboxes: Vec<BBox> = page
        .curves()
        .iter()
        .map(|c| BBox::new(c.x0, c.top, c.x1, c.bottom))
        .filter(|b| bbox_area(b) >= MIN_FIGURE_AREA_PT2)
        .collect();
    path_bboxes.extend(curve_bboxes);

    if image_bboxes.is_empty() && path_bboxes.is_empty() {
        return Vec::new();
    }

    // Merge overlapping bboxes
    let all_image: Vec<(BBox, bool)> = image_bboxes.iter().map(|b| (*b, true)).collect();
    let all_path: Vec<(BBox, bool)> = path_bboxes.iter().map(|b| (*b, false)).collect();
    let mut candidates: Vec<(BBox, bool)> = all_image;
    candidates.extend(all_path);

    let merged = merge_overlapping(candidates);

    // Text char bboxes for overlap exclusion
    let char_bboxes: Vec<BBox> = page.chars().iter().map(|c| c.bbox).collect();

    // Filter: exclude regions heavily overlapping with text
    let mut figures = Vec::new();
    for (bbox, is_image) in merged {
        // Skip full-page background rects
        if bbox.width() >= page_width * 0.95 && bbox.height() >= page_height * 0.95 {
            continue;
        }
        // Skip if more than 30% of the region bbox is covered by text chars
        let text_overlap = text_overlap_fraction(&bbox, &char_bboxes);
        if text_overlap > 0.30 {
            continue;
        }

        let kind = if is_image {
            // Check if there's also path content in same region
            let has_paths = path_bboxes.iter().any(|pb| bboxes_intersect(pb, &bbox));
            if has_paths { FigureKind::Mixed } else { FigureKind::Image }
        } else {
            FigureKind::VectorGraphic
        };

        figures.push(Figure {
            page: page_idx,
            bbox,
            kind,
        });
    }

    figures
}

/// Merge overlapping (bbox, is_image) pairs using greedy union.
///
/// For each candidate, if it overlaps any existing merged region, union them.
/// Otherwise add as new region. O(n²) but n is small in practice.
fn merge_overlapping(mut candidates: Vec<(BBox, bool)>) -> Vec<(BBox, bool)> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut merged: Vec<(BBox, bool)> = Vec::new();

    for (bbox, is_image) in candidates.drain(..) {
        let mut absorbed = false;
        for (existing_bbox, existing_is_image) in &mut merged {
            if bboxes_intersect(existing_bbox, &bbox) {
                *existing_bbox = existing_bbox.union(&bbox);
                *existing_is_image = *existing_is_image || is_image;
                absorbed = true;
                break;
            }
        }
        if !absorbed {
            merged.push((bbox, is_image));
        }
    }

    // One more pass to merge newly-adjacent regions (the greedy pass may miss
    // regions that only become adjacent after a union).
    let mut changed = true;
    while changed {
        changed = false;
        let mut i = 0;
        while i < merged.len() {
            let mut j = i + 1;
            while j < merged.len() {
                if bboxes_intersect(&merged[i].0, &merged[j].0) {
                    let (bbox_j, img_j) = merged.remove(j);
                    merged[i].0 = merged[i].0.union(&bbox_j);
                    merged[i].1 = merged[i].1 || img_j;
                    changed = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }

    merged
}

/// Fraction of `region` bbox area covered by text char bboxes.
/// Returns 0.0 if region has zero area.
/// True if two bboxes overlap (share interior area).
fn bboxes_intersect(a: &BBox, b: &BBox) -> bool {
    a.x0 < b.x1 && a.x1 > b.x0 && a.top < b.bottom && a.bottom > b.top
}

/// Area of a bbox in pt².
fn bbox_area(b: &BBox) -> f64 {
    (b.x1 - b.x0).max(0.0) * (b.bottom - b.top).max(0.0)
}

fn text_overlap_fraction(region: &BBox, chars: &[BBox]) -> f64 {
    let region_area = bbox_area(region);
    if region_area <= 0.0 {
        return 0.0;
    }

    // Sum overlap areas (approximate — doesn't handle double-counting of
    // overlapping chars, but good enough for the 30% threshold check).
    let overlap: f64 = chars
        .iter()
        .map(|c| {
            let ix0 = region.x0.max(c.x0);
            let iy0 = region.top.max(c.top);
            let ix1 = region.x1.min(c.x1);
            let iy1 = region.bottom.min(c.bottom);
            if ix1 > ix0 && iy1 > iy0 {
                (ix1 - ix0) * (iy1 - iy0)
            } else {
                0.0
            }
        })
        .sum();

    (overlap / region_area).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::BBox;

    #[test]
    fn merge_non_overlapping_stays_separate() {
        let a = BBox::new(0.0, 0.0, 100.0, 100.0);
        let b = BBox::new(200.0, 200.0, 300.0, 300.0);
        let result = merge_overlapping(vec![(a, true), (b, false)]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn merge_overlapping_combines() {
        let a = BBox::new(0.0, 0.0, 100.0, 100.0);
        let b = BBox::new(80.0, 0.0, 180.0, 100.0); // overlaps a
        let result = merge_overlapping(vec![(a, false), (b, false)]);
        assert_eq!(result.len(), 1);
        assert!(result[0].0.width() > 150.0);
    }

    #[test]
    fn text_overlap_fraction_zero_chars() {
        let region = BBox::new(0.0, 0.0, 100.0, 100.0);
        let frac = text_overlap_fraction(&region, &[]);
        assert_eq!(frac, 0.0);
    }

    #[test]
    fn text_overlap_fraction_full_overlap() {
        let region = BBox::new(0.0, 0.0, 100.0, 100.0);
        let chars = vec![BBox::new(0.0, 0.0, 100.0, 100.0)];
        let frac = text_overlap_fraction(&region, &chars);
        assert!((frac - 1.0).abs() < 1e-6);
    }

    #[test]
    fn text_overlap_fraction_partial() {
        let region = BBox::new(0.0, 0.0, 100.0, 100.0); // area = 10000
        // Half of the region covered
        let chars = vec![BBox::new(0.0, 0.0, 50.0, 100.0)]; // area = 5000
        let frac = text_overlap_fraction(&region, &chars);
        assert!((frac - 0.5).abs() < 1e-6);
    }
}
