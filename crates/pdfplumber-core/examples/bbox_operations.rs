//! Demonstrates [`BBox`] arithmetic, overlap detection, union, and reading-order sort.
//!
//! [`BBox`] is the coordinate primitive used throughout pdfplumber-rs.
//! PDF uses a coordinate system where `y` increases downward from the page top.
//!
//! ```text
//! (x0, top) ──────────── (x1, top)
//!     │                       │
//! (x0, bottom) ──────── (x1, bottom)
//! ```
//!
//! Run with: `cargo run --example bbox_operations -p pdfplumber-core`

use pdfplumber_core::BBox;

fn main() {
    // ── Construction ──────────────────────────────────────────────────────
    // BBox::new(x0, top, x1, bottom)
    let title = BBox::new(50.0, 50.0, 550.0, 80.0);
    let body = BBox::new(50.0, 90.0, 550.0, 200.0);
    let sidebar = BBox::new(420.0, 90.0, 550.0, 400.0);

    println!("=== BBox dimensions ===");
    for (name, b) in [("title", &title), ("body", &body), ("sidebar", &sidebar)] {
        let w = b.x1 - b.x0;
        let h = b.bottom - b.top;
        println!(
            "  {name:8} x0={:.0} top={:.0} x1={:.0} bottom={:.0}  ({w:.0}×{h:.0} pt)",
            b.x0, b.top, b.x1, b.bottom
        );
    }

    // ── Overlap ───────────────────────────────────────────────────────────
    println!("\n=== Overlap detection ===");
    let pairs = [
        ("title", &title, "sidebar", &sidebar),
        ("body", &body, "sidebar", &sidebar),
        ("title", &title, "body", &body),
    ];
    for (an, a, bn, b) in pairs {
        let overlaps = a.x0 < b.x1 && a.x1 > b.x0 && a.top < b.bottom && a.bottom > b.top;
        println!("  {an} ∩ {bn}: {overlaps}");
    }

    // ── Union ─────────────────────────────────────────────────────────────
    let union = BBox::new(
        title.x0.min(body.x0),
        title.top.min(body.top),
        title.x1.max(body.x1),
        title.bottom.max(body.bottom),
    );
    println!("\n=== Union(title, body) ===");
    println!(
        "  x0={:.0} top={:.0} x1={:.0} bottom={:.0}",
        union.x0, union.top, union.x1, union.bottom
    );

    // ── Point containment ─────────────────────────────────────────────────
    let pt = (300.0f64, 150.0f64);
    let inside = body.x0 <= pt.0 && pt.0 < body.x1 && body.top <= pt.1 && pt.1 < body.bottom;
    println!("\n=== Point containment ===");
    println!("  ({:.0}, {:.0}) ∈ body: {inside}", pt.0, pt.1);

    // ── Reading-order sort (top-to-bottom, left-to-right) ─────────────────
    let mut blocks = vec![("sidebar", sidebar), ("body", body), ("title", title)];
    blocks.sort_by(|(_, a), (_, b)| {
        a.top
            .partial_cmp(&b.top)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal))
    });
    println!("\n=== Reading order (top → bottom) ===");
    for (name, b) in &blocks {
        println!("  {name:8} top={:.0}", b.top);
    }
}
