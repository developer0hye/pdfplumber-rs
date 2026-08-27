//! Extract a document's pages in parallel and report them in page-index order.
//!
//! Usage: `cargo run -p pdfplumber --example parallel_batch --features parallel -- <path-to-pdf>`

use pdfplumber::{Pdf, TextOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Usage: parallel_batch <path-to-pdf>",
        )
    })?;
    let pdf = Pdf::open_path(path, None)?;

    let page_results: Vec<Result<_, _>> = pdf.pages_parallel();
    for (page_index, page_result) in page_results.into_iter().enumerate() {
        let page = page_result?;
        let text = page.extract_text(&TextOptions::default());
        println!("page_index={page_index} text_bytes={}", text.len());
    }

    Ok(())
}
