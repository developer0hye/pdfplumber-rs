use std::path::Path;

use pdfplumber::{Pdf, TextOptions};

use crate::cli::TextFormat;
use crate::page_range::parse_page_range;

pub fn run(file: &Path, pages: Option<&str>, format: &TextFormat, layout: bool) -> Result<(), i32> {
    // Open PDF with user-friendly error messages
    let pdf = open_pdf(file)?;

    // Resolve page indices
    let page_indices = resolve_pages(pages, pdf.page_count())?;

    let text_options = TextOptions {
        layout,
        ..TextOptions::default()
    };

    for &idx in &page_indices {
        let page = pdf.page(idx).map_err(|e| {
            eprintln!("Error reading page {}: {e}", idx + 1);
            1
        })?;

        let text = page.extract_text(&text_options);

        match format {
            TextFormat::Text => {
                println!("--- Page {} ---", idx + 1);
                println!("{text}");
            }
            TextFormat::Json => {
                let obj = serde_json::json!({
                    "page": idx + 1,
                    "text": text,
                });
                println!("{}", serde_json::to_string(&obj).unwrap());
            }
        }
    }

    Ok(())
}

fn open_pdf(file: &Path) -> Result<Pdf, i32> {
    if !file.exists() {
        eprintln!("Error: file not found: {}", file.display());
        return Err(1);
    }

    Pdf::open_file(file, None).map_err(|e| {
        eprintln!("Error: failed to open PDF: {e}");
        1
    })
}

fn resolve_pages(pages: Option<&str>, page_count: usize) -> Result<Vec<usize>, i32> {
    match pages {
        Some(range) => parse_page_range(range, page_count).map_err(|e| {
            eprintln!("Error: {e}");
            1
        }),
        None => Ok((0..page_count).collect()),
    }
}
