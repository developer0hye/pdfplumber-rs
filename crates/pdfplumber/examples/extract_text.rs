//! Extract text from each page of a PDF document.
//!
//! Usage: `cargo run --example extract_text -- <path-to-pdf>`

use pdfplumber::{Pdf, TextOptions};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: extract_text <path-to-pdf>");
        std::process::exit(1);
    });

    let pdf = Pdf::open_file(&path, None).unwrap_or_else(|e| {
        eprintln!("Error opening PDF: {e}");
        std::process::exit(1);
    });

    println!("Pages: {}", pdf.page_count());
    println!();

    for page_result in pdf.pages_iter() {
        let page = page_result.unwrap();
        let text = page.extract_text(&TextOptions::default());
        println!("--- Page {} ---", page.page_number());
        println!("{text}");
        println!();
    }
}
