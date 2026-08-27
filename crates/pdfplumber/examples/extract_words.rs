//! Extract words and their top-left page-space bounding boxes.
//!
//! Usage: `cargo run -p pdfplumber --example extract_words -- <path-to-pdf>`

use pdfplumber::{Pdf, WordOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Usage: extract_words <path-to-pdf>",
        )
    })?;
    let pdf = Pdf::open_path(path, None)?;

    for page_result in pdf.pages() {
        let page = page_result?;
        for word in page.extract_words(&WordOptions::default()) {
            println!(
                "page={} text={:?} bbox=({:.1}, {:.1}, {:.1}, {:.1}) doctop={:.1}",
                page.page_number(),
                word.text,
                word.bbox.x0,
                word.bbox.top,
                word.bbox.x1,
                word.bbox.bottom,
                word.doctop
            );
        }
    }

    Ok(())
}
