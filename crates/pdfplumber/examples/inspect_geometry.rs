//! Inspect page bounds and geometric object counts.
//!
//! Usage: `cargo run -p pdfplumber --example inspect_geometry -- <path-to-pdf>`

use pdfplumber::Pdf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Usage: inspect_geometry <path-to-pdf>",
        )
    })?;
    let pdf = Pdf::open_path(path, None)?;

    for page_result in pdf.pages() {
        let page = page_result?;
        let bbox = page.bbox();
        println!(
            "page={} bbox=({:.1}, {:.1}, {:.1}, {:.1}) lines={} rects={} curves={} images={}",
            page.page_number(),
            bbox.x0,
            bbox.top,
            bbox.x1,
            bbox.bottom,
            page.lines().len(),
            page.rects().len(),
            page.curves().len(),
            page.images().len()
        );
    }

    Ok(())
}
