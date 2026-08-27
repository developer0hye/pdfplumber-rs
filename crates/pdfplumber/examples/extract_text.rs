//! Extract text from each page of a PDF document.
//!
//! Usage: `cargo run -p pdfplumber --example extract_text -- <path-to-pdf>`

use pdfplumber::{Pdf, TextOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Usage: extract_text <path-to-pdf>",
        )
    })?;
    let pdf = Pdf::open_path(&path, None)?;

    println!("Pages: {}", pdf.page_count());
    println!();

    for page_result in pdf.pages() {
        let page = page_result?;
        let text = page.extract_text(&TextOptions::default());
        println!("--- Page {} ---", page.page_number());
        println!("{text}");
        println!();
    }

    Ok(())
}
