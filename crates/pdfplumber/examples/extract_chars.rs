//! Extract individual characters from a PDF with position and font info.
//!
//! Usage: `cargo run -p pdfplumber --example extract_chars -- <path-to-pdf>`

use pdfplumber::Pdf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Usage: extract_chars <path-to-pdf>",
        )
    })?;
    let pdf = Pdf::open_path(&path, None)?;

    for page_result in pdf.pages() {
        let page = page_result?;
        println!(
            "--- Page {} ({:.0} x {:.0}, {} chars) ---",
            page.page_number(),
            page.width(),
            page.height(),
            page.chars().len()
        );

        for ch in page.chars() {
            println!(
                "  '{}' x0={:.1} top={:.1} x1={:.1} bottom={:.1} font={} size={:.1}",
                ch.text, ch.bbox.x0, ch.bbox.top, ch.bbox.x1, ch.bbox.bottom, ch.fontname, ch.size
            );
        }
        println!();
    }

    Ok(())
}
