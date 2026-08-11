//! Parity tests for snapping nearly-aligned table rules together.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! [(len(t), len(t[0])) for t in page.extract_tables()]
//! ```
//!
//! Rules drawn a fraction of a point apart should collapse onto one position.
//! The grouping chains: a rule joins the group when it is close enough to the
//! nearest rule already in it, so a run of slightly drifting rules ends up on a
//! single line however far the drift reaches.

use std::path::PathBuf;

use pdfplumber::{Edge, EdgeSource, Orientation, Pdf, TableSettings, snap_edges};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdfs")
        .join(name)
}

fn vertical_edge(x: f64) -> Edge {
    Edge {
        x0: x,
        top: 0.0,
        x1: x,
        bottom: 50.0,
        orientation: Orientation::Vertical,
        source: EdgeSource::Line,
    }
}

#[test]
fn nearly_aligned_rules_collapse_onto_one_position() {
    // 100, 102, 104: each within 3.0 of the one before, so all three snap
    // together even though the run spans 4.0.
    let snapped = snap_edges(
        vec![
            vertical_edge(100.0),
            vertical_edge(102.0),
            vertical_edge(104.0),
        ],
        3.0,
        3.0,
    );

    let positions: Vec<f64> = snapped.iter().map(|e| e.x0).collect();
    assert_eq!(positions, vec![102.0, 102.0, 102.0]);
}

#[test]
fn rules_further_apart_than_the_tolerance_stay_separate() {
    let snapped = snap_edges(vec![vertical_edge(100.0), vertical_edge(104.0)], 3.0, 3.0);

    let positions: Vec<f64> = snapped.iter().map(|e| e.x0).collect();
    assert_eq!(positions, vec![100.0, 104.0]);
}

#[test]
fn a_drifting_grid_reads_as_one_table() {
    // issue-461-example.pdf draws its header rules a fraction apart. Without
    // chaining they stayed separate columns: pdfplumber reads 1x6 and 4x1.
    let pdf = Pdf::open_file(fixture("issue-461-example.pdf"), None).unwrap();
    let page = pdf.page(0).unwrap();
    let shapes: Vec<(usize, usize)> = page
        .extract_tables(&TableSettings::default())
        .iter()
        .map(|table| (table.len(), table.first().map_or(0, Vec::len)))
        .collect();

    assert_eq!(shapes, vec![(1, 6), (4, 1)]);
}
