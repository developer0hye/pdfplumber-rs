//! Figure detection: image regions and path-dense areas without text.

use pdfplumber_core::{BBox, Image, Rect};

/// A figure: a page region containing visual content (images or path-dense areas)
/// with no meaningful text characters.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Figure {
    /// Bounding box of the figure.
    pub bbox: BBox,
    /// Page number (0-based).
    pub page_number: usize,
    /// Kind of figure content.
    pub kind: FigureKind,
}

/// What kind of content drives this figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FigureKind {
    /// Contains one or more PDF Image XObjects.
    Image,
    /// Contains a dense cluster of paths/rects without text (chart, diagram).
    PathDense,
    /// Contains both images and paths.
    Mixed,
}

/// Minimum area (in sq pts) for a figure region to be reported.
const MIN_FIGURE_AREA: f64 = 1000.0;

/// Detect figures from a page's Image XObjects.
///
/// Each image with non-trivial dimensions becomes a [`Figure`] of kind [`FigureKind::Image`].
pub fn detect_figures_from_images(images: &[Image], page_number: usize) -> Vec<Figure> {
    images
        .iter()
        .filter(|img| img.width > 1.0 && img.height > 1.0)
        .map(|img| Figure {
            bbox: img.bbox(),
            page_number,
            kind: FigureKind::Image,
        })
        .collect()
}

/// Detect path-dense figures from rects that have no overlapping text chars.
///
/// Groups rects that are spatially close (within `cluster_gap` pts of each other),
/// computes union bbox for each cluster, and emits a [`FigureKind::PathDense`] figure
/// if the cluster area exceeds the minimum.
pub fn detect_figures_from_rects(
    rects: &[Rect],
    page_number: usize,
    cluster_gap: f64,
) -> Vec<Figure> {
    if rects.is_empty() {
        return Vec::new();
    }

    // Collect bboxes from rects, sorted top-to-bottom then left-to-right
    let mut sorted_bboxes: Vec<BBox> = rects
        .iter()
        .map(|r| BBox::new(r.x0, r.top, r.x1, r.bottom))
        .collect();
    sorted_bboxes.sort_by(|a, b| {
        a.top
            .partial_cmp(&b.top)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal))
    });

    // Single-pass greedy cluster
    let mut clusters: Vec<BBox> = Vec::new();
    let mut current: Option<BBox> = None;

    for rb in sorted_bboxes {
        match current {
            None => {
                current = Some(rb);
            }
            Some(c) => {
                if bboxes_near(&c, &rb, cluster_gap) {
                    current = Some(c.union(&rb));
                } else {
                    clusters.push(c);
                    current = Some(rb);
                }
            }
        }
    }
    if let Some(c) = current {
        clusters.push(c);
    }

    clusters
        .into_iter()
        .filter(|bbox| bbox.width() * bbox.height() >= MIN_FIGURE_AREA)
        .map(|bbox| Figure {
            bbox,
            page_number,
            kind: FigureKind::PathDense,
        })
        .collect()
}

/// Returns true if two bboxes are within `gap` pts of each other in both axes.
fn bboxes_near(a: &BBox, b: &BBox, gap: f64) -> bool {
    let h_near = a.x0 <= b.x1 + gap && b.x0 <= a.x1 + gap;
    let v_near = a.top <= b.bottom + gap && b.top <= a.bottom + gap;
    h_near && v_near
}

