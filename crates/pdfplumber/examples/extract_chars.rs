//! Extract individual characters from a PDF with position and font info.
//!
//! Usage: `cargo run --example extract_chars -- <path-to-pdf>`

use pdfplumber::Pdf;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: extract_chars <path-to-pdf>");
        std::process::exit(1);
    });

    let pdf = Pdf::open_file(&path, None).unwrap_or_else(|e| {
        eprintln!("Error opening PDF: {e}");
        std::process::exit(1);
    });

    for page_result in pdf.pages_iter() {
        let page = page_result.unwrap();
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
}