/// Merge overlapping figures on the same page into single figures.
///
/// Two figures merge if the smaller one overlaps the larger by more than 30%
/// of its own area. The result has the union bbox; kind becomes [`FigureKind::Mixed`]
/// if the two figures were of different kinds.
pub fn merge_overlapping_figures(mut figures: Vec<Figure>) -> Vec<Figure> {
    let mut merged = true;
    while merged {
        merged = false;
        let mut i = 0;
        while i < figures.len() {
            let mut j = i + 1;
            while j < figures.len() {
                if figures[i].page_number == figures[j].page_number
                    && bbox_overlap_fraction(&figures[i].bbox, &figures[j].bbox) > 0.3
                {
                    let union = figures[i].bbox.union(&figures[j].bbox);
                    let kind = if figures[i].kind == figures[j].kind {
                        figures[i].kind
                    } else {
                        FigureKind::Mixed
                    };
                    figures[i] = Figure {
                        bbox: union,
                        page_number: figures[i].page_number,
                        kind,
                    };
                    figures.remove(j);
                    merged = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }
    figures
}

/// Fraction of the smaller bbox's area that intersects the other bbox.
fn bbox_overlap_fraction(a: &BBox, b: &BBox) -> f64 {
    let ix0 = a.x0.max(b.x0);
    let iy0 = a.top.max(b.top);
    let ix1 = a.x1.min(b.x1);
    let iy1 = a.bottom.min(b.bottom);
    if ix1 <= ix0 || iy1 <= iy0 {
        return 0.0;
    }
    let intersection = (ix1 - ix0) * (iy1 - iy0);
    let area_a = a.width() * a.height();
    let area_b = b.width() * b.height();
    let smaller = area_a.min(area_b);
    if smaller <= 0.0 {
        return 0.0;
    }
    intersection / smaller
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::{BBox, Color, Rect};

    fn make_rect(x0: f64, top: f64, x1: f64, bottom: f64) -> Rect {
        Rect {
            x0,
            top,
            x1,
            bottom,
            line_width: 1.0,
            stroke: true,
            fill: false,
            stroke_color: Color::Gray(0.0),
            fill_color: Color::Gray(1.0),
        }
    }

    fn make_figure(
        x0: f64,
        top: f64,
        x1: f64,
        bottom: f64,
        kind: FigureKind,
        page: usize,
    ) -> Figure {
        Figure {
            bbox: BBox::new(x0, top, x1, bottom),
            page_number: page,
            kind,
        }
    }

    #[test]
    fn detect_figures_empty_images() {
        let figures = detect_figures_from_images(&[], 0);
        assert!(figures.is_empty());
    }

    #[test]
    fn detect_figures_empty_rects() {
        let figures = detect_figures_from_rects(&[], 0, 10.0);
        assert!(figures.is_empty());
    }

    #[test]
    fn detect_figures_rects_too_small() {
        // 10×10 = 100 sq pts < MIN_FIGURE_AREA
        let rects = vec![make_rect(0.0, 0.0, 10.0, 10.0)];
        let figures = detect_figures_from_rects(&rects, 0, 10.0);
        assert!(figures.is_empty());
    }

    #[test]
    fn detect_figures_rects_large_enough() {
        // 200×200 = 40000 > MIN_FIGURE_AREA
        let rects = vec![make_rect(50.0, 100.0, 250.0, 300.0)];
        let figures = detect_figures_from_rects(&rects, 0, 10.0);
        assert_eq!(figures.len(), 1);
        assert_eq!(figures[0].kind, FigureKind::PathDense);
        assert_eq!(figures[0].page_number, 0);
    }

    #[test]
    fn detect_figures_rects_cluster_into_one() {
        // Two rects close together should cluster into a single figure
        let rects = vec![
            make_rect(50.0, 100.0, 150.0, 150.0), // 100×50 = 5000
            make_rect(50.0, 155.0, 150.0, 205.0), // 100×50, gap=5 < cluster_gap=10
        ];
        let figures = detect_figures_from_rects(&rects, 0, 10.0);
        assert_eq!(figures.len(), 1);
        // Union bbox should span both
        assert!((figures[0].bbox.top - 100.0).abs() < 1.0);
        assert!((figures[0].bbox.bottom - 205.0).abs() < 1.0);
    }

    #[test]
    fn detect_figures_rects_separate_clusters() {
        // Two rects far apart should produce two figures
        let rects = vec![
            make_rect(50.0, 100.0, 250.0, 300.0), // 200×200
            make_rect(50.0, 500.0, 250.0, 700.0), // 200×200, gap=200
        ];
        let figures = detect_figures_from_rects(&rects, 0, 10.0);
        assert_eq!(figures.len(), 2);
    }

    #[test]
    fn bbox_overlap_fraction_no_overlap() {
        let a = BBox::new(0.0, 0.0, 10.0, 10.0);
        let b = BBox::new(20.0, 0.0, 30.0, 10.0);
        assert_eq!(bbox_overlap_fraction(&a, &b), 0.0);
    }

    #[test]
    fn bbox_overlap_fraction_full_overlap() {
        let a = BBox::new(0.0, 0.0, 10.0, 10.0);
        let b = BBox::new(0.0, 0.0, 10.0, 10.0);
        assert!((bbox_overlap_fraction(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn bbox_overlap_fraction_half() {
        let a = BBox::new(0.0, 0.0, 10.0, 10.0);
        let b = BBox::new(5.0, 0.0, 15.0, 10.0);
        // intersection=5×10=50, smaller area=100 → 0.5
        assert!((bbox_overlap_fraction(&a, &b) - 0.5).abs() < 0.001);
    }

    #[test]
    fn merge_overlapping_same_kind() {
        let figs = vec![
            make_figure(0.0, 0.0, 10.0, 10.0, FigureKind::Image, 0),
            make_figure(5.0, 0.0, 15.0, 10.0, FigureKind::Image, 0),
        ];
        let merged = merge_overlapping_figures(figs);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].kind, FigureKind::Image);
    }

    #[test]
    fn merge_overlapping_mixed_kinds() {
        let figs = vec![
            make_figure(0.0, 0.0, 10.0, 10.0, FigureKind::Image, 0),
            make_figure(5.0, 0.0, 15.0, 10.0, FigureKind::PathDense, 0),
        ];
        let merged = merge_overlapping_figures(figs);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].kind, FigureKind::Mixed);
    }

    #[test]
    fn merge_different_pages_no_merge() {
        let figs = vec![
            make_figure(0.0, 0.0, 10.0, 10.0, FigureKind::Image, 0),
            make_figure(0.0, 0.0, 10.0, 10.0, FigureKind::Image, 1),
        ];
        let merged = merge_overlapping_figures(figs);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn bboxes_near_touching() {
        let a = BBox::new(0.0, 0.0, 10.0, 10.0);
        let b = BBox::new(10.0, 0.0, 20.0, 10.0); // exactly touching
        assert!(bboxes_near(&a, &b, 5.0));
    }

    #[test]
    fn bboxes_near_far_apart() {
        let a = BBox::new(0.0, 0.0, 10.0, 10.0);
        let b = BBox::new(100.0, 100.0, 110.0, 110.0);
        assert!(!bboxes_near(&a, &b, 5.0));
    }
}
